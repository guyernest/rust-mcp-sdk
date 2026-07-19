//! Composition layer: decide which reference servers a captured
//! [`pmcp_package::TeamPackage`] gets, and wire them into a running stack.
//!
//! - [`derive`] — the pure `derive_attachment` rule (team-mcp iff ≥2 AI
//!   agents, approval-mcp iff ≥1 human role, team-fs/mem-mcp only when
//!   explicitly listed). Implemented atomically in this plan (109-01).
//! - [`resolver`] — the `PackageResolver` seam (`ComponentRef` →
//!   `AgentPackage`) that team-mcp dispatch (109-05/109-06) resolves member
//!   agents through.
//! - [`wiring`] — turns an `AttachmentSet` into an attached, running server
//!   stack (implemented in 109-06).

pub mod derive;
pub mod resolver;
pub mod wiring;

pub use wiring::{EnabledServers, RuntimeError, TeamRuntime, TeamRuntimeBuilder};
