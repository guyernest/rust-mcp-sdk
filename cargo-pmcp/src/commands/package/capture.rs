//! `cargo pmcp package capture` — submit + poll an async workflow capture job
//! against the pmcp.run platform (D-A/D-B). Remote, platform-side: the
//! dependency-graph walk itself runs in the platform's capture worker, never
//! in this CLI (D-10).

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package capture`.
#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// AgentTeam ID (UUID) — the team's id, not its display name.
    ///
    /// v1 requires the exact AgentTeam id: `submitPackageCapture` performs a
    /// DynamoDB `GetItem` by primary key against the `AgentTeam` table, so a
    /// display name will not resolve. A server-side name -> id lookup is a
    /// documented deferral, not supported in v1.
    #[arg(
        value_name = "TEAM_ID",
        long_help = "AgentTeam ID (UUID) — the team's id, not its display name. v1 requires \
                      the exact AgentTeam id (submitPackageCapture does a GetItem by primary \
                      key); a name-to-id lookup is a documented deferral, not supported here."
    )]
    pub team_id: String,

    /// Semver version to capture the workflow package as (e.g. 1.2.3).
    #[arg(long, value_name = "X.Y.Z")]
    pub version: String,

    /// Bump level applied to every component whose deployed bytes changed
    /// since the last capture (major|minor|patch — the single level applies
    /// uniformly across all divergent components; there is no per-component
    /// override). Only consulted when the platform reports
    /// `errorCode=BUMP_REQUIRED`; supply it up front to skip the interactive
    /// TTY prompt (required in non-interactive/CI contexts, where an
    /// unresolved bump-required job fails loud rather than hanging).
    #[arg(long, value_parser = ["major", "minor", "patch"])]
    pub bump: Option<String>,
}

/// Submit a capture job and poll it to a terminal status.
pub async fn execute(_args: CaptureArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("cargo pmcp package capture is not yet implemented")
}
