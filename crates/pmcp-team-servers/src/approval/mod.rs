//! approval-mcp — a dev-grade human-approval MCP server.
//!
//! Reference implementation of the `approval_tool_surface` equation in
//! `contracts/team-servers-v1.yaml`: the two UNNAMESPACED legacy static tools
//! (`resolve_approval`, `get_approval`) plus a dynamic
//! `team_approval__ask_<member>` family (one per human roster member). Empty
//! documented seam — implemented in 109-04.

pub mod channels;
pub mod repository;
pub mod server;
