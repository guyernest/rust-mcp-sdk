//! The ONE human-text renderer the local read verbs share.
//!
//! `package load` reaches this file today; plan 05's `package pull` reaches the
//! same file through the library mount described below. Everything either verb
//! prints about a package it has just read is produced here, so the two cannot
//! drift into two different reports.
//!
//! # This file is compiled TWICE, from one source
//!
//! Genuinely surprising, so it is stated plainly rather than left to be
//! discovered:
//!
//! - into the BIN target as `commands::package::render`, which is how
//!   `load.rs` reaches it (`use super::render`);
//! - into the LIB target as `cargo_pmcp::package_render`, which is how this
//!   module's own unit tests, every file under `cargo-pmcp/tests/`, and plan
//!   05's lib-mounted pull pipeline reach it.
//!
//! That is TWO COMPILATIONS OF ONE SOURCE, not two implementations — the same
//! shape `kind.rs` and `artifact.rs` already live in. Do NOT "fix" it by having
//! one call the other across the lib/bin boundary: to the compiler the two
//! copies' types are distinct even though the source is identical. What proves
//! the two agree is behavioural, not structural — plan 05 asserts that `pull`
//! and `load` emit byte-identical reports.
//!
//! The mount is what makes this module testable at all. `cargo-pmcp/src/lib.rs`
//! declares no `commands` module, so a module declared ONLY in
//! `commands/package/mod.rs` exists in the bin tree and nowhere else — invisible
//! to `cargo test --lib` and unreachable from any integration test. Two
//! consequences follow and both are load-bearing: this file must stay
//! dependency-light (`pmcp_package` types + `std` only — no `clap`, no
//! `GlobalFlags`, no `crate::commands::*`), and it must never name `super::`.
//!
//! # One output shape, matching `inspect`'s visual vocabulary
//!
//! `inspect.rs`'s module header states the principle this module inherits: a
//! reader should never have to learn two output shapes. So there is exactly one
//! rendering here and no second machine-readable flag — a machine-readable
//! rendering is a clean follow-on if a consumer ever asks for one, not a surface
//! shipped on speculation.
//!
//! `inspect.rs`'s own renderers are deliberately NOT moved here. `inspect` is
//! shipped and its output is a surface people already read; this module matches
//! its VISUAL grammar instead — the same two-space indent, the same fixed-width
//! `label:` column, the same section headers — so a reader moving between
//! `inspect` and `load` sees one house style.
//!
//! **Colour is deliberately absent**, and that is the one place the visual match
//! is imperfect. Every function here returns a `String` rather than printing,
//! which is what makes the determinism property testable and what lets `load`
//! and `pull` gate printing on their own quiet flag without duplicating any
//! layout logic. A `String` carrying ANSI escapes would make that determinism
//! depend on whether stdout happened to be a terminal, i.e. on the environment
//! rather than on the inputs — so the escapes are simply not emitted.

use std::fmt::Write as _;

use pmcp_package::oci::UnpackedAttestation;
use pmcp_package::reference::ComponentType;
use pmcp_package::{required_slots, ComponentRef, ConfigSlot, SlotClass, SlotType};

/// Width of the `label:` column, matching `inspect.rs`'s `field` helper so the
/// two commands line up visually.
const LABEL_WIDTH: usize = 14;

/// Maximum rendered length of an ATTACKER-CONTROLLED string.
///
/// 72 rather than a round number for a measured reason: a well-formed
/// `sha256:<64 hex>` subject is exactly 71 characters, so a legitimate claim is
/// never clipped while a hostile annotation carrying megabytes cannot flood the
/// terminal.
const UNTRUSTED_MAX: usize = 72;

/// Everything the report renders about one package, in primitive terms.
///
/// Deliberately built from `pmcp_package` types and `&str` only — never from a
/// command-layer type. `load` (bin) and `pull` (lib) each assemble one of these
/// from whatever their own unpack step produced, and hand it to
/// [`render_report`]. That is the seam that makes ONE renderer serve both verbs
/// rather than two renderers that merely look alike.
#[derive(Debug)]
pub struct PackageReport<'a> {
    /// The package kind's lowercase label (`agent`/`team`/`server`/`workflow`).
    pub kind: &'a str,
    /// The package's declared name.
    pub name: &'a str,
    /// The package's declared version.
    pub version: &'a str,
    /// The package's identity digest, derived locally over the manifest blob.
    pub digest: &'a str,
    /// Where the layout was materialized, rendered verbatim.
    pub destination: &'a str,
    /// Every config slot the package declares.
    pub slots: &'a [ConfigSlot],
    /// Every component reference the package holds. Empty for a server
    /// package, which has no `ComponentRef` field at all.
    pub components: &'a [ComponentRef],
    /// The platform-issued attestation, if the package carried one.
    pub attestation: Option<&'a UnpackedAttestation>,
}

