//! `cargo pmcp configure` — manage named deployment targets in `~/.pmcp/config.toml`.
//!
//! Modeled on `aws configure`. Each named target carries the type (`pmcp-run`,
//! `aws-lambda`, `google-cloud-run`, `cloudflare-workers`), AWS profile, region,
//! and target-type-specific fields. A workspace selects one target as "active"
//! via `.pmcp/active-target` (single-line marker file).
//!
//! Resolution order (REQ-77-04, REQ-77-06):
//!   `PMCP_TARGET` env > `--target` flag > `.pmcp/active-target` > none (Phase 76 pass-through).
//!
//! User-level config: `~/.pmcp/config.toml` (schema_version: 1).
//! References-only secrets policy (REQ-77-07): no raw credentials persisted.

pub mod add;
pub mod banner;
pub mod config;
pub mod list;
pub mod resolver;
pub mod show;
pub mod use_cmd;
pub mod workspace;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp configure <subcommand>`
#[derive(Debug, Subcommand)]
pub enum ConfigureCommand {
    /// Define a new named target in ~/.pmcp/config.toml
    Add(add::AddArgs),
    /// Activate a target for the current workspace (writes .pmcp/active-target)
    #[command(name = "use")]
    Use(use_cmd::UseArgs),
    /// List all defined targets, marking the active one with `*`
    List(list::ListArgs),
    /// Show resolved configuration for a target with per-field source attribution
    Show(show::ShowArgs),
}

impl ConfigureCommand {
    /// Dispatch the subcommand to its handler.
    pub fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            ConfigureCommand::Add(args) => add::execute(args, global_flags),
            ConfigureCommand::Use(args) => use_cmd::execute(args, global_flags),
            ConfigureCommand::List(args) => list::execute(args, global_flags),
            ConfigureCommand::Show(args) => show::execute(args, global_flags),
        }
    }
}
