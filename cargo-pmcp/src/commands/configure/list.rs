//! `cargo pmcp configure list` — placeholder. Filled in by Plan 77-05.

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure list`. Filled in by Plan 77-05.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Reserved (filled in by Plan 05).
    #[arg(hide = true)]
    pub _reserved: Option<String>,
}

/// Stub handler — implemented in Plan 77-05.
pub fn execute(_args: ListArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("configure list: not yet implemented (Plan 77-05)")
}
