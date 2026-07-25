//! Shared live-HTTP harness for every Phase-113 `tests/v2_*.rs` file.
//!
//! Lifted from the Phase-112 `tests/v2_required_headers.rs` harness and extended
//! for MRTR. These helpers drive a REAL `StreamableHttpServer` over a loopback TCP
//! socket with a raw `reqwest` client (NOT the in-memory transport — RESEARCH
//! Pitfall 11) so every header / `_meta` combination crosses the actual axum HTTP
//! boundary.
//!
//! Test reliability doctrine (carried verbatim from Phase 112): EPHEMERAL PORT
//! (`127.0.0.1:0`, address read back from `start()`), READINESS (`start()` binds
//! before returning), SHUTDOWN (`JoinHandle::abort()` after each round-trip).
//!
//! # What changed versus the Phase-112 helper (and why)
//!
//! - **Request id is a parameter.** The Phase-112 helper hard-coded `1`. MRTR
//!   retries MUST use a DIFFERENT JSON-RPC id than the initial request, and plan 08
//!   needs string ids, so [`v2_body`] takes the id.
//! - **`clientCapabilities` is always present.** Every shared request declares all
//!   three MRTR-fulfillable capabilities. A harness that omitted them would make
//!   every MRTR test accidentally exercise the undeclared-capability
//!   (`-32021`) path instead of the happy path. Use [`v2_body_with_caps`] to
//!   deliberately under-declare.
//! - **`Mcp-Name` is ALWAYS emitted**, empty for a name-less method — the locked
//!   cross-plan header rule (Phase-112 D-05; `113-SPEC-RECHECK.md` §
//!   `Mcp-Name Header Rule`).
//! - **[`Resp`] captures `mcp_session_id` and `content_type`**, which HTTP-01
//!   (assert the session header is ABSENT on v2) and HTTP-04 (assert
//!   `text/event-stream`) both need.
//! - **Two spawn helpers.** [`spawn_default_config`] uses
//!   `StreamableHttpServerConfig::default()` — a STATEFUL config with a live
//!   `session_id_generator`. Per RESEARCH Pitfall 1, `::stateless()` is a
//!   BUILD-TIME config, so a test that uses it never exercises the per-request era
//!   gate at all. [`spawn_stateless_config`] is kept for the tests that genuinely
//!   want the build-time stateless branch.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]
// Each consumer test binary uses a different subset of this harness; the unused
// remainder is not dead code, it is another file's entry point.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::shared::http_constants::{
    ACCEPT_STREAMABLE, MCP_METHOD, MCP_NAME, MCP_PROTOCOL_VERSION, MCP_SESSION_ID,
};
use pmcp::types::protocol::{
    ProtocolVersion, LATEST_PROTOCOL_VERSION, PROTOCOL_VERSION_2026_07_28,
};
use pmcp::types::{Content, GetPromptResult, ListResourcesResult, ReadResourceResult, RequestMeta};
use pmcp::ServerCapabilities;
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// The 2026-07-28 protocol version string, sourced from pmcp's own constant so the
/// harness cannot drift from the crate.
pub const V2: &str = PROTOCOL_VERSION_2026_07_28;

/// The v1 protocol version an opted-in server keeps accepting alongside [`V2`].
///
/// Sourced from pmcp's own constant, like [`V2`] — this was the one version string
/// in the harness still spelled as a literal.
pub const V1: &str = LATEST_PROTOCOL_VERSION;

// ===========================================================================
// Reserved `_meta` keys — re-exported from the crate, not re-spelled.
// ===========================================================================

pub use pmcp::testing::{META_CLIENT_CAPABILITIES, META_CLIENT_INFO, META_PROTOCOL_VERSION};

// ===========================================================================
// Handlers — one real dispatch target per MRTR-eligible method.
// ===========================================================================

/// A trivial tool so `tools/call` has a real dispatch target.
pub struct SearchTool;

#[async_trait]
impl ToolHandler for SearchTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Plain payload — must NOT structurally resemble a built CallToolResult
        // (a `content` array) or the double-wrap tripwire (TOUT-02) fires.
        Ok(json!({ "answer": "ok" }))
    }
}

/// A trivial prompt so `prompts/get` has a real dispatch target.
pub struct GreetingPrompt;

#[async_trait]
impl PromptHandler for GreetingPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(vec![], Some("greeting".to_string())))
    }
}

/// A trivial resource handler so `resources/read` has a real dispatch target.
pub struct GreetingResource;

#[async_trait]
impl ResourceHandler for GreetingResource {
    async fn read(
        &self,
        uri: &str,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        Ok(ReadResourceResult::new(vec![Content::resource_with_text(
            uri.to_string(),
            "hello".to_string(),
            "text/plain".to_string(),
        )]))
    }

    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        Ok(ListResourcesResult::new(vec![]))
    }
}

