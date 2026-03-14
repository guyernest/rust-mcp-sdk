//! PMCP Server - MCP developer tools server
//!
//! Provides MCP tools, resources, and prompts for building, testing,
//! and understanding PMCP SDK servers. Served over streamable HTTP.

pub mod content;
pub mod prompts;
pub mod resources;
pub mod tools;

/// Build the PMCP MCP server with all tools, resources, and prompts.
///
/// Returns a minimal server instance. Tools, resources, and prompts
/// will be registered in subsequent plans (02-04) and wired in Plan 05.
pub fn build_server() -> Result<pmcp::server::Server, pmcp::Error> {
    pmcp::Server::builder()
        .name("pmcp")
        .version(env!("CARGO_PKG_VERSION"))
        .build()
}
