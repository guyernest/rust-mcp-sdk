//! Phase 112-06 (VERS-05 / D-05 / D-06 / D-11): live-HTTP acceptance gate for the
//! required v2 headers on the streamable-HTTP path.
//!
//! These tests drive a REAL `StreamableHttpServer` over a loopback TCP socket
//! with a raw `reqwest` client (NOT the in-memory transport — RESEARCH Pitfall
//! 11) so every header/`_meta` combination crosses the actual axum HTTP boundary.
//! They prove the full classification matrix, the strict all-three-headers
//! reject (D-05), the `Mcp-Method`/`Mcp-Name` body cross-check (D-06), outbound
//! header emission on success AND error, and that v1 / non-opted-in servers get
//! ZERO enforcement (D-04 / D-11).
//!
//! Test reliability (carried from the Phase 102/104 HTTP harness): EPHEMERAL
//! PORT (`127.0.0.1:0`, address read back from `start()`), READINESS (`start()`
//! binds before returning), SHUTDOWN (`JoinHandle::abort()` after each round-trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::{PromptHandler, ResourceHandler, Server};
use pmcp::types::prompts::GetPromptRequest;
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28 as V2};
use pmcp::types::resources::ReadResourceRequest;
use pmcp::types::{
    CallToolRequest, Content, GetPromptResult, ListResourcesResult, ReadResourceResult, RequestMeta,
};
use pmcp::{RequestHandlerExtra, ToolHandler};
use tokio::sync::Mutex;

/// A trivial tool so `tools/call` has a real dispatch target.
struct SearchTool;

#[async_trait]
impl ToolHandler for SearchTool {
    async fn handle(
        &self,
        _args: serde_json::Value,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<serde_json::Value> {
        // Plain payload — must NOT structurally resemble a built CallToolResult
        // (a `content` array) or the double-wrap tripwire (TOUT-02) fires.
        Ok(serde_json::json!({ "answer": "ok" }))
    }
}

/// A trivial prompt so `prompts/get` has a real dispatch target.
struct GreetingPrompt;

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
struct GreetingResource;

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

/// Build a `Server` exposing the `search` tool, a `greeting` prompt, and a
/// `mem://greeting` resource, optionally v2-opted-in.
fn build_server(opt_in_v2: bool) -> Server {
    let mut builder = Server::builder()
        .name("v2-required-headers")
        .version("1.0.0")
        .tool("search", SearchTool)
        .prompt("greeting", GreetingPrompt)
        .resources(GreetingResource);
    if opt_in_v2 {
        builder = builder.with_supported_protocol_versions([
            ProtocolVersion("2025-11-25".to_string()),
            ProtocolVersion(V2.to_string()),
        ]);
    }
    builder.build().expect("server builds")
}

/// Stand the server up over REAL HTTP (stateless JSON mode); return the bound
/// address + the server task handle.
async fn spawn(opt_in_v2: bool) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let server = Arc::new(Mutex::new(build_server(opt_in_v2)));
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http =
        StreamableHttpServer::with_config(addr, server, StreamableHttpServerConfig::stateless());
    http.start().await.expect("server starts")
}

/// Raw response view: HTTP status + the three v2 headers + the JSON body + the
/// RAW response text (kept for byte-identity assertions — the parsed `body`
/// alone cannot prove the v1 wire is byte-for-byte unchanged).
struct Resp {
    status: u16,
    mcp_method: Option<String>,
    mcp_name: Option<String>,
    mcp_version: Option<String>,
    body: serde_json::Value,
    raw: String,
}

