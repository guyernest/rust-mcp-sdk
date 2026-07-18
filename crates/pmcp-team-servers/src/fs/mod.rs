//! team-fs — a dev-grade local-directory filesystem MCP server (`fs__*` tools).
//!
//! Reference implementation of the `fs_tool_surface` equation in
//! `contracts/team-servers-v1.yaml` (11 namespaced `fs__*` tools). Empty
//! documented seam — implemented in 109-02 (TEAM-02).

pub mod backend;
pub mod local;
pub mod server;
