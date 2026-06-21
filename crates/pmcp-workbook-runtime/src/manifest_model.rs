//! The logical `Manifest` model — the source of truth that REPLACES "colour as
//! canonical" (DIA-03). RELOCATED into `workbook-runtime` (Phase 11, Plan 05) so
//! the served binary can deserialize the manifest projection WITHOUT linking the
//! offline compiler. `workbook-compiler` re-exports these types (its
//! `manifest::model` surface is unchanged) and keeps manifest SYNTHESIS /
//! ratification / projections on its umya-linked side.
//!
//! Why `umya`-free: this is the type the parser / DAG compiler / artifact
//! emitters / served binary consume. No `umya` type appears in any public
//! signature here.
//!
//! # The four-variant `Role` set (Codex MEDIUM reconciliation)
//!
//! The colour palette emits an `assumption` EVIDENCE label for yellow fills, but
//! the logical model keeps the `Role` set to exactly `Input | Constant | Output
//! | Formula`. A yellow "assumption" is folded into [`Role::Constant`] with
//! `source = "yellow-assumption"`.
//!
//! # The BA-owned governed-data table (Phase 10 Plan 02, D-03)
//!
//! [`Manifest::governed_data`] is the BA-owned constant table — the SOLE route
//! by which a constant may change to close a reconciliation gap. Each
//! [`GovernedDatum`] carries a TYPED [`CellValue`] (NOT a bare `f64`), plus
//! effective-date + approval provenance.
//!
//! Derive note: because [`CellValue`] carries an `f64` (in `Number`) it is
//! `PartialEq` but NOT `Eq`. [`GovernedDatum`] therefore drops `Eq`, and
//! [`Manifest`] relaxes its derive to `PartialEq`-only.

use serde::{Deserialize, Serialize};

use crate::sheet_ir::value::CellValue;

/// The role a cell plays in the workbook's computation, resolved from the
/// MANIFEST (not from colour directly — colour only proposes; D-02). Exactly four
/// variants: a yellow "assumption" is NOT a distinct role — it is a
/// [`Role::Constant`] carrying `source = "yellow-assumption"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// A per-quote overridable input (blue input font in the lighthouse).
    Input,
    /// A governed constant (green fill in the lighthouse). A yellow "assumption"
    /// is also a `Constant`, distinguished by `source = "yellow-assumption"` on
    /// its [`CellRole`] — NEVER a separate role (Codex MEDIUM reconciliation).
    Constant,
    /// A declared output of the workflow (`out_*` named-range convention).
    Output,
    /// A derived/formula cell (default font + a formula `<f>`).
    Formula,
}

impl Role {
    /// Map a named-range NAME prefix to the role it implies (the redundant
    /// "naming convention" evidence channel used by the D-04 overlap check):
    /// `in_` → [`Role::Input`], `const_` → [`Role::Constant`], `out_` →
    /// [`Role::Output`]. Returns `None` for any other prefix (e.g. `Rooms`).
    pub fn from_name_prefix(name: &str) -> Option<Role> {
        if name.starts_with("in_") {
            Some(Role::Input)
        } else if name.starts_with("const_") {
            Some(Role::Constant)
        } else if name.starts_with("out_") {
            Some(Role::Output)
        } else {
            None
        }
    }
}

/// The declared data type of a cell's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Dtype {
    /// A numeric value.
    Number,
    /// A text value.
    Text,
    /// A boolean value.
    Bool,
}

