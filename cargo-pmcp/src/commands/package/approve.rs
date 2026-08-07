//! `cargo pmcp package approve` — write an approval for a workflow package by
//! REFERENCE only (D-05/D-06/D-08). One-shot mutation, no polling — the
//! server resolves BOTH digests from the source `WorkflowPackage` row; the
//! CLI never derives or sends a digest (Codex #1 / T-171-25b). Approval is a
//! governance act: gated by the platform's admin-group claim AND an
//! in-handler organization match (D-05 — Cognito groups are pool-wide, not
//! org-scoped).

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

use super::parse_reference;

/// Arguments for `cargo pmcp package approve`.
#[derive(Debug, Args)]
pub struct ApproveArgs {
    /// Workflow reference in `name@X.Y.Z` form (e.g. `day-trip-planner-team@1.0.0`).
    pub reference: String,

    /// Organization id the approval applies to.
    ///
    /// Required by the platform's `approvePackage` mutation — the server
    /// independently re-verifies this matches the caller's own
    /// `custom:organizationId` claim (D-05's org-match half), so a
    /// mismatched value is rejected outright, never silently approved for
    /// the wrong org. Falls back to the `PMCP_ORGANIZATION_ID` env var if
    /// set, matching this CLI's existing env-var convention.
    #[arg(long, env = "PMCP_ORGANIZATION_ID")]
    pub organization_id: String,

    /// Optional freeform evidence link/note (D-07) — e.g. a test-run trace URL.
    #[arg(long)]
    pub evidence: Option<String>,
}

/// Write the approval. One-shot: no polling — unlike `capture`/`import`'s
/// async-job shape, `approvePackage` is a synchronous, immediate mutation.
pub async fn execute(args: ApproveArgs, global_flags: &GlobalFlags) -> Result<()> {
    let (name, version) = parse_reference(&args.reference)?;

    let credentials = auth::get_credentials()
        .await
        .context("Not authenticated. Run: cargo pmcp login")?;

    let approval = graphql::approve_package(
        &credentials.access_token,
        &args.organization_id,
        name,
        version,
        args.evidence.as_deref(),
    )
    .await
    .with_context(|| format!("Failed to approve package {name}@{version}"))?;

    if global_flags.should_output() {
        println!("{} {}", "Approved".green().bold(), args.reference);
        println!("  id: {}", approval.id);
        println!("  approvedAt: {}", approval.approved_at);
    }

    Ok(())
}
