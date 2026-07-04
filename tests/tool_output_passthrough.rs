//! TOUT-01 pass-through acceptance gate (Phase 104, Plan 02).
//!
//! Proves the `src/server/mod.rs` double-wrap junction is closed WITHOUT any
//! breaking change: a `ToolHandler` that returns `ToolOutput::Result(_)` lands
//! that `CallToolResult` on the wire VERBATIM (its `_meta` preserved, its
//! `content` un-stringified), while a handler that returns `ToolOutput::Payload`
//! (the default for existing handlers) keeps today's text-wrap + create-path
//! behavior exactly.
//!
//! Both native dispatchers are exercised: the high-level `pmcp::Server` (driven
//! via `Server::run` over an in-process duplex transport) and the `ServerCore`
//! `ProtocolHandler` (driven via a server pump), so the shared decision helper
//! (`task_dispatch::resolve_tool_output`, D-05) cannot drift between them.
//!
//! Task 2 (this initial file): verbatim `_meta` survival, Payload regression,
//! task-shaped-Payload create-path precedence, and Server-vs-ServerCore parity.
//! Task 3 appends the D-04a middleware/error regression battery.

#![cfg(not(target_arch = "wasm32"))]

use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
use pmcp::server::typed_tool::TypedTool;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::tasks::RELATED_TASK_META_KEY;
use pmcp::types::{CallToolResult, ClientCapabilities, Content, TaskSupport, ToolExecution};
use pmcp::{Client, Error, RequestHandlerExtra, Result, Server, ToolHandler, ToolOutput};
use serde_json::{json, Value};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// In-process duplex transport (client <-> server), mpsc-backed.
// ---------------------------------------------------------------------------

/// One half of an in-process duplex transport. The client side sends Requests
/// and receives Responses; the server side does the reverse.
#[derive(Debug)]
struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
    /// Create a connected client/server transport pair.
    fn pair() -> (Self, Self) {
        let (client_tx, server_rx) = mpsc::unbounded_channel();
        let (server_tx, client_rx) = mpsc::unbounded_channel();
        (
            Self {
                tx: client_tx,
                rx: client_rx,
                connected: true,
            },
            Self {
                tx: server_tx,
                rx: server_rx,
                connected: true,
            },
        )
    }
}

#[async_trait]
impl Transport for DuplexTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        self.tx
            .send(message)
            .map_err(|_| Error::internal("duplex peer dropped"))
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        self.rx
            .recv()
            .await
            .ok_or_else(|| Error::internal("duplex peer closed"))
    }

    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> &'static str {
        "in-process-duplex"
    }
}