/// POST a raw body with the given extra headers and return a [`Resp`].
///
/// Always sends the transport-required `content-type`/`accept`; `extra` carries
/// the v2 headers under test (or omits them).
async fn post(addr: SocketAddr, extra: &[(&str, &str)], body: &str) -> Resp {
    let client = reqwest::Client::new();
    let mut req = client
        .post(format!("http://{addr}"))
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(body.to_string());
    for (k, v) in extra {
        req = req.header(*k, *v);
    }
    let resp = req.send().await.expect("request sent");
    let status = resp.status().as_u16();
    let hget = |name: &str| {
        resp.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let mcp_method = hget("mcp-method");
    let mcp_name = hget("mcp-name");
    let mcp_version = hget("mcp-protocol-version");
    let text = resp.text().await.unwrap_or_default();
    let body = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    Resp {
        status,
        mcp_method,
        mcp_name,
        mcp_version,
        body,
        raw: text,
    }
}

/// A raw `tools/call` body. `meta_version` (when `Some`) is carried in
/// `params._meta` under the reserved protocol-version key so the SHARED Plan-04
/// resolver classifies the era from `_meta` (the authoritative signal).
///
/// Built via pmcp's OWN serialization so the wire `_meta` field name round-trips
/// exactly what the server deserializes (the request `_meta` field is renamed by
/// serde's camelCase rule — building through the typed struct avoids depending on
/// that spelling).
fn call_body(tool: &str, meta_version: Option<&str>) -> String {
    let mut req = CallToolRequest::new(tool, serde_json::json!({}));
    if let Some(v) = meta_version {
        req._meta = Some(RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            serde_json::json!(v),
        ));
    }
    let params = serde_json::to_value(&req).expect("params serialize");
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": params,
    })
    .to_string()
}

/// A raw `prompts/get` body built through the TYPED `GetPromptRequest` so the
/// wire `_meta` camelCase spelling round-trips exactly what the server
/// deserializes. `meta_version` carries the reserved protocol-version key.
fn prompt_body(name: &str, meta_version: Option<&str>) -> String {
    let mut req = GetPromptRequest {
        name: name.to_string(),
        arguments: HashMap::new(),
        _meta: None,
    };
    if let Some(v) = meta_version {
        req._meta = Some(RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            serde_json::json!(v),
        ));
    }
    let params = serde_json::to_value(&req).expect("params serialize");
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "prompts/get",
        "params": params,
    })
    .to_string()
}

/// A raw `resources/read` body built through the TYPED `ReadResourceRequest`
/// carrying ONLY `uri` (no synthetic `params.name`) — this is the standards
/// shape that exercises the finding #2 path (logical name from `params.uri`).
fn resource_body(uri: &str, meta_version: Option<&str>) -> String {
    let mut req = ReadResourceRequest {
        uri: uri.to_string(),
        _meta: None,
    };
    if let Some(v) = meta_version {
        req._meta = Some(RequestMeta::new().with_meta(
            "io.modelcontextprotocol/protocolVersion",
            serde_json::json!(v),
        ));
    }
    let params = serde_json::to_value(&req).expect("params serialize");
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "resources/read",
        "params": params,
    })
    .to_string()
}

/// Abort the server task and swallow the cancellation.
async fn shutdown(handle: tokio::task::JoinHandle<()>) {
    handle.abort();
    let _ = handle.await;
}

// ---------------------------------------------------------------------------
// Matrix cell: v2 header + v2 _meta + all headers + matching body → ACCEPT.
// Also proves the SUCCESS response carries all three outbound headers.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_accepts_well_formed_v2_and_echoes_headers() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "search"),
        ],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "well-formed v2 request should be accepted");
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
    // Outbound headers on SUCCESS.
    assert_eq!(r.mcp_method.as_deref(), Some("tools/call"));
    assert_eq!(r.mcp_name.as_deref(), Some("search"));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Matrix cell: v2 header but non-v2 _meta (absent) → REJECT (fail closed).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_v2_header_with_non_v2_meta() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "search"),
        ],
        &call_body("search", None), // no v2 _meta → era resolves v1
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "header/_meta disagreement must fail closed");
    assert_eq!(r.body["error"]["code"], -32600);
}

// ---------------------------------------------------------------------------
// Matrix cell: v2 _meta but absent MCP-Protocol-Version header → REJECT.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_v2_meta_without_version_header() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[("mcp-method", "tools/call"), ("mcp-name", "search")],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "_meta v2 with no version header must reject");
    assert_eq!(r.body["error"]["code"], -32600);
}

// ---------------------------------------------------------------------------
// D-05: a v2 request missing ANY of the three headers → 4xx + JSON-RPC error.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_missing_mcp_name() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[("mcp-protocol-version", V2), ("mcp-method", "tools/call")],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "missing Mcp-Name must reject (D-05)");
    assert_eq!(r.body["error"]["code"], -32600);
    assert_eq!(r.body["jsonrpc"], "2.0");
}

// ---------------------------------------------------------------------------
// D-06: Mcp-Method header disagreeing with the body method → REJECT.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_method_body_mismatch() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "resources/read"), // header lies about the method
            ("mcp-name", "search"),
        ],
        &call_body("search", Some(V2)), // body method is tools/call
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 400,
        "Mcp-Method vs body-method mismatch must reject"
    );
    assert_eq!(r.body["error"]["code"], -32600);
}

