//! TOUT-01 D-03 sugar-layer acceptance gate (Phase 104, Plan 04).
//!
//! Task 1 — `ServerBuilder::tool_with_result`: a closure returning a full
//! [`CallToolResult`] lands on the wire VERBATIM (its top-level `_meta` and
//! un-stringified `content` preserved), so a closure author can attach task
//! augmentation in one call without hand-writing a `ToolHandler`.
//!
//! Task 2 — `RequestHandlerExtra::set_result_meta`: an existing Payload-path
//! handler retrofits `_meta` with one call, round-tripping through the
//! encapsulated `Arc<std::sync::Mutex>` slot; merge precedence (handler-set key
//! overwrites same-name key, unrelated widget/native keys preserved), repeated
//! accumulation, no-op-when-never-called, and ignored-on-`ToolOutput::Result`.
//!
//! Both dispatchers are reachable, but these tests drive the high-level
//! `pmcp::Server` over an in-process duplex transport via a real `pmcp::Client`
//! (the sanctioned alternative — `Server::handle_request` is private).

#![cfg(all(not(target_arch = "wasm32"), feature = "schema-generation"))]

use async_trait::async_trait;
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::tasks::{TaskMetadata, RELATED_TASK_META_KEY};
use pmcp::types::{CallToolResult, ClientCapabilities, Content};
use pmcp::{Client, Error, Result, Server};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// In-process duplex transport (client <-> server), mpsc-backed.
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct DuplexTransport {
    tx: mpsc::UnboundedSender<TransportMessage>,
    rx: mpsc::UnboundedReceiver<TransportMessage>,
    connected: bool,
}

impl DuplexTransport {
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
// Task 1: tool_with_result — verbatim full CallToolResult.
// ---------------------------------------------------------------------------

#[derive(Deserialize, JsonSchema)]
struct StartArgs {
    /// The job name to start.
    job: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_with_result_lands_verbatim_meta_on_wire() {
    let server = Server::builder()
        .name("tool-with-result-server")
        .version("1.0.0")
        .tool_with_result("start_job", |args: StartArgs, _extra| {
            Box::pin(async move {
                Ok(
                    CallToolResult::new(vec![Content::text(format!("started {}", args.job))])
                        .with_related_task(TaskMetadata::new("t1")),
                )
            })
        })
        .build()
        .expect("server builds");

    let result = call_via_server(server, "start_job", json!({ "job": "backfill" })).await;
    let v = serde_json::to_value(&result).expect("serialize CallToolResult");

    // Top-level related-task _meta survives verbatim.
    assert_eq!(
        v["_meta"][RELATED_TASK_META_KEY]["taskId"], "t1",
        "top-level _meta[related-task].taskId must survive verbatim"
    );

    // Content is the handler's verbatim text — NOT a stringified envelope.
    let text = v["content"][0]["text"]
        .as_str()
        .expect("content[0].text is a string");
    assert_eq!(
        text, "started backfill",
        "content must be the handler's verbatim text, not a stringified value"
    );
    assert!(
        !text.contains(RELATED_TASK_META_KEY) && !text.contains("_meta"),
        "content must NOT be a stringified envelope (double-wrap bug)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_with_result_deserializes_typed_input() {
    // A closure that echoes a typed field proves TIn deserialization happens.
    let server = Server::builder()
        .name("typed-input-server")
        .version("1.0.0")
        .tool_with_result("echo_job", |args: StartArgs, _extra| {
            Box::pin(async move { Ok(CallToolResult::new(vec![Content::text(args.job)])) })
        })
        .build()
        .expect("server builds");

    let result = call_via_server(server, "echo_job", json!({ "job": "hello-typed" })).await;
    let v = serde_json::to_value(&result).expect("serialize");
    assert_eq!(v["content"][0]["text"], "hello-typed");
}

// ---------------------------------------------------------------------------
// Task 2 handlers + tests are appended below.
// ---------------------------------------------------------------------------
