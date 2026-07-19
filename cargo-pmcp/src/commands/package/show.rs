//! `cargo pmcp package show <path>` — show an AI-Package manifest.
//!
//! Stub (Plan 110-01). Plan 110-05 implements the body and renames the
//! underscore-prefixed params.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package show`.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Path to the AI-Package to inspect.
    pub path: PathBuf,
}

/// Show an AI-Package manifest (stub — implemented in Plan 110-05).
pub fn execute(_args: ShowArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("package show: implemented in plan 110-05")
}