/// One row of the manifest's roles table: a cell's resolved role + the metadata
/// (name/unit/meaning/dtype) the downstream phases consume, plus the colour
/// EVIDENCE (lint-only) and the `source` provenance.
///
/// Derive note: `Eq` is relaxed to `PartialEq`-only because the additive
/// [`CellRole::tier`] carries an [`InputTier`] whose default is a [`CellValue`]
/// (`f64`-bearing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CellRole {
    /// The fully-qualified cell key `sheet!addr` (e.g. `"1_Inputs!E6"`).
    pub cell: String,
    /// The cell's resolved role (manifest-canonical; D-03).
    pub role: Role,
    /// The named-range NAME (`in_*`/`const_*`/`out_*`), when one is assigned.
    pub name: Option<String>,
    /// The unit text (e.g. `"m2"`, `"GBP"`), when known.
    pub unit: Option<String>,
    /// The human-readable meaning (from the block header), when known.
    pub meaning: Option<String>,
    /// The declared data type.
    pub dtype: Dtype,
    /// The colour ARGB evidence that PROPOSED this role (lint-only).
    pub colour_evidence: Option<String>,
    /// The provenance of the role (e.g. `"colour+guide"`, `"yellow-assumption"`).
    pub source: String,
    /// Free-form notes.
    pub notes: Option<String>,
    /// The input TIER of this cell (D-07/D-08), additive + `#[serde(default)]` so
    /// older manifests (no `tier` key) deserialize with `tier == None`.
    ///
    /// LOAD-BEARING contract (Codex HIGH #3 — tier migration):
    /// `None` means STRICT **only for [`Role::Constant`]** — an untiered constant
    /// is BA-only and is rejected as a `calculate` input (enforced via
    /// [`is_strict_constant`]). An untiered [`Role::Input`] is **NOT** a
    /// strict-rejected input: ratification maps an untiered `Role::Input` to
    /// [`InputTier::Variable`].
    #[serde(default)]
    pub tier: Option<InputTier>,
    /// The FROZEN closed-enum domain for this input (D-03/D-07): the EXACT
    /// accepted tokens, in workbook order, trimmed + deduplicated, NEVER sorted.
    /// `Some(tokens)` means the served tool schema bakes a closed JSON-Schema
    /// `enum` for this input; `None` means the input stays DYNAMIC
    /// (allowed-values-in-error + `value-schema://` resource path).
    ///
    /// Additive + `#[serde(default)]` (the [`CellRole::tier`] precedent) so older
    /// manifests (no `allowed_values` key) deserialize with `None`;
    /// `skip_serializing_if` keeps existing `manifest.json` snapshots byte-stable
    /// when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_values: Option<Vec<String>>,
}

/// The input tier of a [`CellRole`] (D-07/D-08, RESEARCH OQ-2): whether (and how)
/// a user may override the cell at quote time.
///
/// A [`Variable`](InputTier::Variable) carries a typed [`CellValue`] default. A
/// [`BoundedVariable`](InputTier::BoundedVariable) additionally carries
/// `min`/`max` which are CARRIED but UNENFORCED in Phase 11 (D-08).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum InputTier {
    /// A freely user-overridable input with a typed default.
    Variable {
        /// The default value applied when the caller omits the input.
        default: CellValue,
    },
    /// A user-overridable input with a declared `[min, max]` range. The range is
    /// CARRIED here but NOT enforced in Phase 11 (D-08).
    BoundedVariable {
        /// The default value applied when the caller omits the input.
        default: CellValue,
        /// The lower bound (carried, unenforced in Phase 11).
        min: CellValue,
        /// The upper bound (carried, unenforced in Phase 11).
        max: CellValue,
    },
}

/// The LLM-facing JSON key for a role-bearing cell: the manifest `name` when present,
/// else the human-readable `meaning`, else the fully-qualified cell key itself.
///
/// This is the SINGLE source of the name/meaning/cell precedence used to map a
/// [`CellRole`] to the LLM-facing key — shared by the `cell_map` emitter and the
/// served tools' input/output schema builders so the precedence cannot drift.
///
/// When the key comes from `role.name`, a SINGLE leading `in_`/`out_` GOVERNANCE
/// prefix is STRIPPED from the served key (`in_gross_income` → `gross_income`): the
/// prefix is a workbook-authoring convention (the named-range marker that
/// `name_named_inputs`/`promote_named_outputs` match on) and must never leak into
/// the caller-facing tool surface. The strip applies ONLY to the `name` branch —
/// the `meaning` and `cell` fallbacks are returned verbatim. `role.name` itself is
/// NOT mutated, so governance/named-range matching still sees the prefixed name.
pub fn json_key_for_role(role: &CellRole) -> String {
    if let Some(name) = role.name.as_deref() {
        return strip_governance_prefix(name).to_string();
    }
    role.meaning.clone().unwrap_or_else(|| role.cell.clone())
}

/// Strip a SINGLE leading `in_`/`out_` governance prefix from a served `json_key`.
///
/// Only the FIRST matching prefix is removed (`in_in_x` → `in_x`), and only from a
/// `role.name`-sourced key (the caller guards that). A name that is EXACTLY the
/// prefix (`in_`) or carries no prefix is returned unchanged, so the strip is
/// idempotent on already-clean keys and never yields an empty string from a
/// non-empty prefixed-only name.
fn strip_governance_prefix(name: &str) -> &str {
    for prefix in ["in_", "out_"] {
        if let Some(rest) = name.strip_prefix(prefix) {
            if !rest.is_empty() {
                return rest;
            }
        }
    }
    name
}