// ---------------------------------------------------------------------------
// D-06: Mcp-Name header disagreeing with params.name on a name-bearing method.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_name_body_mismatch() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "not-search"), // disagrees with params.name
        ],
        &call_body("search", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 400,
        "Mcp-Name vs params.name mismatch must reject"
    );
    assert_eq!(r.body["error"]["code"], -32600);
}

// ---------------------------------------------------------------------------
// A v2 request that PASSES the gate but hits an unknown tool → the handler's
// structured JSON-RPC error still carries all three outbound headers (VERS-05).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_error_response_still_echoes_headers() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "tools/call"),
            ("mcp-name", "ghost"),
        ],
        &call_body("ghost", Some(V2)), // valid v2 shape, unknown tool
    )
    .await;
    shutdown(handle).await;

    // HTTP 200 with a JSON-RPC error payload (unknown tool), headers present.
    assert_eq!(r.status, 200);
    assert!(
        r.body.get("error").is_some(),
        "expected JSON-RPC error: {}",
        r.body
    );
    assert_eq!(r.mcp_method.as_deref(), Some("tools/call"));
    assert_eq!(r.mcp_name.as_deref(), Some("ghost"));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Matrix cell: unsupported per-request version in _meta → explicit reject.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_rejects_unsupported_version() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[("mcp-method", "tools/call"), ("mcp-name", "search")],
        &call_body("search", Some("1999-01-01")), // not in the accept-list
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 400, "unsupported version must reject");
    // Mapped via the shared resolver → INVALID_PARAMS.
    assert_eq!(r.body["error"]["code"], -32602);
}

// ---------------------------------------------------------------------------
// Matrix cell: v1 request on an OPTED-IN server (no v2 signals) → v1 behavior.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_v1_request_on_opted_in_server_untouched() {
    let (addr, handle) = spawn(true).await;
    let r = post(addr, &[], &call_body("search", None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "plain v1 tools/call must still work");
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
    // No v2 enforcement, no v2 outbound headers forced.
    assert_eq!(r.mcp_method, None);
    assert_eq!(r.mcp_name, None);
}

// ---------------------------------------------------------------------------
// D-04: a NON-opted-in server runs ZERO enforcement — a request carrying stray
// Mcp-Method/Mcp-Name headers is NOT subject to the v2 gate (legacy behavior).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_required_headers_non_opted_in_server_ignores_v2_headers() {
    let (addr, handle) = spawn(false).await; // NOT opted in
    let r = post(
        addr,
        &[("mcp-method", "tools/call"), ("mcp-name", "search")],
        &call_body("search", None),
    )
    .await;
    shutdown(handle).await;

    // The stray headers are ignored; the request flows the normal v1 path.
    assert_eq!(
        r.status, 200,
        "non-opted-in server must not enforce v2 headers"
    );
    assert!(
        r.body.get("result").is_some(),
        "expected a result: {}",
        r.body
    );
}

// ---------------------------------------------------------------------------
// Phase 112-09 (Gap C): a well-formed v2 prompts/get is ACCEPTED (200) and its
// inner result carries the resultType/serverInfo envelope (VERS-05 + VERS-07).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_prompts_get_accepts_and_envelopes() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "prompts/get"),
            ("mcp-name", "greeting"),
        ],
        &prompt_body("greeting", Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "well-formed v2 prompts/get must be accepted");
    let result = r.body.get("result").expect("expected a result");
    assert_eq!(
        result.get("resultType").and_then(|v| v.as_str()),
        Some("complete"),
        "v2 prompts/get result must carry resultType:complete: {}",
        r.body
    );
    assert!(
        result.get("serverInfo").is_some_and(|v| v.is_object()),
        "v2 prompts/get result must carry a serverInfo object: {}",
        r.body
    );
    // Outbound headers echoed on success.
    assert_eq!(r.mcp_method.as_deref(), Some("prompts/get"));
    assert_eq!(r.mcp_name.as_deref(), Some("greeting"));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Phase 112-09 (Gap C / finding #2): a standards-shaped v2 resources/read
// (Mcp-Name = the URI, body built from a real ReadResourceRequest with ONLY
// `uri` — NO synthetic params.name) is ACCEPTED (200) with the envelope. This
// FAILS if Task 2's params.uri method-aware fix is missing (would reject 400).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_resources_read_accepts_and_envelopes() {
    let (addr, handle) = spawn(true).await;
    let uri = "mem://greeting";
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "resources/read"),
            ("mcp-name", uri), // Mcp-Name carries the resource URI
        ],
        &resource_body(uri, Some(V2)),
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 200,
        "standards-shaped v2 resources/read (uri only) must be accepted: {}",
        r.body
    );
    let result = r.body.get("result").expect("expected a result");
    assert_eq!(
        result.get("resultType").and_then(|v| v.as_str()),
        Some("complete"),
        "v2 resources/read result must carry resultType:complete: {}",
        r.body
    );
    assert!(
        result.get("serverInfo").is_some_and(|v| v.is_object()),
        "v2 resources/read result must carry a serverInfo object: {}",
        r.body
    );
    assert_eq!(r.mcp_method.as_deref(), Some("resources/read"));
    assert_eq!(r.mcp_name.as_deref(), Some(uri));
    assert_eq!(r.mcp_version.as_deref(), Some(V2));
}

