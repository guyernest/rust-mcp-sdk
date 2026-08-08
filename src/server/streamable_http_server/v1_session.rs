//! MCP 2025-11-25 (v1) session and SSE-resumability state — the `v1-compat` half.
//!
//! # What this module is
//!
//! This is the REAL half of a paired module. Its twin is
//! `v1_session_off.rs`, and exactly one of the two is compiled:
//!
//! ```text
//! #[cfg_attr(feature = "v1-compat", path = "streamable_http_server/v1_session.rs")]
//! #[cfg_attr(not(feature = "v1-compat"), path = "streamable_http_server/v1_session_off.rs")]
//! mod v1;
//! ```
//!
//! Every item declared here MUST also be declared, with an identical signature,
//! by the twin — otherwise `cargo build --no-default-features --features full-v2`
//! stops compiling. `tests/v1_severability_tripwire.rs` asserts the inclusion
//! direction that a build cannot: that the twin declares nothing this module
//! does not.
//!
//! # Why a paired module rather than scattered `#[cfg]`
//!
//! The alternative — sprinkling `#[cfg(feature = "v1-compat")]` through the
//! 6,000-line transport file — puts the severance boundary in dozens of places
//! that no reviewer can hold in their head at once. A pair puts the whole
//! boundary in two files: what v1 IS, and what v2 answers INSTEAD.
//!
//! # Operations, never borrows
//!
//! Every entry point below returns an OWNED answer or performs a whole
//! operation. None of them hands out a `&Arc<RwLock<HashMap<…>>>`, and that is a
//! hard rule rather than a style preference: a zero-sized twin has no map to
//! return a reference to, so a borrow-shaped accessor would be
//! *unimplementable* on the `full-v2` build and would force a `#[cfg]` back into
//! the transport. An `Option`-returning accessor is fine wherever the twin can
//! answer `None` honestly; a reference-to-collection accessor never is.
//!
//! # Why these take `&V1State` and the era chokepoints take `&ServerState`
//!
//! The state operations take `&V1State`, so a call site reads
//! `ServerState::v1` on BOTH feature sets. The era chokepoints keep their
//! `&ServerState` signature because it is the SHIPPED one and changing it would
//! invite a second era resolver (Phase 112 D-11 / 113 Pitfall 2).
//!
//! That split is not cosmetic. Give the operations `&ServerState` too and every
//! null twin ignores its `state` argument, so nothing reads `ServerState::v1` on
//! a `full-v2` build and `RUSTFLAGS="-D warnings"` fails the severance build
//! with `field `v1` is never read`. The only ways out are a blanket dead-code
//! `allow` on the seam field — which blunts the exact lint plan 117-05 wired the
//! CI gate around — or this signature. This is the signature.
//!
//! # Scope in this plan
//!
//! Plan 117-06 landed the mechanism on a small payload. Plan 117-09 collapsed
//! the three v1 fields off `ServerState` into [`V1State`] and moved the session
//! and resumability chokepoints here. Plans 117-12 and 117-13 move the
//! SSE-replay and header machinery, at which point several of the fine-grained
//! operations below fold into the function bodies that call them.
//!
//! # Removal, not just gating
//!
//! Gating is reversible and semver-safe; DELETING this pair is a major-version
//! change tracked as SMPL-F1 (pmcp 3.0). The (deliberately date-free) policy
//! that decides when that happens is `docs/v1-sunset-policy.md`.

// Why: this is a `pub(crate) mod`, so `pub(crate)` on its items is correct
// (internal-only, never part of the public API) but clippy's nursery
// `redundant_pub_crate` flags it while the crate-level `unreachable_pub` warn
// rejects plain `pub`. The two lints conflict for an internal `pub(crate)`
// module; keeping `pub(crate)` items + this scoped allow is the idiomatic
// resolution already used by `src/server/task_dispatch.rs` and
// `src/shared/http_body_cap.rs`. The twin carries the identical allow.
#![allow(clippy::redundant_pub_crate)]

use super::{EventStoreHandle, ServerState, StreamableHttpServerConfig};
use crate::shared::http_constants::MCP_SESSION_ID;
use crate::shared::TransportMessage;
use crate::types::protocol::Era;
use axum::http::{HeaderMap, HeaderValue};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Type alias for the live v1 SSE stream map.
///
/// Named so the field type below reads at a glance and so the twin can be
/// compared against a short, stable signature rather than a nested generic.
pub(crate) type SseStreamMap = HashMap<String, mpsc::UnboundedSender<TransportMessage>>;

/// What the transport remembers about one MCP 2025-11-25 session.
///
/// Both fields are private to this module: every read and write goes through an
/// operation below, so the twin never has to model a shape it does not hold.
#[derive(Debug, Clone)]
pub(crate) struct SessionInfo {
    initialized: bool,
    protocol_version: Option<String>,
}

