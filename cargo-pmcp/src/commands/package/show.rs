//! `cargo pmcp package show` — fetch and render a published `WorkflowManifest`
//! by `name@version` from the pmcp.run platform (D-D). Remote, platform-side
//! read-only client — renders only, never re-sorts (the crate already
//! guarantees stable `(component_type, name)` / slot-key ordering).

use anyhow::Result;
use clap::Args;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package show`.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Workflow reference in `name@X.Y.Z` form (e.g. `support-triage@1.2.0`).
    pub reference: String,

    /// Emit stable diff JSON instead of the default human-readable tree.
    ///
    /// This is STABLE PRESENTATION JSON for `diff`-ing two `show` outputs —
    /// it is NOT necessarily the canonical digest-serialization bytes used by
    /// `WorkflowManifest::manifest_digest()`.
    #[arg(long)]
    pub json: bool,
}

/// Fetch and render a published workflow manifest.
pub async fn execute(_args: ShowArgs, _global_flags: &GlobalFlags) -> Result<()> {
    anyhow::bail!("cargo pmcp package show is not yet implemented")
}
