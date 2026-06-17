//! The shared version-changelog data model (Phase 13, Plan 01 — D-13/D-15).
//!
//! This model is the CONTRACT that crosses two trust boundaries:
//!
//! 1. **offline-compiler → served-binary (via bundle):** the promote step RECORDS
//!    a prev→current [`VersionChangelog`] into the bundle (D-15); the served
//!    `diff_version` MCP tool DESERIALIZES it and serves it.
//! 2. **`workbook-runtime` ↔ `workbook-compiler` (crate link boundary):** because
//!    the served binary links ONLY `workbook-runtime` (umya-free), every TYPE the
//!    `diff_version` tool reads from a bundle MUST be defined HERE — never in
//!    `workbook-compiler`. `just purity-check` (cargo-tree) enforces this.
//!
//! In particular [`ChangeClass`] lives HERE (review item 7): `OutputDelta`'s
//! `change_class` is the closed serde enum, NOT a stringly-typed label. An unknown
//! wire tag fails deserialization at the served boundary, so a forged class cannot
//! masquerade as a known routing class (T-13-19). This enum carries NO derivation
//! logic — Plan 03's `classify`/`policy` operate on the SAME enum but the
//! classification rules stay compiler-side.
//!
//! Owned-only fields; no foreign types cross the public boundary (the project's
//! owned-types-at-boundary idiom). All records derive
//! `Serialize + Deserialize + schemars::JsonSchema` (so `diff_version` can
//! advertise an output schema), following the exact derive style of
//! [`crate::manifest_model::ChangelogEntry`].

use serde::{Deserialize, Serialize};

/// The auto-derived class of a single output change (D-08). Six closed variants,
/// each mapping onto a manifest-model dimension; the offline classifier
/// (Plan 03's `classify`/`policy`) AND the served `diff_version` tool share this
/// ONE definition (review item 7).
///
/// Wire tags are kebab-case so they match the D-08 class labels
/// (`output-schema`, `governed-data`, …). The closed set is LOAD-BEARING: an
/// unknown tag fails deserialization at the served boundary (T-13-19), so a forged
/// class label can never masquerade as a known routing class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeClass {
    /// A named output region's declared `meaning`/`unit`/`source` (provenance)
    /// changed.
    OutputSchema,
    /// A `GovernedDatum.value` changed (`manifest.governed_data`).
    GovernedData,
    /// The compiled IR (formula AST) for a region changed.
    FormulaLogic,
    /// A `Role::Input` cell was added / removed / retyped.
    InputSchema,
    /// `manifest.capability_calls` (`CapabilityDecl`) changed.
    CapabilityContract,
    /// A yellow assumption (`Role::Constant` with `source == "yellow-assumption"`)
    /// changed.
    Assumption,
}

/// The severity of an output change (D-14). Exactly TWO variants:
///
/// - [`Drift`](Severity::Drift): a pure value change with identical schema (handled
///   by the corpus / `--accept` numeric gate).
/// - [`Redefinition`](Severity::Redefinition): a change to a named output's declared
///   `meaning`/`unit`/`source` or the IDENTITY of what it computes (BA review;
///   flagged; never silently re-baselined).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// A pure value change with identical schema.
    Drift,
    /// A schema/identity change (BA review, never silently re-baselined).
    Redefinition,
}

/// The declared metadata of a named output region at a single version: the
/// `meaning`/`unit`/`provenance` triple the redefinition predicate (D-14) compares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OutputMeta {
    /// The human-readable meaning of the output region.
    pub meaning: Option<String>,
    /// The unit text (e.g. `"GBP"`, `"m2"`), when known.
    pub unit: Option<String>,
    /// The provenance/source of the output region (e.g. `"colour+guide"`).
    pub provenance: Option<String>,
}

/// One per-changed-output record (D-13). Aligns with the cargo-pmcp schema-diff
/// compare-and-summarize shape; serde-clean so the served `diff_version` tool can
/// deserialize it. `change_class` is the [`ChangeClass`] ENUM (NOT a `String`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OutputDelta {
    /// The named output region (`sheet!addr` or named-range name).
    pub region: String,
    /// The auto-derived change class (from the shared [`ChangeClass`] enum).
    pub change_class: ChangeClass,
    /// The region metadata at the previous version.
    pub old: OutputMeta,
    /// The region metadata at the current version.
    pub new: OutputMeta,
    /// Whether the change is a drift or a redefinition.
    pub severity: Severity,
}