/// Spawn a server pump that serves a `ProtocolHandler` (ServerCore) over the
/// server side of a duplex transport: receive Request, dispatch, send Response.
fn spawn_core_pump(mut server_transport: DuplexTransport, handler: Arc<dyn ProtocolHandler>) {
    tokio::spawn(async move {
        while let Ok(message) = server_transport.receive().await {
            if let TransportMessage::Request { id, request } = message {
                let response = handler.handle_request(id, request, None).await;
                if server_transport
                    .send(TransportMessage::Response(response))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Test handlers.
// ---------------------------------------------------------------------------

/// A hand-written `ToolHandler` (the pmcp.run case) that OVERRIDES
/// `handle_output` to own the full `CallToolResult` envelope, including a
/// top-level `_meta[relatedTask]`. Its `handle` returns a sentinel that must
/// NEVER reach the wire on the native path (the dispatcher calls `handle_output`).
struct VerbatimTool;

#[async_trait]
impl ToolHandler for VerbatimTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
        // If this value ever lands on the wire, the dispatcher wrongly called
        // handle() instead of handle_output() — the assertions will catch it.
        Ok(json!({ "via": "handle-should-not-be-called" }))
    }

    async fn handle_output(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<ToolOutput> {
        let mut meta = serde_json::Map::new();
        meta.insert(
            RELATED_TASK_META_KEY.to_string(),
            json!({ "taskId": "hint-123" }),
        );
        let result = CallToolResult::new(vec![Content::text("verbatim content")]).with_meta(meta);
        Ok(ToolOutput::Result(result))
    }
}

/// A default (Payload-path) tool: returns a plain value the server text-wraps.
fn payload_tool() -> impl ToolHandler {
    TypedTool::new_with_schema(
        "payload_tool",
        json!({ "type": "object" }),
        |_args: Value, _extra| Box::pin(async { Ok(json!({ "answer": 42 })) }),
    )
    .with_description("A plain Payload-path tool")
}

/// A task-shaped tool (TaskSupport::Required) that returns a synchronously
/// completing task value — used to prove the Payload create-path gate still wins.
fn completing_task_tool() -> impl ToolHandler {
    TypedTool::new_with_schema(
        "complete_now",
        json!({ "type": "object" }),
        |_args: Value, _extra| {
            Box::pin(async {
                Ok(json!({
                    "taskId": "tool-fabricated",
                    "status": "completed",
                    "ttl": 60000,
                    "createdAt": "2026-07-04T00:00:00Z",
                    "lastUpdatedAt": "2026-07-04T00:00:00Z",
                    "result": {
                        "content": [ { "type": "text", "text": "terminal result payload" } ]
                    }
                }))
            })
        },
    )
    .with_description("A task tool that completes synchronously")
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))
}

// ---------------------------------------------------------------------------
// Server builders + drivers.
// ---------------------------------------------------------------------------

fn build_core_with_verbatim() -> Arc<dyn ProtocolHandler> {
    Arc::new(
        ServerCoreBuilder::new()
            .name("passthrough-core")
            .version("1.0.0")
            .tool("verbatim_tool", VerbatimTool)
            .tool("payload_tool", payload_tool())
            .build()
            .expect("core builds"),
    )
}

fn build_server_with_verbatim() -> Server {
    Server::builder()
        .name("passthrough-server")
        .version("1.0.0")
        .tool("verbatim_tool", VerbatimTool)
        .tool("payload_tool", payload_tool())
        .build()
        .expect("server builds")
}

/// Drive a `tools/call` through a real `pmcp::Client` against a ServerCore pump.
async fn call_via_core(core: Arc<dyn ProtocolHandler>, name: &str, args: Value) -> CallToolResult {
    let (client_t, server_t) = DuplexTransport::pair();
    spawn_core_pump(server_t, core);
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against core");
    client
        .call_tool(name.to_string(), args)
        .await
        .expect("tools/call succeeds against core")
}

/// Drive a `tools/call` through a real `pmcp::Client` against a high-level
/// `Server` running its own transport loop.
async fn call_via_server(server: Server, name: &str, args: Value) -> CallToolResult {
    let (client_t, server_t) = DuplexTransport::pair();
    tokio::spawn(async move {
        let _ = server.run(server_t).await;
    });
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes against server");
    client
        .call_tool(name.to_string(), args)
        .await
        .expect("tools/call succeeds against server")
}

// ---------------------------------------------------------------------------
// Assertions helpers.
// ---------------------------------------------------------------------------

/// Assert a verbatim ToolOutput::Result: top-level `_meta[relatedTask].taskId`
/// present AND `content[0].text` is the verbatim string, NOT a stringified
/// envelope (i.e. it does not contain the `_meta` key text).
fn assert_verbatim(result: &CallToolResult) {
    let v = serde_json::to_value(result).expect("serialize CallToolResult");
    assert_eq!(
        v["_meta"][RELATED_TASK_META_KEY]["taskId"], "hint-123",
        "top-level _meta[relatedTask].taskId must survive verbatim"
    );
    let text = v["content"][0]["text"]
        .as_str()
        .expect("content[0].text is a string");
    assert_eq!(
        text, "verbatim content",
        "content must be the handler's verbatim text, not a stringified value"
    );
    assert!(
        !text.contains(RELATED_TASK_META_KEY) && !text.contains("_meta"),
        "content must NOT be a stringified envelope (double-wrap bug)"
    );
    assert!(
        !text.contains("handle-should-not-be-called"),
        "dispatcher must call handle_output, not handle"
    );
}

// ---------------------------------------------------------------------------
// Task 2 tests.
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn verbatim_result_meta_survives_on_server() {
    let result = call_via_server(build_server_with_verbatim(), "verbatim_tool", json!({})).await;
    assert_verbatim(&result);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn verbatim_result_meta_survives_on_core() {
    let result = call_via_core(build_core_with_verbatim(), "verbatim_tool", json!({})).await;
    assert_verbatim(&result);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn payload_path_text_wraps_and_has_no_verbatim_meta_on_server() {
    let result = call_via_server(build_server_with_verbatim(), "payload_tool", json!({})).await;
    let v = serde_json::to_value(&result).expect("serialize");
    // Payload tools are text-wrapped: content[0].text is the stringified value.
    let text = v["content"][0]["text"].as_str().expect("text present");
    let parsed: Value = serde_json::from_str(text).expect("payload text is the stringified value");
    assert_eq!(parsed, json!({ "answer": 42 }), "payload value round-trips");
    // Non-widget, non-verbatim payload tool carries no handler-owned _meta.
    assert!(
        v.get("_meta").map_or(true, Value::is_null),
        "payload path must not inject a verbatim _meta"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn payload_path_text_wraps_on_core() {
    let result = call_via_core(build_core_with_verbatim(), "payload_tool", json!({})).await;
    let v = serde_json::to_value(&result).expect("serialize");
    let text = v["content"][0]["text"].as_str().expect("text present");
    let parsed: Value = serde_json::from_str(text).expect("payload text is the stringified value");
    assert_eq!(parsed, json!({ "answer": 42 }), "payload value round-trips");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_shaped_payload_still_routes_through_create_path() {
    // A task-augmented call to a task-shaped Payload tool must still mint a task
    // (create-path precedence preserved) — NOT fall onto the verbatim path.
    let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
    let core: Arc<dyn ProtocolHandler> = Arc::new(
        ServerCoreBuilder::new()
            .name("create-path-core")
            .version("1.0.0")
            .tool("complete_now", completing_task_tool())
            .task_store(store)
            .build()
            .expect("core builds"),
    );

    let (client_t, server_t) = DuplexTransport::pair();
    spawn_core_pump(server_t, core);
    let mut client = Client::new(client_t);
    client
        .initialize(ClientCapabilities::default())
        .await
        .expect("client initializes");

    let response = client
        .call_tool_with_task("complete_now".to_string(), json!({}))
        .await
        .expect("task-augmented tools/call succeeds");

    match response {
        pmcp::client::ToolCallResponse::Task(task) => {
            // Store-minted id, never the tool-fabricated one (D-STORE-MINTS-ID).
            assert_ne!(
                task.task_id, "tool-fabricated",
                "create-path must mint the canonical id"
            );
        },
        pmcp::client::ToolCallResponse::Result(r) => {
            panic!("create-path precedence lost — got a CallToolResult: {r:?}");
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn server_and_core_emit_equal_verbatim_result() {
    let via_server =
        call_via_server(build_server_with_verbatim(), "verbatim_tool", json!({})).await;
    let via_core = call_via_core(build_core_with_verbatim(), "verbatim_tool", json!({})).await;

    let sv = serde_json::to_value(&via_server).expect("serialize server result");
    let cv = serde_json::to_value(&via_core).expect("serialize core result");
    assert_eq!(
        sv, cv,
        "Server and ServerCore must emit byte-equal serialized results for the same ToolOutput::Result"
    );
}
