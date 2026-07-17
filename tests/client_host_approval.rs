//! Duplex approval / denial round-trip tests for the client host surface
//! (Phase 106, HOST-04).
//!
//! Proves the two host-side denial paths keep the connection alive:
//!   * a **preflight-denied** sampling request returns a sanitized `-32603`
//!     policy denial to the server, the LLM handler is NEVER invoked, and a
//!     subsequent client request still completes; and
//!   * a **known-but-unhandled** host method (`elicitation/create` with no
//!     registered handler) returns `-32601`, and a subsequent client request
//!     still completes.
//!
//! Both drive the *server* side by hand over the duplex transport (see the
//! sibling `client_host_roundtrip.rs` for why a raw pump is used instead of
//! `Server::run` + `PeerHandle`): the fake server answers `initialize`, waits
//! for the client's in-flight `tools/list`, injects a raw inbound request,
//! captures the client's `Response`, answers the first call, then answers a
//! SECOND `tools/list` to prove the connection survived the error.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::client::host::{ApprovalDecision, HostSamplingHandler};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::jsonrpc::{JSONRPCResponse, ResponsePayload};
use pmcp::types::protocol::{ClientRequest, Request, ServerRequest};
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
use pmcp::types::{
    ClientCapabilities, Content, Implementation, InitializeResult, RequestId, ServerCapabilities,
};
use pmcp::ClientBuilder;

// ---------------------------------------------------------------------------
// Mock host sampling handler that records whether it was invoked.
// ---------------------------------------------------------------------------

struct TrackingSampling {
    invoked: Arc<AtomicBool>,
}

#[async_trait]
impl HostSamplingHandler for TrackingSampling {
    async fn handle_create_message(
        &self,
        _params: CreateMessageParams,
    ) -> pmcp::Result<CreateMessageResult> {
        self.invoked.store(true, Ordering::SeqCst);
        Ok(CreateMessageResult::new(
            Content::text("host completion"),
            "host-model",
        ))
    }
}

// ---------------------------------------------------------------------------
// Raw duplex pump helpers.
// ---------------------------------------------------------------------------

async fn recv_request_id(t: &mut duplex::DuplexTransport) -> RequestId {
    loop {
        if let TransportMessage::Request { id, .. } = t.receive().await.expect("recv request") {
            return id;
        }
    }
}

async fn recv_response(t: &mut duplex::DuplexTransport) -> JSONRPCResponse {
    loop {
        if let TransportMessage::Response(r) = t.receive().await.expect("recv response") {
            return r;
        }
    }
}

async fn send_success(t: &mut duplex::DuplexTransport, id: RequestId, result: Value) {
    t.send(TransportMessage::Response(JSONRPCResponse::success(
        id, result,
    )))
    .await
    .expect("send response");
}

fn init_result_value() -> Value {
    serde_json::to_value(InitializeResult::new(
        Implementation::new("fake-server", "1.0.0"),
        ServerCapabilities::tools_only(),
    ))
    .unwrap()
}

/// Drive: initialize, wait for the first in-flight `tools/list`, inject
/// `inbound`, capture the client's `Response`, answer the first call, then
/// answer a SECOND `tools/list` (the connection-survival probe). Returns the
/// captured inbound `Response`.
async fn pump_inbound_then_survive(
    mut server_t: duplex::DuplexTransport,
    inbound_id: RequestId,
    inbound: Request,
) -> JSONRPCResponse {
    // 1. Answer initialize.
    let init_id = recv_request_id(&mut server_t).await;
    send_success(&mut server_t, init_id, init_result_value()).await;

    // 2. Wait for the first in-flight client request (skips `initialized`).
    let call_id_1 = recv_request_id(&mut server_t).await;

    // 3. Inject the raw inbound request while the first call is in flight.
    server_t
        .send(TransportMessage::Request {
            id: inbound_id,
            request: inbound,
        })
        .await
        .expect("send inbound request");

    // 4. Capture the client's Response to the inbound request.
    let inbound_response = recv_response(&mut server_t).await;

    // 5. Answer the first call so the first `list_tools` returns.
    send_success(&mut server_t, call_id_1, json!({ "tools": [] })).await;

    // 6. Connection-survival probe: answer a SECOND `tools/list`.
    let call_id_2 = recv_request_id(&mut server_t).await;
    send_success(&mut server_t, call_id_2, json!({ "tools": [] })).await;

    inbound_response
}

