//! `cargo pmcp package load` — read a movable `.tar` artifact back into a
//! working OCI layout directory, fully offline.
//!
//! The inverse of `package save`, and the local half of what `pull` will do
//! with bytes fetched from the platform. Everything about the input is
//! untrusted: a tar handed to `load` came from somewhere, and nothing about its
//! provenance is known at read time.
//!
//! # The ordering, and why it is not restated here
//!
//! This command reads the tar, hands the bytes to
//! [`artifact::read_verified`] — which touches the filesystem zero times — and
//! then hands the resulting [`artifact::VerifiedArtifact`] to
//! [`artifact::install_layout`], which stages, semantically validates the
//! STAGING layout, and renames into place only on success. All three steps'
//! guarantees live in `artifact.rs`, at the functions that provide them,
//! precisely so this file cannot drift from them.
//!
//! `install_layout` is the ONLY function here that materializes a layout, and
//! the kind dispatch runs exactly once — inside the closure, against staging.
//! Unpacking a second time after the install would waste the work, but far
//! worse, it would create a second call site where a future change could
//! reintroduce the install-then-validate ordering this design exists to remove.
//!
//! # What the tracer prints, and what it does not
//!
//! Enough to prove the path: kind, name, version, destination. The full report
//! — config slots, pin facts, carriage states, the attestation verdict — is
//! `package inspect`'s job and belongs to a later plan. That is a functionality
//! gap, not an architectural one: the renderer would consume the same
//! `unpack_*` result this command already holds, through the same kind dispatch
//! `inspect.rs` uses.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp_package::oci::{
    unpack_agent, unpack_server, unpack_team, unpack_workflow, OciLayout, UnpackedServer,
    UnpackedTeam,
};
use pmcp_package::{AgentPackage, WorkflowManifest};