/// The top-level recorded changelog (D-15): the prev→current transition the promote
/// step records into the bundle and the served `diff_version` tool serves. No
/// multiple bundle versions are kept loaded at runtime — this single recorded
/// changelog is the served artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VersionChangelog {
    /// The version this changelog transitions FROM.
    pub from_version: String,
    /// The version this changelog transitions TO.
    pub to_version: String,
    /// The per-changed-output records.
    pub deltas: Vec<OutputDelta>,
    /// A human-readable summary of the transition (D-13).
    pub summary: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_changelog() -> VersionChangelog {
        VersionChangelog {
            from_version: "1.0.0".to_string(),
            to_version: "1.1.0".to_string(),
            deltas: vec![
                OutputDelta {
                    region: "7_Quote!C11".to_string(),
                    change_class: ChangeClass::GovernedData,
                    old: OutputMeta {
                        meaning: Some("supply total".to_string()),
                        unit: Some("GBP".to_string()),
                        provenance: Some("colour+guide".to_string()),
                    },
                    new: OutputMeta {
                        meaning: Some("supply total".to_string()),
                        unit: Some("GBP".to_string()),
                        provenance: Some("colour+guide".to_string()),
                    },
                    severity: Severity::Drift,
                },
                OutputDelta {
                    region: "7_Quote!C12".to_string(),
                    change_class: ChangeClass::OutputSchema,
                    old: OutputMeta {
                        meaning: Some("install total".to_string()),
                        unit: Some("GBP".to_string()),
                        provenance: None,
                    },
                    new: OutputMeta {
                        meaning: Some("install total (inc VAT)".to_string()),
                        unit: Some("GBP".to_string()),
                        provenance: Some("colour+guide".to_string()),
                    },
                    severity: Severity::Redefinition,
                },
            ],
            summary: "1 drift, 1 redefinition".to_string(),
        }
    }

    /// Locks the serde JSON round-trip (serialize → deserialize → equality),
    /// mirroring the `ir_round_trip`-style shape locking.
    #[test]
    fn version_changelog_round_trips() {
        let original = sample_changelog();
        let json = serde_json::to_string_pretty(&original).expect("serialize");
        let restored: VersionChangelog = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, restored);
    }

    /// `OutputDelta` deserializes from a hand-written JSON fixture — proving the
    /// served `diff_version` tool can read a recorded changelog member, and that
    /// `change_class` is the kebab-case enum tag (NOT a free-form string).
    #[test]
    fn output_delta_deserializes_from_fixture() {
        let fixture = r#"{
            "region": "7_Quote!C11",
            "change_class": "governed-data",
            "old": { "meaning": "supply total", "unit": "GBP", "provenance": "colour+guide" },
            "new": { "meaning": "supply total", "unit": "GBP", "provenance": "colour+guide" },
            "severity": "drift"
        }"#;
        let delta: OutputDelta = serde_json::from_str(fixture).expect("deserialize fixture");
        assert_eq!(delta.region, "7_Quote!C11");
        assert_eq!(delta.change_class, ChangeClass::GovernedData);
        assert_eq!(delta.severity, Severity::Drift);
    }

    /// `Severity` serializes to EXACTLY two kebab-case wire tags: `drift` and
    /// `redefinition`. A closed two-variant set is the D-14 contract.
    #[test]
    fn severity_has_exactly_two_variants() {
        assert_eq!(
            serde_json::to_string(&Severity::Drift).expect("serialize drift"),
            "\"drift\""
        );
        assert_eq!(
            serde_json::to_string(&Severity::Redefinition).expect("serialize redefinition"),
            "\"redefinition\""
        );
        // Round-trip both variants so the closed set is locked end-to-end.
        for variant in [Severity::Drift, Severity::Redefinition] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let restored: Severity = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, restored);
        }
    }

    /// `ChangeClass` serializes to EXACTLY six stable kebab-case wire tags
    /// (review item 7) and round-trips identically. The closed set is what makes a
    /// forged class label fail deserialization at the served boundary (T-13-19).
    #[test]
    fn change_class_has_exactly_six_variants() {
        let expected = [
            (ChangeClass::OutputSchema, "\"output-schema\""),
            (ChangeClass::GovernedData, "\"governed-data\""),
            (ChangeClass::FormulaLogic, "\"formula-logic\""),
            (ChangeClass::InputSchema, "\"input-schema\""),
            (ChangeClass::CapabilityContract, "\"capability-contract\""),
            (ChangeClass::Assumption, "\"assumption\""),
        ];
        assert_eq!(expected.len(), 6);
        for (variant, tag) in expected {
            assert_eq!(
                serde_json::to_string(&variant).expect("serialize"),
                tag,
                "wire tag for {variant:?}"
            );
            let restored: ChangeClass =
                serde_json::from_str(tag).expect("deserialize from stable tag");
            assert_eq!(variant, restored);
        }
    }

    /// An unknown `change_class` wire tag MUST fail deserialization (T-13-19): a
    /// forged class label cannot masquerade as a known routing class.
    #[test]
    fn unknown_change_class_tag_is_rejected() {
        let forged = "\"super-admin-bypass\"";
        let result: Result<ChangeClass, _> = serde_json::from_str(forged);
        assert!(result.is_err(), "unknown ChangeClass tag must be rejected");
    }
}
