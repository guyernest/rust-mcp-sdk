//! `cargo pmcp package <subcommand>` — inspect local AI-Package bundles.
//!
//! Mirrors the `workbook` command-group shape (D-01) with an ASYNC `execute`.
//! `inspect` is a LOCAL, offline OCI-layout inspector. The verbs `show` and
//! `capture` are deliberately NOT defined here — they are reserved for the
//! platform's REMOTE capture service (remote manifest fetch / dependency-graph
//! capture), which has opposite (remote) semantics and will land as a
//! coordinated thin client against the platform's contract.

pub mod inspect;
pub mod kind;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp package <subcommand>` — the package command group.
#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    /// Inspect the kind and key fields of a local AI-Package, fully offline
    Inspect(inspect::InspectArgs),
}

impl PackageCommand {
    /// Dispatch the subcommand to its handler.
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            PackageCommand::Inspect(args) => inspect::execute(args, global_flags),
        }
    }
}
