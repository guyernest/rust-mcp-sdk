//! The v1 null twin — what a `full-v2` build answers INSTEAD of MCP 2025-11-25.
//!
//! # Read this file to know what severance means
//!
//! This is the SMPL-02 deliverable. When the `v1-compat` feature is off, the MCP
//! 2025-11-25 session lifecycle and SSE resumability **do not exist** in the
//! compiled crate. Not "are skipped at runtime", not "return early" — the code
//! is not there. Every item below is the v2 constant answer to a question only
//! v1 ever asked.
//!
//! There is no session map here, no event store here, and nothing here reads the
//! `Last-Event-ID` request header. That is verifiable *by inspection*: the file
//! is short enough to read end to end, and what it does not contain is as
//! load-bearing as what it does. `tests/v1_severability_tripwire.rs` turns that
//! reading into an assertion, so the property survives the next edit.
//!
//! # How the pair is selected
//!
//! This file's twin is `v1_session.rs`, which holds the real v1 state. Exactly
//! one of the two is compiled, chosen by two `cfg_attr` path attributes on a
//! single `mod v1;` declaration in `src/server/streamable_http_server.rs`. The
//! transport therefore names `v1::…` unconditionally and never grows a
//! feature-gated call site.
//!
//! Because the compiler picks the half, a signature that drifts between the two
//! is a build failure on one feature set — the fastest possible feedback. The
//! tripwire covers the direction a build cannot see: that this file declares
//! nothing its twin does not, i.e. that severance never grows machinery of its
//! own.
//!
//! # Why the parameters are still here
//!
//! Every function below takes the same arguments as its real counterpart and
//! ignores them — most visibly the resolved `era`. Dropping a parameter the twin
//! does not read would make the two signatures diverge, and a caller that no
//! longer has to supply an era is a caller that is one edit away from resolving
//! one for itself. Phase 112 D-11 and Phase 113 Pitfall 2 forbid a second era
//! resolver in the transport: the era is resolved ONCE at ingress and CONSUMED
//! everywhere else. Identical parameter lists are how that stays true on both
//! builds.
//!
//! # This file is temporary
//!
//! Gating v1 off is reversible and semver-safe. DELETING the pair outright is a
//! major-version change, tracked as SMPL-F1 (pmcp 3.0) and gated on public
//! client adoption of the 2026-07-28 protocol. The policy — deliberately with no
//! date in it — is `docs/v1-sunset-policy.md`.

// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. The two lints conflict for an internal `pub(crate)`
// module; keeping `pub(crate)` items + this scoped allow is the idiomatic
// resolution already used by `src/server/task_dispatch.rs` and
// `src/shared/http_body_cap.rs`. The real half carries the identical allow.
#![allow(clippy::redundant_pub_crate)]

use super::{EventStoreHandle, ServerState, StreamableHttpServerConfig};
use crate::shared::TransportMessage;
use crate::types::protocol::Era;
use axum::http::HeaderMap;
use axum::response::Response;
use tokio::sync::mpsc;

/// The zero-sized stand-in for the v1 session and resumability state.
///
/// A unit struct, not an empty-braced one: the absence of fields is the whole
/// point, and the unit form makes it impossible to add one without changing the
/// declaration that the tripwire reads. Nothing is allocated, nothing is locked,
/// and nothing is retained, because on this build there are no sessions to track
/// and no events to replay.
#[derive(Clone, Debug, Default)]
pub(crate) struct V1State;

impl V1State {
    /// The v2 constant answer to "give me v1 state": a value with no contents.
    ///
    /// Callable identically to its twin so the transport's construction site is
    /// written once. Allocating nothing here is not an optimisation; it is the
    /// observable difference between the two builds.
    pub(crate) const fn new(_config: &StreamableHttpServerConfig) -> Self {
        Self
    }
}

// ---------------------------------------------------------------------------
// v1 session-map operations — every answer is a constant.
// ---------------------------------------------------------------------------

/// No session is ever tracked, so no session id is ever known.
pub(crate) const fn session_exists(_state: &V1State, _session_id: &str) -> bool {
    false
}

/// Recording a session is a no-op: there is nowhere to record it.
pub(crate) fn insert_session(
    _state: &V1State,
    _session_id: String,
    _initialized: bool,
    _protocol_version: Option<String>,
) {
}

/// No session, so no version was ever negotiated against one.
///
/// The 2026-07-28 transport carries its version per request, so a session-scoped
/// version would be the wrong authority even if one existed.
pub(crate) const fn session_protocol_version(
    _state: &V1State,
    _session_id: &str,
) -> Option<String> {
    None
}

/// Forgetting a session is a no-op: there is nothing to forget.
pub(crate) const fn remove_session(_state: &V1State, _session_id: &str) {}

