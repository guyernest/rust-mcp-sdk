//! `cargo pmcp configure show` — placeholder. Filled in by Plan 77-05.

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure show`. Filled in by Plan 77-05.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Target name (filled in by Plan 05).
    #[arg(hide = true)]
    pub name: Option<String>,
}

/// Stub handler — implemented in Plan 77-05.
pub fn execute(_args: ShowArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("configure show: not yet implemented (Plan 77-05)")
}
