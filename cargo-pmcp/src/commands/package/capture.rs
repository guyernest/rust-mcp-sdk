//! `cargo pmcp package capture <path>` — capture a package for a target.
//!
//! Stub (Plan 110-01). Plan 110-05 implements the body and renames the
//! underscore-prefixed params. The capture-local `--target` flag (Codex MEDIUM
//! review decision) selects the platform target via `resolve_target` INSIDE
//! this handler — it is NOT a top-level `GlobalFlags` flag, so `package` never
//! clobbers `PMCP_TARGET`/AWS env for other commands.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package capture`.
#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// Path to the AI-Package to capture.
    pub path: PathBuf,
    /// Capture-local platform target selector (resolved via `resolve_target`).
    #[arg(long)]
    pub target: Option<String>,
}

/// Capture a package for a platform target (stub — implemented in Plan 110-05).
pub async fn execute(_args: CaptureArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("package capture: implemented in plan 110-05")
}