/// The reserved META-tool names the served workbook binary ALWAYS registers
/// (`explain`, `get_manifest`, `diff_version`, `render_workbook`) — the SINGLE source
/// of the reserved set (H3). An output-Table tool name that sanitizes to ANY of these
/// would silently last-writer-wins over the meta tool at registration, so the offline
/// compiler REJECTS it (a cell-precise compile failure) by checking against THIS
/// const, not a hand-copied list. The served toolkit handlers' `NAME` constants
/// (`ExplainHandler::NAME` etc.) are asserted EQUAL to these entries by a binding
/// test in the toolkit, so the reserved set cannot drift from what is registered.
///
/// Lives in the runtime LEAF (not the toolkit) so the compiler reads it WITHOUT a
/// compiler→toolkit dependency (which would breach the purity boundary / `make
/// purity-check`); both the toolkit handlers and the compiler gate read the one const.
pub const RESERVED_TOOL_NAMES: [&str; 4] =
    ["explain", "get_manifest", "diff_version", "render_workbook"];

/// Sanitize a raw output-Table name into an MCP tool name matching
/// `^[a-zA-Z0-9_-]{1,64}$` (T-100-10). This is the SINGLE shared sanitizer — the
/// served toolkit's registration AND the offline compiler's post-sanitize
/// collision lint both call it, so "what we register" and "what we collision-check"
/// cannot drift. The LOCKED five-rule semantics:
///
/// 1. **Lowercase** every ASCII letter (`Calculate_Tax` → `calculate_tax`).
/// 2. **Collapse** each maximal RUN of illegal characters (anything not
///    `[a-z0-9_-]` after lowercasing) to a SINGLE `_` (`"a  b"`/`"a@@b"` →
///    `"a_b"`), never one `_` per illegal char.
/// 3. **Trim** leading/trailing `_`/`-` (no governance-noise edges).
/// 4. **Truncate** to 64 chars AFTER the above.
/// 5. If the result is **empty** (the input was empty or all-illegal) return
///    `Err` carrying the offending raw name — fail-closed.
///
/// # Errors
/// Returns `Err(raw.to_string())` when the input has no character mappable to the
/// charset (empty or all-illegal).
pub fn sanitize_tool_name(raw: &str) -> Result<String, String> {
    let mut out = String::with_capacity(raw.len());
    let mut pending_underscore = false;
    for ch in raw.chars() {
        let lc = ch.to_ascii_lowercase();
        if lc.is_ascii_alphanumeric() || lc == '_' || lc == '-' {
            if pending_underscore && !out.is_empty() {
                out.push('_');
            }
            pending_underscore = false;
            out.push(lc);
        } else {
            pending_underscore = true;
        }
    }
    let trimmed: String = out
        .trim_matches(|c| c == '_' || c == '-')
        .chars()
        .take(64)
        .collect();
    let trimmed = trimmed.trim_matches(|c| c == '_' || c == '-').to_string();
    if trimmed.is_empty() {
        return Err(raw.to_string());
    }
    Ok(trimmed)
}

/// Whether a [`CellRole`] is a STRICT constant — a BA-only governed value that
/// must be REJECTED if a caller tries to supply it as a `calculate` input
/// (Codex HIGH #3). The rule keys on [`Role::Constant`] + `tier == None`, NOT on
/// every untiered cell: an untiered [`Role::Input`] is a Variable candidate
/// (mapped at ratification), never strict-rejected.
pub fn is_strict_constant(role: &CellRole) -> bool {
    matches!(role.role, Role::Constant) && role.tier.is_none()
}

/// Whether a [`CellRole`] is COMPUTED — derived by the bundle IR
/// ([`Role::Output`] or [`Role::Formula`]) and therefore never caller-seedable
/// (WR-02: seeding a computed cell would let a caller pin a served output under
/// a valid provenance stamp). The SINGLE predicate shared by the served tools'
/// override reject gate and their allowed-override list, so "what we reject"
/// and "what we advertise as overridable" cannot drift.
pub fn is_computed(role: &CellRole) -> bool {
    matches!(role.role, Role::Output | Role::Formula)
}

/// Find the manifest [`CellRole`] whose fully-qualified `cell` key equals
/// `cell_key` — the SINGLE exact-cell-key lookup shared by the served tools'
/// schema builder, input validator, and explain trace, so the matching
/// semantics cannot drift between them. (Lookups by `name`-or-`cell` are a
/// DIFFERENT, looser predicate and stay separate.)
pub fn role_for_cell<'a>(manifest: &'a Manifest, cell_key: &str) -> Option<&'a CellRole> {
    manifest.cells.iter().find(|c| c.cell == cell_key)
}

