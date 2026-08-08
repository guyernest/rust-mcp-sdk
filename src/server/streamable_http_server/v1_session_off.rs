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

/// The zero-sized stand-in for the v1 session and resumability state.
///
/// A unit struct, not an empty-braced one: the absence of fields is the whole
/// point, and the unit form makes it impossible to add one without changing the
/// declaration that the tripwire reads. Nothing is allocated, nothing is locked,
/// and nothing is retained, because on this build there are no sessions to track
/// and no events to replay.
#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // Why: plan 117-09 wires this into `ServerState`; remove the allow there.
pub(crate) struct V1State;

impl V1State {
    /// The v2 constant answer to "give me v1 state": a value with no contents.
    ///
    /// Callable identically to its twin so the transport's construction site is
    /// written once. Allocating nothing here is not an optimisation; it is the
    /// observable difference between the two builds.
    #[allow(dead_code)] // Why: plan 117-09 calls this from `make_server_state`.
    pub(crate) const fn new() -> Self {
        Self
    }
}
