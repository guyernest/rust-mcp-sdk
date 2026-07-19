//! Real `Server::run` + real `Client` proof for the Phase 108 Transport Actor
//! (D-01/D-02/D-03).
//!
//! Unlike `tests/client_host_roundtrip.rs` (which drives the server side by hand
//! with a raw pump because the OLD serialized loop could not answer an in-tool
//! `peer.sample()`), these cases run the STOCK high-level `Server::run` against a
//! STOCK `Client`. They prove the never-block transport actor:
//!
//!   * `sampling`   — a tool that awaits `extra.peer().sample()` completes.
//!   * `list_roots` — a tool that awaits `extra.peer().list_roots()` completes.
//!   * `saturation` — a SECOND `tools/call` queued while the first handler is
//!     parked on its sampling round-trip is still received and processed (the
//!     receive path never blocks on request execution / queue capacity).
//!   * `shutdown`   — closing the transport makes `run()` return without hanging.
//!   * `with_tools` — (Task 3) end-to-end `sample_with_tools` carrying a
//!     `ToolUse` block, added alongside the Task 2 `WithTools` surface.

#![cfg(not(target_arch = "wasm32"))]

#[path = "common/duplex.rs"]
mod duplex;

use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use pmcp::client::host::{HostSamplingHandler, HostSamplingHandlerWithTools};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResult, CreateMessageResultWithTools, SamplingMessage,
    SamplingMessageContent,
};
use pmcp::types::{ClientCapabilities, Content, JSONRPCResponse, Request, RequestId, Role};
use pmcp::{ClientBuilder, RequestHandlerExtra, Result, Server, ToolHandler};

// ---------------------------------------------------------------------------
// Host sampling handler answering with a canned single-content completion.
// ---------------------------------------------------------------------------

struct CannedSampling {
    model: String,
}

#[async_trait]
impl HostSamplingHandler for CannedSampling {
    async fn handle_create_message(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResult> {
        Ok(CreateMessageResult::new(
            Content::text("ok"),
            self.model.clone(),
        ))
    }
}

// ---------------------------------------------------------------------------
// Server tools that call back into the client via the peer handle.
// ---------------------------------------------------------------------------

/// Tool that awaits `extra.peer().sample()` and echoes the model name.
struct SamplerTool;

#[async_trait]
impl ToolHandler for SamplerTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra
            .peer()
            .expect("peer must be attached on the stock loop")
            .clone();
        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "summarize".to_string(),
                meta: None,
            },
        )]);
        let result = peer.sample(params).await?;
        Ok(json!(format!("sampled:{}", result.model)))
    }
}

/// Tool that awaits `extra.peer().list_roots()` and echoes the root count.
struct RootsTool;

#[async_trait]
impl ToolHandler for RootsTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra.peer().expect("peer must be attached").clone();
        let roots = peer.list_roots().await?;
        Ok(json!(format!("roots:{}", roots.roots.len())))
    }
}

/// Tool that awaits `extra.peer().sample_with_tools()` and reports the first
/// `tool_use` block it received (Task 3 / AGNT-04 proof).
struct SamplerWithToolsTool;

#[async_trait]
impl ToolHandler for SamplerWithToolsTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let peer = extra.peer().expect("peer must be attached").clone();
        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "pick a tool".to_string(),
                meta: None,
            },
        )]);
        let result: CreateMessageResultWithTools = peer.sample_with_tools(params).await?;
        let tool_use = result
            .content
            .iter()
            .find_map(|c| match c {
                SamplingMessageContent::ToolUse { id, name, .. } => Some(format!("{name}#{id}")),
                _ => None,
            })
            .unwrap_or_else(|| "none".to_string());
        Ok(json!(format!("tooluse:{tool_use}")))
    }
}

/// Trivial tool that returns immediately (no peer round-trip). Used to prove a
/// second request is drained + processed while another handler is parked.
struct FastTool;

#[async_trait]
impl ToolHandler for FastTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!("fast-done"))
    }
}

fn build_server() -> Server {
    Server::builder()
        .name("peer-roundtrip-server")
        .version("0.1.0")
        .tool("sampler", SamplerTool)
        .tool("roots", RootsTool)
        .tool("sampler_with_tools", SamplerWithToolsTool)
        .tool("fast", FastTool)
        .build()
        .expect("server builds")
}

fn result_text(result: &pmcp::types::CallToolResult) -> String {
    serde_json::to_value(result).unwrap().to_string()
}

