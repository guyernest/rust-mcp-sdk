//! `cargo pmcp agent <subcommand>` — scaffold and run deploy-anywhere agents.
//!
//! Mirrors the `workbook` command-group shape (D-01), but the group's
//! `execute` is ASYNC: `agent dev` drives the `pmcp-agent` loop over an
//! OpenAI-compatible completion source. Plan 110-01 lands the group + stubbed
//! handlers; Plans 110-02 (`new`) and 110-03 (`dev`) fill the bodies.

pub mod dev;
pub mod new;
pub mod run;
pub mod sources;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp agent <subcommand>` — the agent command group.
#[derive(Debug, Subcommand)]
pub enum AgentCommand {
    /// Scaffold a new agent package (delivered by Plan 110-02).
    New(new::NewArgs),
    /// Run an agent loop against a completion source (delivered by Plan 110-03).
    Dev(dev::DevArgs),
}

impl AgentCommand {
    /// Dispatch the subcommand to its handler.
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            AgentCommand::New(args) => new::execute(args, global_flags),
            AgentCommand::Dev(args) => dev::execute(args, global_flags).await,
        }
    }
}
