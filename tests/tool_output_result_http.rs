//! D-14 live-HTTP `_meta`-at-top-level acceptance gate (Phase 104, TOUT-04).
//!
//! This is the phase's wire-shape regression lock: a REAL HTTP round-trip (no
//! in-process shim) that drives a `tools/call` against a high-level, store-less
//! `pmcp::Server` whose tool returns
//! [`ToolOutput::Result`](pmcp::ToolOutput::Result)`(CallToolResult)` — the
//! SEP-1686 task-augmented result path (Plan 02/04). It consumes the REAL
//! dispatch output over `StreamableHttpServer` + `StreamableHttpTransport` and
//! asserts on the RAW wire JSON that:
//!
//! 1. `result._meta` is present at the result TOP LEVEL (the `_meta[related-task]`
//!    envelope survives dispatch — a `_meta`-sniffing client detects the task); AND
//! 2. `result.content[0].text`, if present, does NOT contain a stringified `_meta`
//!    (the agent-lake double-wrap bug — the whole `CallToolResult` serialized into
//!    `content[0].text` — must NEVER reappear).
//!
//! We read the raw JSON via the transport's `TransportMessage::Response` payload,
//! which carries the `tools/call` result as an untyped `serde_json::Value` — NOT
//! deserialized into `CallToolResult` — so the wire shape is observed faithfully
//! and never from a hand-authored fixture (the note's ask #4).
//!
//! Test reliability (carried from the Phase 102 HTTP harness):
//! - EPHEMERAL PORT — binds `127.0.0.1:0`, reads the bound address back from
//!   `StreamableHttpServer::start()` (no hardcoded port).
//! - READINESS — `start()` binds the listener before returning (no fixed sleep).
//! - SHUTDOWN — the spawned server `JoinHandle` is `abort()`ed after the round-trip.

#![cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::shared::{Transport, TransportMessage};
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::tasks::TaskMetadata;
use pmcp::types::{
    CallToolRequest, CallToolResult, ClientCapabilities, ClientRequest, Content, Implementation,
    InitializeRequest, Request,
};
use pmcp::{RequestHandlerExtra, Server, ToolHandler, ToolOutput};
use tokio::sync::Mutex;
use url::Url;

/// The task id the tool stamps into the related-task `_meta` envelope.
const RELATED_TASK_ID: &str = "t-http";

/// A tool whose `handle_output` returns [`ToolOutput::Result`] — a fully-formed
/// `CallToolResult` carrying a top-level `_meta[related-task]` envelope that must
/// reach the wire VERBATIM.
struct AugmentedResultTool;

impl AugmentedResultTool {
    /// The verbatim envelope: `content = [text("done")]` + related-task `_meta`.
    fn envelope() -> CallToolResult {
        CallToolResult::new(vec![Content::text("done")])
            .with_related_task(TaskMetadata::new(RELATED_TASK_ID))
    }
}

#[async_trait]
impl ToolHandler for AugmentedResultTool {
    async fn handle(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        // Serialize fallback for non-dispatch callers; the dispatch path uses
        // `handle_output` below.
        Ok(serde_json::to_value(Self::envelope())?)
    }

    async fn handle_output(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ToolOutput> {
        // Verbatim: the handler owns the full envelope (Plan 02/04, D-04a).
        Ok(ToolOutput::Result(Self::envelope()))
    }
}

/// Build a high-level, store-LESS `Server` exposing the augmented-result tool.
///
/// No `task_store` here: this is a plain synchronous tool result that merely
/// POINTS at a related task via `_meta` — exactly the SEP-1686 shape whose wire
/// survival this gate locks.
fn build_server() -> pmcp::Result<Server> {
    Server::builder()
        .name("tool-output-result-http")
        .version("1.0.0")
        .tool("augmented", AugmentedResultTool)
        .build()
}

/// Stand the server up over REAL HTTP; return the read-back bound address + handle.
///
/// EPHEMERAL PORT + READINESS: bind `127.0.0.1:0` and read the assigned address
/// back from `start()`, which binds the listener before returning (no sleep).
async fn spawn_http_server() -> pmcp::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let server = Arc::new(Mutex::new(build_server()?));
    let bind_addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback addr");
    let http_server = StreamableHttpServer::new(bind_addr, server);
    let (bound, handle) = http_server.start().await?;
    Ok((bound, handle))
}

