//! Smoke tests for the shared Phase-113 v2 HTTP harness (`tests/common/v2.rs`).
//!
//! Six downstream plans (04, 06, 08, 10, 11, 13) build their requests from that one
//! harness, so a defect in it would look like a server defect in all six. These
//! tests prove the two properties every consumer depends on:
//!
//! 1. **Happy path** — a `tools/call` built by [`common::v2::v2_body`] +
//!    [`common::v2::v2_headers`] reaches a real handler and comes back 200 with a
//!    `result`.
//! 2. **The empty-`Mcp-Name` header rule** — a NAME-LESS method sends
//!    `Mcp-Name: ""` and is ACCEPTED. This is the cross-plan tripwire for the
//!    locked rule that `Mcp-Name` is present on EVERY v2 request (Phase-112 D-05;
//!    `113-SPEC-RECHECK.md` § `Mcp-Name Header Rule`). If a future plan relaxes
//!    `require_three_headers`, or the harness stops emitting the empty value, this
//!    test is what notices.
//!
//! The name-less method used for (2) is `server/discover`, NOT `tools/list`: today
//! `ListToolsRequest` carries no `_meta` field at all, so `extract_request_meta_value`
//! returns `None` for it and a v2 `tools/list` is rejected as "header claims v2 but
//! `_meta` disagrees" before any header rule is reached. That gap is pinned by
//! [`forward_tripwire_tools_list_cannot_be_a_v2_request`] and is plan 04's to close.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::v2::{
    build_v2_server, post, request_meta_key, spawn_default_config, spawn_stateless_config, v2_body,
    v2_body_with_caps, v2_discover_body, v2_headers, META_CLIENT_CAPABILITIES,
};
use serde_json::json;

