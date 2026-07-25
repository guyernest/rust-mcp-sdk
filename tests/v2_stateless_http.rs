//! Phase 113-04 (HTTP-01): live-HTTP acceptance gate for handshake-free,
//! session-free v2 on the streamable-HTTP transport.
//!
//! These tests drive a REAL `StreamableHttpServer` over a loopback TCP socket with
//! a raw `reqwest` client (NOT the in-memory transport — RESEARCH Pitfall 11), so
//! every status code, response header and JSON-RPC envelope crosses the actual
//! axum HTTP boundary.
//!
//! # Every server here is spawned with the STATEFUL default config
//!
//! [`common::v2::spawn_default_config`] uses `StreamableHttpServerConfig::default()`,
//! which keeps a LIVE `session_id_generator`. That is deliberate and load-bearing.
//!
//! `stateless()` is a BUILD-TIME config: it clears the generator once, when the
//! server is constructed. If these tests spawned a `stateless()` server, every
//! assertion below would be VACUOUS — the session machinery would already be gone
//! and the PER-REQUEST era gate that HTTP-01 exists to build would never be the
//! thing under test (RESEARCH Pitfall 1). A dual-version production server is
//! built with `Default::default()`, so the stateful config IS the realistic case.
//!
//! **Do not swap these spawns for the build-time stateless helper.** The
//! harness's other spawn function must never appear in this file — a grep for it
//! here is required to return zero, which is why it is not even named above.
//!
//! Test reliability doctrine (carried from `tests/v2_required_headers.rs`):
//! EPHEMERAL PORT (`127.0.0.1:0`, address read back from `start()`), READINESS
//! (`start()` binds before returning), SHUTDOWN (`JoinHandle::abort()` after each
//! round-trip).
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, default_client_capabilities, delete, get, header, post, post_raw,
    spawn_default_config, v1_body, v2_body, v2_headers, META_CLIENT_CAPABILITIES, META_CLIENT_INFO,
    META_PROTOCOL_VERSION, REQUEST_META_KEY, V1, V2,
};
use pmcp::types::protocol::error_codes::{
    HEADER_MISMATCH, METHOD_NOT_FOUND, PARSE_ERROR, UNSUPPORTED_PROTOCOL_VERSION,
};
use serde_json::{json, Value};

/// A v2-SHAPED request body whose reserved `_meta` claims `version` rather than
/// [`V2`], for the unsupported-version rejection test.
fn body_claiming_version(method: &str, id: Value, params: Value, version: &str) -> String {
    let meta = json!({
        META_PROTOCOL_VERSION: version,
        META_CLIENT_INFO: { "name": "pmcp-test-client", "version": "0.0.0" },
        META_CLIENT_CAPABILITIES: default_client_capabilities(),
    });
    let mut params = params;
    params
        .as_object_mut()
        .expect("params is an object")
        .insert(REQUEST_META_KEY.to_string(), meta);
    // Built through a `Map` rather than the `json!` macro: the macro BORROWS its
    // interpolated values, which would leave `id`/`params` passed by value but
    // never consumed.
    let mut body = serde_json::Map::new();
    body.insert("jsonrpc".to_string(), json!("2.0"));
    body.insert("id".to_string(), id);
    body.insert("method".to_string(), json!(method));
    body.insert("params".to_string(), params);
    Value::Object(body).to_string()
}

/// The canonical v2 `tools/call` this file drives most assertions from.
fn v2_call_body(id: Value) -> String {
    v2_body(
        "tools/call",
        id,
        json!({ "name": "search", "arguments": {} }),
    )
}

/// A v1 `initialize` body — the only way to mint a session on the v1 path.
fn v1_initialize_body() -> String {
    v1_body(
        "initialize",
        json!(1),
        json!({
            "protocolVersion": V1,
            "capabilities": {},
            "clientInfo": { "name": "v1-client", "version": "1.0.0" },
        }),
    )
}

// ===========================================================================
// HTTP-01: a v2 request runs session-free on a STATEFUL-config server.
// ===========================================================================

/// The load-bearing HTTP-01 assertion: the response carries NO `Mcp-Session-Id`.
#[tokio::test]
async fn no_session_id_on_v2() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_call_body(json!(1)),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body: {}", response.raw);
    assert_eq!(
        response.mcp_session_id, None,
        "a v2 response must NOT mint or echo a session id; raw: {}",
        response.raw
    );
}

