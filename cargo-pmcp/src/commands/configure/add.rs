//! `cargo pmcp configure add` — placeholder. Filled in by Plan 77-04.

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp configure add`. Filled in by Plan 77-04.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// Target name (filled in by Plan 04).
    #[arg(hide = true)]
    pub name: Option<String>,
}

/// Stub handler — implemented in Plan 77-04.
pub fn execute(_args: AddArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("configure add: not yet implemented (Plan 77-04)")
}
