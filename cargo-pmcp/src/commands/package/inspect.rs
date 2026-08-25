//! `cargo pmcp package inspect <path>` — inspect a local `.pmcp` package offline.
//!
//! Opens a local OCI image-layout `.pmcp` package, rejects a zero/multiple-
//! manifest index, resolves the package kind by running the pure
//! [`kind::detect_kind`] leaf over BOTH the manifest `artifactType` AND the
//! config/layer media types (Consensus concern #3), unpacks the typed manifest
//! via `pmcp-package`'s own API (D-04 — fully offline, no network), and renders
//! the kind + key fields. Digest verification lives inside `unpack_*`; failures
//! surface verbatim (V6), never bypassed.
//!
//! Named `inspect` (not `show`): this is a LOCAL, offline operation. The verb
//! `show` is reserved for the platform's remote manifest-fetch thin client.
//!
//! # What this reports about attestations, and what it does NOT verify
//!
//! For a server package this renders whether the package carries an
//! attestation, and when it does, the issuer, the subject digest it CLAIMS,
//! and the payload's media type. Those are read from the attestation layer's
//! descriptor annotations; the payload bytes themselves are never parsed.
//!
//! Stated plainly, because this phrase must be honest wherever it appears:
//! the only verification performed locally is the subject-digest comparison —
//! does the digest this attestation names actually correspond to this
//! package. The SDK holds NO signing or verification keys and checks no
//! signature. Verifying an attestation against the issuing platform's
//! identity is a REMOTE call, and this command deliberately does not make it.
//! An attestation rendered here is a claim that has been carried, not a claim
//! that has been proven.
//!
//! # A subject mismatch is NOT a digest-verification failure
//!
//! The V6 rule above — digest verification lives inside `unpack_*` and its
//! failures surface verbatim, never bypassed — still holds exactly as written,
//! and the subject check DEPARTS from it deliberately rather than by
//! oversight:
//!
//! **Integrity failure means the bytes are corrupt; subject mismatch means the
//! bytes are fine but the claim is wrong.**
//!
//! A corrupt blob fails INSIDE `unpack_server` and no value comes back at all,
//! because nothing about corrupt bytes is worth rendering. A subject mismatch
//! is the opposite: every blob verified, and the claim written over them is
//! false. So it surfaces as a rendered verdict — issuer, claimed subject and
//! actual re-derived digest, side by side — followed by a non-zero exit, which
//! makes it gateable in CI without parsing stdout (D-06). The exit is emitted
//! outside the quiet-mode gate, so suppressing output cannot suppress the
//! verdict.
//!
//! **These two behaviours must NOT be harmonized in a later cleanup.** The
//! difference is the decision (D-03), and the same instruction is recorded at
//! the other site it lives, `pmcp_package::oci::SubjectVerdict`.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp_package::oci::{
    unpack_agent, unpack_server, unpack_team, unpack_workflow, OciLayout, UnpackedServer,
    UnpackedTeam,
};
use pmcp_package::{AgentPackage, WorkflowManifest};

