//! `cargo pmcp package <subcommand>` — inspect local AI-Package bundles, and
//! capture/show remote workflow packages against the pmcp.run platform.
//!
//! Mirrors the `workbook` command-group shape (D-01) with an ASYNC `execute`.
//! `inspect` is a LOCAL, offline OCI-layout inspector (unchanged by this
//! module). `capture` and `show` are the platform's REMOTE thin client (D-10
//! — no agent-runtime semantics live in the CLI; the capture walk executes
//! platform-side): `capture` submits an async dependency-graph-walk job for a
//! team and polls it to completion; `show` fetches and renders an already
//! published `WorkflowManifest` by `name@version`.

pub mod capture;
pub mod inspect;
pub mod kind;
pub mod show;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp package <subcommand>` — the package command group.
#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    /// Inspect the kind and key fields of a local AI-Package, fully offline
    Inspect(inspect::InspectArgs),
    /// Submit an async capture job for a team's workflow dependency graph
    /// (remote, platform-side — polls to a terminal status)
    Capture(capture::CaptureArgs),
    /// Fetch and render a published workflow manifest (remote, platform-side)
    Show(show::ShowArgs),
}

impl PackageCommand {
    /// Dispatch the subcommand to its handler.
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            PackageCommand::Inspect(args) => inspect::execute(args, global_flags),
            PackageCommand::Capture(args) => capture::execute(args, global_flags).await,
            PackageCommand::Show(args) => show::execute(args, global_flags).await,
        }
    }
}