/// Build the pmcp HTTP transport pointed at `bound` (single-response JSON path).
fn http_transport(bound: SocketAddr) -> pmcp::Result<StreamableHttpTransport> {
    let config = StreamableHttpTransportConfig {
        url: Url::parse(&format!("http://{bound}"))
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?,
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true,
        on_resumption_token: None,
        http_middleware_chain: None,
    };
    Ok(StreamableHttpTransport::new(config))
}

/// Extract the raw JSON-RPC `result` Value from a received response message.
fn expect_result_value(msg: TransportMessage) -> pmcp::Result<serde_json::Value> {
    match msg {
        TransportMessage::Response(resp) => match resp.payload {
            ResponsePayload::Result(value) => Ok(value),
            ResponsePayload::Error(err) => Err(pmcp::Error::internal(format!(
                "expected a result over HTTP, got JSON-RPC error: {}",
                err.message
            ))),
        },
        other => Err(pmcp::Error::internal(format!(
            "expected a Response message, got: {other:?}"
        ))),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_output_result_carries_meta_at_top_level_over_http() -> pmcp::Result<()> {
    let (bound, server_handle) = spawn_http_server().await?;

    // Guard the round-trip so the server handle is ALWAYS aborted, even on failure.
    let outcome = run_round_trip(bound).await;

    // SHUTDOWN: abort the spawned server task (no lingering listener, no hang).
    server_handle.abort();
    match server_handle.await {
        Ok(()) => {},
        Err(e) if e.is_cancelled() => {},
        Err(e) => panic!("HTTP server task ended unexpectedly: {e}"),
    }

    outcome
}

async fn run_round_trip(bound: SocketAddr) -> pmcp::Result<()> {
    let mut transport = http_transport(bound)?;

    // 1) initialize over HTTP (establishes the session + protocol version that the
    //    transport reuses on subsequent sends).
    transport
        .send(TransportMessage::Request {
            id: 1i64.into(),
            request: Request::Client(Box::new(ClientRequest::Initialize(InitializeRequest::new(
                Implementation::new("tout-04-gate", "1.0.0"),
                ClientCapabilities::default(),
            )))),
        })
        .await?;
    let _init = transport.receive().await?;

    // 2) tools/call over the REAL transport -> consume the RAW dispatch output.
    transport
        .send(TransportMessage::Request {
            id: 2i64.into(),
            request: Request::Client(Box::new(ClientRequest::CallTool(CallToolRequest::new(
                "augmented",
                serde_json::json!({}),
            )))),
        })
        .await?;
    let result = expect_result_value(transport.receive().await?)?;

    // ASSERTION 1 — `_meta` survives at the result TOP LEVEL.
    let meta = result.get("_meta").ok_or_else(|| {
        pmcp::Error::internal(format!(
            "result._meta must be present at top level over HTTP; got: {result}"
        ))
    })?;
    assert!(
        meta.get("io.modelcontextprotocol/related-task").is_some(),
        "result._meta must carry the related-task envelope, got: {meta}"
    );
    assert_eq!(
        meta["io.modelcontextprotocol/related-task"]["taskId"].as_str(),
        Some(RELATED_TASK_ID),
        "related-task taskId must equal the tool's minted id over HTTP"
    );

    // ASSERTION 2 — content[0].text (if any) must NOT be a stringified envelope.
    if let Some(text) = result
        .get("content")
        .and_then(|c| c.get(0))
        .and_then(|c0| c0.get("text"))
        .and_then(serde_json::Value::as_str)
    {
        assert_eq!(
            text, "done",
            "content[0].text must be the real text, not a wrapped envelope"
        );
        assert!(
            !text.contains("_meta"),
            "content[0].text must NOT contain a stringified `_meta` (the double-wrap bug), got: {text}"
        );
    }

    Ok(())
}
