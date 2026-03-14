//! MCP tool implementations for the PMCP server.
//!
//! Tools:
//! - test_check: Protocol compliance testing
//! - test_generate: Test scenario generation
//! - test_apps: MCP Apps metadata validation
//! - scaffold: Code template generation
//! - schema_export: Schema discovery and export

use mcp_tester::ServerTester;
use std::time::Duration;

pub mod scaffold;
pub mod schema_export;
pub mod test_apps;
pub mod test_check;
pub mod test_generate;

pub use scaffold::ScaffoldTool;
pub use schema_export::SchemaExportTool;
pub use test_apps::TestAppsTool;
pub use test_check::TestCheckTool;
pub use test_generate::TestGenerateTool;

pub(crate) const DEFAULT_TIMEOUT_SECS: u64 = 30;

pub(crate) const fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

pub(crate) fn create_tester(url: &str, timeout_secs: u64) -> pmcp::Result<ServerTester> {
    ServerTester::new(
        url,
        Duration::from_secs(timeout_secs),
        false, // insecure
        None,  // api_key
        None,  // transport (auto-detect)
        None,  // http_middleware_chain
    )
    .map_err(internal_err)
}

/// Adapter for `.map_err()` — `pmcp::Error::internal()` takes `impl Into<String>`
/// which doesn't match `impl Display` error types as a function pointer.
pub(crate) fn internal_err(e: impl std::fmt::Display) -> pmcp::Error {
    pmcp::Error::Internal(e.to_string())
}
