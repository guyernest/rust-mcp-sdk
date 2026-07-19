//! `cargo pmcp agent new <name>` — scaffold a new agent package.
//!
//! Stub (Plan 110-01). Plan 110-02 implements the body and renames the
//! underscore-prefixed params.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp agent new`.
#[derive(Debug, Args)]
pub struct NewArgs {
    /// Name of the agent package to scaffold.
    pub name: String,
    /// Directory to create the package in (defaults to `./<name>`).
    #[arg(long)]
    pub path: Option<PathBuf>,
    /// Overwrite an existing directory.
    #[arg(long)]
    pub force: bool,
}

/// Scaffold a new agent package (stub — implemented in Plan 110-02).
pub fn execute(_args: NewArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("agent new: implemented in plan 110-02")
}