/// Render the whole report for one loaded package.
///
/// The single entry point both verbs call, so byte-identical inputs produce
/// byte-identical output on either side by construction.
#[must_use]
pub fn render_report(report: &PackageReport<'_>) -> String {
    let mut out = String::new();
    out.push_str(&render_identity(report));
    out.push_str(&render_required_slots(report.slots));
    out.push_str(&render_component_pins(report.components));
    out.push_str(&render_carriage(report.attestation));
    out
}

/// Render the package's own identity block: kind, name, version, digest and
/// where it landed.
#[must_use]
pub fn render_identity(report: &PackageReport<'_>) -> String {
    String::new()
}

/// Render the inventory of slots a target environment must fill.
///
/// The enumerator is [`required_slots`], and that choice is not
/// interchangeable. `detect_deviation` — the neighbouring function a reader may
/// reach for — answers a DIFFERENT question: it compares one already-known
/// tested/proposed pair and returns `None` for every identity-bearing slot by
/// design, which makes it structurally incapable of ever naming a credential.
/// This rendering asks the ENUMERATION question ("what must the target
/// environment supply?"), so it must use the enumerator. Do not "improve" this
/// by switching.
///
/// # Two strings that are never the same string
///
/// A slot's `name` is the ENVIRONMENT VARIABLE the target environment sets;
/// `config_key` is the dotted CONFIG PATH the resolved value is written to.
/// They are labelled distinctly and never substituted for one another — a
/// variable name derived from a config path (`BACKEND.BASE_URL`) is one no
/// environment can portably set.
///
/// # Ordering
///
/// Exactly the order [`required_slots`] returns: by `SlotType::key()`, with a
/// stable sort that preserves the relative order of equal-keyed duplicates.
/// Nothing is re-sorted here.
#[must_use]
pub fn render_required_slots(slots: &[ConfigSlot]) -> String {
    String::new()
}

/// Render what THIS PACKAGE records about each of its component references.
///
/// # Three states, never two
///
/// - a `Range` was declared and is not resolved in this package;
/// - a `Pinned` whose `resolved_from` is `Some(range)` shows the declared range
///   alongside the resolved version and digest;
/// - a `Pinned` whose `resolved_from` is `None` was pinned directly, and the
///   declared range CANNOT BE REPORTED.
///
/// That third state is not a formatting nicety. `PinnedRef::resolved_from`'s
/// own documentation names this phase and states the obligation verbatim:
///
/// > Anything building skew reporting on this field — Phase 123's dev-to-prod
/// > import check is the named one — MUST treat `None` as "cannot report" and
/// > NEVER as "no skew". Reading an absent fact as a positive claim is precisely
/// > the failure this field exists to prevent.
///
/// # Ordering
///
/// Sorted by component NAME, with component TYPE as the tiebreak (`server <
/// agent < team`, the declaration order `ComponentType`'s `Ord` derives). The
/// sort is stable, so two references agreeing on both keys keep their relative
/// input order.
#[must_use]
pub fn render_component_pins(components: &[ComponentRef]) -> String {
    String::new()
}

/// Render what the package carries by way of an attestation — all three states.
///
/// The verdict is read from `SubjectVerdict::matches()` and never re-derived
/// here: a CLI-side digest comparison would be a second implementation, free to
/// drift from `inspect`'s.
///
/// `claimed`, `issuer` and `payload_type` are ATTACKER-CONTROLLED strings read
/// off layer annotations. They are rendered as bounded, escaped DATA and are
/// never joined onto a path or interpreted.
#[must_use]
pub fn render_carriage(attestation: Option<&UnpackedAttestation>) -> String {
    String::new()
}

/// Append a section header, matching `inspect.rs`'s `header` shape.
fn section(out: &mut String, title: &str) {
    let _ = writeln!(out, "\n{title}");
}

/// Append a `  label:        value` line, matching `inspect.rs`'s `field` shape.
fn field(out: &mut String, indent: &str, label: &str, value: &str) {
    let _ = writeln!(
        out,
        "{indent}{:<width$} {value}",
        format!("{label}:"),
        width = LABEL_WIDTH
    );
}

