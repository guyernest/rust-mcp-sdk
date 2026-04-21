//! `cargo pmcp auth token` — raw access token to stdout. STUB (Task 2.3).

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// `cargo pmcp auth token <url>` — STUB args (Task 2.1).
#[derive(Debug, Args)]
pub struct TokenArgs {
    /// URL of the cached MCP server.
    pub url: String,
}

/// Placeholder handler; replaced in Task 2.3.
pub async fn execute(_args: TokenArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("cargo pmcp auth token not yet implemented (Task 2.3)")
}
