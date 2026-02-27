//! `cargo pmcp loadtest` CLI subcommands.
//!
//! Provides `run` (execute a load test) and `init` (generate starter config).

mod init;
mod run;

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
            } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(run::execute_run(
                    url, config, vus, duration, iterations, no_report, no_color,
                ))
            },
            LoadtestCommand::Init { url, force } => {
                let runtime = tokio::runtime::Runtime::new()?;
                runtime.block_on(init::execute_init(url, force))
            },
        }
    }
}