// ---------------------------------------------------------------------------
// (a) In-tool peer.sample() completes on the stock loop.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn in_tool_sample_completes_on_stock_loop() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling(CannedSampling {
            model: "host-model".to_string(),
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("sampler".to_string(), json!({})),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    assert!(
        result_text(&result).contains("sampled:host-model"),
        "tool must observe the host completion model: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}

// ---------------------------------------------------------------------------
// (b) In-tool peer.list_roots() completes on the stock loop.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn in_tool_list_roots_completes_on_stock_loop() {
    use pmcp::types::roots::{ListRootsResult, Root};

    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_roots(|| async {
            Ok(ListRootsResult {
                roots: vec![
                    Root {
                        uri: "file:///a".to_string(),
                        name: Some("a".to_string()),
                    },
                    Root {
                        uri: "file:///b".to_string(),
                        name: Some("b".to_string()),
                    },
                ],
            })
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("roots".to_string(), json!({})),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    assert!(
        result_text(&result).contains("roots:2"),
        "tool must observe the two host roots: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}

// ---------------------------------------------------------------------------
// (c) SATURATION: a second request queued while the first handler is parked on
// its sampling round-trip is still received and processed.
//
// Driven at the raw transport level because the high-level `Client` issues one
// request at a time; here we interleave two `tools/call`s by hand so the second
// lands while the worker is blocked awaiting the sampling answer.
// ---------------------------------------------------------------------------

/// Build an inbound client->server `Request` from method + params, bypassing the
/// `#[non_exhaustive]` request structs via deserialization.
fn client_req(method: &str, params: Value) -> Request {
    let mut obj = serde_json::Map::new();
    obj.insert("method".to_string(), Value::from(method));
    obj.insert("params".to_string(), params);
    let cr = serde_json::from_value(Value::Object(obj)).expect("valid ClientRequest");
    Request::Client(Box::new(cr))
}

fn init_params() -> Value {
    json!({
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": { "name": "raw-test-client", "version": "1.0.0" }
    })
}

#[tokio::test]
async fn second_request_is_processed_while_first_handler_parks() {
    let (mut client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    // Handshake.
    client_t
        .send(TransportMessage::Request {
            id: RequestId::from(0i64),
            request: client_req("initialize", init_params()),
        })
        .await
        .unwrap();

    // Read until the initialize response arrives.
    loop {
        if let TransportMessage::Response(r) = client_t.receive().await.unwrap() {
            if r.id == RequestId::from(0i64) {
                break;
            }
        }
    }

    // Issue call #1 (sampler — will park on peer.sample) then call #2 (fast).
    // Both frames go out BEFORE we answer the sampling request, so #2 must be
    // drained off the wire and queued while the single worker is parked on #1.
    client_t
        .send(TransportMessage::Request {
            id: RequestId::from(1i64),
            request: client_req("tools/call", json!({ "name": "sampler", "arguments": {} })),
        })
        .await
        .unwrap();
    client_t
        .send(TransportMessage::Request {
            id: RequestId::from(2i64),
            request: client_req("tools/call", json!({ "name": "fast", "arguments": {} })),
        })
        .await
        .unwrap();

    // Now drive the client side: answer the inbound sampling request, then
    // collect both tool responses. If the receive path blocked while the worker
    // parked, the server could never read our sampling answer -> timeout.
    let mut got_1 = false;
    let mut got_2 = false;
    let driver = async {
        while !(got_1 && got_2) {
            match client_t.receive().await.unwrap() {
                TransportMessage::Request { id, request: _ } => {
                    // The only inbound request is the server's sampling call.
                    let answer = CreateMessageResult::new(Content::text("done"), "host-model");
                    client_t
                        .send(TransportMessage::Response(JSONRPCResponse::success(
                            id,
                            serde_json::to_value(&answer).unwrap(),
                        )))
                        .await
                        .unwrap();
                },
                TransportMessage::Response(r) => {
                    if r.id == RequestId::from(1i64) {
                        got_1 = true;
                    } else if r.id == RequestId::from(2i64) {
                        got_2 = true;
                    }
                },
                TransportMessage::Notification(_) => {},
            }
        }
    };

    tokio::time::timeout(Duration::from_secs(5), driver)
        .await
        .expect("both tool calls must complete (no deadlock)");

    assert!(got_1 && got_2, "both queued requests must be answered");
    server_handle.abort();
}

// ---------------------------------------------------------------------------
// (d) SHUTDOWN: closing the transport makes run() return.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_returns_when_transport_closes() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move { server.run(server_t).await });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling(CannedSampling {
            model: "host-model".to_string(),
        })
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");
    let _ = client
        .call_tool("sampler".to_string(), json!({}))
        .await
        .expect("tools/call succeeds");

    // Drop the client -> the server's transport.receive() errors -> the actor
    // breaks -> run() returns.
    drop(client);

    let run_result = tokio::time::timeout(Duration::from_secs(5), server_handle)
        .await
        .expect("run() must return after the transport closes")
        .expect("server task joins");
    assert!(run_result.is_ok(), "run() returns Ok on clean shutdown");
}

// ---------------------------------------------------------------------------
// (e) WithTools end-to-end (AGNT-04): a tool that awaits
// peer.sample_with_tools() receives a ToolUse block from a WithTools host
// handler, intact, on the stock loop.
// ---------------------------------------------------------------------------

/// `WithTools` host handler answering with a `tool_use` block.
struct ToolUseSampling;

#[async_trait]
impl HostSamplingHandlerWithTools for ToolUseSampling {
    async fn handle_create_message_with_tools(
        &self,
        _params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools> {
        Ok(CreateMessageResultWithTools::new(
            "tool-model",
            Role::Assistant,
            vec![SamplingMessageContent::ToolUse {
                name: "search".to_string(),
                id: "call-42".to_string(),
                input: json!({ "q": "rust" }),
                meta: None,
            }],
        ))
    }
}

#[tokio::test]
async fn in_tool_sample_with_tools_preserves_tool_use_end_to_end() {
    let (client_t, server_t) = duplex::DuplexTransport::pair();
    let server = build_server();
    let server_handle = tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });

    let mut client = ClientBuilder::new(client_t)
        .on_sampling_with_tools(ToolUseSampling)
        .build();
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("initialize");

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        client.call_tool("sampler_with_tools".to_string(), json!({})),
    )
    .await
    .expect("call must not hang")
    .expect("tools/call succeeds");

    // The ToolUse block (name + id) survives to the server-side
    // CreateMessageResultWithTools.
    assert!(
        result_text(&result).contains("tooluse:search#call-42"),
        "tool_use block (name + id) must survive end-to-end: {}",
        result_text(&result)
    );

    drop(client);
    server_handle.abort();
}
