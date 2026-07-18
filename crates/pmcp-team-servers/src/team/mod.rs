//! team-mcp — a dev-grade member-dispatch MCP server (`team_mcp__<member>`).
//!
//! Reference implementation of the `team_dispatch_surface` equation in
//! `contracts/team-servers-v1.yaml`: a per-request dynamic member-tool family
//! that forwards a `tools/call` to a member agent under depth + self-call +
//! ancestor-cycle guards, carrying guard state as namespaced `_meta`.
//!
//! - [`identity`] — the `MemberId` identity type + `MemberTaskForwarding`
//!   contract enum (implemented atomically here, 109-01).
//! - [`member`] — per-request member-tool advertisement (implemented in 109-05).
//! - [`guards`] — depth / self-call / ancestor-cycle guard checks
//!   (implemented in 109-05).
//! - [`server`] — builds the team-mcp `pmcp::Server` (implemented in 109-05).

pub mod guards;
pub mod identity;
pub mod member;
pub mod server;
