//! `cargo pmcp team <subcommand>` — run reference team servers.
//!
//! Mirrors the `workbook` command-group shape (D-01) with an ASYNC `execute`:
//! `team dev` composes the `pmcp-team-servers` reference set (in-process or
//! HTTP-served). Plan 110-01 lands the group + stubbed handler; Plan 110-04
//! fills the body.

pub mod dev;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp team <subcommand>` — the team command group.
#[derive(Debug, Subcommand)]
pub enum TeamCommand {
    /// Run an in-process small team (member agents + the four reference servers)
    Dev(dev::DevArgs),
}

impl TeamCommand {
    /// Dispatch the subcommand to its handler.
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            TeamCommand::Dev(args) => dev::execute(args, global_flags).await,
        }
    }
}