/// The reverse-DNS extension id the v2-opted-in server advertises in its
/// `capabilities.extensions` map, so a `server/discover` projection has a
/// non-empty `extensions` map to assert over (VERS-04).
pub const DISCOVER_EXTENSION_KEY: &str = "io.example/experimental";

/// A `ServerCapabilities` carrying ONLY the extensions map. Registering handlers
/// after `.capabilities(..)` layers the tool/prompt/resource sub-capabilities on
/// top (each set only when absent), so the extensions survive.
pub fn extensions_capabilities() -> ServerCapabilities {
    let mut caps = ServerCapabilities::default();
    let mut ext = HashMap::new();
    ext.insert(
        DISCOVER_EXTENSION_KEY.to_string(),
        json!({ "enabled": true }),
    );
    caps.extensions = Some(ext);
    caps
}

/// Build a v2-OPTED-IN `Server` exposing the `search` tool, the `greeting` prompt
/// and the `mem://greeting` resource, so all three MRTR-eligible methods have a
/// real handler.
///
/// The accept-list carries BOTH [`V1`] and [`V2`]; the extensions map is pre-seeded
/// BEFORE the handlers (which layer their own sub-capabilities on top).
pub fn build_v2_server() -> Server {
    Server::builder()
        .name("v2-harness")
        .version("1.0.0")
        .capabilities(extensions_capabilities())
        .with_supported_protocol_versions([
            ProtocolVersion(V1.to_string()),
            ProtocolVersion(V2.to_string()),
        ])
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource)
        .build()
        .expect("server builds")
}

// ===========================================================================
// Spawning.
// ===========================================================================

/// Spawn `server` over REAL HTTP with `StreamableHttpServerConfig::default()`.
///
/// **This is the DEFAULT choice for Phase-113 tests, and it is a deliberate
/// deviation from the Phase-112 helper** (which used `::stateless()`). RESEARCH
/// Pitfall 1: `stateless()` is a BUILD-TIME config that removes the session
/// machinery before a request is ever seen, so a test that uses it can never prove
/// the PER-REQUEST era gate suppresses sessions on v2. The default config keeps a
/// live `session_id_generator` (and `enable_json_response: false`, hence SSE-framed
/// responses — [`Resp`] unwraps those transparently).
///
/// Async because `StreamableHttpServer::start` binds the socket before returning,
/// which is what gives the caller its readiness guarantee.
pub async fn spawn_default_config(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_with(server, StreamableHttpServerConfig::default()).await
}

/// Spawn `server` with the BUILD-TIME `::stateless()` config, for the tests that
/// genuinely want that branch (and for Phase-112 parity).
pub async fn spawn_stateless_config(server: Server) -> (SocketAddr, JoinHandle<()>) {
    spawn_with(server, StreamableHttpServerConfig::stateless()).await
}

/// Spawn `server` with an arbitrary config on an ephemeral loopback port.
pub async fn spawn_with(
    server: Server,
    config: StreamableHttpServerConfig,
) -> (SocketAddr, JoinHandle<()>) {
    let server = Arc::new(Mutex::new(server));
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http = StreamableHttpServer::with_config(addr, server, config);
    http.start().await.expect("server starts")
}

// ===========================================================================
// Request construction.
// ===========================================================================

/// The client capabilities every shared request declares: all three
/// MRTR-fulfillable kinds.
///
/// A server MUST NOT send an `inputRequests` entry for an undeclared capability
/// (it must answer `-32021 MissingRequiredClientCapability` instead), so a harness
/// that omitted these would silently route every MRTR test down the error path.
pub fn default_client_capabilities() -> Value {
    json!({ "elicitation": {}, "sampling": {}, "roots": {} })
}

/// A JSON-RPC v2 request body whose `params._meta` carries all three reserved keys,
/// including [`default_client_capabilities`].
///
/// `id` is a PARAMETER because an MRTR retry MUST use a different JSON-RPC id than
/// the initial request, and some plans need string ids.
pub fn v2_body(method: &str, id: Value, params: Value) -> String {
    v2_body_with_caps(method, id, params, default_client_capabilities())
}

/// The spec spelling of the per-request reserved-metadata object.
///
/// Phase-113 plan 04 (finding D-113-A) fixed the typed request structs, which
/// previously carried a struct-level `#[serde(rename_all = "camelCase")]` that
/// renamed the `_meta` FIELD to `meta` and so silently dropped a conformant
/// client's era signal. Both ingress paths — the raw `server/discover` read and
/// every typed request — now agree on `_meta`, so this harness emits ONE
/// spelling. `tests/common_harness_smoke.rs` carries the regression guard.
pub const REQUEST_META_KEY: &str = "_meta";

