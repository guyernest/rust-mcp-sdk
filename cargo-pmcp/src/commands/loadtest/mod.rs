//! `cargo pmcp loadtest` CLI subcommands.
//!
//! Provides `run` (execute a load test), `init` (generate starter config),
//! and `upload` (send config to pmcp.run for cloud execution).

mod init;
mod run;
mod upload;

use anyhow::Result;
use clap::Subcommand;
use std::path::PathBuf;

/// Load test commands for MCP servers.
#[derive(Debug, Subcommand)]
pub enum LoadtestCommand {
    /// Run a load test against an MCP server
    ///
    /// Executes a load test using the scenario defined in .pmcp/loadtest.toml
    /// (or a custom config path). Reports results to the terminal and writes
    /// a JSON report to .pmcp/reports/.
    Run {
        /// Target MCP server URL
        url: String,

        /// Path to config file (default: auto-discover .pmcp/loadtest.toml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Number of virtual users (overrides config)
        #[arg(long)]
        vus: Option<u32>,

        /// Test duration in seconds (overrides config)
        #[arg(long)]
        duration: Option<u64>,

        /// Iteration limit (overrides config)
        #[arg(long)]
        iterations: Option<u64>,

        /// Disable JSON report output
        #[arg(long)]
        no_report: bool,

        /// Disable colored output
        #[arg(long)]
        no_color: bool,

        /// API key for authentication (sent as Bearer token)
        #[arg(long, env = "MCP_API_KEY")]
        api_key: Option<String>,

        /// OAuth client ID (triggers OAuth flow)
        #[arg(long, env = "MCP_OAUTH_CLIENT_ID")]
        oauth_client_id: Option<String>,

        /// OAuth issuer URL (auto-discovered from server if omitted)
        #[arg(long, env = "MCP_OAUTH_ISSUER")]
        oauth_issuer: Option<String>,

        /// OAuth scopes (comma-separated, default: openid)
        #[arg(long, env = "MCP_OAUTH_SCOPES", value_delimiter = ',')]
        oauth_scopes: Option<Vec<String>>,

        /// Disable OAuth token caching
        #[arg(long)]
        oauth_no_cache: bool,

        /// OAuth redirect port for localhost callback (default: 8080)
        #[arg(long, env = "MCP_OAUTH_REDIRECT_PORT", default_value = "8080")]
        oauth_redirect_port: u16,
    },

    /// Generate a starter loadtest config file
    ///
    /// Creates .pmcp/loadtest.toml with sensible defaults. If a server URL
    /// is provided, discovers available tools/resources/prompts and populates
    /// the scenario with real tool names.
    Init {
        /// Optional server URL for schema discovery
        url: Option<String>,

        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
    },

    /// Upload a loadtest config to pmcp.run
    ///
    /// Validates the TOML config locally (parses it and checks that scenarios
    /// exist), then uploads it to pmcp.run for cloud-based load test execution.
    Upload {
        /// Server ID (deployment ID) on pmcp.run
        #[arg(long)]
        server_id: String,

        /// Path to the loadtest TOML config file
        #[arg(required = true)]
        path: PathBuf,

        /// Override config name (defaults to filename stem)
        #[arg(long)]
        name: Option<String>,

        /// Description for the config
        #[arg(long)]
        description: Option<String>,
    },
}

impl LoadtestCommand {
    /// Execute the selected loadtest subcommand.
    pub fn execute(self) -> Result<()> {
        match self {
            LoadtestCommand::Run {
                url,
                config,
                vus,
                duration,
                iterations,
                no_report,
                no_color,
                api_key,
                oauth_client_id,
                oauth_issuer,
                oauth_scopes,
                oauth_no_cache,
                oauth_redirect_port,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(run::execute_run(
                    url,
                    config,
                    vus,
                    duration,
                    iterations,
                    no_report,
                    no_color,
                    api_key,
                    oauth_client_id,
                    oauth_issuer,
                    oauth_scopes,
                    oauth_no_cache,
                    oauth_redirect_port,
                ))
            },
            LoadtestCommand::Init { url, force } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(init::execute_init(url, force))
            },
            LoadtestCommand::Upload {
                server_id,
                path,
                name,
                description,
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(upload::execute(server_id, path, name, description))
            },
        }
    }
}