/// ...and the request is not REQUIRED to carry one either, even though the
/// server-wide config would demand it for v1.
#[tokio::test]
async fn v2_requires_no_session_id() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_call_body(json!(2)),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 200,
        "a v2 request sending no Mcp-Session-Id must be served; body: {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_some(),
        "expected a result, got: {}",
        response.raw
    );
}

/// Spec: "An `Mcp-Session-Id` header on a request: ignore it, and do not mint or
/// echo session IDs." A bogus inbound id is INERT, not a rejection — which is
/// also what makes an attacker-supplied session id harmless on v2 (T-113-06).
#[tokio::test]
async fn v2_ignores_inbound_session_id() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let mut headers = v2_headers("tools/call", "search");
    headers.push(header("mcp-session-id", "nope"));
    let response = post(addr, &headers, &v2_call_body(json!(3))).await;
    handle.abort();

    assert_eq!(
        response.status, 200,
        "an inbound Mcp-Session-Id on v2 must be IGNORED, not rejected; body: {}",
        response.raw
    );
    assert_eq!(
        response.mcp_session_id, None,
        "...and still nothing is echoed back; raw: {}",
        response.raw
    );
}

/// The v1 half of the same server: sessions are minted, echoed and validated
/// exactly as before (T-113-19 — the era gate must not disable v1 sessions).
#[tokio::test]
async fn v1_session_unchanged() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;

    // 1. A v1 `initialize` MINTS a session id and echoes it back.
    let init = post(addr, &[], &v1_initialize_body()).await;
    assert_eq!(init.status, 200, "v1 initialize: {}", init.raw);
    let session_id = init
        .mcp_session_id
        .clone()
        .expect("a v1 initialize on a stateful server MUST mint a session id");
    assert!(!session_id.is_empty());

    // 2. A subsequent v1 request VALIDATES against it and is served.
    let listed = post(
        addr,
        &[header("mcp-session-id", &session_id)],
        &v1_body("tools/list", json!(2), json!({})),
    )
    .await;
    assert_eq!(listed.status, 200, "v1 tools/list: {}", listed.raw);
    assert!(
        listed.body.get("result").is_some(),
        "expected a result, got: {}",
        listed.raw
    );

    // 3. ...and a v1 request WITHOUT one is still rejected, so the era gate did
    //    not quietly turn the whole server stateless.
    let bare = post(addr, &[], &v1_body("tools/list", json!(3), json!({}))).await;
    handle.abort();
    assert_eq!(
        bare.status, 400,
        "a v1 non-init request with no session id must still be rejected; body: {}",
        bare.raw
    );
    assert!(
        bare.raw.contains("Session ID required"),
        "expected the v1 session gate, got: {}",
        bare.raw
    );
}

// ===========================================================================
// HTTP-01: GET / DELETE are 405 on v2, unchanged on v1.
// ===========================================================================

/// Spec: "HTTP GET or DELETE to the MCP endpoint: respond with 405 Method Not
/// Allowed." The bogus session id proves the guard runs BEFORE session
/// validation — a v1 GET with the same id answers 404 (see
/// [`v1_get_delete_unchanged`]), so a v2 GET never reaches session state
/// (T-113-18).
#[tokio::test]
async fn v2_get_405() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = get(
        addr,
        &[
            header("mcp-protocol-version", V2),
            header("mcp-session-id", "nope"),
        ],
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 405,
        "a v2 GET must be 405 Method Not Allowed; body: {}",
        response.raw
    );
}

#[tokio::test]
async fn v2_delete_405() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = delete(
        addr,
        &[
            header("mcp-protocol-version", V2),
            header("mcp-session-id", "nope"),
        ],
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 405,
        "a v2 DELETE must be 405 Method Not Allowed; body: {}",
        response.raw
    );
}