#[tokio::test]
async fn harness_happy_path_tools_call_returns_a_result() {
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_body(
            "tools/call",
            json!(1),
            json!({ "name": "search", "arguments": {} }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(response.status, 200, "body: {}", response.raw);
    assert!(
        response.body.get("result").is_some(),
        "expected a result, got: {}",
        response.raw
    );
    assert_eq!(response.mcp_method.as_deref(), Some("tools/call"));
    assert_eq!(response.mcp_name.as_deref(), Some("search"));
}

#[tokio::test]
async fn harness_empty_mcp_name_is_accepted_for_a_name_less_method() {
    // THE cross-plan header-rule tripwire: `Mcp-Name` is emitted on EVERY v2
    // request, with the EMPTY STRING for a method that carries no logical name.
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;
    let headers = v2_headers("server/discover", "");
    assert_eq!(
        headers[1],
        ("mcp-name".to_string(), String::new()),
        "the harness must emit an EMPTY Mcp-Name, not omit the header"
    );
    let response = post(addr, &headers, &v2_discover_body(json!(2))).await;
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
    assert_eq!(response.mcp_method.as_deref(), Some("server/discover"));
}

/// FORWARD TRIPWIRE for plan 04.
///
/// `tools/list` is the natural name-less method to exercise the empty-`Mcp-Name`
/// rule with, and the plan called for it — but `ListToolsRequest` carries NO
/// `_meta` field, so `extract_request_meta_value` returns `None` for it and the era
/// resolver falls back to v1. A v2 `tools/list` is therefore rejected as a
/// header/`_meta` disagreement before any header rule is evaluated.
///
/// A stateless v2 server (HTTP-01) has no handshake, so EVERY method must be able
/// to carry the per-request `_meta` signal — including `tools/list`. Plan 04 must
/// close this and flip the assertion below to 200.
#[tokio::test]
async fn forward_tripwire_tools_list_cannot_be_a_v2_request() {
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/list", ""),
        &v2_body("tools/list", json!(2), json!({})),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 400,
        "plan 04 flips this to 200 once tools/list can carry _meta; body: {}",
        response.raw
    );
    assert!(
        response.raw.contains("_meta protocolVersion disagrees"),
        "expected the era-disagreement rejection, got: {}",
        response.raw
    );
}

/// FORWARD TRIPWIRE for plan 04 / plan 11 (conformance).
///
/// pmcp's typed request structs rename the `_meta` field via
/// `#[serde(rename_all = "camelCase")]`, so they currently serialize and ACCEPT
/// `meta` rather than the spec-mandated `_meta`. A conformant v2 client sending
/// `_meta` therefore gets NO era detection at all. The harness works around it by
/// emitting both spellings; this test pins the underlying spelling so the
/// workaround cannot silently outlive the defect.
#[tokio::test]
async fn forward_tripwire_typed_requests_rename_meta_away_from_the_spec_spelling() {
    assert_eq!(
        request_meta_key(),
        "meta",
        "if this is now `_meta`, the typed-request rename is fixed: \
         drop the dual-spelling emission in tests/common/v2.rs::v2_body_with_caps"
    );
}

#[tokio::test]
async fn harness_prompts_get_and_resources_read_have_real_handlers() {
    let (addr, handle) = spawn_stateless_config(build_v2_server()).await;

    let prompt = post(
        addr,
        &v2_headers("prompts/get", "greeting"),
        &v2_body(
            "prompts/get",
            json!(3),
            json!({ "name": "greeting", "arguments": {} }),
        ),
    )
    .await;
    let resource = post(
        addr,
        &v2_headers("resources/read", "mem://greeting"),
        &v2_body(
            "resources/read",
            json!(4),
            json!({ "uri": "mem://greeting" }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(prompt.status, 200, "body: {}", prompt.raw);
    assert!(prompt.body.get("result").is_some(), "{}", prompt.raw);
    assert_eq!(resource.status, 200, "body: {}", resource.raw);
    assert!(resource.body.get("result").is_some(), "{}", resource.raw);
}

#[tokio::test]
async fn harness_always_declares_client_capabilities() {
    // Codex Plan-02 HIGH #3: a harness that omitted `clientCapabilities` would make
    // every MRTR test accidentally exercise the -32021 undeclared-capability path.
    let body: serde_json::Value = serde_json::from_str(&v2_body(
        "tools/call",
        json!(1),
        json!({ "name": "search" }),
    ))
    .unwrap();
    let caps = &body["params"]["_meta"][META_CLIENT_CAPABILITIES];
    assert!(caps.get("elicitation").is_some(), "body: {body}");
    assert!(caps.get("sampling").is_some(), "body: {body}");
    assert!(caps.get("roots").is_some(), "body: {body}");

    // ...and the under-declaring escape hatch really under-declares.
    let narrow: serde_json::Value = serde_json::from_str(&v2_body_with_caps(
        "tools/call",
        json!(1),
        json!({ "name": "search" }),
        json!({ "roots": {} }),
    ))
    .unwrap();
    let caps = &narrow["params"]["_meta"][META_CLIENT_CAPABILITIES];
    assert!(caps.get("elicitation").is_none(), "body: {narrow}");
    assert!(caps.get("roots").is_some(), "body: {narrow}");
}

/// FORWARD TRIPWIRE for HTTP-01, owned by plan 04.
///
/// [`spawn_default_config`] builds a STATEFUL server (`session_id_generator` is
/// live). Today a v2 `tools/call` without an `Mcp-Session-Id` is rejected by the
/// server-wide session gate, because the PER-REQUEST era gate that suppresses
/// sessions on v2 does not exist yet — that is exactly what plan 04 (HTTP-01)
/// builds. This test pins the CURRENT behaviour so plan 04 has to come here and
/// flip it, rather than silently landing a change nothing observes.
///
/// When HTTP-01 lands: this must become `status == 200` with `mcp_session_id ==
/// None`.
#[tokio::test]
async fn forward_tripwire_stateful_config_still_demands_a_session_on_v2() {
    let (addr, handle) = spawn_default_config(build_v2_server()).await;
    let response = post(
        addr,
        &v2_headers("tools/call", "search"),
        &v2_body(
            "tools/call",
            json!(1),
            json!({ "name": "search", "arguments": {} }),
        ),
    )
    .await;
    handle.abort();

    assert_eq!(
        response.status, 400,
        "HTTP-01 (plan 04) flips this to 200 with no Mcp-Session-Id; body: {}",
        response.raw
    );
    assert!(
        response.raw.contains("Session ID required"),
        "expected the pre-HTTP-01 session gate, got: {}",
        response.raw
    );
}
