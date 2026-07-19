//! `cargo pmcp package show` — fetch and render a published `WorkflowManifest`
//! by `name@version` from the pmcp.run platform (D-D). Remote, platform-side
//! read-only client — renders only, never re-sorts (the crate already
//! guarantees stable `(component_type, name)` / slot-key ordering).

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp_package::reference::ComponentType;
use pmcp_package::{ComponentRef, ConfigSlot, WorkflowManifest};

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

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
pub async fn execute(args: ShowArgs, global_flags: &GlobalFlags) -> Result<()> {
    let (name, version) = parse_reference(&args.reference)?;

    let credentials = auth::get_credentials()
        .await
        .context("Not authenticated. Run: cargo pmcp login")?;

    let response = graphql::get_workflow_package(&credentials.access_token, name, version)
        .await
        .with_context(|| format!("Failed to fetch workflow package {name}@{version}"))?;

    // Deserialize the canonical WorkflowManifest JSON as-is — never re-sorted,
    // the crate already guarantees `(component_type, name)` / slot-key order.
    let manifest: WorkflowManifest = serde_json::from_str(&response.manifest_json)
        .context("Failed to parse WorkflowManifest JSON returned by the platform")?;

    if args.json {
        // Stable diff JSON (D-D) — NOT necessarily the canonical digest bytes.
        println!("{}", serde_json::to_string_pretty(&manifest)?);
    } else if global_flags.should_output() {
        render_tree(&manifest, &response.manifest_digest);
    }

    Ok(())
}

/// Split a `name@X.Y.Z` reference on the LAST `@` (a component name may
/// legitimately contain `@`), validating both halves are non-empty and the
/// version half parses as semver.
fn parse_reference(reference: &str) -> Result<(&str, &str)> {
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

/// Render the default human-readable tree. Components and slots are printed
/// in the ORDER the crate already produced (canonical `(component_type,
/// name)` / slot-key sort) — this fn never sorts anything itself.
fn render_tree(manifest: &WorkflowManifest, manifest_digest: &str) {
    println!(
        "\n{} {} @ {}",
        "Workflow".bright_cyan().bold(),
        manifest.name.bright_green().bold(),
        manifest.version
    );
    println!("  {} {}", "Digest:".bright_black(), manifest_digest);
    println!(
        "  {} {} ({})",
        "Captured:".bright_black(),
        manifest.provenance.timestamp,
        manifest.provenance.source_environment
    );

    println!("\n  {}", "Components:".bright_black());
    if manifest.components.is_empty() {
        println!("    (none)");
    }
    for component in &manifest.components {
        render_component(component);
    }

    if !manifest.aggregated_slots.is_empty() {
        println!("\n  {}", "Config slots:".bright_black());
        for slot in &manifest.aggregated_slots {
            render_slot(slot);
        }
    }
    println!();
}

/// One `Components:` line — pins show version + digest; a stray unpinned
/// range (should never happen in a published `WorkflowManifest`, but this is
/// a display path, not the validation boundary) is called out explicitly
/// rather than panicking.
fn render_component(component: &ComponentRef) {
    let type_label = component_type_label(component.component_type());
    match component {
        ComponentRef::Pinned(pinned) => {
            println!(
                "    - [{}] {} @ {}  {}",
                type_label,
                pinned.name,
                pinned.version,
                pinned.digest.as_str().bright_black()
            );
        },
        ComponentRef::Range { name, range, .. } => {
            println!("    - [{type_label}] {name} @ {range} (unpinned range)");
        },
    }
}

/// Lowercase display label for a `ComponentType`.
fn component_type_label(component_type: ComponentType) -> &'static str {
    match component_type {
        ComponentType::Server => "server",
        ComponentType::Agent => "agent",
        ComponentType::Team => "team",
    }
}

/// One `Config slots:` line. Behavior-relevant slots (`LlmProvider`,
/// `BudgetOverride`) show their `tested_value`; identity-bearing slots
/// (`Secret`, `OauthClient`, `ChannelBinding`, `HumanRole`) show only their
/// name — the type structurally has no value field to leak.
fn render_slot(slot: &ConfigSlot) {
    let (kind, name) = slot.slot.key();
    match slot.slot.tested_value() {
        Some(value) => println!("    - {kind}:{name} = {value}"),
        None => println!("    - {kind}:{name}"),
    }
}
