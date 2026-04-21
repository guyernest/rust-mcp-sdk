//! `cargo pmcp auth login` — PKCE + optional DCR, cache result.
//!
//! STUB — full implementation in Task 2.3.

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// `cargo pmcp auth login <url> [flags]` — STUB args surface (Task 2.1 scaffold).
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// URL of the MCP server to authenticate against
    pub url: String,
}

/// Placeholder handler; replaced in Task 2.3.
pub async fn execute(_args: LoginArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("cargo pmcp auth login not yet implemented (Task 2.3)")
}
