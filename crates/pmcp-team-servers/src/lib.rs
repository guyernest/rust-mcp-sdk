//! # pmcp-team-servers
//!
//! Dev-grade **reference implementations** of the four team-server tool
//! surfaces contracted in `contracts/team-servers-v1.yaml`:
//!
//! - **team-fs** — a local-directory filesystem server (`fs__*` tools).
//! - **mem-mcp** — an in-memory BM25-searchable memory server (`mem__*` tools).
//! - **approval-mcp** — a human-approval server (`resolve_approval`,
//!   `get_approval`, and a dynamic `team_approval__ask_<member>` family).
//! - **team-mcp** — a member-dispatch server (`team_mcp__<member>`) that
//!   forwards to member agents under depth + ancestor-cycle guards.
//!
//! Each server lives behind its own cargo feature (all on by `default`) so a
//! deployment can build a single-server binary via
//! `--no-default-features --features <server>`.
//!
//! This is a **0.x / experimental** crate: it is the SDK's *reference* team
//! stack (contracts + dev-grade impls), not a scaled production backend.
//! Scaled team-memory/approval backends stay on the platform (design DEFER-03).

pub mod compose;
pub mod transport;

#[cfg(feature = "approval-mcp")]
pub mod approval;
#[cfg(feature = "conformance")]
pub mod conformance;
#[cfg(feature = "team-fs")]
pub mod fs;
#[cfg(feature = "mem-mcp")]
pub mod mem;
#[cfg(feature = "team-mcp")]
pub mod team;

pub use transport::DuplexTransport;
