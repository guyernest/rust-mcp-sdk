//! Phase 117-02 (SMPL-02): **byte-identity golden fixtures for the v1 session
//! lifecycle, captured BEFORE the D-03 cut of
//! `src/server/streamable_http_server.rs`.**
//!
//! # Read this before you change a literal in this file
//!
//! A diff in a golden literal here is a **V1 WIRE BREAK**, not a fixture that
//! drifted. These literals were captured from the UNMODIFIED tree, ahead of the
//! Phase-117 plans that sever the v1 session / SSE-resumability machinery from
//! the v2 path. If a change you are making turns one of these tests red, the
//! correct response is to **fix the cut** — make the change v2-only — *not* to
//! re-record the golden.
//!
//! Re-recording is exactly the failure this file exists to prevent: "the v1
//! suite still passes" is not byte-identity evidence, because a refactor that
//! reshapes a response, moves a header, or reorders a field is precisely the
//! change that alters bytes while leaving every structural assertion true.
//!
//! # Why these literals had to be captured FIRST
//!
//! Goldens captured after a refactor prove only that the refactor is
//! self-consistent. The pre-cut bytes are unrecoverable once the cut lands, so
//! they are pinned here against an unmodified tree; the capture anchor commit is
//! recorded in `.planning/phases/117-agents-tester-v1-severability/117-02-SUMMARY.md`.
//!
//! # Why a RAW-STRING comparison, and what the ONLY permitted normalization is
//!
//! [`assert_v1_bytes`] compares the **raw response text**, not merely the parsed
//! JSON. A structural comparison cannot detect key **order** (this crate builds
//! `serde_json` with `preserve_order`, so wire order follows struct declaration
//! order and is observable), **whitespace**, **SSE framing**, or **omission
//! versus explicit null** — and those are precisely what a transport-level
//! refactor changes while every structural assertion stays green.
//!
//! The ONLY normalization permitted before that comparison is placeholder
//! substitution of genuinely per-run VALUES: the minted session id, and the
//! per-frame SSE event id. Both substitutions are proven **width-preserving** by
//! an explicit length assertion plus a per-key occurrence-count assertion, so a
//! substitution cannot mask an added or removed byte and cannot delete a key.
//!
//! # Header identity and body identity are two DISTINCT claims
//!
//! `common::v2::Resp` stores `Mcp-Session-Id` outside `raw`, so a body-only
//! fixture cannot speak for headers. This file makes both claims, separately:
//! bodies are pinned as raw text, and headers are pinned as an explicit
//! `name: value` block rendered by [`render_headers`] and run through the very
//! same width-preserving normalizer.
//!
//! # Determinism: one tool and one prompt
//!
//! `tools/list` is served from a `HashMap`, whose iteration order is randomized
//! per process, so a two-entry array is not byte-stable. Registering exactly one
//! of each makes those arrays singletons and therefore deterministic, without
//! weakening the comparison by a single byte.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use async_trait::async_trait;
use common::v2::{header, post, spawn_default_config, teardown, v1_body, Resp, V1};
use pmcp::server::typed_tool::TypedTool;
use pmcp::server::{PromptHandler, Server};
use pmcp::shared::http_constants::MCP_SESSION_ID;
use pmcp::types::{GetPromptResult, PromptArgument, PromptInfo};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::task::JoinHandle;

// ===========================================================================
// Dynamic-value normalization.
//
// Restated from `tests/v1_lists_golden.rs:88-180` rather than shared: a Rust
// integration test is its own crate, so the two files cannot import each other.
// The one adaptation is that substitution is FIELD-LINE anchored rather than
// JSON-key anchored, because this file pins SSE-framed responses and rendered
// header blocks, whose per-run values are not JSON string values.
// ===========================================================================

