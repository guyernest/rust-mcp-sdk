//! Phase 113-05 (CLNT-01): live acceptance that the pmcp `Client` speaks v2.
//!
//! Every test here drives a REAL `pmcp::Client` over a REAL
//! `StreamableHttpTransport` against a REAL `StreamableHttpServer` on a loopback
//! TCP socket. This is the first end-to-end proof that pmcp's OWN client is
//! accepted by pmcp's OWN Phase-112 strict v2 header gate — RESEARCH Pitfall 7
//! measured the gap (the client transport emitted ZERO `Mcp-Method`/`Mcp-Name`
//! headers, so every v2 request from a pmcp client was rejected).
//!
//! # The assertion style is deliberate
//!
//! Most tests here assert only that the call SUCCEEDS. That is not a weak
//! assertion: the server under test runs `require_three_headers` +
//! `cross_check_method` + `cross_check_name` + the header/`_meta` era matrix and
//! answers `-32020 HEADER_MISMATCH` at HTTP 400 for any disagreement. A success
//! therefore proves the client emitted all three headers, with the right
//! `Mcp-Name` derivation, and stamped `params._meta` with the v2 era signal. The
//! two tests that need to observe what the server RECEIVED use a thin recording
//! `ServerHttpMiddleware` rather than parsing logs.
//!
//! Servers are spawned with [`common::v2::spawn_default_config`] — the STATEFUL
//! `StreamableHttpServerConfig::default()` — so the per-request era gate (not a
//! build-time stateless config) is what makes these session-free (RESEARCH
//! Pitfall 1).
//!
//! Test reliability doctrine (carried from `tests/v2_required_headers.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after each
//! round-trip).
//!
//! # Structure
//!
//! Plan 07 (mock-transport MRTR) and plan 13 EXTEND this file. Helpers live at the
//! top, tests below, so later work APPENDS rather than rewrites.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, extensions_capabilities, spawn_default_config, spawn_with, GreetingPrompt,
    GreetingResource, SearchTool, V1, V2,
};

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::http_middleware::{
    ServerHttpContext, ServerHttpMiddleware, ServerHttpMiddlewareChain, ServerHttpRequest,
};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
use pmcp::server::Server;
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
use pmcp::shared::StreamableHttpTransport;
use pmcp::types::protocol::ProtocolVersion;
use pmcp::types::ClientCapabilities;
use pmcp::{Client, ClientBuilder};
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};
use tokio::task::JoinHandle;
use url::Url;

// ===========================================================================
// Helpers.
// ===========================================================================

/// A tool whose name is NOT header-safe, so `Mcp-Name` must travel in the
/// `=?base64?…?=` sentinel form (T-113-47). Decoding it on the server is plan
/// 04's work, which is why this plan depends on 113-04.
const NON_ASCII_TOOL: &str = "поиск-☂";

/// The `mem://` resource `build_v2_server` registers a handler for.
const RESOURCE_URI: &str = "mem://greeting";

/// A second trivial tool, registered under [`NON_ASCII_TOOL`].
struct UnicodeTool;

#[async_trait]
impl ToolHandler for UnicodeTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "answer": "unicode ok" }))
    }
}

/// The v2-opted-in harness server, plus a tool whose name needs sentinel
/// encoding.
fn build_server_with_unicode_tool() -> Server {
    Server::builder()
        .name("v2-client-harness")
        .version("1.0.0")
        .capabilities(extensions_capabilities())
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool("search", SearchTool)
        .tool(NON_ASCII_TOOL, UnicodeTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource)
        .build()
        .expect("server builds")
}

/// What a [`RecordingMiddleware`] observed about the requests that arrived.
#[derive(Debug, Default)]
struct Observed {
    /// Set when ANY request body carried `"method":"initialize"` or
    /// `"notifications/initialized"`.
    handshake: AtomicBool,
    /// Set when ANY request carried an inbound `Mcp-Session-Id` header.
    inbound_session_id: AtomicBool,
    /// Set when at least one request arrived at all (guards vacuous assertions).
    traffic_arrived: AtomicBool,
}

/// A thin recording wrapper at the HTTP boundary.
///
/// Preferred over log parsing (and over a tool-handler wrapper) because the two
/// facts under observation — "was `initialize` ever sent?" and "did an inbound
/// `Mcp-Session-Id` arrive?" — live at the transport layer, not in a handler.
struct RecordingMiddleware {
    observed: Arc<Observed>,
}