fn assert_error_code(response: &JSONRPCResponse, expected: i32) -> String {
    match &response.payload {
        ResponsePayload::Error(e) => {
            assert_eq!(e.code, expected, "unexpected JSON-RPC error code");
            e.message.clone()
        },
        ResponsePayload::Result(r) => panic!("expected error {expected}, got result: {r:?}"),
    }
}

// ---------------------------------------------------------------------------
// HOST-04: preflight denial -> sanitized -32603 -> connection survives.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sampling_preflight_deny_survives_connection() {
    let invoked = Arc::new(AtomicBool::new(false));
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let inbound_id = RequestId::from("sample-deny-1".to_string());
    // Inbound sampling parses as the CLIENT-alias variant (parse ambiguity).
    let inbound = Request::Client(Box::new(ClientRequest::CreateMessage(Box::new(
        CreateMessageParams::new(Vec::new()),
    ))));
    let server_task = tokio::spawn(pump_inbound_then_survive(
        server_t,
        inbound_id.clone(),
        inbound,
    ));

    let mut client = ClientBuilder::new(client_t)
        .on_sampling(TrackingSampling {
            invoked: invoked.clone(),
        })
        .on_sampling_approval(|_params| async {
            ApprovalDecision::Deny("local-policy-secret".to_string())
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    // First call: the server injects a sampling request mid-flight; the client
    // must deny it (preflight) and still complete this call.
    let _ = client.list_tools(None).await.expect("first list_tools");
    // Second call: proves the connection survived the denial.
    let _ = client.list_tools(None).await.expect("second list_tools");

    let inbound_response = server_task.await.expect("server task");
    assert_eq!(inbound_response.id, inbound_id);

    // The denial is a sanitized -32603 whose message is the generic policy
    // string — the raw deny reason must never cross the wire.
    let message = assert_error_code(&inbound_response, -32603);
    assert_eq!(message, "request denied by host policy");
    assert!(
        !message.contains("local-policy-secret"),
        "deny reason must be logged locally, not forwarded: {message}"
    );

    // The LLM handler was NEVER invoked — the real denial-of-wallet fix.
    assert!(
        !invoked.load(Ordering::SeqCst),
        "preflight Deny must prevent the LLM call entirely"
    );
}

// ---------------------------------------------------------------------------
// Orchestrator addendum: a KNOWN host method with no registered handler
// returns -32601, and a subsequent request still completes.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unhandled_known_method_returns_method_not_found_and_survives() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();

    let inbound_id = RequestId::from("elicit-unhandled-1".to_string());
    // `elicitation/create` is a KNOWN host method, but no handler is registered.
    let inbound = Request::Server(Box::new(ServerRequest::ElicitationCreate(Box::new(
        pmcp::types::elicitation::ElicitRequestParams::Form {
            message: "confirm?".to_string(),
            requested_schema: json!({"type": "object"}),
        },
    ))));
    let server_task = tokio::spawn(pump_inbound_then_survive(
        server_t,
        inbound_id.clone(),
        inbound,
    ));

    // No elicitation handler registered.
    let mut client = ClientBuilder::new(client_t).build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let _ = client.list_tools(None).await.expect("first list_tools");
    let _ = client.list_tools(None).await.expect("second list_tools");

    let inbound_response = server_task.await.expect("server task");
    assert_eq!(inbound_response.id, inbound_id);
    // Known-but-unhandled => -32601 (method not found), connection preserved.
    let _ = assert_error_code(&inbound_response, -32601);
}
