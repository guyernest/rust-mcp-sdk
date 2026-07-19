//! `cargo pmcp agent dev` — run an agent loop against a completion source.
//!
//! Stub (Plan 110-01). Plan 110-03 implements the body, refines `source` to a
//! clap `ValueEnum`, and renames the underscore-prefixed params.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp agent dev`.
#[derive(Debug, Args)]
pub struct DevArgs {
    /// Completion source driving the agent loop.
    #[arg(long, default_value = "openai-compat")]
    pub source: String,
    /// Endpoint URL for the completion source.
    #[arg(long)]
    pub endpoint: Option<String>,
    /// Path to the agent package to run.
    #[arg(long)]
    pub package: Option<PathBuf>,
    /// Environment variable holding the API key.
    #[arg(long)]
    pub api_key_env: Option<String>,
    /// Allow a plain-HTTP (non-TLS) endpoint.
    #[arg(long)]
    pub allow_insecure_http: bool,
}

/// Run an agent loop (stub — implemented in Plan 110-03).
pub async fn execute(_args: DevArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("agent dev: implemented in plan 110-03")
}