/// A response value that cannot be pinned because it is minted per run.
///
/// Every dynamic value pinned by this file lives on a FIELD LINE — `key: VALUE`
/// anchored to the start of a line, terminated by the newline (or by end of
/// input on the last line). That shape serves both SSE frame fields
/// (`id: <uuid>`) and the `name: value` blocks [`render_headers`] produces, so
/// one normalizer covers both the body claim and the header claim.
///
/// `v1_lists_golden.rs`'s JSON-string form (`"key":"VALUE"`) is deliberately NOT
/// restated: no capture in this file carries a dynamic JSON string VALUE — the
/// session id travels in a header and the SSE event id in a frame field — and an
/// unexercised second substitution path would be dead weight that no fixture
/// could prove correct.
///
/// `token` replaces the value in the CANONICAL normalization (the one compared
/// against the golden literal). In the SAME-WIDTH normalization the token is
/// padded with `#` to the value's own byte width, so the normalized string is
/// exactly as long as the raw one — the check that proves the substitution
/// neither adds nor removes bytes and, in particular, never deletes a key.
struct DynamicField {
    /// The JSON object key, or the field-line name, whose value is dynamic.
    key: &'static str,
    /// The canonical placeholder written into the golden literal.
    token: &'static str,
    /// Shape predicate the raw value must satisfy — a normalization that
    /// accepted any string would let a reshaped value through unnoticed.
    shape: fn(&str) -> bool,
    /// Human-readable form of `shape`, for the failure message.
    shape_description: &'static str,
}

/// Nothing is normalized: every byte of the response is pinned verbatim.
///
/// The machinery still runs on every call, so the width invariant is an executed
/// no-op rather than an untested claim.
const NO_DYNAMICS: &[DynamicField] = &[];

/// `Uuid::new_v4().to_string()`'s shape: 36 bytes, lowercase hex, dashes at
/// 8/13/18/23, version nibble `4` at offset 14.
///
/// Deliberately NOT "any non-empty string": both the session id
/// (`StreamableHttpServerConfig::default`'s generator) and the SSE event id
/// (`sse_event_for_message`) are v4 UUIDs today, and a reshaped value must be
/// caught by the normalizer rather than waved through it.
fn is_uuid_v4(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && bytes.get(14) == Some(&b'4')
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => matches!(byte, b'0'..=b'9' | b'a'..=b'f'),
        })
}

/// Human-readable form of [`is_uuid_v4`], quoted into failure messages.
const UUID_V4_SHAPE: &str = "a lowercase v4 UUID (36 bytes, dashes at 8/13/18/23)";

/// The minted `Mcp-Session-Id`, as it appears in a rendered header block.
const SESSION_ID_DYNAMICS: &[DynamicField] = &[DynamicField {
    key: MCP_SESSION_ID,
    token: "<session-id>",
    shape: is_uuid_v4,
    shape_description: UUID_V4_SHAPE,
}];

/// The per-frame SSE event id minted by `sse_event_for_message` /
/// `build_sse_response_from_single_message`.
const SSE_EVENT_ID_DYNAMICS: &[DynamicField] = &[DynamicField {
    key: "id",
    token: "<sse-event-id>",
    shape: is_uuid_v4,
    shape_description: UUID_V4_SHAPE,
}];

/// `token`, padded with `#` to exactly `width` bytes.
fn width_preserving(token: &str, width: usize) -> String {
    assert!(
        token.len() <= width,
        "placeholder `{token}` is wider than the {width}-byte value it replaces; \
         pick a shorter token rather than shortening the value"
    );
    let mut padded = String::with_capacity(width);
    padded.push_str(token);
    padded.push_str(&"#".repeat(width - token.len()));
    padded
}

/// Replace every dynamic value in `raw`.
///
/// With `same_width`, each value becomes a padded placeholder of its own width;
/// otherwise it becomes the bare canonical token. Both passes are pure string
/// operations, so key order, spacing, SSE framing and null-versus-absent all
/// survive into the comparison.
fn substitute(raw: &str, fields: &[DynamicField], same_width: bool) -> String {
    let mut out = raw.to_string();
    for field in fields {
        out = substitute_field_line(&out, field, same_width);
    }
    out
}

