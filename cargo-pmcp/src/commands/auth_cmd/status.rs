//! `cargo pmcp auth status` — tabular cache inspection. STUB (Task 2.3).

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// `cargo pmcp auth status [<url>]` — STUB args (Task 2.1).
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// URL to inspect. If absent, prints a table of all cached servers.
    pub url: Option<String>,
}

/// Placeholder handler; replaced in Task 2.3.
pub async fn execute(_args: StatusArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("cargo pmcp auth status not yet implemented (Task 2.3)")
}
