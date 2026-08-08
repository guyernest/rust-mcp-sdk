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

use super::{EventStoreHandle, StreamableHttpServerConfig};
use crate::shared::TransportMessage;
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
// v1 resumability state.
// ---------------------------------------------------------------------------

/// The configured v1 resumability store, if any.
///
/// An `Option`-returning accessor passes the twin test: `full-v2` answers `None`
/// honestly, because that build has no store at all. Callers must still route
/// through `resumability_store`, which additionally gates on the request's
/// era.
pub(crate) fn event_store(state: &V1State) -> Option<&EventStoreHandle> {
    state.event_store.as_ref()
}
