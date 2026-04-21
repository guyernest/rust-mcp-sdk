//! `cargo pmcp auth refresh` — force-refresh. STUB (Task 2.3).

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// `cargo pmcp auth refresh <url>` — STUB args (Task 2.1).
#[derive(Debug, Args)]
pub struct RefreshArgs {
    /// URL of the cached MCP server to force-refresh.
    pub url: String,
}

/// Placeholder handler; replaced in Task 2.3.
pub async fn execute(_args: RefreshArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("cargo pmcp auth refresh not yet implemented (Task 2.3)")
}