#[async_trait]
impl ServerHttpMiddleware for RecordingMiddleware {
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        _context: &ServerHttpContext,
    ) -> pmcp::Result<()> {
        self.observed.traffic_arrived.store(true, Ordering::SeqCst);
        if request.get_header(MCP_SESSION_ID).is_some() {
            self.observed
                .inbound_session_id
                .store(true, Ordering::SeqCst);
        }
        let method = serde_json::from_slice::<Value>(&request.body)
            .ok()
            .and_then(|body| {
                body.get("method")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
        if matches!(
            method.as_deref(),
            Some("initialize" | "notifications/initialized")
        ) {
            self.observed.handshake.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

/// Spawn the harness server with a [`RecordingMiddleware`] installed.
async fn spawn_recording(server: Server) -> (SocketAddr, JoinHandle<()>, Arc<Observed>) {
    let observed = Arc::new(Observed::default());
    let mut chain = ServerHttpMiddlewareChain::new();
    chain.add(Arc::new(RecordingMiddleware {
        observed: observed.clone(),
    }));
    // The STATEFUL default config (a live `session_id_generator`), so what makes
    // these round trips session-free is the PER-REQUEST era gate, not a
    // build-time stateless branch (RESEARCH Pitfall 1).
    let config = StreamableHttpServerConfig {
        http_middleware: Some(Arc::new(chain)),
        ..StreamableHttpServerConfig::default()
    };
    let (addr, handle) = spawn_with(server, config).await;
    (addr, handle, observed)
}

/// A `StreamableHttpTransport` pointed at `addr`.
fn transport_for(addr: SocketAddr) -> StreamableHttpTransport {
    let url = Url::parse(&format!("http://{addr}/")).expect("loopback URL parses");
    StreamableHttpTransport::new(StreamableHttpTransportConfigBuilder::new(url).build())
}

/// A pmcp client that OPTED INTO `2026-07-28` — no handshake, v2 headers,
/// per-request `_meta`, no session.
fn v2_client(addr: SocketAddr) -> Client<StreamableHttpTransport> {
    ClientBuilder::new(transport_for(addr))
        .with_protocol_version(ProtocolVersion(V2.to_string()))
        .expect("2026-07-28 is selectable")
        .build()
}

/// A pmcp client that made NO era selection — today's behavior, handshake and all.
fn v1_client(addr: SocketAddr) -> Client<StreamableHttpTransport> {
    ClientBuilder::new(transport_for(addr)).build()
}

// ===========================================================================
// Tests.
// ===========================================================================

/// A v2 `tools/call` is ACCEPTED by the strict Phase-112 header gate.
///
/// Success is the assertion: a missing or mismatched `Mcp-Method` / `Mcp-Name` /
/// `MCP-Protocol-Version`, or a missing `params._meta` era signal, is a 400.
#[tokio::test]
async fn emits_required_headers() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.call_tool("search".to_string(), json!({})).await;

    handle.abort();
    assert!(
        result.is_ok(),
        "a v2 pmcp client must be accepted by pmcp's own v2 header gate: {result:?}"
    );
}

/// The client half of the empty-`Mcp-Name` rule.
///
/// `tools/list` carries no logical name. If the client OMITTED `Mcp-Name`, the
/// server's `require_three_headers` would 400 this request; if it omitted the
/// `_meta` era signal, the header/`_meta` matrix would 400 it as a
/// `HEADER_MISMATCH`. Neither struct has a `_meta` field, so this only passes
/// because the v2 frame is assembled and stamped by the client itself.
#[tokio::test]
async fn nameless_method_accepted() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.list_tools(None).await;

    handle.abort();
    let tools = result.expect("a v2 tools/list must be accepted");
    assert!(
        tools.tools.iter().any(|tool| tool.name == "search"),
        "the listing must be the real one: {:?}",
        tools.tools
    );
}

/// `Mcp-Name` for `resources/read` comes from `params.uri`, NOT `params.name`.
///
/// A `ReadResourceRequest` has no `name` field, so a client that read the wrong
/// key would send an empty `Mcp-Name` and fail the server's body cross-check.
#[tokio::test]
async fn mcp_name_from_uri_for_resources_read() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.read_resource(RESOURCE_URI.to_string()).await;

    handle.abort();
    let read = result.expect("a v2 resources/read must be accepted");
    assert!(!read.contents.is_empty(), "the handler must have run");
}

/// A non-header-safe tool name round-trips through the `=?base64?…?=` sentinel.
///
/// This proves the client ENCODER and the server DECODER agree — the reason this
/// plan depends on 113-04, which shipped the decode half.
#[tokio::test]
async fn mcp_name_sentinel_for_non_ascii() {
    let (addr, handle) = spawn_default_config(build_server_with_unicode_tool()).await;
    let client = v2_client(addr);

    let result = client
        .call_tool(NON_ASCII_TOOL.to_string(), json!({}))
        .await;

    handle.abort();
    assert!(
        result.is_ok(),
        "a sentinel-encoded Mcp-Name must survive the server's cross-check: {result:?}"
    );
}

/// A v2 client completes a `tools/call` having sent NO `initialize` and no
/// `notifications/initialized`.
#[tokio::test]
async fn no_initialize_on_v2() {
    let (addr, handle, observed) = spawn_recording(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client.call_tool("search".to_string(), json!({})).await;

    handle.abort();
    assert!(result.is_ok(), "the call must succeed: {result:?}");
    assert!(
        observed.traffic_arrived.load(Ordering::SeqCst),
        "the recording middleware must have seen the traffic (guard against a vacuous pass)"
    );
    assert!(
        !observed.handshake.load(Ordering::SeqCst),
        "v2 has no handshake — neither initialize nor notifications/initialized may be sent"
    );
}

/// A v2 client never puts `Mcp-Session-Id` on the wire (T-113-06 / HTTP-01).
#[tokio::test]
async fn no_session_id_from_v2_client() {
    let (addr, handle, observed) = spawn_recording(build_v2_server()).await;
    let client = v2_client(addr);

    // Two round trips: if the server ever handed out a session id, a naive
    // client would echo it on the second request.
    let first = client.call_tool("search".to_string(), json!({})).await;
    let second = client.list_tools(None).await;

    handle.abort();
    assert!(first.is_ok(), "first call must succeed: {first:?}");
    assert!(second.is_ok(), "second call must succeed: {second:?}");
    assert!(
        !observed.inbound_session_id.load(Ordering::SeqCst),
        "no v2 request may carry Mcp-Session-Id, on any round trip"
    );
}

/// Regression guard for the `assert_capability` blocker.
///
/// `server_capabilities` is populated only by `initialize`, which v2 does not
/// have. Before this plan every `call_tool` on a v2 client failed LOCALLY, before
/// a byte was sent. This client never calls `server_discover`.
#[tokio::test]
async fn capability_check_does_not_block_v2() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    assert!(
        client.get_server_capabilities().is_none(),
        "this test is only meaningful with nothing observed"
    );
    let result = client.call_tool("search".to_string(), json!({})).await;

    handle.abort();
    assert!(
        result.is_ok(),
        "a v2 client must not fail a capability check it could not possibly have learned: {result:?}"
    );
}

/// `server/discover` — the v2 replacement for the `initialize` handshake.
///
/// It is EXPLICIT (pmcp never calls it implicitly, and never to CHOOSE an era —
/// D-08), and it STORES the projection, after which capability enforcement is as
/// strict as v1's.
#[tokio::test]
async fn server_discover_from_v2_client() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let mut client = v2_client(addr);

    let discovered = client.server_discover().await;
    let discovered = match discovered {
        Ok(value) => value,
        Err(error) => {
            handle.abort();
            panic!("server/discover must succeed on a v2 client: {error:?}");
        },
    };

    assert_eq!(discovered.protocol_version, V2);
    assert!(
        discovered.capabilities.tools.is_some(),
        "the projection must carry the server's real capabilities: {:?}",
        discovered.capabilities
    );
    assert!(
        client.get_server_capabilities().is_some(),
        "server_discover must STORE what it learned"
    );

    // And enforcement now runs against the DISCOVERED capabilities.
    let result = client.call_tool("search".to_string(), json!({})).await;
    handle.abort();
    assert!(
        result.is_ok(),
        "a discovered `tools` capability must let the call through: {result:?}"
    );
}