/// Bound and neutralize an untrusted string before it reaches a terminal.
///
/// Two things happen, and both matter. Length is clipped to [`UNTRUSTED_MAX`]
/// so a hostile annotation cannot flood the output. Control characters —
/// including ESC — are rendered as escapes rather than emitted, because an ANSI
/// sequence smuggled through an annotation could otherwise repaint the terminal
/// and forge a verdict line the renderer never wrote.
fn untrusted(s: &str) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_package::oci::SubjectVerdict;
    use pmcp_package::{ManifestDigest, PinnedRef};

    fn secret_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key")
    }

    fn endpoint_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Endpoint {
            name: "TFL_BASE_URL".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url")
    }

    fn digest_of(seed: &[u8]) -> ManifestDigest {
        ManifestDigest::from_bytes(seed)
    }

    fn range_ref() -> ComponentRef {
        ComponentRef::Range {
            name: "triage-agent".to_string(),
            range: semver::VersionReq::parse("^1.2").unwrap(),
            component_type: ComponentType::Agent,
        }
    }

    fn pinned_with_range() -> ComponentRef {
        ComponentRef::Pinned(PinnedRef {
            name: "london-tube".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("1.3.0").unwrap(),
            digest: digest_of(b"london-tube-1.3.0"),
            resolved_from: Some(semver::VersionReq::parse("^1.2").unwrap()),
        })
    }

    fn pinned_without_range() -> ComponentRef {
        ComponentRef::Pinned(PinnedRef {
            name: "support-team".to_string(),
            component_type: ComponentType::Team,
            version: semver::Version::parse("2.0.0").unwrap(),
            digest: digest_of(b"support-team-2.0.0"),
            resolved_from: None,
        })
    }

    fn attestation(claimed: String, unattested: ManifestDigest) -> UnpackedAttestation {
        UnpackedAttestation {
            bytes: b"opaque".to_vec(),
            subject: SubjectVerdict {
                claimed,
                unattested_digest: unattested,
            },
            issuer: "https://issuer.test.invalid/pmcp-run".to_string(),
            payload_type: "application/vnd.test.attestation-payload".to_string(),
        }
    }

    /// Behavior 1: one line per required slot, with the ENVIRONMENT VARIABLE
    /// and the CONFIG PATH under DIFFERENT labels — and a secret slot carrying
    /// no value, because an identity-bearing slot structurally has none.
    #[test]
    fn a_required_slot_renders_its_variable_and_config_path_under_different_labels() {
        let rendered = render_required_slots(&[secret_slot(), endpoint_slot()]);

        assert!(
            rendered.contains("Env var:"),
            "the variable name needs its own label: {rendered}"
        );
        assert!(
            rendered.contains("Config path:"),
            "the config path needs its own label: {rendered}"
        );
        assert!(rendered.contains("TFL_APP_KEY"), "{rendered}");
        assert!(
            rendered.contains("backend.auth.query_params.app_key"),
            "{rendered}"
        );

        // The two strings must not be conflated: the config path must never be
        // rendered where the variable name belongs.
        for line in rendered.lines() {
            if line.contains("Env var:") {
                assert!(
                    !line.contains('.'),
                    "a dotted config path was rendered as a variable name: {line}"
                );
            }
        }

        // The secret slot's block carries no value at all.
        let secret_block = rendered
            .split("Env var:")
            .find(|chunk| chunk.contains("TFL_APP_KEY"))
            .expect("the secret slot is rendered");
        assert!(
            !secret_block.contains("Tested value:"),
            "an identity-bearing slot must render no value: {secret_block}"
        );
    }

    /// Behavior 2: a declared range says it was declared and is unresolved.
    #[test]
    fn a_declared_range_renders_as_declared_but_unresolved() {
        let rendered = render_component_pins(&[range_ref()]);
        assert!(rendered.contains("triage-agent"), "{rendered}");
        assert!(rendered.contains("^1.2"), "{rendered}");
        assert!(
            rendered.contains("not resolved"),
            "a range must say it is unresolved: {rendered}"
        );
    }

    /// Behavior 3: a pin that recorded its range shows BOTH the declared range
    /// and the resolved version plus digest.
    #[test]
    fn a_pin_that_recorded_its_range_renders_the_range_and_the_resolution() {
        let rendered = render_component_pins(&[pinned_with_range()]);
        assert!(rendered.contains("london-tube"), "{rendered}");
        assert!(rendered.contains("^1.2"), "the declared range: {rendered}");
        assert!(rendered.contains("1.3.0"), "the resolved version: {rendered}");
        assert!(
            rendered.contains(digest_of(b"london-tube-1.3.0").as_str()),
            "the resolved digest: {rendered}"
        );
    }

    /// Behavior 4 — the one this whole module exists to get right. An absent
    /// `resolved_from` reads as CANNOT REPORT, and NEVER as an assertion that
    /// the declared range and the pin agree.
    #[test]
    fn a_pin_without_a_recorded_range_renders_cannot_report_and_never_no_skew() {
        let rendered = render_component_pins(&[pinned_without_range()]);
        assert!(rendered.contains("support-team"), "{rendered}");
        assert!(
            rendered.to_lowercase().contains("cannot report"),
            "an absent resolved_from must read as 'cannot report': {rendered}"
        );

        let lowered = rendered.to_lowercase();
        for forbidden in [
            "no skew",
            "no drift",
            "matches the declared range",
            "in range",
            "satisfies the declared",
            "agrees with",
            "up to date",
        ] {
            assert!(
                !lowered.contains(forbidden),
                "an absent fact was rendered as a positive claim ({forbidden}): {rendered}"
            );
        }
    }

    /// Behavior 5: no attestation says so explicitly, so "unattested" is never
    /// indistinguishable from "this build does not know about attestations".
    #[test]
    fn no_attestation_renders_as_unattested() {
        let rendered = render_carriage(None);
        assert!(
            rendered.contains("unattested"),
            "an unattested package must say so: {rendered}"
        );
        assert!(
            !rendered.contains("sha256:"),
            "nothing claims a subject, so no subject may be printed: {rendered}"
        );
    }

    /// Behavior 6: a matching subject renders issuer, payload type and a
    /// verdict saying the subject matches.
    #[test]
    fn a_matching_attestation_renders_issuer_payload_type_and_a_matching_verdict() {
        let real = digest_of(b"the-real-package");
        let carried = attestation(real.as_str().to_string(), real.clone());
        let rendered = render_carriage(Some(&carried));

        assert!(
            rendered.contains("https://issuer.test.invalid/pmcp-run"),
            "{rendered}"
        );
        assert!(
            rendered.contains("application/vnd.test.attestation-payload"),
            "{rendered}"
        );
        assert!(rendered.contains(real.as_str()), "{rendered}");
        assert!(
            rendered.contains("subject matches this package"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("MISMATCH"),
            "a matching subject must not render a mismatch: {rendered}"
        );
    }

    /// Behavior 7: a mismatched subject renders issuer, the CLAIM and the
    /// ACTUAL re-derived digest side by side — that juxtaposition IS the
    /// diagnostic.
    #[test]
    fn a_mismatched_attestation_renders_issuer_claimed_and_actual_side_by_side() {
        let real = digest_of(b"the-real-package");
        let claimed = digest_of(b"an entirely different package")
            .as_str()
            .to_string();
        let carried = attestation(claimed.clone(), real.clone());
        let rendered = render_carriage(Some(&carried));

        assert!(
            rendered.contains("https://issuer.test.invalid/pmcp-run"),
            "{rendered}"
        );
        assert!(rendered.contains(&claimed), "the claim: {rendered}");
        assert!(rendered.contains(real.as_str()), "the actual: {rendered}");
        assert!(rendered.contains("SUBJECT MISMATCH"), "{rendered}");
    }

    /// Behavior 8: rendering identical inputs twice produces identical strings.
    /// This is what makes the report diffable and what a hash-map iteration
    /// order would silently break.
    #[test]
    fn rendering_identical_inputs_twice_produces_identical_strings() {
        let slots = vec![endpoint_slot(), secret_slot()];
        let components = vec![pinned_without_range(), range_ref(), pinned_with_range()];
        let real = digest_of(b"the-real-package");
        let carried = attestation(real.as_str().to_string(), real);

        let build = || {
            render_report(&PackageReport {
                kind: "team",
                name: "support-team",
                version: "2.0.0",
                digest: "sha256:abc",
                destination: "/tmp/layout",
                slots: &slots,
                components: &components,
                attestation: Some(&carried),
            })
        };

        assert_eq!(build(), build(), "the report must be a function of its inputs");
    }
}
