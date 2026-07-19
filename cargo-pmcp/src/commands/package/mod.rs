//! `cargo pmcp package <subcommand>` — inspect and capture AI-Package bundles.
//!
//! Mirrors the `workbook` command-group shape (D-01) with an ASYNC `execute`:
//! `show` is sync file-I/O; `capture` resolves a platform target and pulls a
//! bundle asynchronously. Plan 110-01 lands the group + stubbed handlers; Plan
//! 110-05 fills both bodies.

pub mod capture;
pub mod capture_upload;
pub mod kind;
pub mod show;

use anyhow::Result;
use clap::Subcommand;

use super::GlobalFlags;

/// `cargo pmcp package <subcommand>` — the package command group.
#[derive(Debug, Subcommand)]
pub enum PackageCommand {
    /// Show the contents of an AI-Package manifest (delivered by Plan 110-05).
    Show(show::ShowArgs),
    /// Capture a package for a platform target (delivered by Plan 110-05).
    Capture(capture::CaptureArgs),
}

impl PackageCommand {
    /// Dispatch the subcommand to its handler.
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            PackageCommand::Show(args) => show::execute(args, global_flags),
            PackageCommand::Capture(args) => capture::execute(args, global_flags).await,
        }
    }
}