use super::kind::{artifact_type_from_manifest_json, detect_kind, PackageKind};
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package inspect`.
#[derive(Debug, Args)]
pub struct InspectArgs {
    /// Path to the AI-Package (OCI image-layout directory) to inspect.
    pub path: PathBuf,
}

/// Inspect an AI-Package manifest, fully offline.
pub fn execute(args: InspectArgs, global_flags: &GlobalFlags) -> Result<()> {
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
    // Run the pure, never-panic untrusted-parse leaf over the RAW manifest bytes
    // (the exact boundary plan 110-06 fuzzes) as one candidate source.
    if let Ok(raw) = layout.read_blob(descriptor) {
        if let Some(at) = artifact_type_from_manifest_json(&raw) {
            candidates.push(at);
        }
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
        .ok_or_else(|| {
            anyhow!(
                "unknown package kind: candidates=[{}]",
                candidates.join(", ")
            )
        })?;

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
            let unpacked = unpack_team(layout).context("unpack team package")?;
            if output {
                render_team(&unpacked);
            }
        },
        PackageKind::Server => {
            let unpacked = unpack_server(layout).context("unpack server package")?;
            if output {
                render_server(&unpacked);
            }
            // DELIBERATELY outside the `if output` block, and deliberately
            // AFTER the rendering. Outside, so the non-zero exit holds under
            // `--quiet` too — a mismatch that went silent when output was
            // suppressed would be a gate hole in exactly the automated context
            // that needs the check most, and it matches this function's
            // standing rule that only the decorative rendering is gated.
            // After, so a human reading the terminal sees the full diagnostic
            // before the command fails.
            refuse_a_subject_that_does_not_name_this_package(&unpacked)?;
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

/// Turn a subject mismatch into a non-zero exit, so the verdict is gateable in
/// CI by exit code alone — no stdout parsing (D-06).
///
/// This is NOT a digest-verification failure and must not be confused with one:
/// every blob in a mismatched package verifies. See this module's header for
/// the distinction, and `pmcp_package::oci::SubjectVerdict` for the rule that
/// the two behaviours must stay different.
fn refuse_a_subject_that_does_not_name_this_package(unpacked: &UnpackedServer) -> Result<()> {
    let Some(attestation) = unpacked.attestation.as_ref() else {
        // No attestation, no claim, nothing to be wrong about.
        return Ok(());
    };
    if attestation.subject.matches() {
        return Ok(());
    }
    bail!(
        "attestation subject mismatch: the attestation names {}, but this package's unattested \
         manifest digest is {}",
        attestation.subject.claimed,
        attestation.subject.unattested_digest
    );
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

fn render_team(unpacked: &UnpackedTeam) {
    let pkg = &unpacked.package;
    header(PackageKind::Team);
    field("Name", &pkg.name);
    field("Version", &pkg.version);
    field("Members", pkg.members.len());
    field("Human roles", pkg.human_roles.len());
    field("Built-in", pkg.built_in_servers.len());
}

fn render_server(unpacked: &UnpackedServer) {
    let pkg = &unpacked.package;
    header(PackageKind::Server);
    field("Name", &pkg.name);
    field("Version", &pkg.version);
    field("Config slots", pkg.config_slots.len());
    render_attestation(unpacked);
}

/// Render what the package carries by way of an attestation.
///
/// ALL THREE states are rendered explicitly (D-06): attested with a matching
/// subject, attested but with a subject that does not name this package, and
/// unattested. An unattested package says so on its own line rather than
/// rendering nothing, so "unattested" is never indistinguishable from "this
/// build of `inspect` does not know about attestations".
///
/// The subject is printed as the attestation's own CLAIM, with the verdict on
/// its own line beneath. On a mismatch the actual re-derived digest is printed
/// too, so the claim and the reality are visible side by side — that IS the
/// diagnostic. The exit-code decision lives in
/// [`refuse_a_subject_that_does_not_name_this_package`], not here, so that
/// rendering can be skipped under `--quiet` while the exit code cannot.
fn render_attestation(unpacked: &UnpackedServer) {
    let Some(attestation) = unpacked.attestation.as_ref() else {
        field("Attestation", "none (package is unattested)");
        return;
    };
    println!("\n{}", "Attestation".bright_cyan().bold());
    field("Issuer", &attestation.issuer);
    field("Subject", &attestation.subject.claimed);
    field("Payload type", &attestation.payload_type);

    // Emphasised like `header` so the verdict reads as a verdict rather than
    // as one more data line.
    if attestation.subject.matches() {
        field(
            "Verdict",
            "subject matches this package".bright_green().bold(),
        );
    } else {
        field(
            "Verdict",
            "SUBJECT MISMATCH — this attestation is not about this package"
                .bright_red()
                .bold(),
        );
        field("Actual", &attestation.subject.unattested_digest);
    }
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