/// A client that never opted in is byte-identical to today: full `initialize`
/// handshake against the SAME server, then a normal `tools/call`.
#[tokio::test]
async fn v1_client_unchanged() {
    let (addr, handle, observed) = spawn_recording(build_v2_server()).await;
    let mut client = v1_client(addr);

    let init = client.initialize(ClientCapabilities::default()).await;
    let init = match init {
        Ok(value) => value,
        Err(error) => {
            handle.abort();
            panic!("a v1 client must still handshake: {error:?}");
        },
    };
    assert_eq!(init.protocol_version.as_str(), V1);

    let result = client.call_tool("search".to_string(), json!({})).await;
    handle.abort();

    assert!(result.is_ok(), "a v1 tools/call must succeed: {result:?}");
    assert!(
        observed.handshake.load(Ordering::SeqCst),
        "the v1 path MUST still send initialize"
    );
}

/// Not one of the nine required tests, but the cheapest proof that the whole
/// prompt surface is reachable too: `prompts/get` derives `Mcp-Name` from
/// `params.name`, the third row of the shared table.
#[tokio::test]
async fn mcp_name_from_name_for_prompts_get() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let client = v2_client(addr);

    let result = client
        .get_prompt("greeting".to_string(), std::collections::HashMap::new())
        .await;

    handle.abort();
    let prompt = result.expect("a v2 prompts/get must be accepted");
    // Assert on something the handler actually produces. The previous
    // `is_empty() || !is_empty()` was a tautology and pinned nothing.
    assert_eq!(
        prompt.description.as_deref(),
        Some("greeting"),
        "the response must come from the registered GreetingPrompt handler"
    );
}
