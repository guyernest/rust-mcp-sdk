//! Duplex round-trip tests for the client host surface (Phase 106).
//!
//! Proves that a [`Client`](pmcp::Client) answers inbound server -> client
//! requests from its registered host handlers, while one of the client's own
//! requests is in flight (the nested flow):
//!   * sampling (`sampling/createMessage`) — HOST-01
//!   * roots (`roots/list`) — HOST-03
//!   * elicitation (`elicitation/create`) — HOST-02
//!
//! ## Why a raw duplex pump (not `Server::run` + `PeerHandle`)
//!
//! All three cases drive the *server* side by hand: a fake server answers the
//! client's `initialize`, then pushes a raw inbound request frame over the
//! duplex transport while a client request (`tools/list`) is in flight, and
//! asserts the client's `Response` carries the value produced by the registered
//! host handler.
//!
//! This is deliberate. The high-level `Server::run` handles inbound messages on
//! a single serialized loop that `await`s each request handler inline, so a tool
//! that blocks on `extra.peer().sample()` cannot be answered by the same client
//! whose response the server must read — the server loop is busy awaiting the
//! handler (a pre-existing server-side serialization limitation, tracked for the
//! Phase 108 `SamplingSource` work). `PeerHandle` is also intentionally NOT
//! extended with an elicit method this phase. The raw pump exercises exactly the
//! client behavior under test — answering inbound requests from the registry —
//! without depending on that server-side concurrency.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;
#[path = "common/host_pump.rs"]
mod host_pump;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use proptest::prelude::*;
use serde_json::{json, Value};

use pmcp::client::host::{HostElicitationHandler, HostSamplingHandler};
use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
use pmcp::types::jsonrpc::{JSONRPCResponse, ResponsePayload};
use pmcp::types::protocol::{ClientRequest, Request, ServerRequest};
use pmcp::types::roots::{ListRootsResult, Root};
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResult, SamplingMessage, SamplingMessageContent, ToolChoice,
};
use pmcp::types::tools::ToolInfo;
use pmcp::types::{ClientCapabilities, Content, RequestId, Role};
use pmcp::{ClientBuilder, Result};

// ---------------------------------------------------------------------------
// Mock host handlers
// ---------------------------------------------------------------------------

/// Host sampling handler that records the params it observed and answers with a
/// canned completion.
struct CapturingSampling {
    observed: Arc<Mutex<Option<CreateMessageParams>>>,
    model: String,
}

#[async_trait]
impl HostSamplingHandler for CapturingSampling {
    async fn handle_create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResult> {
        *self.observed.lock().unwrap() = Some(params);
        Ok(CreateMessageResult::new(
            Content::text("host completion"),
            self.model.clone(),
        ))
    }
}

/// Host elicitation handler answering with a canned accepted result.
struct MockElicit;

#[async_trait]
impl HostElicitationHandler for MockElicit {
    async fn handle_elicitation(&self, _params: ElicitRequestParams) -> Result<ElicitResult> {
        let mut content = std::collections::HashMap::new();
        content.insert("answer".to_string(), json!("yes"));
        Ok(ElicitResult {
            action: ElicitAction::Accept,
            content: Some(content),
        })
    }
}

// ---------------------------------------------------------------------------
// Raw duplex pump: fake server that answers one inbound request nested inside
// an in-flight client `tools/list`.
// ---------------------------------------------------------------------------

/// Drive: initialize handshake, wait for the client's in-flight `tools/list`,
/// inject `inbound`, capture the client's `Response`, then answer the call so
/// the client returns. Yields the captured inbound `Response`. Thin wrapper
/// over the shared [`host_pump::inject_and_capture`] prefix.
async fn pump_one_inbound(
    mut server_t: duplex::DuplexTransport,
    inbound_id: RequestId,
    inbound: Request,
) -> JSONRPCResponse {
    host_pump::inject_and_capture(&mut server_t, inbound_id, inbound).await
}

fn result_payload(response: &JSONRPCResponse) -> Value {
    match &response.payload {
        ResponsePayload::Result(v) => v.clone(),
        ResponsePayload::Error(e) => panic!("expected a success result, got error: {e:?}"),
    }
}