/// One entry in the [`Manifest::changelog`] (ART-02): a version stamp recording a
/// workbook-hash transition + a human note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChangelogEntry {
    /// The manifest/workflow version this entry records.
    pub version: String,
    /// The source workbook content hash at this version.
    pub workbook_hash: String,
    /// A human-readable note describing the change.
    pub note: String,
}

/// One declared capability call (ART-02 — DECLARE-ONLY seam). Phase 11 keeps
/// capability cells OUT of scope (PROJECT.md); this only DECLARES the contract a
/// future capability cell would honour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CapabilityDecl {
    /// The cell key (`sheet!addr`) that would host the capability.
    pub cell: String,
    /// The capability kind (e.g. `"rust"`, `"remote"`, `"mcp-tool"`).
    pub kind: String,
    /// The declared contract the capability must honour (free-form for now).
    pub declared_contract: String,
}

/// The declared loop block (the `Rooms` per-room iteration). Populated ONLY from a
/// CONFIRMED `Rooms` named range (Plan 05's round-trip path; D-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LoopDecl {
    /// The loop name (e.g. `"Rooms"`).
    pub loop_name: String,
    /// The A1 range the loop iterates over.
    pub loop_range: String,
    /// The header row reference.
    pub header_row: String,
    /// The output column references.
    pub output_cols: Vec<String>,
    /// The 1-based first iteration row.
    pub start_row: u32,
    /// The 1-based last iteration row.
    pub end_row: u32,
}

/// One row of the BA-owned governed-data table (Phase 10 Plan 02, D-03): a
/// constant the BA has authorised, identified by a stable `key`, carrying a TYPED
/// [`CellValue`] (NOT a bare `f64`) + effective-date + approval provenance.
///
/// Derive note: drops `Eq` because [`CellValue`] carries an `f64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GovernedDatum {
    /// The stable key identifying the constant (e.g. a `const_*` named range or a
    /// fully-qualified `sheet!addr` cell key).
    pub key: String,
    /// The TYPED governed value (money/text/bool/empty — NOT a bare `f64`).
    pub value: CellValue,
    /// The date this governed value became effective (ISO-8601 string).
    pub effective_date: Option<String>,
    /// Who approved this governed value, when recorded (D-03).
    pub approved_by: Option<String>,
    /// Free-form provenance (e.g. a BA-doc citation) for the audit trail.
    pub provenance: Option<String>,
}

/// One declared output/cell annotation (D-18): a neutral, additive note binding
/// a human-readable `meaning` to a `target` (a cell key or output name).
///
/// Annotations are PURELY descriptive metadata the served tools may surface; they
/// carry no integrity or routing semantics. The field is additive and
/// `#[serde(default)]` on [`Manifest`], so older manifests without an
/// `annotations` key deserialize unchanged (the [`CellRole::allowed_values`]
/// additive-serde precedent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AnnotationDecl {
    /// The annotation name (a stable label).
    pub name: String,
    /// The annotation target — a cell key (`sheet!addr`) or an output name.
    pub target: String,
    /// The human-readable meaning this annotation conveys.
    pub meaning: String,
}

