//! Test MCP servers using mcp-tester library
//!
//! This module provides commands for testing MCP servers both locally and remotely:
//! - `check`: Quick sanity check of MCP server connectivity and compliance
//! - `run`: Run test scenarios against local or deployed servers
//! - `generate`: Generate test scenarios from server capabilities
//! - `upload`: Upload scenarios to pmcp.run for scheduled testing
//! - `download`: Download scenarios from pmcp.run
//! - `list`: List scenarios on pmcp.run

mod apps;
mod check;
mod download;
mod generate;
mod list;
mod run;
mod upload;

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

use super::GlobalFlags;

#[derive(Debug, Subcommand)]
pub enum TestCommand {
    /// Validate MCP App metadata compliance
    ///
    /// Checks tools for App-capable metadata (ui.resourceUri), validates MIME types,
    /// cross-references with resources, and optionally validates host-specific keys.
    Apps {
        /// URL of the MCP server to validate
        #[arg(long, required = true)]
        url: String,

        /// Validation mode: standard, chatgpt, or claude-desktop
        #[arg(long)]
        mode: Option<String>,

        /// Test specific tool only
        #[arg(long)]
        tool: Option<String>,

        /// Strict mode (promote warnings to failures)
        #[arg(long)]
        strict: bool,

        /// Transport type: http, jsonrpc, or stdio
        #[arg(long)]
        transport: Option<String>,

        /// Show verbose output
        #[arg(long, short)]
        verbose: bool,

        /// Connection timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Quick sanity check of an MCP server
    ///
    /// Verifies that an MCP server is reachable, responds correctly to the
    /// initialize handshake, and reports its capabilities. This is the fastest
    /// way to verify a server is working before running full test scenarios.
    ///
    /// Use --verbose to see raw JSON-RPC messages for debugging non-compliant servers.
    Check {
        /// URL of the MCP server to check
        #[arg(long, required = true)]
        url: String,

        /// Transport type: http (SSE streaming), jsonrpc (simple POST), or stdio
        /// Auto-detected by default based on URL patterns
        #[arg(long)]
        transport: Option<String>,

        /// Show verbose output including raw JSON-RPC messages
        #[arg(long, short)]
        verbose: bool,

        /// Connection timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,
    },

    /// Run test scenarios against an MCP server
    ///
    /// Run tests against a local development server or a deployed remote server.
    /// Scenarios are loaded from the local filesystem.
    Run {
        /// Name of the local server to test (uses localhost)
        #[arg(long)]
        server: Option<String>,

        /// URL of the MCP server to test (for remote testing)
        #[arg(long)]
        url: Option<String>,

        /// Port to connect to (default: 3000)
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Path to scenario files or directory
        #[arg(long)]
        scenarios: Option<PathBuf>,

        /// Transport type: http (SSE streaming), jsonrpc (simple POST), or stdio
        /// Auto-detected by default based on URL patterns
        #[arg(long)]
        transport: Option<String>,

        /// Show detailed test output
        #[arg(long)]
        detailed: bool,
    },

    /// Generate test scenarios from server capabilities
    ///
    /// Connects to a running MCP server and generates test scenarios
    /// based on its declared tools, resources, and prompts.
    Generate {
        /// Name of the local server (uses localhost)
        #[arg(long)]
        server: Option<String>,

        /// URL of the MCP server
        #[arg(long)]
        url: Option<String>,

        /// Port to connect to (default: 3000)
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Output file path
        #[arg(long)]
        output: Option<PathBuf>,

        /// Transport type: http (SSE streaming), jsonrpc (simple POST), or stdio
        /// Auto-detected by default based on URL patterns
        #[arg(long)]
        transport: Option<String>,

        /// Include all tools in generated scenarios
        #[arg(long, default_value = "true")]
        all_tools: bool,

        /// Include resource operations
        #[arg(long, default_value = "true")]
        with_resources: bool,

        /// Include prompt operations
        #[arg(long, default_value = "true")]
        with_prompts: bool,
    },

    /// Upload test scenarios to pmcp.run
    ///
    /// Upload local scenario files to pmcp.run for scheduled testing
    /// and cloud-based test execution.
    Upload {
        /// Server name (deployment ID) on pmcp.run
        #[arg(long)]
        server: String,

        /// Path(s) to scenario files or directories
        #[arg(required = true)]
        paths: Vec<PathBuf>,

        /// Override scenario name (only for single file uploads)
        #[arg(long)]
        name: Option<String>,

        /// Description for the scenario
        #[arg(long)]
        description: Option<String>,
    },

    /// Download test scenarios from pmcp.run
    ///
    /// Download scenario files from pmcp.run to edit locally.
    Download {
        /// Scenario ID to download
        #[arg(long)]
        scenario_id: String,

        /// Output file path
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Output format (yaml or json)
        #[arg(long, default_value = "yaml")]
        format: Option<String>,
    },

    /// List test scenarios on pmcp.run
    ///
    /// Show all scenarios configured for an MCP server on pmcp.run.
    List {
        /// Server name (deployment ID) on pmcp.run
        #[arg(long)]
        server: String,

        /// Show all scenarios including disabled ones
        #[arg(long)]
        all: bool,
    },
}

impl TestCommand {
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            TestCommand::Apps {
                url,
                mode,
                tool,
                strict,
                transport,
                verbose,
                timeout,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(apps::execute(
                    url,
                    mode,
                    tool,
                    strict,
                    transport,
                    verbose,
                    timeout,
                    global_flags,
                ))
            },

            TestCommand::Check {
                url,
                transport,
                verbose,
                timeout,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(check::execute(
                    url,
                    transport,
                    verbose,
                    timeout,
                    global_flags,
                ))
            },

            TestCommand::Run {
                server,
                url,
                port,
                scenarios,
                transport,
                detailed,
            } => run::execute(
                server,
                url,
                port,
                scenarios,
                transport,
                detailed,
                global_flags,
            ),

            TestCommand::Generate {
                server,
                url,
                port,
                output,
                transport,
                all_tools,
                with_resources,
                with_prompts,
            } => generate::execute(
                server,
                url,
                port,
                output,
                transport,
                all_tools,
                with_resources,
                with_prompts,
                global_flags,
            ),

            TestCommand::Upload {
                server,
                paths,
                name,
                description,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(upload::execute(
                    server,
                    paths,
                    name,
                    description,
                    global_flags,
                ))
            },

            TestCommand::Download {
                scenario_id,
                output,
                format,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(download::execute(scenario_id, output, format, global_flags))
            },

            TestCommand::List { server, all } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(list::execute(server, all, global_flags))
            },
        }
    }
}