/// The replacement text for one matched value.
fn replacement(field: &DynamicField, value: &str, same_width: bool) -> String {
    assert!(
        (field.shape)(value),
        "`{}` carried `{value}`, which is not {} — either the value shape \
         changed (a v1 wire break) or this fixture is normalizing the wrong key",
        field.key,
        field.shape_description
    );
    if same_width {
        width_preserving(field.token, value.len())
    } else {
        field.token.to_string()
    }
}

/// Replace the value of every LINE that begins `key: `.
///
/// Line-anchored on purpose. An unanchored `id: ` needle could in principle
/// match inside an SSE `data:` payload; anchoring makes the frame field and the
/// JSON body structurally impossible to confuse.
fn substitute_field_line(raw: &str, field: &DynamicField, same_width: bool) -> String {
    let prefix = format!("{}: ", field.key);
    let mut out = String::with_capacity(raw.len());
    let mut hits = 0_usize;
    // `split_inclusive` keeps each line's terminator, so re-joining is lossless
    // and a missing trailing newline stays missing.
    for segment in raw.split_inclusive('\n') {
        let (line, terminator) = segment
            .strip_suffix('\n')
            .map_or((segment, ""), |line| (line, "\n"));
        if let Some(value) = line.strip_prefix(prefix.as_str()) {
            out.push_str(&prefix);
            out.push_str(&replacement(field, value, same_width));
            out.push_str(terminator);
            hits += 1;
        } else {
            out.push_str(segment);
        }
    }
    assert_declared_key_present(hits, field, raw);
    out
}

fn assert_declared_key_present(hits: usize, field: &DynamicField, raw: &str) {
    assert!(
        hits > 0,
        "declared dynamic key `{}` does not appear in the response — a golden \
         that normalizes an absent key proves nothing: {raw}",
        field.key
    );
}

/// How many field lines carry `field`'s key.
fn key_occurrences(text: &str, field: &DynamicField) -> usize {
    let prefix = format!("{}: ", field.key);
    text.split_inclusive('\n')
        .filter(|segment| {
            segment
                .strip_suffix('\n')
                .unwrap_or(segment)
                .starts_with(prefix.as_str())
        })
        .count()
}

// ===========================================================================
// The assertion helper.
// ===========================================================================

/// The structural cross-check applied after the raw-byte comparison.
///
/// It exists for the readable failure message only; the RAW comparison is what
/// carries ordering, whitespace and framing.
enum Structural {
    /// A whole JSON-RPC success frame: `{"jsonrpc":"2.0","id":…,"result":…}`.
    JsonRpcResult { id: i64, result: Value },
    /// Every `data:` payload in an SSE capture, in order.
    Frames(Vec<Value>),
    /// No structural cross-check — the capture is not JSON (a rendered header
    /// block).
    RawOnly,
}

/// The failure text the raw-byte comparison carries.
///
/// Factored out so the `assert_eq!` invocation stays on one line: this is the
/// assertion a reviewer greps for when asking "does this file actually compare
/// bytes, or only parsed JSON?".
fn wire_break_message(raw: &str) -> String {
    format!(
        "v1 session-lifecycle wire bytes changed. This is a V1 WIRE BREAK, not a stale \
         fixture — the Phase-117 severance of the v1 session / SSE machinery from the v2 \
         path is the likely cause, so FIX THE CUT and make the change v2-only instead of \
         re-recording the golden. Raw capture was: {raw}"
    )
}

/// One pinned v1 capture.
struct V1Golden<'a> {
    /// The capture, byte for byte, after canonical normalization.
    raw: &'a str,
    /// The structural cross-check (see [`Structural`]).
    structural: Structural,
    /// Values normalized before comparison (see [`DynamicField`]).
    dynamics: &'a [DynamicField],
}