/// [`v2_body`] with an explicit `clientCapabilities` value, for tests that
/// deliberately under-declare.
pub fn v2_body_with_caps(method: &str, id: Value, params: Value, caps: Value) -> String {
    let mut params = match params {
        Value::Object(map) => Value::Object(map),
        _ => json!({}),
    };
    // Built through pmcp's OWN `RequestMeta` serialization so the reserved-key
    // spelling round-trips exactly what the server deserializes.
    let meta = RequestMeta::new()
        .with_meta(META_PROTOCOL_VERSION, json!(V2))
        .with_meta(
            META_CLIENT_INFO,
            json!({ "name": "pmcp-test-client", "version": "0.0.0" }),
        )
        .with_meta(META_CLIENT_CAPABILITIES, caps);
    let meta = serde_json::to_value(&meta).expect("request meta serializes");
    if let Some(object) = params.as_object_mut() {
        object.insert(REQUEST_META_KEY.to_string(), meta);
    }
    jsonrpc_envelope(method, id, params)
}

/// Assemble a JSON-RPC request envelope, CONSUMING `id` and `params`.
///
/// Built through a `serde_json::Map` rather than the `json!` macro because the
/// macro borrows its interpolated values, which would leave `id`/`params` as
/// pass-by-value-but-not-consumed parameters.
fn jsonrpc_envelope(method: &str, id: Value, params: Value) -> String {
    let mut body = serde_json::Map::new();
    body.insert("jsonrpc".to_string(), json!("2.0"));
    body.insert("id".to_string(), id);
    body.insert("method".to_string(), json!(method));
    body.insert("params".to_string(), params);
    Value::Object(body).to_string()
}

/// A `server/discover` request body — a v2-capable method that carries no logical
/// name, so it exercises the empty-`Mcp-Name` header rule end to end.
///
/// Since plan 04 closed D-113-B, `tools/list` (and every other list-shaped method)
/// can also carry the v2 `_meta` signal and is equally usable for that rule.
pub fn v2_discover_body(id: Value) -> String {
    v2_body("server/discover", id, json!({}))
}

/// A JSON-RPC v1 request body — no reserved `_meta` keys at all.
pub fn v1_body(method: &str, id: Value, params: Value) -> String {
    jsonrpc_envelope(method, id, params)
}

// ===========================================================================
// `Mcp-Name` value encoding.
// ===========================================================================

// The sentinel markers are NOT re-exported here. They were only ever used by the
// hand-copied encoder this module used to carry; a test that needs them should read
// `pmcp::testing::HEADER_SENTINEL_PREFIX` / `_SUFFIX` directly, which is one hop
// from the production constants rather than two.

/// pmcp's `Mcp-Name` value encoder — the PRODUCTION one.
///
/// This used to be a hand-copied mirror, and the mirror had already drifted: it
/// omitted the `MAX_HEADER_VALUE_LEN` clause from its passthrough predicate, so the
/// harness would emit a raw >8 KiB header where the real encoder sentinel-encodes.
/// Six Phase-113 plans build every request through this file, so the tests were
/// validating the harness against itself. Now it calls the shipped codec via the
/// `pmcp::testing` seam and cannot drift at all.
pub use pmcp::testing::encode_mcp_name as encode_header_value;

/// The three required v2 headers, with `name` sentinel-encoded as needed.
///
/// `Mcp-Name` is ALWAYS emitted. Pass `""` for a name-less method such as
/// `tools/list` — that is the locked cross-plan header rule, and plan 05's client
/// does exactly the same.
pub fn v2_headers(method: &str, name: &str) -> Vec<(String, String)> {
    v2_headers_raw(method, &encode_header_value(name))
}

/// [`v2_headers`] without the value encoder, for tests that deliberately send a
/// malformed sentinel or a raw non-ASCII value.
pub fn v2_headers_raw(method: &str, raw_name: &str) -> Vec<(String, String)> {
    vec![
        (MCP_METHOD.to_string(), method.to_string()),
        (MCP_NAME.to_string(), raw_name.to_string()),
        (MCP_PROTOCOL_VERSION.to_string(), V2.to_string()),
    ]
}

