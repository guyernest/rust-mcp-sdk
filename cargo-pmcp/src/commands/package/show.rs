//! `cargo pmcp package show <path>` — inspect a local `.pmcp` package offline.
//!
//! Opens a local OCI image-layout `.pmcp` package, rejects a zero/multiple-
//! manifest index, resolves the package kind by running the pure
//! [`kind::detect_kind`] leaf over BOTH the manifest `artifactType` AND the
//! config/layer media types (Consensus concern #3), unpacks the typed manifest
//! via `pmcp-package`'s own API (D-04 — fully offline, no network), and renders
//! the kind + key fields. Digest verification lives inside `unpack_*`; failures
//! surface verbatim (V6), never bypassed.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp_package::oci::{unpack_agent, unpack_server, unpack_team, unpack_workflow, OciLayout};
use pmcp_package::{AgentPackage, ServerPackage, TeamPackage, WorkflowManifest};

use super::kind::{detect_kind, PackageKind};
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package show`.
#[derive(Debug, Args)]
pub struct ShowArgs {
    /// Path to the AI-Package (OCI image-layout directory) to inspect.
    pub path: PathBuf,
}

/// Show an AI-Package manifest, fully offline.
pub fn execute(args: ShowArgs, global_flags: &GlobalFlags) -> Result<()> {
    let path = &args.path;

    // V5: validate the path is a real OCI layout BEFORE any unpack. `index.json`
    // is the required entry point of an OCI image layout — its absence means
    // this is not a `.pmcp` package.
    if !path.join("index.json").exists() {
        bail!(
            "{} is not an OCI image layout (.pmcp package) — missing index.json",
            path.display()
        );
    }

    let layout = OciLayout::open(path);
    let index = layout
        .read_index()
        .with_context(|| format!("read index.json from {}", path.display()))?;

    // Reject a zero/multiple-manifest index — do NOT index blindly (Codex MEDIUM).
    let manifests = index.manifests();
    if manifests.len() != 1 {
        bail!(
            "expected exactly one manifest in {}, found {} — not a single-package .pmcp layout",
            path.display(),
            manifests.len()
        );
    }
    let descriptor = &manifests[0];

    // Gather candidate type strings from BOTH sources (Consensus concern #3):
    // the index descriptor's artifactType, the manifest's own artifactType, and
    // the config/layer media types. `read_manifest` does not itself verify the
    // digest — the authoritative digest check runs inside `unpack_*` below.
    let mut candidates: Vec<String> = Vec::new();
    if let Some(at) = descriptor.artifact_type() {
        candidates.push(at.to_string());
    }
    let manifest = layout
        .read_manifest(descriptor)
        .with_context(|| format!("read the package manifest from {}", path.display()))?;
    if let Some(at) = manifest.artifact_type() {
        candidates.push(at.to_string());
    }
    candidates.push(manifest.config().media_type().to_string());
    for layer in manifest.layers() {
        candidates.push(layer.media_type().to_string());
    }

    let kind = candidates
        .iter()
        .find_map(|c| detect_kind(c))
        .ok_or_else(|| anyhow!("unknown package kind: candidates=[{}]", candidates.join(", ")))?;

    render_kind(&layout, kind, global_flags.should_output())
}

/// Unpack the resolved kind (digest-verified) and render it when output is
/// enabled. Unpacking runs even in quiet mode so tamper/digest failures still
/// surface — only the decorative rendering is gated.
fn render_kind(layout: &OciLayout, kind: PackageKind, output: bool) -> Result<()> {
    match kind {
        PackageKind::Agent => {
            let pkg = unpack_agent(layout).context("unpack agent package")?;
            if output {
                render_agent(&pkg);
            }
        },
        PackageKind::Team => {
            let pkg = unpack_team(layout).context("unpack team package")?;
            if output {
                render_team(&pkg);
            }
        },
        PackageKind::Server => {
            let (pkg, _bootstrap) = unpack_server(layout).context("unpack server package")?;
            if output {
                render_server(&pkg);
            }
        },
        PackageKind::Workflow => {
            let manifest = unpack_workflow(layout).context("unpack workflow manifest")?;
            if output {
                render_workflow(&manifest);
            }
        },
    }
    Ok(())
}

/// Print a `label: value` line with a consistent, colored layout.
fn field(label: &str, value: impl std::fmt::Display) {
    println!("  {:<14} {}", format!("{label}:").bright_black(), value);
}

/// Print the `Kind:` header line (lowercase kind label — matched by tests).
fn header(kind: PackageKind) {
    println!("\n{}", "Package".bright_cyan().bold());
    field("Kind", kind.label().bright_green().bold());
}

fn render_agent(pkg: &AgentPackage) {
    header(PackageKind::Agent);
    field("Name", &pkg.name);
    field("Version", &pkg.version);
    field("Instructions", truncate(&pkg.instructions, 72));
    field("Max tokens", pkg.max_tokens);
    field("Max iterations", pkg.max_iterations);
    field("Connectors", pkg.connectors.len());
}

fn render_team(pkg: &TeamPackage) {
    header(PackageKind::Team);
    field("Name", &pkg.name);
    field("Version", &pkg.version);
    field("Members", pkg.members.len());
    field("Human roles", pkg.human_roles.len());
    field("Built-in", pkg.built_in_servers.len());
}

fn render_server(pkg: &ServerPackage) {
    header(PackageKind::Server);
    field("Name", &pkg.name);
    field("Version", &pkg.version);
    field("Config slots", pkg.config_slots.len());
}

fn render_workflow(manifest: &WorkflowManifest) {
    header(PackageKind::Workflow);
    field("Name", &manifest.name);
    field("Version", &manifest.version);
    field("Components", manifest.components.len());
    field("Slots", manifest.aggregated_slots.len());
}

/// Truncate `s` to at most `max` chars, appending an ellipsis when clipped.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let head: String = s.chars().take(max).collect();
    format!("{head}…")
}