/// The logical manifest — the source of truth for cell roles + metadata that
/// REPLACES colour as canonical (DIA-03). Synthesis builds a CANDIDATE
/// (`ratified = false`); BA ratification (Plan 05) makes it conformant.
///
/// Derive note: `Eq` is relaxed to `PartialEq`-only because the
/// [`Manifest::governed_data`] table carries a [`CellValue`] (`f64`-bearing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Manifest {
    /// The manifest schema version.
    pub schema_version: u32,
    /// The workflow name this manifest describes.
    pub workflow: String,
    /// The source workbook content hash (when stamped; round-trip is Plan 05).
    pub workbook_hash: Option<String>,
    /// `false` for a synthesized CANDIDATE (D-06); `true` only after BA
    /// ratification (Plan 05). Roles are canonical only when ratified.
    pub ratified: bool,
    /// Who ratified the manifest (when ratified).
    pub ratified_by: Option<String>,
    /// When the manifest was ratified (ISO-8601 string; when ratified).
    pub ratified_at: Option<String>,
    /// The per-cell roles table.
    pub cells: Vec<CellRole>,
    /// The declared loop block — `None` until a confirmed `Rooms` named range is
    /// read (D-10; synthesis only hints).
    pub loop_block: Option<LoopDecl>,
    /// The BA-owned governed-data table (Phase 10 Plan 02, D-03): the SOLE route
    /// by which the reconciliation classifier may change a constant. Default
    /// empty; each entry carries a typed [`CellValue`] value + provenance.
    #[serde(default)]
    pub governed_data: Vec<GovernedDatum>,
    /// The manifest changelog (ART-02): version/workbook-hash/note entries.
    #[serde(default)]
    pub changelog: Vec<ChangelogEntry>,
    /// Declared capability calls (ART-02 — DECLARE-ONLY seam).
    #[serde(default)]
    pub capability_calls: Vec<CapabilityDecl>,
    /// Additive output/cell annotations (D-18): purely descriptive metadata the
    /// served tools may surface. `#[serde(default)]` so old manifests without the
    /// key deserialize to an empty Vec; `skip_serializing_if` keeps existing
    /// `manifest.json` snapshots byte-stable when empty (the `allowed_values`
    /// additive-serde precedent).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<AnnotationDecl>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_has_exactly_the_four_variants() {
        let all = [Role::Input, Role::Constant, Role::Output, Role::Formula];
        for r in all {
            match r {
                Role::Input | Role::Constant | Role::Output | Role::Formula => {},
            }
        }
        assert_eq!(all.len(), 4, "Role must have exactly four variants");
    }

    #[test]
    fn from_name_prefix_maps_the_three_role_prefixes() {
        assert_eq!(Role::from_name_prefix("in_total_area"), Some(Role::Input));
        assert_eq!(Role::from_name_prefix("const_margin"), Some(Role::Constant));
        assert_eq!(Role::from_name_prefix("out_first_fix"), Some(Role::Output));
        assert_eq!(Role::from_name_prefix("Rooms"), None);
        assert_eq!(Role::from_name_prefix("unprefixed"), None);
    }

    #[test]
    fn manifest_round_trips_through_serde_json() {
        let manifest = Manifest {
            schema_version: 1,
            workflow: "ufh-quote".to_string(),
            workbook_hash: Some("abc123".to_string()),
            ratified: false,
            ratified_by: None,
            ratified_at: None,
            cells: vec![
                CellRole {
                    cell: "1_Inputs!E6".to_string(),
                    role: Role::Input,
                    name: Some("in_total_area".to_string()),
                    unit: Some("m2".to_string()),
                    meaning: Some("Total floor area".to_string()),
                    dtype: Dtype::Number,
                    colour_evidence: Some("FF0000FF".to_string()),
                    source: "colour+guide".to_string(),
                    notes: None,
                    tier: None,
                    allowed_values: None,
                },
                CellRole {
                    cell: "2_Constants!B2".to_string(),
                    role: Role::Constant,
                    name: None,
                    unit: None,
                    meaning: None,
                    dtype: Dtype::Number,
                    colour_evidence: Some("FFFFFF00".to_string()),
                    source: "yellow-assumption".to_string(),
                    notes: Some("BA assumption".to_string()),
                    tier: None,
                    allowed_values: None,
                },
            ],
            loop_block: None,
            governed_data: vec![
                GovernedDatum {
                    key: "const_coil_divisor".to_string(),
                    value: CellValue::Number(100.0),
                    effective_date: Some("2026-06-06".to_string()),
                    approved_by: Some("BA".to_string()),
                    provenance: Some("design §11.1".to_string()),
                },
                GovernedDatum {
                    key: "const_pipe_family".to_string(),
                    value: CellValue::Text("16mm".to_string()),
                    effective_date: None,
                    approved_by: None,
                    provenance: None,
                },
            ],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        };

        let json = serde_json::to_string(&manifest).expect("serialize Manifest");
        let back: Manifest = serde_json::from_str(&json).expect("deserialize Manifest");
        assert_eq!(manifest, back, "Manifest must serde round-trip to equality");
    }

    #[test]
    fn governed_data_table_round_trips_a_non_numeric_typed_value() {
        let manifest = Manifest {
            schema_version: 1,
            workflow: "ufh-quote".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: Some("BA".to_string()),
            ratified_at: Some("2026-06-06".to_string()),
            cells: vec![],
            loop_block: None,
            governed_data: vec![GovernedDatum {
                key: "const_install_enabled".to_string(),
                value: CellValue::Bool(true),
                effective_date: Some("2026-06-06".to_string()),
                approved_by: Some("BA".to_string()),
                provenance: Some("BA-doc §4".to_string()),
            }],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        };
        let json = serde_json::to_string(&manifest).expect("serialize Manifest");
        let back: Manifest = serde_json::from_str(&json).expect("deserialize Manifest");
        assert_eq!(manifest, back);
        assert_eq!(back.governed_data[0].value, CellValue::Bool(true));
    }

    #[test]
    fn governed_data_defaults_to_empty_when_absent_from_json() {
        let json = r#"{
            "schema_version": 1,
            "workflow": "ufh-quote",
            "workbook_hash": null,
            "ratified": false,
            "ratified_by": null,
            "ratified_at": null,
            "cells": [],
            "loop_block": null
        }"#;
        let m: Manifest = serde_json::from_str(json).expect("deserialize without governed_data");
        assert!(m.governed_data.is_empty());
    }

    #[test]
    fn yellow_assumption_is_a_constant_with_source() {
        let cell = CellRole {
            cell: "2_Constants!B2".to_string(),
            role: Role::Constant,
            name: None,
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: Some("FFFFFF00".to_string()),
            source: "yellow-assumption".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        };
        assert_eq!(cell.role, Role::Constant);
        assert_eq!(cell.source, "yellow-assumption");
    }

    #[test]
    fn schema_for_manifest_produces_a_schema_without_panic() {
        let schema = schemars::schema_for!(Manifest);
        let json = serde_json::to_value(&schema).expect("schema serializes");
        assert_eq!(json["title"], "Manifest");
    }

    fn role_with_tier(role: Role, tier: Option<InputTier>) -> CellRole {
        CellRole {
            cell: "1_Inputs!E6".to_string(),
            role,
            name: None,
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier,
            allowed_values: None,
        }
    }

    #[test]
    fn tier_defaults_to_none_when_absent() {
        let json = r#"{
            "cell": "1_Inputs!E6",
            "role": "input",
            "name": null,
            "unit": null,
            "meaning": null,
            "dtype": "number",
            "colour_evidence": null,
            "source": "test",
            "notes": null
        }"#;
        let r: CellRole = serde_json::from_str(json).expect("deserialize without tier");
        assert_eq!(r.tier, None, "absent tier must default to None");
    }

    #[test]
    fn variable_tier_round_trips() {
        let r = role_with_tier(
            Role::Input,
            Some(InputTier::Variable {
                default: CellValue::Number(0.37),
            }),
        );
        let json = serde_json::to_string(&r).expect("serialize CellRole with Variable tier");
        let back: CellRole = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r, back, "Variable-tier CellRole must serde round-trip");
    }

    #[test]
    fn bounded_variable_carries_unenforced_range() {
        let r = role_with_tier(
            Role::Input,
            Some(InputTier::BoundedVariable {
                default: CellValue::Number(0.2),
                min: CellValue::Number(0.1),
                max: CellValue::Number(0.3),
            }),
        );
        let json = serde_json::to_string(&r).expect("serialize BoundedVariable tier");
        let back: CellRole = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            r, back,
            "BoundedVariable carries min/max through round-trip"
        );
        match back.tier {
            Some(InputTier::BoundedVariable { min, max, .. }) => {
                assert_eq!(min, CellValue::Number(0.1));
                assert_eq!(max, CellValue::Number(0.3));
            },
            other => panic!("expected BoundedVariable, got {other:?}"),
        }
    }

    #[test]
    fn allowed_values_defaults_to_none_when_absent() {
        // A manifest JSON serialized BEFORE the allowed_values field existed
        // must still deserialize (serde default → None).
        let json = r#"{
            "cell": "1_Inputs!C6",
            "role": "input",
            "name": null,
            "unit": null,
            "meaning": null,
            "dtype": "text",
            "colour_evidence": null,
            "source": "test",
            "notes": null
        }"#;
        let r: CellRole = serde_json::from_str(json).expect("deserialize without allowed_values");
        assert_eq!(
            r.allowed_values, None,
            "absent allowed_values must default to None"
        );
    }

    #[test]
    fn allowed_values_round_trips_when_some() {
        let mut r = role_with_tier(Role::Input, None);
        r.allowed_values = Some(vec!["heat_pump".to_string(), "boiler".to_string()]);
        let json = serde_json::to_string(&r).expect("serialize CellRole with allowed_values");
        let back: CellRole = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            r, back,
            "Some(allowed_values) CellRole must serde round-trip to equality"
        );
        assert_eq!(
            back.allowed_values,
            Some(vec!["heat_pump".to_string(), "boiler".to_string()]),
            "workbook order is preserved through the round-trip"
        );
    }

    #[test]
    fn allowed_values_is_skipped_from_json_when_none() {
        // skip_serializing_if keeps existing manifest.json snapshots byte-stable:
        // a None allowed_values must NOT appear as a key at all.
        let r = role_with_tier(Role::Input, None);
        let v = serde_json::to_value(&r).expect("serialize CellRole");
        assert!(
            v.get("allowed_values").is_none(),
            "None allowed_values must be skipped from serialization, got {v}"
        );
    }

    #[test]
    fn changelog_and_capability_calls_default_empty() {
        let json = r#"{
            "schema_version": 1,
            "workflow": "ufh-quote",
            "workbook_hash": null,
            "ratified": false,
            "ratified_by": null,
            "ratified_at": null,
            "cells": [],
            "loop_block": null
        }"#;
        let m: Manifest =
            serde_json::from_str(json).expect("deserialize without changelog/capability_calls");
        assert!(m.changelog.is_empty(), "absent changelog defaults empty");
        assert!(
            m.capability_calls.is_empty(),
            "absent capability_calls defaults empty"
        );
    }

    #[test]
    fn annotations_default_to_empty_when_absent_from_json() {
        // A manifest JSON serialized BEFORE the annotations field existed must
        // still deserialize (serde default → empty Vec). D-18 additive contract.
        let json = r#"{
            "schema_version": 1,
            "workflow": "tax-calc",
            "workbook_hash": null,
            "ratified": false,
            "ratified_by": null,
            "ratified_at": null,
            "cells": [],
            "loop_block": null
        }"#;
        let m: Manifest = serde_json::from_str(json).expect("deserialize without annotations");
        assert!(
            m.annotations.is_empty(),
            "absent annotations must default to an empty Vec"
        );
    }

    #[test]
    fn annotations_round_trip_to_equality_when_present() {
        let mut m: Manifest = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "workflow": "tax-calc",
                "workbook_hash": null,
                "ratified": false,
                "ratified_by": null,
                "ratified_at": null,
                "cells": [],
                "loop_block": null
            }"#,
        )
        .expect("base manifest");
        m.annotations = vec![
            AnnotationDecl {
                name: "headline".to_string(),
                target: "out_total".to_string(),
                meaning: "The total payable amount".to_string(),
            },
            AnnotationDecl {
                name: "rate".to_string(),
                target: "1_Inputs!E6".to_string(),
                meaning: "The applied tax rate".to_string(),
            },
        ];
        let json = serde_json::to_string(&m).expect("serialize Manifest with annotations");
        let back: Manifest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(m, back, "annotations must serde round-trip to equality");
    }

    #[test]
    fn empty_annotations_are_skipped_from_serialization() {
        // skip_serializing_if keeps existing manifest.json snapshots byte-stable:
        // an empty annotations Vec must NOT appear as a key at all.
        let m: Manifest = serde_json::from_str(
            r#"{
                "schema_version": 1,
                "workflow": "tax-calc",
                "workbook_hash": null,
                "ratified": false,
                "ratified_by": null,
                "ratified_at": null,
                "cells": [],
                "loop_block": null
            }"#,
        )
        .expect("base manifest");
        let v = serde_json::to_value(&m).expect("serialize Manifest");
        assert!(
            v.get("annotations").is_none(),
            "empty annotations must be skipped from serialization, got {v}"
        );
    }

    #[test]
    fn role_ontology_still_has_exactly_four() {
        let all = [Role::Input, Role::Constant, Role::Output, Role::Formula];
        for r in all {
            match r {
                Role::Input | Role::Constant | Role::Output | Role::Formula => {},
            }
        }
        assert_eq!(all.len(), 4, "Role must still have exactly four variants");
    }

    #[test]
    fn untiered_input_role_documented_not_strict() {
        let untiered_input = role_with_tier(Role::Input, None);
        let untiered_const = role_with_tier(Role::Constant, None);
        assert!(
            !is_strict_constant(&untiered_input),
            "an untiered Role::Input must NOT be treated as a strict constant"
        );
        assert!(
            is_strict_constant(&untiered_const),
            "an untiered Role::Constant IS a strict constant (fails closed)"
        );
        let tiered_const = role_with_tier(
            Role::Constant,
            Some(InputTier::Variable {
                default: CellValue::Number(1.0),
            }),
        );
        assert!(
            !is_strict_constant(&tiered_const),
            "a Constant with an explicit tier is no longer strict"
        );
    }

    // ---- F3: governance-prefix stripping on the served json_key ------------

    fn named_role(role: Role, name: &str) -> CellRole {
        let mut r = role_with_tier(role, None);
        r.name = Some(name.to_string());
        r
    }

    #[test]
    fn json_key_strips_leading_in_prefix_from_name() {
        let r = named_role(Role::Input, "in_gross_income");
        assert_eq!(
            json_key_for_role(&r),
            "gross_income",
            "the served input key must drop the in_ governance prefix"
        );
    }

    #[test]
    fn json_key_strips_leading_out_prefix_from_name() {
        let r = named_role(Role::Output, "out_tax_owed");
        assert_eq!(json_key_for_role(&r), "tax_owed");
    }

    #[test]
    fn json_key_does_not_mutate_role_name() {
        let r = named_role(Role::Input, "in_gross_income");
        let _ = json_key_for_role(&r);
        assert_eq!(
            r.name.as_deref(),
            Some("in_gross_income"),
            "role.name must stay prefixed for governance/named-range matching"
        );
    }

    #[test]
    fn json_key_strips_only_a_single_prefix() {
        // Only the FIRST governance prefix is removed.
        let r = named_role(Role::Input, "in_in_x");
        assert_eq!(json_key_for_role(&r), "in_x");
    }

    #[test]
    fn json_key_leaves_unprefixed_name_untouched() {
        let r = named_role(Role::Input, "loan_amount");
        assert_eq!(json_key_for_role(&r), "loan_amount");
        // A substring-but-not-prefix match must NOT be stripped.
        let r2 = named_role(Role::Input, "margin_in_pct");
        assert_eq!(json_key_for_role(&r2), "margin_in_pct");
    }

    #[test]
    fn json_key_does_not_strip_prefix_only_name() {
        // A name that is EXACTLY the prefix must not degenerate to "".
        let r = named_role(Role::Input, "in_");
        assert_eq!(json_key_for_role(&r), "in_");
        let r2 = named_role(Role::Output, "out_");
        assert_eq!(json_key_for_role(&r2), "out_");
    }

    #[test]
    fn json_key_strip_does_not_apply_to_meaning_or_cell_fallback() {
        // name absent → meaning verbatim (NOT stripped even if it looks prefixed).
        let mut r = role_with_tier(Role::Input, None);
        r.name = None;
        r.meaning = Some("in_some_label".to_string());
        assert_eq!(json_key_for_role(&r), "in_some_label");
        // name + meaning absent → cell key verbatim.
        let mut r2 = role_with_tier(Role::Output, None);
        r2.name = None;
        r2.meaning = None;
        assert_eq!(json_key_for_role(&r2), "1_Inputs!E6");
    }

    #[test]
    fn prop_strip_removes_at_most_one_prefix_and_is_loss_free() {
        // PROPERTY (deterministic corpus): a SINGLE strip removes AT MOST one
        // governance prefix (by design — the locked decision is "strip a single
        // leading in_/out_"), never touches a non-prefixed name, and never yields
        // an empty key from a non-empty input.
        let corpus = [
            "in_gross_income",
            "out_tax_owed",
            "in_in_x",
            "loan_amount",
            "margin_in_pct",
            "in_",
            "out_",
            "x",
            "in_a",
            "outflow", // starts with "out" but not the "out_" prefix
            "inflow",  // starts with "in" but not the "in_" prefix
        ];
        for raw in corpus {
            let once = strip_governance_prefix(raw);
            assert!(
                !once.is_empty(),
                "non-empty name {raw:?} must not strip to empty"
            );
            // A single strip removes 0 or 1 prefix: the result is either the input
            // verbatim, or exactly the input with one in_/out_ prefix removed.
            let removed_one = raw
                .strip_prefix("in_")
                .or_else(|| raw.strip_prefix("out_"))
                .map_or(false, |rest| !rest.is_empty() && once == rest);
            assert!(
                once == raw || removed_one,
                "strip removes at most one prefix for {raw:?} (got {once:?})"
            );
            if !raw.starts_with("in_") && !raw.starts_with("out_") {
                assert_eq!(once, raw, "non-prefixed {raw:?} must be returned verbatim");
            }
        }
    }

    #[test]
    fn prop_strip_is_idempotent_on_served_keys() {
        // PROPERTY: on the keys callers actually see (single-prefixed or clean),
        // stripping IS idempotent — re-stripping a served key is a no-op. This is
        // the invariant the served-key path relies on (json_key_for_role applies
        // the strip exactly once per role).
        for served in [
            "gross_income",
            "tax_owed",
            "loan_amount",
            "x",
            "in_", // prefix-only is preserved, so re-strip is a no-op
        ] {
            assert_eq!(
                strip_governance_prefix(served),
                served,
                "an already-served key {served:?} must be a strip no-op"
            );
        }
    }
}