/// The SAME requests without the v2 version header keep today's behavior: an
/// unknown session id on a stateful server is `404 Unknown session ID` for both
/// verbs. Asserting the concrete observable (404, and NOT 405) is what proves the
/// era gate is additive rather than a blanket route removal.
#[tokio::test]
async fn v1_get_delete_unchanged() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let unknown_session = [header("mcp-session-id", "nope")];

    let got = get(addr, &unknown_session).await;
    let deleted = delete(addr, &unknown_session).await;
    handle.abort();

    assert_ne!(got.status, 405, "a v1 GET must NOT be 405: {}", got.raw);
    assert_eq!(
        got.status, 404,
        "a v1 GET with an unknown session id is still 404; body: {}",
        got.raw
    );

    assert_ne!(
        deleted.status, 405,
        "a v1 DELETE must NOT be 405: {}",
        deleted.raw
    );
    assert_eq!(
        deleted.status, 404,
        "a v1 DELETE with an unknown session id is still 404; body: {}",
        deleted.raw
    );
}

// ===========================================================================
// HTTP-01: v2 status mapping for unknown methods and rejections.
// ===========================================================================

/// Spec: "If the server does not implement the requested RPC method, it MUST
/// respond with 404 Not Found and a JSON-RPC error with code -32601."
///
/// Driven through `post_raw` with a method string that cannot deserialize into a
/// typed `ClientRequest`, so what is under test is the RAW-level mapping: a
/// mapper that only inspected an already-built typed response could never see
/// this request at all.
#[tokio::test]
async fn v2_unknown_method_404() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post_raw(
        addr,
        &v2_headers("totally/unknown", ""),
        &v2_body("totally/unknown", json!(7), json!({})),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 404,
        "a v2 unknown method must be HTTP 404; body: {}",
        response.raw
    );
    assert_eq!(
        response.body["error"]["code"], METHOD_NOT_FOUND,
        "...with JSON-RPC METHOD_NOT_FOUND; body: {}",
        response.raw
    );
    assert_eq!(
        response.body["id"], 7,
        "the ORIGINAL request id must survive a body that never typed-parses; body: {}",
        response.raw
    );
}

/// The v1 status for an unimplemented method is UNCHANGED — the 404 mapping is
/// era-gated, not global.
///
/// pmcp has two distinct v1 unknown-method paths and both are pinned here:
///
/// - `server/discover` is a v2-only method that IS classified at ingress, so on
///   v1 it reaches dispatch and answers JSON-RPC `-32601` at **HTTP 200** (the
///   Phase-112 D-10 decision).
/// - An arbitrary unknown method string never produces a typed request at all,
///   so it fails the transport parse and answers `PARSE_ERROR` at **HTTP 400**
///   with `id: null`, exactly as before this plan.
///
/// Neither is 404.
#[tokio::test]
async fn v1_unknown_method_still_200() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;

    // v1 still demands a session on this stateful server, so mint one first —
    // otherwise the session gate would answer before the method is ever routed.
    let init = post(addr, &[], &v1_initialize_body()).await;
    let session = init
        .mcp_session_id
        .clone()
        .expect("v1 initialize mints a session");
    let session_header = [header("mcp-session-id", &session)];

    let discover = post_raw(
        addr,
        &session_header,
        &v1_body("server/discover", json!(8), json!({})),
    )
    .await;
    let arbitrary = post_raw(
        addr,
        &session_header,
        &v1_body("totally/unknown", json!(9), json!({})),
    )
    .await;
    handle.abort();

    assert_eq!(
        discover.status, 200,
        "a v1 unimplemented method stays at HTTP 200; body: {}",
        discover.raw
    );
    assert_eq!(discover.body["error"]["code"], METHOD_NOT_FOUND);
    assert_eq!(discover.body["id"], 8, "v1 preserves the id too");

    assert_ne!(
        arbitrary.status, 404,
        "the 404 mapping must be v2-only; body: {}",
        arbitrary.raw
    );
    assert_eq!(
        arbitrary.status, 400,
        "an unparseable v1 method string is still a 400 parse error; body: {}",
        arbitrary.raw
    );
    assert_eq!(arbitrary.body["error"]["code"], PARSE_ERROR);
}

/// A v2 request missing a required header is `400` with `HEADER_MISMATCH`.
#[tokio::test]
async fn v2_header_gate_rejection_is_400() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    // Mcp-Method OMITTED; the other two headers are present and well-formed.
    let response = post(
        addr,
        &[
            header("mcp-name", "search"),
            header("mcp-protocol-version", V2),
        ],
        &v2_call_body(json!(4)),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 400,
        "a v2 header-gate rejection must be HTTP 400; body: {}",
        response.raw
    );
    assert_eq!(
        response.body["error"]["code"], HEADER_MISMATCH,
        "...with HEADER_MISMATCH; body: {}",
        response.raw
    );
}

