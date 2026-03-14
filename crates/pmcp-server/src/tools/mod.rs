//! MCP tool implementations for the PMCP server.
//!
//! Tools:
//! - test_check: Protocol compliance testing
//! - test_generate: Test scenario generation
//! - test_apps: MCP Apps metadata validation
//! - scaffold: Code template generation (future)
//! - schema_export: Schema discovery and export (future)

pub mod test_check;

pub use test_check::TestCheckTool;