// ---------------------------------------------------------------------------
// HOST-01: inbound sampling answered by the client host handler
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sampling_answered_by_host_handler() {
    let observed = Arc::new(Mutex::new(None));
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let sent = CreateMessageParams::new(vec![SamplingMessage::new(
        Role::User,
        SamplingMessageContent::Text {
            text: "summarize this".to_string(),
            meta: None,
        },
    )])
    .with_tools(vec![ToolInfo::new(
        "search",
        Some("search tool".to_string()),
        json!({"type": "object"}),
    )])
    .with_tool_choice(ToolChoice::auto());

    let inbound_id = RequestId::from("sample-1".to_string());
    // Inject the CLIENT-alias variant: this is what a real transport's
    // `parse_request` yields for an inbound `sampling/createMessage` (it tries
    // the client grammar first — the parse ambiguity the dispatcher handles).
    let inbound = Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(
        sent.clone(),
    ))));
    let server_task = tokio::spawn(pump_one_inbound(server_t, inbound_id.clone(), inbound));

    let mut client = ClientBuilder::new(client_t)
        .on_sampling(CapturingSampling {
            observed: observed.clone(),
            model: "host-model".to_string(),
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    let _ = client.list_tools(None).await.expect("list_tools succeeds");

    let response = server_task.await.expect("server task");
    assert_eq!(response.id, inbound_id);
    let result: CreateMessageResult =
        serde_json::from_value(result_payload(&response)).expect("CreateMessageResult");
    assert_eq!(result.model, "host-model");

    // The handler observed exactly the params the server sent (passthrough).
    let obs = observed.lock().unwrap().clone().expect("handler ran");
    assert_eq!(
        serde_json::to_value(&sent).unwrap(),
        serde_json::to_value(&obs).unwrap(),
        "params must pass through unchanged"
    );
}

// ---------------------------------------------------------------------------
// HOST-03: inbound roots answered by the client host provider
// ---------------------------------------------------------------------------

#[tokio::test]
async fn roots_answered_by_host_provider() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let inbound_id = RequestId::from("roots-1".to_string());
    let inbound = Request::Server(Box::new(ServerRequest::ListRoots));
    let server_task = tokio::spawn(pump_one_inbound(server_t, inbound_id.clone(), inbound));

    let mut client = ClientBuilder::new(client_t)
        .on_roots(|| async {
            Ok(ListRootsResult {
                roots: vec![Root {
                    uri: "file:///workspace".to_string(),
                    name: Some("workspace".to_string()),
                }],
            })
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    let _ = client.list_tools(None).await.expect("list_tools succeeds");

    let response = server_task.await.expect("server task");
    assert_eq!(response.id, inbound_id);
    let result: ListRootsResult =
        serde_json::from_value(result_payload(&response)).expect("ListRootsResult");
    assert_eq!(result.roots.len(), 1);
    assert_eq!(result.roots[0].uri, "file:///workspace");
}

// ---------------------------------------------------------------------------
// HOST-02: inbound elicitation answered by the client host handler
// ---------------------------------------------------------------------------

#[tokio::test]
async fn elicitation_answered_via_raw_pump() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let inbound_id = RequestId::from("elicit-1".to_string());
    let params = ElicitRequestParams::Form {
        message: "confirm?".to_string(),
        requested_schema: json!({"type": "object"}),
    };
    let inbound = Request::Server(Box::new(ServerRequest::ElicitationCreate(Box::new(params))));
    let server_task = tokio::spawn(pump_one_inbound(server_t, inbound_id.clone(), inbound));

    let mut client = ClientBuilder::new(client_t)
        .on_elicitation(MockElicit)
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    let _ = client.list_tools(None).await.expect("list_tools succeeds");

    let response = server_task.await.expect("server task");
    assert_eq!(response.id, inbound_id);
    let result: ElicitResult =
        serde_json::from_value(result_payload(&response)).expect("ElicitResult");
    assert_eq!(result.action, ElicitAction::Accept);
    assert_eq!(
        result.content.and_then(|c| c.get("answer").cloned()),
        Some(json!("yes")),
        "elicitation answer must come from the registered host handler"
    );
}

// ---------------------------------------------------------------------------
// WR-01: inbound `ping` answered with an empty result, connection survives
// ---------------------------------------------------------------------------

#[tokio::test]
async fn inbound_ping_answered_with_empty_result() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let inbound_id = RequestId::from("ping-1".to_string());
    // A server -> client ping parses via the client grammar.
    let inbound = Request::Client(Box::new(ClientRequest::Ping));
    let server_task = tokio::spawn(pump_one_inbound(server_t, inbound_id.clone(), inbound));

    // No host handlers registered: ping must still succeed (it is spec-level,
    // not registry-dependent).
    let mut client = ClientBuilder::new(client_t).build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    // The connection survives the inbound ping: the client's own request still
    // completes normally.
    let _ = client
        .list_tools(None)
        .await
        .expect("list_tools succeeds after inbound ping");

    let response = server_task.await.expect("server task");
    assert_eq!(response.id, inbound_id);
    assert!(
        response.is_success(),
        "inbound ping must be answered with a success result, got: {response:?}"
    );
    assert_eq!(
        result_payload(&response),
        json!({}),
        "ping response must be an empty object per the MCP spec"
    );
}