// ===========================================================================
// The locked `Mcp-Name` rule, live over HTTP, in BOTH directions.
// ===========================================================================

/// A name-LESS v2 method sending an EMPTY `Mcp-Name` is ACCEPTED. This is the
/// live half of the cross-plan header rule; plan 05's client emits exactly this.
#[tokio::test]
async fn v2_nameless_method_empty_mcp_name_accepted() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let headers = v2_headers("tools/list", "");
    assert_eq!(
        headers[1],
        ("mcp-name".to_string(), String::new()),
        "the harness must emit an EMPTY Mcp-Name, not omit the header"
    );
    let response = post(addr, &headers, &v2_body("tools/list", json!(5), json!({}))).await;
    handle.abort();

    assert_eq!(
        response.status, 200,
        "an EMPTY Mcp-Name on a name-less v2 method must be ACCEPTED; body: {}",
        response.raw
    );
    assert!(
        response.body.get("result").is_some(),
        "expected a result, got: {}",
        response.raw
    );
}

/// ...but OMITTING the header entirely is a rejection: presence is required on
/// EVERY v2 request (Phase-112 D-05; the Phase-113 DRIFT-1 adjudication keeps
/// pmcp stricter than the draft transport spec, deliberately).
#[tokio::test]
async fn v2_nameless_method_absent_mcp_name_rejected() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &[
            header("mcp-method", "tools/list"),
            header("mcp-protocol-version", V2),
        ],
        &v2_body("tools/list", json!(6), json!({})),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 400,
        "an ABSENT Mcp-Name must be rejected even for a name-less method; body: {}",
        response.raw
    );
    assert_eq!(response.body["error"]["code"], HEADER_MISMATCH);
}

// ===========================================================================
// Structured rejection payloads and adversarial bodies.
// ===========================================================================

/// An unsupported per-request version is `400` + `UNSUPPORTED_PROTOCOL_VERSION`,
/// and the payload MUST list what the server DOES accept so the client can pick a
/// mutually supported version instead of probing (T-113-51).
#[tokio::test]
async fn v2_unsupported_version_400_with_supported() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &[
            header("mcp-method", "tools/call"),
            header("mcp-name", "search"),
            header("mcp-protocol-version", "1999-01-01"),
        ],
        &body_claiming_version(
            "tools/call",
            json!(10),
            json!({ "name": "search", "arguments": {} }),
            "1999-01-01",
        ),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 400,
        "an unsupported version must be HTTP 400; body: {}",
        response.raw
    );
    assert_eq!(
        response.body["error"]["code"], UNSUPPORTED_PROTOCOL_VERSION,
        "body: {}",
        response.raw
    );
    assert!(
        response.body["error"]["data"]["supported"].is_array(),
        "the rejection MUST carry an error.data.supported ARRAY; body: {}",
        response.raw
    );
    assert!(
        response.body["error"]["data"]["supported"]
            .as_array()
            .is_some_and(|versions| versions.iter().any(|v| v == V2)),
        "...listing the server's accept-list; body: {}",
        response.raw
    );
}

/// A malformed body is a clean `400`, never a panic — the process must still be
/// serving afterwards, which the follow-up request proves.
#[tokio::test]
async fn v2_malformed_json_400() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let broken = post_raw(addr, &v2_headers("tools/call", "search"), "{not json").await;
    // The server survived: a well-formed request on the SAME socket still works.
    let after = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_call_body(json!(11)),
    )
    .await;
    handle.abort();

    assert_eq!(
        broken.status, 400,
        "malformed JSON must be a clean 400; body: {}",
        broken.raw
    );
    assert_eq!(
        after.status, 200,
        "the server must still serve: {}",
        after.raw
    );
}

/// A JSON-RPC id may be a STRING, and it must come back byte-identical (HTTP-05
/// depends on ids always deriving from the live request; plan 08 asserts the
/// same invariant across the MRTR retry).
#[tokio::test]
async fn v2_string_id_preserved() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_call_body(json!("req-abc")),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body: {}", response.raw);
    assert_eq!(
        response.body["id"], "req-abc",
        "a string id must be preserved verbatim; body: {}",
        response.raw
    );
}