use super::artifact;
use super::kind::{artifact_type_from_manifest_json, detect_kind, PackageKind};
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package load`.
#[derive(Debug, Args)]
pub struct LoadArgs {
    /// The `.tar` artifact to read.
    pub input: PathBuf,

    /// Directory to materialize the OCI layout into.
    #[arg(long, short = 'o')]
    pub output: PathBuf,

    /// Replace an existing `--output` directory.
    #[arg(long)]
    pub force: bool,
}

/// Whatever kind the artifact turned out to be, already unpacked and
/// digest-verified.
///
/// Every variant is boxed: `UnpackedServer` is far larger than the others, and
/// an unboxed enum would size every variant to it.
#[derive(Debug)]
pub enum LoadedPackage {
    /// An agent package.
    Agent(Box<AgentPackage>),
    /// A team package plus its optional attestation.
    Team(Box<UnpackedTeam>),
    /// A server package plus its binary, files and optional attestation.
    Server(Box<UnpackedServer>),
    /// A workflow manifest.
    Workflow(Box<WorkflowManifest>),
}

impl LoadedPackage {
    /// The package kind, for rendering.
    pub fn kind(&self) -> PackageKind {
        match self {
            LoadedPackage::Agent(_) => PackageKind::Agent,
            LoadedPackage::Team(_) => PackageKind::Team,
            LoadedPackage::Server(_) => PackageKind::Server,
            LoadedPackage::Workflow(_) => PackageKind::Workflow,
        }
    }

    /// The package's declared name.
    pub fn name(&self) -> &str {
        match self {
            LoadedPackage::Agent(pkg) => &pkg.name,
            LoadedPackage::Team(unpacked) => &unpacked.package.name,
            LoadedPackage::Server(unpacked) => &unpacked.package.name,
            LoadedPackage::Workflow(manifest) => &manifest.name,
        }
    }

    /// The package's declared version.
    pub fn version(&self) -> String {
        match self {
            LoadedPackage::Agent(pkg) => pkg.version.to_string(),
            LoadedPackage::Team(unpacked) => unpacked.package.version.to_string(),
            LoadedPackage::Server(unpacked) => unpacked.package.version.to_string(),
            LoadedPackage::Workflow(manifest) => manifest.version.to_string(),
        }
    }
}

/// Resolve the package kind of `layout` and unpack it — the SEMANTIC gate.
///
/// Runs against the STAGING layout, never the destination. Kind resolution
/// mirrors `inspect.rs`'s candidate aggregation exactly (the index descriptor's
/// artifact type, the raw manifest parse, the manifest's own artifact type, the
/// config media type, then every layer media type), so the two commands can
/// never disagree about what a package is.
///
/// This is where the substantive validation happens: `unpack_*` checks manifest
/// structure, required media types, the config blob, the pre-0.2.0 legacy shape
/// and every deserialization, and re-verifies every blob digest inside
/// `pmcp-package` (the V6 rule). A package that is correctly content-addressed
/// but semantically malformed fails HERE — against staging, with the
/// destination untouched.
fn detect_and_unpack(layout: &OciLayout) -> Result<LoadedPackage> {
    let index = layout
        .read_index()
        .context("read index.json from the staged layout")?;
    let manifests = index.manifests();
    let descriptor = manifests
        .first()
        .ok_or_else(|| anyhow!("the staged layout's index.json declares no manifest"))?;

    let mut candidates: Vec<String> = Vec::new();
    if let Some(at) = descriptor.artifact_type() {
        candidates.push(at.to_string());
    }
    if let Ok(raw) = layout.read_blob(descriptor) {
        if let Some(at) = artifact_type_from_manifest_json(&raw) {
            candidates.push(at);
        }
    }
    let manifest = layout
        .read_manifest(descriptor)
        .context("read the package manifest from the staged layout")?;
    if let Some(at) = manifest.artifact_type() {
        candidates.push(at.to_string());
    }
    candidates.push(manifest.config().media_type().to_string());
    for layer in manifest.layers() {
        candidates.push(layer.media_type().to_string());
    }

    let kind = candidates
        .iter()
        .find_map(|candidate| detect_kind(candidate))
        .ok_or_else(|| {
            anyhow!(
                "unknown package kind: candidates=[{}]",
                candidates.join(", ")
            )
        })?;

    let loaded = match kind {
        PackageKind::Agent => LoadedPackage::Agent(Box::new(
            unpack_agent(layout).context("unpack agent package")?,
        )),
        PackageKind::Team => LoadedPackage::Team(Box::new(
            unpack_team(layout).context("unpack team package")?,
        )),
        PackageKind::Server => LoadedPackage::Server(Box::new(
            unpack_server(layout).context("unpack server package")?,
        )),
        PackageKind::Workflow => LoadedPackage::Workflow(Box::new(
            unpack_workflow(layout).context("unpack workflow manifest")?,
        )),
    };
    Ok(loaded)
}

/// Read a movable artifact back into a working layout directory.
pub fn execute(args: LoadArgs, global_flags: &GlobalFlags) -> Result<()> {
    let tar_bytes = std::fs::read(&args.input)
        .with_context(|| format!("read the artifact {}", args.input.display()))?;

    let verified = artifact::read_verified(&tar_bytes)
        .with_context(|| format!("verify the artifact {}", args.input.display()))?;

    let installed =
        artifact::install_layout(&verified, &args.output, args.force, detect_and_unpack)
            .with_context(|| format!("install the package at {}", args.output.display()))?;

    if global_flags.should_output() {
        println!(
            "\n{} {} {}@{} {} {}",
            "Loaded".bright_green().bold(),
            installed.unpacked.kind().label().bright_green().bold(),
            installed.unpacked.name(),
            installed.unpacked.version(),
            "->".bright_black(),
            installed.layout.root().display()
        );
        // The package's IDENTITY, derived locally over the manifest blob's own
        // bytes — never read out of the archive. Printing it is what lets an
        // operator confirm that the thing they just loaded is the thing they
        // were told to expect, without a second command.
        println!(
            "  {} {}",
            "digest:".bright_black(),
            verified.manifest_digest
        );
    }
    Ok(())
}