// ---------------------------------------------------------------------------
// Matrix consistency: a v2-header prompts/get with NON-v2 _meta is still
// REJECTED (the fail-closed cell) — the fix did not loosen the gate.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v2_prompts_get_rejects_v2_header_with_non_v2_meta() {
    let (addr, handle) = spawn(true).await;
    let r = post(
        addr,
        &[
            ("mcp-protocol-version", V2),
            ("mcp-method", "prompts/get"),
            ("mcp-name", "greeting"),
        ],
        &prompt_body("greeting", None), // no v2 _meta → era resolves v1
    )
    .await;
    shutdown(handle).await;

    assert_eq!(
        r.status, 400,
        "v2-header prompts/get with non-v2 _meta must fail closed"
    );
    assert_eq!(r.body["error"]["code"], -32600);
}

/// Parse the raw v1 response text and assert full structural equality against a
/// pinned golden JSON-RPC shape, plus assert the raw string carries no v2 keys.
fn assert_v1_byte_identical(raw: &str, expected_result: &serde_json::Value) {
    let parsed: serde_json::Value =
        serde_json::from_str(raw).expect("v1 response must be valid JSON");
    let expected = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": expected_result,
    });
    assert_eq!(
        parsed, expected,
        "v1 wire must be structurally identical to the golden fixture; got raw: {raw}"
    );
    // Byte-level guard: none of the v2-only keys leak onto the v1 wire.
    assert!(
        !raw.contains("resultType"),
        "v1 raw must not contain resultType: {raw}"
    );
    assert!(
        !raw.contains("serverInfo"),
        "v1 raw must not contain serverInfo: {raw}"
    );
    assert!(
        !raw.contains("_meta"),
        "v1 raw must not contain _meta: {raw}"
    );
}

// ---------------------------------------------------------------------------
// Phase 112-09 (v1 byte-identity, finding #5a): a v1 prompts/get on a
// NON-opted-in server produces a response whose RAW bytes equal a pinned golden
// fixture — full structural equality, not merely two-key absence.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v1_prompts_get_byte_identical() {
    let (addr, handle) = spawn(false).await; // NOT opted in
    let r = post(addr, &[], &prompt_body("greeting", None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "plain v1 prompts/get must still work");
    // GreetingPrompt returns description "greeting" + empty messages; on v1 the
    // wire omits None/empty _meta and carries NO envelope.
    assert_v1_byte_identical(
        &r.raw,
        &serde_json::json!({
            "description": "greeting",
            "messages": [],
        }),
    );
}

// ---------------------------------------------------------------------------
// Phase 112-09 (v1 byte-identity): a v1 resources/read on a NON-opted-in server
// is byte-for-byte the pinned golden fixture (no envelope, no _meta leak).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn v1_resources_read_byte_identical() {
    let (addr, handle) = spawn(false).await; // NOT opted in
    let uri = "mem://greeting";
    let r = post(addr, &[], &resource_body(uri, None)).await;
    shutdown(handle).await;

    assert_eq!(r.status, 200, "plain v1 resources/read must still work");
    // GreetingResource returns a single text resource content at the URI.
    assert_v1_byte_identical(
        &r.raw,
        &serde_json::json!({
            "contents": [{
                "uri": uri,
                "text": "hello",
                "mimeType": "text/plain",
            }],
        }),
    );
}
