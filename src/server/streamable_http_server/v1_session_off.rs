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

use super::{EventStoreHandle, StreamableHttpServerConfig};
use crate::shared::TransportMessage;
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

/// Nothing was ever initialized, because nothing was ever created.
pub(crate) const fn session_is_initialized(_state: &V1State, _session_id: &str) -> bool {
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

/// Marking a session initialized is a no-op: no session was created to mark.
pub(crate) fn mark_session_initialized(
    _state: &V1State,
    _session_id: &str,
    _negotiated_version: Option<String>,
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
// v1 resumability state — there is none.
// ---------------------------------------------------------------------------

/// There is no store to hand out, on any request, ever.
///
/// The 2026-07-28 transport spec is verbatim that resumable SSE streams via
/// `Last-Event-ID` are not supported, so `None` is the whole answer rather than
/// a degraded one.
pub(crate) const fn event_store(_state: &V1State) -> Option<&EventStoreHandle> {
    None
}