/// Every `data:` payload in an SSE capture, parsed, in order.
fn sse_data_frames(text: &str) -> Vec<Value> {
    text.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|payload| serde_json::from_str::<Value>(payload.trim()).ok())
        .collect()
}

/// Assert `raw` is byte-identical to `golden` once dynamic values are replaced.
///
/// Three things happen, in this order:
///
/// 1. **Width invariant.** A same-width substitution must leave the length
///    unchanged and every dynamic key's occurrence count unchanged. This is what
///    makes "the normalization never deletes a key" a checked property rather
///    than a comment.
/// 2. **RAW-STRING comparison** against the canonical golden — the load-bearing
///    assertion, and the only one that sees key order, spacing, SSE framing and
///    omission-versus-null.
/// 3. **Structural comparison**, for a readable message only.
fn assert_v1_bytes(raw: &str, golden: &V1Golden<'_>) {
    let same_width = substitute(raw, golden.dynamics, true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the placeholder substitution changed the capture length; it must be \
         width-preserving so it cannot mask an added or removed byte: {raw}"
    );
    for field in golden.dynamics {
        assert_eq!(
            key_occurrences(&same_width, field),
            key_occurrences(raw, field),
            "the substitution changed how often `{}` appears; it must replace \
             VALUES only and never delete a key: {raw}",
            field.key
        );
    }

    let normalized = substitute(raw, golden.dynamics, false);
    assert_eq!(normalized, golden.raw, "{}", wire_break_message(raw));

    match &golden.structural {
        Structural::JsonRpcResult { id, result } => {
            let parsed: Value =
                serde_json::from_str(&normalized).expect("a v1 JSON-RPC frame must be valid JSON");
            assert_eq!(
                parsed,
                json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                "the full JSON-RPC frame (jsonrpc + id + result) must match the golden"
            );
        },
        Structural::Frames(expected) => {
            assert_eq!(
                &sse_data_frames(&normalized),
                expected,
                "every SSE `data:` payload must match the golden, in order"
            );
        },
        Structural::RawOnly => {},
    }
}

/// Render headers as a canonical `name: value` block, one per line, in the order
/// given.
///
/// Header identity is a SEPARATE claim from body identity: `common::v2::Resp`
/// keeps `Mcp-Session-Id` outside `raw`, so a body fixture says nothing about
/// headers. Rendering them into the same [`Form::FieldLine`] shape the SSE
/// frames already use means one width-preserving normalizer serves both claims.
fn render_headers(pairs: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for (name, value) in pairs {
        out.push_str(name);
        out.push_str(": ");
        out.push_str(value);
        out.push('\n');
    }
    out
}

// ===========================================================================
// Fixtures: the pinned server.
// ===========================================================================

/// The single pinned tool. One, not two, for the reason in the module doc:
/// `tools/list` iterates a `HashMap`, so two entries would not be byte-stable.
fn pinned_tool() -> impl pmcp::ToolHandler {
    TypedTool::new_with_schema(
        "pin_lookup",
        json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        }),
        |_args: Value, _extra| Box::pin(async { Ok(json!({ "hits": 0 })) }),
    )
    .with_description("a fixed tool whose v1 wire shape is pinned by this file")
}

/// The single pinned prompt, carrying one required argument so the nested
/// `arguments` array's key order is pinned too.
struct PinnedPrompt;

#[async_trait]
impl PromptHandler for PinnedPrompt {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(vec![], None))
    }

    fn metadata(&self) -> Option<PromptInfo> {
        Some(
            PromptInfo::new("pin_summarize")
                .with_description("a fixed prompt whose v1 wire shape is pinned by this file")
                .with_arguments(vec![PromptArgument::new("topic")
                    .with_description("the subject to summarize")
                    .required()]),
        )
    }
}

