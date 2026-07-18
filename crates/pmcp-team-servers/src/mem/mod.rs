//! mem-mcp — a dev-grade in-memory memory MCP server (`mem__*` tools).
//!
//! Reference implementation of the `mem_tool_surface` equation in
//! `contracts/team-servers-v1.yaml` (6 namespaced `mem__*` tools) with a
//! dependency-free BM25 search. Empty documented seam — implemented in 109-03.

pub mod backend;
pub mod bm25;
pub mod server;
