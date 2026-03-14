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
/// Registers 5 tools, 1 resource handler (9 documentation URIs), and
/// 7 workflow prompts for MCP SDK development assistance.
pub fn build_server() -> Result<pmcp::server::Server, pmcp::Error> {
    pmcp::Server::builder()
        .name("pmcp")
        .version(env!("CARGO_PKG_VERSION"))
        // Testing tools
        .tool("test_check", tools::TestCheckTool)
        .tool("test_generate", tools::TestGenerateTool)
        .tool("test_apps", tools::TestAppsTool)
        // Build tools
        .tool("scaffold", tools::ScaffoldTool)
        .tool("schema_export", tools::SchemaExportTool)
        // Documentation resources
        .resources(resources::DocsResourceHandler)
        // Workflow prompts
        .prompt("quickstart", prompts::QuickstartPrompt)
        .prompt("create-mcp-server", prompts::CreateMcpServerPrompt)
        .prompt("add-tool", prompts::AddToolPrompt)
        .prompt("diagnose", prompts::DiagnosePrompt)
        .prompt("setup-auth", prompts::SetupAuthPrompt)
        .prompt("debug-protocol-error", prompts::DebugProtocolErrorPrompt)
        .prompt("migrate", prompts::MigratePrompt)
        .build()
}
