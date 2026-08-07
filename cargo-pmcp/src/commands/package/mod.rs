//! `cargo pmcp package <subcommand>` — inspect local AI-Package bundles, and
//! capture/show/import/approve remote workflow packages against the pmcp.run
//! platform.
//!
//! Mirrors the `workbook` command-group shape (D-01) with an ASYNC `execute`.
//! `inspect` is a LOCAL, offline OCI-layout inspector (unchanged by this
//! module). `capture`/`show`/`import`/`approve` are the platform's REMOTE
//! thin client (D-10 — no agent-runtime semantics live in the CLI): `capture`
//! submits an async dependency-graph-walk job for a team and polls it to
//! completion; `show` fetches and renders an already published
//! `WorkflowManifest` by `name@version`; `import` submits an async dry-run
//! pre-flight import job and polls/renders the report (D-02 — dry-run is the
//! ONLY supported mode this phase, no execute flag); `approve` writes an
//! approval by workflow REFERENCE only (D-05/D-06/D-08 — the server resolves
//! both digests, never a caller-supplied one).

pub mod approve;
pub mod capture;
pub mod import;
pub mod inspect;
pub mod kind;
pub mod show;

use anyhow::{anyhow, bail, Context, Result};
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
    /// Submit an async dry-run pre-flight import job and render the report
    /// (remote, platform-side — dry-run is the ONLY mode this phase)
    Import(import::ImportArgs),
    /// Approve a workflow package by reference (admin-group + org-match
    /// gated; the server resolves both digests — never a caller-supplied one)
    Approve(approve::ApproveArgs),
}

impl PackageCommand {
    /// Dispatch the subcommand to its handler.
    pub async fn execute(self, global_flags: &GlobalFlags) -> Result<()> {
        match self {
            PackageCommand::Inspect(args) => inspect::execute(args, global_flags),
            PackageCommand::Capture(args) => capture::execute(args, global_flags).await,
            PackageCommand::Show(args) => show::execute(args, global_flags).await,
            PackageCommand::Import(args) => import::execute(args, global_flags).await,
            PackageCommand::Approve(args) => approve::execute(args, global_flags).await,
        }
    }
}

/// Split a `name@X.Y.Z` reference on the LAST `@` (a component name may
/// legitimately contain `@`), validating both halves are non-empty and the
/// version half parses as semver. Shared by `import`/`approve` (mirrors
/// `show.rs`'s own private copy of this exact logic, kept there unchanged
/// since `show.rs` is outside this plan's `files_modified` scope).
pub(super) fn parse_reference(reference: &str) -> Result<(&str, &str)> {
    let (name, version) = reference
        .rsplit_once('@')
        .ok_or_else(|| anyhow!("invalid workflow reference '{reference}' — expected NAME@X.Y.Z"))?;
    if name.is_empty() {
        bail!("invalid workflow reference '{reference}' — component name is empty");
    }
    if version.is_empty() {
        bail!("invalid workflow reference '{reference}' — version is empty");
    }
    semver::Version::parse(version).with_context(|| {
        format!("invalid workflow reference '{reference}' — '{version}' is not valid semver")
    })?;
    Ok((name, version))
}