// ---------------------------------------------------------------------------
// v1 SSE stream operations — every answer is a constant.
// ---------------------------------------------------------------------------

/// No stream is ever open, because none is ever registered.
pub(crate) const fn sse_stream_exists(_state: &V1State, _session_id: &str) -> bool {
    false
}

/// Registering a stream is a no-op; the sender is dropped on the spot.
pub(crate) fn register_sse_stream(
    _state: &V1State,
    _session_id: String,
    _sender: mpsc::UnboundedSender<TransportMessage>,
) {
}

/// Closing a stream is a no-op: none was ever opened.
pub(crate) const fn remove_sse_stream(_state: &V1State, _session_id: &str) {}

/// The message is always handed straight back, never routed anywhere.
///
/// There is no stream to deliver into, so the caller always frames the reply for
/// the caller that actually asked for it. That is the v2 rule stated positively:
/// a response goes to its requester and to nobody else.
pub(crate) const fn route_to_session_stream(
    _state: &V1State,
    _session_id: &str,
    message: TransportMessage,
) -> Option<TransportMessage> {
    Some(message)
}

// ---------------------------------------------------------------------------
// Session era gate — the v2 constant answers.
//
// Every signature below is its real counterpart's, arity and parameter types
// intact, `era` included. The twins ignore `era`; they do NOT drop it. A caller
// that no longer had to supply an era would be one edit away from resolving one
// for itself, and Phase 112 D-11 / Phase 113 Pitfall 2 forbid a second era
// resolver in the transport: the era is resolved ONCE at ingress and CONSUMED
// everywhere else. Keeping the parameter is how that stays true on this build
// too. (Unused names carry a leading underscore, which is Rust's marker for
// "deliberately ignored" — the types and the arity are what callers see.)
//
// The pure `_for` rule and the state-reading `fn` stay SEPARATE items here as
// well. Flattening them would leave the real half with a rule the twin has no
// counterpart for, and the rule is exactly what the truth-table and property
// tests exercise without constructing a live `ServerState`.
// ---------------------------------------------------------------------------

/// The session-era rule, collapsed to a constant.
///
/// There is no MCP 2025-11-25 session concept in this build, so a configured
/// generator is not evidence of anything and the era does not need consulting:
/// the answer is `false` for every input.
pub(crate) const fn sessions_active_for(_cfg_has_generator: bool, _era: Option<Era>) -> bool {
    false
}

/// Sessions are never live on this build.
///
/// Routed through [`sessions_active_for`] rather than returning `false` inline,
/// for two reasons: the rule stays the single place the answer comes from on
/// BOTH halves, and `era` is visibly CONSUMED here instead of discarded.
pub(crate) const fn sessions_active(_state: &ServerState, era: Option<Era>) -> bool {
    sessions_active_for(false, era)
}

/// There is never a session-id generator to hand out, so nothing can mint one.
pub(crate) const fn active_session_generator(
    _state: &ServerState,
    _era: Option<Era>,
) -> Option<&(dyn Fn() -> String + Send + Sync)> {
    None
}

/// No `Mcp-Session-Id` response header is ever emitted, on any request.
///
/// The real half is "the ONE place" that header is written; here there is no
/// such place at all, which is the stronger form of the same invariant. The
/// `headers` argument is taken and left untouched.
pub(crate) const fn apply_session_header(
    _headers: &mut HeaderMap,
    _response_session_id: Option<&String>,
    _sessions_on: bool,
) {
}

// ---------------------------------------------------------------------------
// Resumability era gate — the v2 constant answers.
//
// The 2026-07-28 transport spec is verbatim that resumable SSE streams via
// `Last-Event-ID` are not supported. On this build that is not a runtime
// refusal: there is no store to reach, no replay path compiled behind these
// functions, and nothing that reads the header.
// ---------------------------------------------------------------------------

/// The resumability rule, collapsed to a constant.
///
/// `false` for every input: this build offers no resumability to gate.
pub(crate) const fn resumability_active_for(_cfg_has_event_store: bool, _era: Option<Era>) -> bool {
    false
}

/// Resumability is never live on this build.
///
/// Routed through [`resumability_active_for`] for the same two reasons
/// [`sessions_active`] is routed through its rule.
pub(crate) const fn resumability_active(_state: &ServerState, era: Option<Era>) -> bool {
    resumability_active_for(false, era)
}

/// There is no store to hand out, on any request, ever.
///
/// The gated borrow degenerates to a constant `None`, so no caller can replay
/// from a store or write to one — because there is none to reach.
pub(crate) const fn resumability_store(
    _state: &ServerState,
    _era: Option<Era>,
) -> Option<&EventStoreHandle> {
    None
}

