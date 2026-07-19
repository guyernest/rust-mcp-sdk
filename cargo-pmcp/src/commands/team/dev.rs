//! `cargo pmcp team dev` — run the reference team servers locally.
//!
//! Stub (Plan 110-01). Plan 110-04 implements the body and renames the
//! underscore-prefixed params.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp team dev`.
#[derive(Debug, Args)]
pub struct DevArgs {
    /// Serve the team over HTTP instead of running in-process.
    #[arg(long)]
    pub serve: bool,
    /// Port for the HTTP serve path.
    #[arg(long, default_value_t = 8080)]
    pub port: u16,
    /// LLM endpoint for team members backed by a real model.
    #[arg(long)]
    pub llm: Option<String>,
    /// Path to the team package to run.
    #[arg(long)]
    pub package: Option<PathBuf>,
    /// Directory for team-fs / mem-mcp state.
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Allow a plain-HTTP (non-TLS) serve/LLM endpoint.
    #[arg(long)]
    pub allow_insecure_http: bool,
}

/// Run the reference team servers (stub — implemented in Plan 110-04).
pub async fn execute(_args: DevArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("team dev: implemented in plan 110-04")
}