/// Convenience constructor for one extra header.
pub fn header(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

// ===========================================================================
// Response capture.
// ===========================================================================

/// Raw response view: HTTP status + the v2 headers + the session header + the
/// content type + the JSON body + the RAW response text.
///
/// `raw` is kept for byte-identity assertions — the parsed `body` alone cannot
/// prove a v1 wire is byte-for-byte unchanged.
#[derive(Debug, Clone)]
pub struct Resp {
    /// HTTP status code.
    pub status: u16,
    /// The echoed `Mcp-Method` header, if any.
    pub mcp_method: Option<String>,
    /// The echoed `Mcp-Name` header, if any.
    pub mcp_name: Option<String>,
    /// The echoed `MCP-Protocol-Version` header, if any.
    pub mcp_version: Option<String>,
    /// The `Mcp-Session-Id` header — MUST be absent on a v2 response (HTTP-01).
    pub mcp_session_id: Option<String>,
    /// The response `Content-Type` — `text/event-stream` for an SSE reply.
    pub content_type: Option<String>,
    /// The parsed JSON body. An SSE reply is unwrapped from its first `data:`
    /// frame, so callers assert the same way in both framings.
    pub body: Value,
    /// The verbatim response text, SSE framing included.
    pub raw: String,
}

/// Parse a response body that may be either bare JSON or a single SSE frame.
///
/// `StreamableHttpServerConfig::default()` has `enable_json_response: false`, so a
/// POST reply arrives as `event: message\ndata: {…}`. Unwrapping it here means every
/// consumer asserts on `body` identically regardless of which spawn helper it used.
fn parse_body(text: &str, content_type: Option<&str>) -> Value {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return value;
    }
    let looks_like_sse = content_type.is_some_and(|ct| ct.starts_with("text/event-stream"));
    if looks_like_sse || text.contains("data:") {
        for line in text.lines() {
            if let Some(payload) = line.strip_prefix("data:") {
                if let Ok(value) = serde_json::from_str::<Value>(payload.trim()) {
                    return value;
                }
            }
        }
    }
    Value::Null
}

/// Drive a prepared request to completion and capture a [`Resp`].
///
/// NOTE: this reads the body to EOF. A long-lived `subscriptions/listen` stream
/// (HTTP-04) needs a streaming reader instead — that is plan 13's surface, not this
/// request/response helper.
async fn send(request: reqwest::RequestBuilder, extra: &[(String, String)]) -> Resp {
    let mut request = request;
    for (name, value) in extra {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.expect("request sent");
    let status = response.status().as_u16();
    let hget = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let mcp_method = hget(MCP_METHOD);
    let mcp_name = hget(MCP_NAME);
    let mcp_version = hget(MCP_PROTOCOL_VERSION);
    let mcp_session_id = hget(MCP_SESSION_ID);
    let content_type = hget("content-type");
    let raw = response.text().await.unwrap_or_default();
    let body = parse_body(&raw, content_type.as_deref());
    Resp {
        status,
        mcp_method,
        mcp_name,
        mcp_version,
        mcp_session_id,
        content_type,
        body,
        raw,
    }
}

/// The `Accept` value a v2 client sends: both content types, per the transport spec.
pub const ACCEPT_BOTH: &str = ACCEPT_STREAMABLE;

/// POST a body with the given extra headers.
pub async fn post(addr: SocketAddr, extra: &[(String, String)], body: &str) -> Resp {
    post_with_accept(addr, ACCEPT_BOTH, extra, body).await
}

/// One shared `reqwest::Client` for the whole harness.
///
/// Each `Client::new()` builds a fresh rustls `ClientConfig` and root-certificate
/// store and gets its own connection pool, so a per-call client meant no connection
/// was ever reused across the hundreds of requests the Phase-113 test files make.
static CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(reqwest::Client::new);

/// [`post`] with an explicit `Accept` value, for the content-negotiation tests.
pub async fn post_with_accept(
    addr: SocketAddr,
    accept: &str,
    extra: &[(String, String)],
    body: &str,
) -> Resp {
    let request = CLIENT
        .post(format!("http://{addr}"))
        .header("content-type", "application/json")
        .header("accept", accept)
        .body(body.to_string());
    send(request, extra).await
}

/// POST RAW bytes with NO JSON validation, so a test can send malformed JSON, an
/// unknown method or a string id at the wire level.
pub async fn post_raw(addr: SocketAddr, extra: &[(String, String)], raw_body: &str) -> Resp {
    post_with_accept(addr, ACCEPT_BOTH, extra, raw_body).await
}

/// GET the MCP endpoint — v2 must answer 405 (HTTP-01).
pub async fn get(addr: SocketAddr, extra: &[(String, String)]) -> Resp {
    let request = CLIENT
        .get(format!("http://{addr}"))
        .header("accept", "text/event-stream");
    send(request, extra).await
}

/// DELETE the MCP endpoint — v2 must answer 405 (HTTP-01).
pub async fn delete(addr: SocketAddr, extra: &[(String, String)]) -> Resp {
    let request = CLIENT
        .delete(format!("http://{addr}"))
        .header("accept", "application/json");
    send(request, extra).await
}