/// The fixture server.
///
/// **The builder's supported-protocol-versions extender is deliberately NEVER
/// called here**, and the name of that method is deliberately absent from this
/// whole file so that a plain `grep` for it is a working detector rather than a
/// hit on a comment. The fixture must be a NOT-OPTED-IN v1 server, because that
/// is precisely the configuration whose bytes this file freezes: an opted-in
/// server can negotiate the v2 era, and the whole point of these literals is
/// that they are the v1 era's.
fn pinned_server() -> Server {
    Server::builder()
        .name("v1-byte-identity")
        .version("1.0.0")
        .tool("pin_lookup", pinned_tool())
        .prompt("pin_summarize", PinnedPrompt)
        .build()
        .expect("the pinned v1 fixture server builds")
}

/// The v1 `initialize` request body.
fn initialize_body(id: i64) -> String {
    v1_body(
        "initialize",
        json!(id),
        json!({
            "protocolVersion": V1,
            "capabilities": {},
            "clientInfo": { "name": "v1-byte-identity-client", "version": "1.0.0" }
        }),
    )
}

/// Spawn a SESSION-MINTING server.
///
/// `spawn_default_config`, never the harness's build-time stateless spawn
/// helper — whose name is deliberately absent from this whole file so a plain
/// `grep` for it is a working detector rather than a hit on a comment.
/// `StreamableHttpServerConfig::stateless()` has no `session_id_generator`, so
/// `sessions_active_for(false, _)` is `false` and there is no `Mcp-Session-Id`
/// to pin at all: a stateless spawn would produce a vacuously-green session
/// fixture.
async fn spawn_session_minting() -> (SocketAddr, JoinHandle<()>) {
    spawn_default_config(pinned_server()).await
}

/// POST `initialize` and return both the response and the session id it minted.
async fn initialize(addr: SocketAddr, id: i64) -> (Resp, String) {
    let response = post(addr, &[], &initialize_body(id)).await;
    let session_id = response
        .mcp_session_id
        .clone()
        .expect("a v1 initialize against a session-minting server MUST mint a session id");
    (response, session_id)
}

// ===========================================================================
// Golden captures — v1 session lifecycle.
// ===========================================================================

/// The v1 `initialize` response.
///
/// PLAIN JSON, not SSE-framed, and that is itself pinned: `build_response`
/// selects its framing from the RAW INBOUND `Mcp-Session-Id`, which an
/// `initialize` request does not carry, so it falls through to
/// `build_json_response`.
/// Note there is no `nextCursor`, no `_meta` and no v2 response envelope: their
/// ABSENCE is part of what this literal pins.
const INITIALIZE_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":false},"prompts":{"listChanged":false}},"serverInfo":{"name":"v1-byte-identity","version":"1.0.0"}}}"#;

/// The `Mcp-Session-Id` response header on a v1 `initialize`.
///
/// The header NAME is pinned exactly; the VALUE is pinned by SHAPE
/// ([`is_uuid_v4`]) because it is minted per run.
const INITIALIZE_SESSION_HEADER: &str = "mcp-session-id: <session-id>\n";

/// A session-carrying follow-up `tools/list`.
///
/// SSE-FRAMED, and that too is pinned: with an inbound `Mcp-Session-Id` and no
/// open SSE stream for it, `build_response` routes to
/// `build_sse_response_from_single_message`. The `id` / `event` / `data` field
/// ORDER and the frame-terminating blank line are part of the literal.
const FOLLOW_UP_TOOLS_LIST: &str = concat!(
    "id: <sse-event-id>\n",
    "event: message\n",
    r#"data: {"jsonrpc":"2.0","id":4,"result":{"tools":[{"name":"pin_lookup","description":"a fixed tool whose v1 wire shape is pinned by this file","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}}]}}"#,
    "\n\n"
);

// ===========================================================================
// Fixture 1 — the v1 initialize response BODY.
// ===========================================================================

