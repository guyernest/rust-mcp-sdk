//! The pure composition-derivation rule `derive_attachment`.
//!
//! Given a captured [`pmcp_package::TeamPackage`], decide which reference
//! servers attach: `team-mcp` iff the roster has ≥2 AI agents, `approval-mcp`
//! iff it declares ≥1 human role, and `team-fs`/`mem-mcp` only when explicitly
//! listed as `built_in_servers` opt-ins.
//!
//! Implemented atomically in this plan (109-01) — the first export of
//! `derive_attachment` is the real rule, never a placeholder.