/// All state that exists ONLY for MCP 2025-11-25.
///
/// The three fields are the v1 session lifecycle (`sessions`), the v1 live SSE
/// fan-out those sessions address (`sse_streams`), and v1 SSE resumability
/// (`event_store`). None of the three has a v2 counterpart: the 2026-07-28
/// transport is handshake-free and session-free, and states that resumable SSE
/// via `Last-Event-ID` is not supported.
///
/// On a `full-v2` build this type is the zero-sized twin, so none of these
/// allocations happen — that is the structural half of the SMPL-02 claim, and
/// it is a property of the TYPE rather than of a runtime branch anyone could
/// forget to take.
#[derive(Clone, Default)]
pub(crate) struct V1State {
    /// Active v1 SSE streams, keyed by session id.
    pub(crate) sse_streams: Arc<RwLock<SseStreamMap>>,
    /// v1 session tracking: session id -> session info.
    pub(crate) sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    /// The v1 resumability event store, type-erased from `config.event_store`.
    ///
    /// Always derived from the config in production ([`V1State::new`] is the
    /// only constructor). It lives here rather than being read straight off the
    /// config so every resumability helper can be written against the
    /// [`EventStore`](super::EventStore) trait — see [`EventStoreHandle`] for
    /// why the public config field's concrete type must not change. Reach it
    /// ONLY through `resumability_store`, never directly.
    pub(crate) event_store: Option<EventStoreHandle>,
}