// ---------------------------------------------------------------------------
// v1 session LIFECYCLE — the v2 constant answers (plan 117-12, SMPL-02).
//
// The 2026-07-28 transport is handshake-free and session-free: there is no
// `initialize`, no `Mcp-Session-Id` to mint, demand or echo, and no session
// record to hold a negotiated version. Every stage below therefore collapses to
// the answer "there is no session", and the v1 bodies that computed those
// answers are not compiled into this build at all.
//
// # Why the parameters are still here — `session_id: Option<String>` in particular
//
// The POST pipeline threads `session_id: Option<String>` through roughly ten
// functions. Keeping it means it is simply always `None` on this build; dropping
// it would mean rewriting all ten, in a plan whose whole point is that the v1
// wire stays byte-identical. Worse, a call site that no longer had to supply a
// session id would be one edit away from deciding for itself whether sessions
// apply — a SECOND era decision, which Phase 112 D-11 and Phase 113 Pitfall 2
// exist to forbid. Identical signatures are what keep the single decision
// single.
// ---------------------------------------------------------------------------

// Four of the seven twins below are plain `fn` rather than `const fn`: they take
// an OWNED `Option<String>`, whose destructor cannot be evaluated at compile
// time (E0493). Constness follows the SIGNATURE, which is fixed by the real
// half; it is never bought by changing a parameter type.

/// No session is ever minted, because there is no `initialize` to mint one for.
///
/// `(None, false)` is exactly what the real function answers when sessions are
/// inactive — "no session id, and nothing was newly created".
pub(crate) fn process_init_session(
    _state: &ServerState,
    _era: Option<Era>,
    _session_id: Option<String>,
    _protocol_version: Option<String>,
) -> std::result::Result<(Option<String>, bool), Response> {
    Ok((None, false))
}

/// Nothing is required and nothing is validated, so nothing can be rejected.
///
/// An inbound `Mcp-Session-Id` is IGNORED rather than rejected, which is the
/// transport spec taken literally — and here it is structural: there is no
/// session map to look the id up in, so it cannot be consulted by accident.
pub(crate) fn validate_non_init_session(
    _state: &ServerState,
    _era: Option<Era>,
    _session_id: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    Ok(None)
}

/// No version is ever negotiated out of an `initialize` result, because this
/// build has no `initialize` exchange to read one from.
///
/// The 2026-07-28 transport carries its version per request, in a header, so a
/// handshake-negotiated version would be the wrong authority even if one existed.
pub(crate) const fn extract_negotiated_version(_response: &TransportMessage) -> Option<String> {
    None
}

/// Recording the outcome of an initialization is a no-op: nothing was recorded
/// to update.
pub(crate) fn update_session_after_init(
    _state: &ServerState,
    _session_id: Option<&String>,
    _negotiated_version: Option<String>,
) {
}

/// A per-request version can never disagree with a session-recorded one,
/// because no session records one.
///
/// This is the same `Ok(())` the real function returns from its own first line
/// when sessions are inactive — so the twin is not a behaviour change but the
/// compile-time realisation of a behaviour that already held. On this build the
/// per-request `MCP-Protocol-Version` is the sole authority, which is the
/// Phase-112 lock stated positively.
pub(crate) const fn validate_protocol_version_matches_session(
    _state: &ServerState,
    _era: Option<Era>,
    _session_id: Option<&String>,
    _protocol_version: Option<&String>,
) -> std::result::Result<(), Response> {
    Ok(())
}

/// Nothing is ever an `initialize` request, because the 2026-07-28 transport
/// has no handshake.
///
/// The caller uses this flag to decide session minting, so a constant `false`
/// means the mint path is unreachable on this build by construction rather than
/// by a runtime guard.
pub(crate) const fn is_initialize_request(_message: &TransportMessage) -> bool {
    false
}

/// There is never a response session id, on any request, ever.
///
/// Both branches — mint on `initialize`, validate otherwise — collapse to the
/// same `Ok(None)`. They are still WRITTEN as two branches, mirroring the real
/// half, for the reason plan 117-09 recorded for the era twins: routing through
/// the two stage functions keeps them the single place the answer comes from on
/// BOTH halves, and keeps `is_init_request` visibly CONSUMED rather than
/// discarded.
pub(crate) fn resolve_session_for_request(
    state: &ServerState,
    era: Option<Era>,
    is_init_request: bool,
    session_id: Option<String>,
    protocol_version: Option<String>,
) -> std::result::Result<Option<String>, Response> {
    if is_init_request {
        let (sid, _is_new) = process_init_session(state, era, session_id, protocol_version)?;
        Ok(sid)
    } else {
        validate_non_init_session(state, era, session_id)
    }
}
