//! `cargo pmcp auth logout` — remove cached credentials. STUB (Task 2.3).

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// `cargo pmcp auth logout [<url> | --all]` — STUB args (Task 2.1).
#[derive(Debug, Args)]
pub struct LogoutArgs {
    /// URL of the MCP server to log out from (mutually exclusive with `--all`).
    #[arg(conflicts_with = "all")]
    pub url: Option<String>,

    /// Log out from every cached server.
    #[arg(long)]
    pub all: bool,
}

/// Placeholder handler; replaced in Task 2.3.
pub async fn execute(_args: LogoutArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("cargo pmcp auth logout not yet implemented (Task 2.3)")
}