impl V1State {
    /// Build the v1 state a server starts with, from its configuration.
    ///
    /// Type-erases the configured store ONCE, here, so every resumability helper
    /// is written against the [`EventStore`](super::EventStore) trait and never
    /// touches the concrete `InMemoryEventStore` the public config field pins.
    ///
    /// This is called from `make_server_state`, the transport's single
    /// `ServerState` construction site, with no `#[cfg]` around it — the twin
    /// takes the same argument and allocates nothing.
    pub(crate) fn new(config: &StreamableHttpServerConfig) -> Self {
        Self {
            sse_streams: Arc::new(RwLock::new(SseStreamMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            event_store: config
                .event_store
                .clone()
                .map(|store| store as EventStoreHandle),
        }
    }
}

// ---------------------------------------------------------------------------
// v1 session-map operations.
// ---------------------------------------------------------------------------

/// Is a v1 session with this id being tracked?
pub(crate) fn session_exists(state: &V1State, session_id: &str) -> bool {
    state.sessions.read().contains_key(session_id)
}

/// Has this session already completed `initialize`?
///
/// `false` for an id we have never seen, which is exactly what the
/// re-initialization guard wants: an unknown session cannot have been
/// initialized, so the guard falls through rather than rejecting.
pub(crate) fn session_is_initialized(state: &V1State, session_id: &str) -> bool {
    state
        .sessions
        .read()
        .get(session_id)
        .is_some_and(|info| info.initialized)
}

/// Start tracking a v1 session.
///
/// Takes the two [`SessionInfo`] fields as arguments rather than the struct, so
/// the transport never has to name a type the twin does not have.
pub(crate) fn insert_session(
    state: &V1State,
    session_id: String,
    initialized: bool,
    protocol_version: Option<String>,
) {
    state.sessions.write().insert(
        session_id,
        SessionInfo {
            initialized,
            protocol_version,
        },
    );
}

/// Mark a session initialized and record the version it negotiated.
///
/// A session that negotiated nothing explicit is recorded at
/// `DEFAULT_PROTOCOL_VERSION`, unchanged from before the move. An unknown id is
/// a no-op.
pub(crate) fn mark_session_initialized(
    state: &V1State,
    session_id: &str,
    negotiated_version: Option<String>,
) {
    if let Some(info) = state.sessions.write().get_mut(session_id) {
        info.initialized = true;
        info.protocol_version =
            negotiated_version.or_else(|| Some(crate::DEFAULT_PROTOCOL_VERSION.to_string()));
    }
}

/// The protocol version recorded against a session, if it has one.
///
/// Deliberately collapses "no such session" and "session with no recorded
/// version" into the same `None`. Both callers already treated the two cases
/// identically — one falls back to `DEFAULT_PROTOCOL_VERSION`, the other skips
/// its comparison — so the collapse is behaviour-preserving and removes a
/// distinction the twin could not represent.
pub(crate) fn session_protocol_version(state: &V1State, session_id: &str) -> Option<String> {
    state
        .sessions
        .read()
        .get(session_id)
        .and_then(|info| info.protocol_version.clone())
}

/// Stop tracking a v1 session.
pub(crate) fn remove_session(state: &V1State, session_id: &str) {
    state.sessions.write().remove(session_id);
}

// ---------------------------------------------------------------------------
// v1 SSE stream operations.
// ---------------------------------------------------------------------------

/// Is an SSE stream already open for this session?
pub(crate) fn sse_stream_exists(state: &V1State, session_id: &str) -> bool {
    state.sse_streams.read().contains_key(session_id)
}

/// Register the sending half of a newly opened v1 SSE stream.
pub(crate) fn register_sse_stream(
    state: &V1State,
    session_id: String,
    sender: mpsc::UnboundedSender<TransportMessage>,
) {
    state.sse_streams.write().insert(session_id, sender);
}

/// Close the SSE stream for a session, if one is open.
pub(crate) fn remove_sse_stream(state: &V1State, session_id: &str) {
    state.sse_streams.write().remove(session_id);
}

/// Try to hand a response to a session's live SSE stream.
///
/// Returns `None` when the message went into a live stream — the caller then
/// answers `202 Accepted` — and `Some(message)` when there was no stream to
/// take it, giving ownership back so the caller can frame it as a one-shot SSE
/// response instead.
///
/// This is the SSE-stream read that outlives every later plan in this phase
/// (`build_response` is in no plan's move list), so its shape is load-bearing.
/// A `&Arc<RwLock<SseStreamMap>>` accessor could not be implemented by the
/// zero-sized twin; moving ownership of `message` in and, on the
/// not-delivered path, back out keeps the whole lock scope on this side of the
/// seam.
pub(crate) fn route_to_session_stream(
    state: &V1State,
    session_id: &str,
    message: TransportMessage,
) -> Option<TransportMessage> {
    let streams = state.sse_streams.read();
    let Some(sender) = streams.get(session_id) else {
        return Some(message);
    };
    // Best-effort, exactly as before the move: a receiver that has gone away
    // still yields `202 Accepted` rather than a fallback body.
    let _ = sender.send(message);
    None
}

// ---------------------------------------------------------------------------
// Session era gate (Plan 113-04, HTTP-01; MOVED here by plan 117-09).
//
// `stateless()` is a BUILD-TIME config: it clears `session_id_generator` once,
// when the server is constructed. A dual-version server is built with
// `Default::default()`, which keeps a live generator — so every session decision
// that keys off the CONFIG would mint, demand and echo session ids for v2
// requests too (RESEARCH Pitfall 1). HTTP-01 requires the opposite: on v2 there
// is no handshake and no session at all.
//
// The fix is one predicate, not a transport fork. Every session decision routes
// through `sessions_active`, which makes the ERA the decider and leaves the v1
// path byte-for-byte unchanged.
// ---------------------------------------------------------------------------

/// The pure session-era rule: are sessions live for THIS request?
///
/// | `cfg_has_generator` | `era`            | result | why |
/// |---------------------|------------------|--------|-----|
/// | `true`              | `Some(Era::V2)`  | `false`| v2 is handshake-free and session-free (HTTP-01) |
/// | `true`              | `Some(Era::V1)`  | `true` | v1 session behavior is untouched |
/// | `true`              | `None`           | `true` | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `false`             | anything         | `false`| an explicitly `stateless()` server stays stateless |
///
/// Split out from [`sessions_active`] so the RULE is unit- and property-testable
/// without constructing a live [`ServerState`].
pub(crate) const fn sessions_active_for(cfg_has_generator: bool, era: Option<Era>) -> bool {
    !matches!(era, Some(Era::V2)) && cfg_has_generator
}

/// Are sessions live for this request? THE single reader of
/// `config.session_id_generator`'s presence.
///
/// `era` is the ALREADY-RESOLVED [`ProtocolContext::era`](crate::types::protocol::ProtocolContext)
/// being CONSUMED here — this layer never runs a second era resolver (Pitfall 2 /
/// D-11). The POST entrypoints resolve it once via the v2 header gate and thread
/// that same value into every session decision below.
///
/// `None` means the server is NOT opted into v2, so no era detection ran at all
/// and the v1 path executes with zero era code (D-04).
pub(crate) fn sessions_active(state: &ServerState, era: Option<Era>) -> bool {
    sessions_active_for(state.config.session_id_generator.is_some(), era)
}

/// The session-id generator to use for THIS request, or `None` when sessions are
/// not active for it.
///
/// The second (and last) permitted reader of `config.session_id_generator`: it
/// gates the borrow behind [`sessions_active`] so no caller can reach the
/// generator on a request whose era suppresses sessions.
pub(crate) fn active_session_generator(
    state: &ServerState,
    era: Option<Era>,
) -> Option<&(dyn Fn() -> String + Send + Sync)> {
    if !sessions_active(state, era) {
        return None;
    }
    state.config.session_id_generator.as_deref()
}

/// The ONE place a `Mcp-Session-Id` response header is emitted.
///
/// `response_session_id` is already `None` for a v2 request (both session
/// resolvers return `None` when [`sessions_active`] is false), so this is
/// defense in depth: even a future caller that manufactured a session id could
/// not leak it onto a v2 response. Non-panicking — an unrepresentable id is
/// skipped rather than unwrapped (T-112-13 discipline).
pub(crate) fn apply_session_header(
    headers: &mut HeaderMap,
    response_session_id: Option<&String>,
    sessions_on: bool,
) {
    if !sessions_on {
        return;
    }
    let Some(sid) = response_session_id else {
        return;
    };
    if let Ok(value) = HeaderValue::from_str(sid) {
        headers.insert(MCP_SESSION_ID, value);
    }
}

// ---------------------------------------------------------------------------
// Resumability era gate (Plan 113-08, HTTP-05; MOVED here by plan 117-09).
//
// The 2026-07-28 transport spec is verbatim: "Resumable SSE streams via
// `Last-Event-ID` are not supported", and a `Last-Event-ID` header "ignore it".
// The official conformance suite has already retired its `sse-polling` scenario
// for this revision.
//
// The gate mirrors [`sessions_active`] exactly: ONE predicate, consuming the
// ALREADY-RESOLVED era, routing every read / replay / store decision. It is
// deliberately INDEPENDENT of the session gate. Before plan 113-08 a v2 request
// happened not to reach the event store, but only INCIDENTALLY — the store write
// is conditioned on a `response_session_id`, which the session gate already
// zeroes on v2. An incidental guarantee is not a guarantee: the SSE-stream
// routing bug that plan fixed is exactly what happens when one of those two
// couplings is broken and the other is assumed to cover it.
//
// `EventStoreHandle` itself stays in the transport rather than moving here —
// see the SEVERABILITY note beside its declaration for why the null twin must
// not be the thing that declares `Arc<dyn EventStore>`.
// ---------------------------------------------------------------------------

/// The pure resumability rule: is event replay/retention live for THIS request?
///
/// | `cfg_has_event_store` | `era`           | result | why |
/// |-----------------------|-----------------|--------|-----|
/// | `true`                | `Some(Era::V2)` | `false`| v2 does not offer resumability at all (HTTP-05) |
/// | `true`                | `Some(Era::V1)` | `true` | v1 resumability is untouched |
/// | `true`                | `None`          | `true` | not opted into v2 → zero era code, v1 path unchanged (D-04) |
/// | `false`               | anything        | `false`| no store configured, nothing to read or write |
///
/// Split out from [`resumability_active`] so the RULE is unit- and
/// property-testable without constructing a live [`ServerState`].
pub(crate) const fn resumability_active_for(cfg_has_event_store: bool, era: Option<Era>) -> bool {
    // The RULE is shared with `sessions_active_for` — both facilities are
    // "v1-only, and only when configured". Sharing the pure predicate does NOT
    // couple the two GATES (the point of keeping them independent): each still
    // reads its own config field, `event_store` here and `session_id_generator`
    // there. What it removes is a second copy of the era rule that had to be
    // edited in lockstep, along with a cloned truth table and a cloned proptest.
    sessions_active_for(cfg_has_event_store, era)
}

/// Is resumability live for this request? THE single reader of the event
/// store's presence.
///
/// `era` is the ALREADY-RESOLVED [`ProtocolContext::era`](crate::types::protocol::ProtocolContext)
/// being CONSUMED here — this layer never runs a second era resolver (Pitfall 2 /
/// D-11), exactly as [`sessions_active`] does not.
///
/// `None` means the server is NOT opted into v2, so no era detection ran at all
/// and the v1 path executes with zero era code (D-04).
pub(crate) fn resumability_active(state: &ServerState, era: Option<Era>) -> bool {
    resumability_active_for(state.v1.event_store.is_some(), era)
}

/// The event store to use for THIS request, or `None` when its era suppresses
/// resumability.
///
/// The second (and last) permitted reader of the v1 event store: it gates
/// the borrow behind [`resumability_active`], so no caller can reach the store —
/// to REPLAY from it or to WRITE to it — on a v2 request. Storing without
/// replaying would be dead retention of response envelopes, which is precisely
/// the material an id-replay bug feeds on (T-113-30).
pub(crate) fn resumability_store(
    state: &ServerState,
    era: Option<Era>,
) -> Option<&EventStoreHandle> {
    if !resumability_active(state, era) {
        return None;
    }
    state.v1.event_store.as_ref()
}
