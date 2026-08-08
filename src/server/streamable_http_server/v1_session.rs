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
//! # Scope in this plan
//!
//! Plan 117-06 lands only [`V1State`] plus its constructor, so the mechanism is
//! proven on a small payload before the transport depends on it. Plan 117-09
//! collapses the three v1 fields off `ServerState` into [`V1State`] and wires
//! the session/resumability chokepoints through this pair; plans 117-12 and
//! 117-13 move the SSE-replay and header machinery. The `#[allow(dead_code)]`
//! attributes below exist only for that gap and are removed by 117-09.
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

use super::{EventStoreHandle, SessionInfo};
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
#[allow(dead_code)] // Why: plan 117-09 wires this into `ServerState`; remove the allow there.
pub(crate) struct V1State {
    /// Active v1 SSE streams, keyed by session id.
    pub(crate) sse_streams: Arc<RwLock<SseStreamMap>>,
    /// v1 session tracking: session id -> session info.
    pub(crate) sessions: Arc<RwLock<HashMap<String, SessionInfo>>>,
    /// The v1 resumability event store, type-erased from `config.event_store`.
    pub(crate) event_store: Option<EventStoreHandle>,
}

impl V1State {
    /// Allocate empty v1 session and SSE state with no event store.
    ///
    /// Plan 117-09 replaces this with the `ServerState` collapse, which threads
    /// the configured store in. Until then the constructor exists so both halves
    /// of the pair present the same surface.
    #[allow(dead_code)] // Why: plan 117-09 calls this from `make_server_state`.
    pub(crate) fn new() -> Self {
        Self::default()
    }
}