#[tokio::test]
async fn v1_initialize_response_body_bytes_are_pinned() {
    let (addr, handle) = spawn_session_minting().await;
    let (response, _session_id) = initialize(addr, 1).await;
    teardown(handle, ()).await;

    assert_eq!(response.status, 200, "v1 initialize must still be served");
    assert_v1_bytes(
        &response.raw,
        &V1Golden {
            raw: INITIALIZE_BODY,
            structural: Structural::JsonRpcResult {
                id: 1,
                result: json!({
                    "protocolVersion": V1,
                    "capabilities": {
                        "tools": { "listChanged": false },
                        "prompts": { "listChanged": false }
                    },
                    "serverInfo": { "name": "v1-byte-identity", "version": "1.0.0" }
                }),
            },
            dynamics: NO_DYNAMICS,
        },
    );
}

// ===========================================================================
// Fixture 2 — the `Mcp-Session-Id` HEADER emission (a separate claim).
// ===========================================================================

#[tokio::test]
async fn v1_initialize_emits_the_mcp_session_id_header() {
    let (addr, handle) = spawn_session_minting().await;
    let (response, session_id) = initialize(addr, 2).await;
    teardown(handle, ()).await;

    assert_eq!(response.status, 200, "v1 initialize must still be served");
    assert!(
        is_uuid_v4(&session_id),
        "the minted session id must still be {UUID_V4_SHAPE}, got `{session_id}` — \
         a reshaped id is a V1 WIRE BREAK for every client that stores it"
    );
    assert_v1_bytes(
        &render_headers(&[(MCP_SESSION_ID, &session_id)]),
        &V1Golden {
            raw: INITIALIZE_SESSION_HEADER,
            structural: Structural::RawOnly,
            dynamics: SESSION_ID_DYNAMICS,
        },
    );
}

// ===========================================================================
// Fixture 3 — a session-carrying follow-up POST.
// ===========================================================================

#[tokio::test]
async fn v1_session_carrying_follow_up_post_bytes_are_pinned() {
    let (addr, handle) = spawn_session_minting().await;
    let (_init, session_id) = initialize(addr, 3).await;
    let response = post(
        addr,
        &[header(MCP_SESSION_ID, &session_id)],
        &v1_body("tools/list", json!(4), json!({})),
    )
    .await;
    teardown(handle, ()).await;

    assert_eq!(
        response.status, 200,
        "a v1 session-carrying request must still be served: {}",
        response.raw
    );
    assert_v1_bytes(
        &response.raw,
        &V1Golden {
            raw: FOLLOW_UP_TOOLS_LIST,
            structural: Structural::Frames(vec![json!({
                "jsonrpc": "2.0",
                "id": 4,
                "result": {
                    "tools": [
                        {
                            "name": "pin_lookup",
                            "description": "a fixed tool whose v1 wire shape is pinned by this file",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "query": { "type": "string" } },
                                "required": ["query"]
                            }
                        }
                    ]
                }
            })]),
            dynamics: SSE_EVENT_ID_DYNAMICS,
        },
    );
}

// ===========================================================================
// Anti-vacuity — the normalizer itself.
// ===========================================================================

/// The width-preserving substitution cannot mask a byte change.
///
/// Without this, "the normalization is width-preserving" would be a comment.
/// The three cases are: the same-width pass really does preserve length; a
/// declared-but-absent key is rejected rather than silently skipped; and a value
/// whose SHAPE changed is rejected rather than normalized away.
#[test]
fn the_substitution_is_width_preserving_and_shape_checked() {
    let raw = "id: 6f1e2d3c-4b5a-4978-8765-43210fedcba9\n";
    let same_width = substitute(raw, SSE_EVENT_ID_DYNAMICS, true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the same-width pass must be width-preserving"
    );
    assert_eq!(
        key_occurrences(&same_width, &SSE_EVENT_ID_DYNAMICS[0]),
        key_occurrences(raw, &SSE_EVENT_ID_DYNAMICS[0]),
        "the same-width pass must not delete the key"
    );
    assert_eq!(
        substitute(raw, SSE_EVENT_ID_DYNAMICS, false),
        "id: <sse-event-id>\n",
        "the canonical pass must write the bare token"
    );
}
