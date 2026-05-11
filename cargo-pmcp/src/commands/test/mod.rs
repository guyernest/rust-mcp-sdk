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
mod conformance;
mod download;
mod generate;
mod list;
mod run;
mod upload;

use anyhow::Result;
use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

use super::flags::{AuthFlags, FormatValue, ServerFlags};
use super::GlobalFlags;

/// Output format for `cargo pmcp test {check, conformance, apps}` subcommands.
///
/// Phase 79 Wave 0 (Plan 79-05): introduces `--format=json` so the post-deploy
/// verifier (Plan 79-03) can consume `mcp_tester::PostDeployReport` directly
/// without regex-parsing pretty terminal output. `Pretty` (default) preserves
/// the existing human-readable UX byte-identically.
///
/// Local to `test/mod.rs` so it does NOT disturb the existing `FormatValue`
/// (which is shared with the `download` subcommand for text/json scenario
/// output).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TestFormatValue {
    /// Human-readable terminal output (default; existing UX preserved byte-for-byte).
    Pretty,
    /// Machine-readable JSON document on stdout (one `PostDeployReport` per invocation).
    Json,
}

impl TestFormatValue {
    /// Return the format as a static string slice (avoids heap allocation).
    /// Used by integration tests to round-trip the parsed flag value.
    #[allow(dead_code)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Json => "json",
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum TestCommand {
    /// Validate MCP App metadata compliance.
    ///
    /// Checks tools for App-capable metadata (ui.resourceUri), validates MIME
    /// types, cross-references with resources, and optionally validates
    /// host-specific keys.
    ///
    /// MODES:
    ///   * `standard` (default) - permissive: emits ONE summary Warning per
    ///     widget (MCP Apps is optional in the spec).
    ///   * `chatgpt` - also checks for openai/* _meta keys ChatGPT requires.
    ///     For widget validation specifically, `chatgpt` mode is a no-op
    ///     (no widget rows emitted) — preserves the previous behavior.
    ///   * `claude-desktop` - STRICT: statically inspects each widget HTML body
    ///     fetched via resources/read for the @modelcontextprotocol/ext-apps
    ///     import, the new App({...}) constructor, the four required protocol
    ///     handlers (onteardown, ontoolinput, ontoolcancelled, onerror),
    ///     and the app.connect() call. Missing signals are emitted as ERROR
    ///     (one row per missing handler). Honors `--tool` to restrict the
    ///     check to a single tool's widget. Recommended pre-deploy check
    ///     for servers shipping to Claude clients.
    ///
    /// SOURCE vs BUNDLE SCAN:
    ///   * Default — fetches each widget HTML body via resources/read on the
    ///     remote server (BUNDLE scan). Required for CI runs against deployed
    ///     servers.
    ///   * --widgets-dir <path> — reads <path>/*.html from the local
    ///     filesystem (SOURCE scan). Higher-confidence pre-deploy check
    ///     because source files have unmangled identifiers. Mirrors
    ///     `cargo pmcp preview --widgets-dir` semantics.
    Apps {
        /// URL of the MCP server to validate
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

        /// Connection timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,

        /// Path to widgets directory for source-scan mode
        ///
        /// When set, scans `<path>/*.html` source files directly INSTEAD of
        /// fetching widget bodies via `resources/read`. Source HTML has unmangled
        /// identifiers and intact import statements — higher-confidence
        /// pre-deploy check than scanning the bundle. Mirrors `cargo pmcp preview
        /// --widgets-dir` flag semantics.
        #[arg(long)]
        widgets_dir: Option<String>,

        /// Output format: pretty (default, human-readable) or json (machine-readable
        /// for CI / Phase 79 post-deploy verifier consumption).
        #[arg(long, value_enum, default_value = "pretty")]
        format: TestFormatValue,

        #[command(flatten)]
        auth_flags: AuthFlags,
    },

    /// MCP protocol conformance validation
    ///
    /// Validates a server against the MCP protocol spec (2025-11-25).
    /// Tests 5 domains: Core (handshake, version, errors), Tools (list, call),
    /// Resources (list, read), Prompts (list, get), Tasks (lifecycle).
    Conformance {
        /// URL of the MCP server to validate
        url: String,

        /// Strict mode (promote warnings to failures)
        #[arg(long)]
        strict: bool,

        /// Run only specific domains (comma-separated: core,tools,resources,prompts,tasks)
        #[arg(long, value_delimiter = ',')]
        domain: Option<Vec<String>>,

        /// Transport type: http, jsonrpc, or stdio
        #[arg(long)]
        transport: Option<String>,

        /// Connection timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,

        /// Output format: pretty (default, human-readable) or json (machine-readable
        /// for CI / Phase 79 post-deploy verifier consumption).
        #[arg(long, value_enum, default_value = "pretty")]
        format: TestFormatValue,

        #[command(flatten)]
        auth_flags: AuthFlags,
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
        url: String,

        /// Transport type: http (SSE streaming), jsonrpc (simple POST), or stdio
        /// Auto-detected by default based on URL patterns
        #[arg(long)]
        transport: Option<String>,

        /// Connection timeout in seconds
        #[arg(long, default_value = "30")]
        timeout: u64,

        /// Output format: pretty (default, human-readable) or json (machine-readable
        /// for CI / Phase 79 post-deploy verifier consumption).
        #[arg(long, value_enum, default_value = "pretty")]
        format: TestFormatValue,

        #[command(flatten)]
        auth_flags: AuthFlags,
    },

    /// Run test scenarios against an MCP server
    ///
    /// Run tests against a local development server or a deployed remote server.
    /// Scenarios are loaded from the local filesystem.
    Run {
        /// MCP server URL or --server for local testing
        #[command(flatten)]
        server_flags: ServerFlags,

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

        #[command(flatten)]
        auth_flags: AuthFlags,
    },

    /// Generate test scenarios from server capabilities
    ///
    /// Connects to a running MCP server and generates test scenarios
    /// based on its declared tools, resources, and prompts.
    Generate {
        /// MCP server URL or --server for local testing
        #[command(flatten)]
        server_flags: ServerFlags,

        /// Port to connect to (default: 3000)
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Output file path
        #[arg(long, short)]
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

        #[command(flatten)]
        auth_flags: AuthFlags,
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

        /// Output format (text or json)
        #[arg(long, value_enum, default_value = "json")]
        format: FormatValue,
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
                timeout,
                widgets_dir,
                format,
                auth_flags,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(apps::execute(
                    url,
                    mode,
                    tool,
                    strict,
                    transport,
                    timeout,
                    widgets_dir,
                    format,
                    &auth_flags,
                    global_flags,
                ))
            },

            TestCommand::Conformance {
                url,
                strict,
                domain,
                transport,
                timeout,
                format,
                auth_flags,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(conformance::execute(
                    url,
                    strict,
                    domain,
                    transport,
                    timeout,
                    format,
                    &auth_flags,
                    global_flags,
                ))
            },

            TestCommand::Check {
                url,
                transport,
                timeout,
                format,
                auth_flags,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(check::execute(
                    url,
                    transport,
                    timeout,
                    format,
                    &auth_flags,
                    global_flags,
                ))
            },

            TestCommand::Run {
                server_flags,
                port,
                scenarios,
                transport,
                auth_flags,
            } => run::execute(
                server_flags,
                port,
                scenarios,
                transport,
                &auth_flags,
                global_flags,
            ),

            TestCommand::Generate {
                server_flags,
                port,
                output,
                transport,
                all_tools,
                with_resources,
                with_prompts,
                auth_flags,
            } => generate::execute(
                server_flags,
                port,
                output,
                transport,
                all_tools,
                with_resources,
                with_prompts,
                &auth_flags,
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
