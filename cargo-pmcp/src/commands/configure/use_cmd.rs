//! `cargo pmcp configure use` — placeholder. Filled in by Plan 77-04.

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure use`. Filled in by Plan 77-04.
#[derive(Debug, Args)]
pub struct UseArgs {
    /// Target name to activate (filled in by Plan 04).
    #[arg(hide = true)]
    pub name: Option<String>,
}

/// Stub handler — implemented in Plan 77-04.
pub fn execute(_args: UseArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("configure use: not yet implemented (Plan 77-04)")
}