// ---------------------------------------------------------------------------
// Passthrough proptest: params observed by the host handler == server sent
// ---------------------------------------------------------------------------

/// Build a varied [`CreateMessageParams`] exercising tools / `tool_choice` and
/// `tool_use` / `tool_result` content blocks.
fn arb_params() -> impl Strategy<Value = CreateMessageParams> {
    (
        proptest::option::of("[a-z ]{0,16}"),
        proptest::option::of(0.0f64..1.0f64),
        proptest::option::of(1u32..4096u32),
        any::<bool>(),
        prop_oneof![
            Just(None),
            Just(Some(ToolChoice::auto())),
            Just(Some(ToolChoice::required())),
            Just(Some(ToolChoice::none())),
        ],
        "[a-z ]{1,24}",
        any::<bool>(),
    )
        .prop_map(
            |(system_prompt, temperature, max_tokens, has_tools, tool_choice, text, tool_msgs)| {
                let mut messages = vec![SamplingMessage::new(
                    Role::User,
                    SamplingMessageContent::Text { text, meta: None },
                )];
                if tool_msgs {
                    messages.push(SamplingMessage::new(
                        Role::Assistant,
                        SamplingMessageContent::ToolUse {
                            name: "search".to_string(),
                            id: "call-1".to_string(),
                            input: json!({"q": "rust"}),
                            meta: None,
                        },
                    ));
                    messages.push(SamplingMessage::new(
                        Role::User,
                        SamplingMessageContent::ToolResult {
                            tool_use_id: "call-1".to_string(),
                            content: vec![Content::text("found")],
                            structured_content: None,
                            is_error: None,
                            meta: None,
                        },
                    ));
                }
                let mut params = CreateMessageParams::new(messages);
                params.system_prompt = system_prompt;
                params.temperature = temperature;
                params.max_tokens = max_tokens;
                if has_tools {
                    params = params.with_tools(vec![ToolInfo::new(
                        "search",
                        Some("search".to_string()),
                        json!({"type": "object"}),
                    )]);
                }
                if let Some(tc) = tool_choice {
                    params = params.with_tool_choice(tc);
                }
                params
            },
        )
}

/// Drive one inbound-sampling round-trip and return the params the host handler
/// observed.
async fn observe_sampling(params: CreateMessageParams) -> CreateMessageParams {
    let observed = Arc::new(Mutex::new(None));
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let inbound = Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(params))));
    let server_task = tokio::spawn(pump_one_inbound(
        server_t,
        RequestId::from("s".to_string()),
        inbound,
    ));

    let mut client = ClientBuilder::new(client_t)
        .on_sampling(CapturingSampling {
            observed: observed.clone(),
            model: "m".to_string(),
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    let _ = client.list_tools(None).await.expect("list_tools");
    let _ = server_task.await.expect("server task");

    let obs = observed.lock().unwrap().clone();
    obs.expect("handler ran")
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, ..ProptestConfig::default() })]

    #[test]
    fn prop_sampling_passthrough(params in arb_params()) {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let observed = rt.block_on(observe_sampling(params.clone()));
        prop_assert_eq!(
            serde_json::to_value(&params).unwrap(),
            serde_json::to_value(&observed).unwrap()
        );
    }
}
