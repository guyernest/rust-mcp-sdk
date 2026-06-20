//! The OFFLINE artifact-emission layer — write the served bundle.
//!
//! This module is BUILD-TIME ONLY: it lives in `pmcp-workbook-compiler` (the
//! umya-quarantined offline crate) and is NEVER linked into the served binary. It
//! assembles ONE versioned bundle directory `{bundle_id}@{version}/` holding the
//! SEVEN members the Phase 92 served loader requires (and ONLY those — the loader
//! fails closed on any extra member):
//!
//! - `executable.ir.json` — the typed IR ([`executable`]); re-parses to an equal IR.
//! - `manifest.json` — the (tier-ratified) semantic manifest.
//! - `cell_map.json` — the manifest-driven I/O map ([`cell_map`]).
//! - `layout.json` — the FULL captured workbook layout ([`layout`]).
//! - `evidence/changelog.json` — the recorded prev→current [`VersionChangelog`].
//! - `evidence/parser_equivalence.json` — the D-08 drift-gate record ([`evidence`]).
//! - `BUNDLE.lock` — per-artifact + combined SHA-256 hash-of-hashes + the
//!   `workbook_hash` provenance anchor ([`bundle_lock`]); emits `bundle_id` (D-17).
//!
//! # Determinism + the shared hash (the two security properties)
//!
//! Every member is written through the deterministic [`serialize`] choke point
//! (stable key order + pretty + no-trailing-newline, matching the Phase 92
//! golden), so two emits of the same content are byte-identical (threat
//! T-93-05-NONDET). The `BUNDLE.lock` combined hash and the evidence-dir hash are
//! computed via the runtime's OWN `build_bundle_lock` / `fold_evidence_hash`
//! ([`bundle_lock`], [`evidence`]) — NEVER hand-rolled — so the served loader
//! recomputes the SAME hashes at boot (threat T-93-05-HASH).

pub mod bundle_lock;
pub mod cell_map;
pub mod evidence;
pub mod executable;
pub mod layout;
pub mod serialize;

use std::collections::HashMap;
use std::path::Path;

use pmcp_workbook_runtime::sheet_ir::Cell;
use pmcp_workbook_runtime::{CellRole, InputTier, Manifest, Role, VersionChangelog};

pub use bundle_lock::{
    build_bundle_lock, fold_evidence_hash, sha256_hex, ArtifactHashes, BundleLock,
};
pub use cell_map::{
    build_cell_map, build_tools, comparison_from_outputs_for_tool, reconcile_tools,
    tool_name_collision_findings, CellEntry, CellMap, Comparison, ComparisonRow, OutputTable, Tool,
    ToolReconcileReport,
};
pub use evidence::{
    emit_evidence, parser_equivalence_json, read_gate_marker, write_gate_marker, EvidenceInputs,
    GateMarker, ParserEquivalence, EVIDENCE_GATE_DIGEST, EVIDENCE_GATE_MARKER,
};
pub use executable::emit_executable;
pub use layout::{build_layout_descriptor, CellLayout, LayoutDescriptor, SheetLayout};
pub use serialize::{to_bundle_json, to_bundle_json_sorted_map};

/// An artifact-emission failure: I/O, serde, tier ratification, or cell-map
/// derivation. Carries owned `String` detail (no foreign type crosses the
/// boundary).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EmitError {
    /// A bundle artifact file could not be written.
    #[error("failed to write {path}: {detail}")]
    Io {
        /// The path that failed to write.
        path: String,
        /// The underlying I/O error rendered as text.
        detail: String,
    },
    /// An artifact could not be serialized to JSON.
    #[error("failed to serialize {what}: {detail}")]
    Serde {
        /// What was being serialized (e.g. `"executable.ir.json"`).
        what: String,
        /// The serde error rendered as text.
        detail: String,
    },
    /// A `Role::Input` cell could not be tiered during emission (fail loud).
    /// Carries the offending cell key.
    #[error("cannot tier input cell {cell}: {detail}")]
    Untierable {
        /// The cell key that could not be tiered.
        cell: String,
        /// Why it could not be tiered.
        detail: String,
    },
    /// The cell map could not be built (e.g. the manifest declares no output).
    #[error("cannot build cell_map: {0}")]
    CellMap(String),
}

/// RATIFY tiers on a manifest clone before emission: every untiered
/// [`Role::Input`] is mapped to [`InputTier::Variable`] whose default is a TYPED
/// seed from the cell's declared `dtype`. Emission FAILS LOUD
/// ([`EmitError::Untierable`]) if a `Role::Input` cell cannot be tiered.
///
/// An untiered [`Role::Constant`] is left untouched (it is a STRICT constant —
/// BA-only, rejected as an input). Outputs/formulas are unaffected.
///
/// WR-01: a FROZEN-enum input (`allowed_values: Some(..)`) is ALSO left untiered —
/// it is OPTIONAL/advertised-only. Ratifying it to `Variable` would seed the
/// dtype-neutral default (e.g. `Text("")`) into the evaluator on EVERY call — an
/// OUT-OF-ENUM value that bypasses the present-only membership gate (threat
/// T-93-05-ENUM).
fn ratify_tiers(manifest: &Manifest) -> Result<Manifest, EmitError> {
    let mut ratified = manifest.clone();
    for role in &mut ratified.cells {
        if matches!(role.role, Role::Input) && role.tier.is_none() && role.allowed_values.is_none()
        {
            role.tier = Some(default_variable_tier(role)?);
        }
    }
    Ok(ratified)
}

/// Derive the default [`InputTier::Variable`] for an untiered `Role::Input` cell.
/// The default value is a type-appropriate neutral seed from the cell's declared
/// `dtype` (the real per-call value always overrides it). `Dtype` is exhaustive,
/// so every `Role::Input` is tierable — the fail-loud [`EmitError::Untierable`]
/// path exists for forward-compatibility if the dtype set ever grows.
fn default_variable_tier(role: &CellRole) -> Result<InputTier, EmitError> {
    use pmcp_workbook_runtime::{CellValue, Dtype};
    let default = match role.dtype {
        Dtype::Number => CellValue::Number(0.0),
        Dtype::Text => CellValue::Text(String::new()),
        Dtype::Bool => CellValue::Bool(false),
    };
    Ok(InputTier::Variable { default })
}

/// Emit the COMPLETE versioned bundle directory `{bundle_id}@{version}/` holding
/// the SEVEN members the Phase 92 served loader requires, then return the
/// [`BundleLock`].
///
/// Steps:
/// 0. RATIFY tiers ([`ratify_tiers`]) — untiered `Role::Input` → `Variable`
///    (WR-01: frozen-enum inputs skipped), fail loud otherwise.
/// 1. `cell_map.json` from the ratified manifest.
/// 2. `layout.json` — the FULL captured workbook layout.
/// 3. `manifest.json` — the ratified manifest.
/// 4. `executable.ir.json` ([`emit_executable`]).
/// 5. `evidence/changelog.json` + `evidence/parser_equivalence.json`, folding the
///    frozen evidence member set into the evidence hash via the runtime helper
///    (the served loader's `recompute_evidence_hash` mirrors it).
/// 6. `BUNDLE.lock` ([`build_bundle_lock`]) over the executable, the ratified
///    manifest, and the evidence hash — emitting `bundle_id` (D-17).
///
/// `bundle_id`/`version` are PARAMETERS. `workbook_hash` is the caller-supplied
/// canonical content projection (`source_workbook_hash`), recorded verbatim. The
/// `layout`'s `source_workbook_hash` MUST equal `workbook_hash` and the
/// `changelog`'s `to_version` MUST equal `version` — the served loader's
/// stamp-binding gate (threat T-92-02) cross-checks both.
///
/// # Errors
/// Returns [`EmitError`] on any serialization, I/O, tiering, or cell-map failure.
#[allow(clippy::too_many_arguments)]
pub fn emit_bundle(
    bundle_id: &str,
    version: &str,
    ir: &HashMap<String, Cell>,
    manifest: &Manifest,
    layout: &LayoutDescriptor,
    changelog: &VersionChangelog,
    parser_equivalence: &ParserEquivalence,
    workbook_hash: String,
    out_root: &Path,
) -> Result<BundleLock, EmitError> {
    // (0) Ratify tiers on a clone (fail loud on an untierable input).
    let ratified = ratify_tiers(manifest)?;

    // The single versioned directory all members are emitted into TOGETHER.
    let dir = out_root.join(format!("{bundle_id}@{version}"));
    std::fs::create_dir_all(&dir).map_err(|e| EmitError::Io {
        path: dir.display().to_string(),
        detail: e.to_string(),
    })?;

    // (1) cell_map.json — manifest-driven, through the deterministic choke point.
    let cell_map = build_cell_map(&ratified).map_err(EmitError::CellMap)?;
    let cell_map_json = to_bundle_json(&cell_map, "cell_map.json")?;
    write_file(&dir.join("cell_map.json"), &cell_map_json)?;

    // (2) layout.json — the FULL captured workbook layout.
    let layout_json = to_bundle_json(layout, "layout.json")?;
    write_file(&dir.join("layout.json"), &layout_json)?;

    // (3) manifest.json — the RATIFIED manifest.
    let manifest_json = to_bundle_json(&ratified, "manifest.json")?;
    write_file(&dir.join("manifest.json"), &manifest_json)?;

    // (4) executable.ir.json (deterministic, sorted-key pretty JSON).
    let ir_json = emit_executable(ir, &dir)?;

    // (5) evidence/ — changelog + parser_equivalence; returns the evidence-dir
    // hash folding the frozen member set (cell_map + changelog + parser_equiv +
    // layout) via the runtime helper. The served loader recomputes the SAME fold.
    let changelog_json = to_bundle_json(changelog, "evidence/changelog.json")?;
    let parser_equiv_json = parser_equivalence_json(parser_equivalence)?;
    let evidence_inputs = EvidenceInputs {
        cell_map_json: &cell_map_json,
        changelog_json: &changelog_json,
        parser_equivalence_json: &parser_equiv_json,
        layout_json: &layout_json,
    };
    let evidence_hash = emit_evidence(&evidence_inputs, &dir)?;

    // (6) BUNDLE.lock — per-artifact + combined hash-of-hashes via the runtime
    // helper, emitting bundle_id (D-17). NEVER hand-rolled.
    let lock = build_bundle_lock(
        bundle_id,
        version,
        workbook_hash,
        &ir_json,
        &manifest_json,
        &evidence_hash,
    );
    let lock_json = to_bundle_json(&lock, "BUNDLE.lock")?;
    write_file(&dir.join("BUNDLE.lock"), &lock_json)?;

    Ok(lock)
}

/// Write `contents` to `path`, mapping an I/O failure to [`EmitError::Io`].
fn write_file(path: &Path, contents: &str) -> Result<(), EmitError> {
    std::fs::write(path, contents).map_err(|e| EmitError::Io {
        path: path.display().to_string(),
        detail: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::sheet_ir::CellExpr;
    use pmcp_workbook_runtime::{CellValue, Dtype};

    fn input_role(cell: &str, dtype: Dtype, allowed: Option<Vec<String>>) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Input,
            name: Some(cell.to_string()),
            unit: None,
            meaning: None,
            dtype,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: allowed,
        }
    }

    fn output_role(cell: &str, name: &str) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Output,
            name: Some(name.to_string()),
            unit: Some("USD".to_string()),
            meaning: Some("output".to_string()),
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn manifest_of(workflow: &str, cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: workflow.to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: Some("test-approver".to_string()),
            ratified_at: Some("2026-06-12".to_string()),
            cells,
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    fn sample_ir() -> HashMap<String, Cell> {
        let mut ir = HashMap::new();
        ir.insert(
            "3_Outputs!B2".to_string(),
            Cell {
                key: "3_Outputs!B2".to_string(),
                expr: CellExpr::Literal(CellValue::Number(42.0)),
            },
        );
        ir
    }

    /// A minimal layout whose `source_workbook_hash` binds to the lock anchor.
    fn sample_layout(hash: &str) -> LayoutDescriptor {
        LayoutDescriptor {
            descriptor_version: layout::LAYOUT_DESCRIPTOR_VERSION,
            source_workbook_hash: Some(hash.to_string()),
            sheets: vec![],
        }
    }

    fn sample_changelog(version: &str) -> VersionChangelog {
        VersionChangelog {
            from_version: String::new(),
            to_version: version.to_string(),
            deltas: vec![],
            summary: format!("seed {version}"),
        }
    }

    fn sample_parser_equiv() -> ParserEquivalence {
        ParserEquivalence {
            checked_cells: 1,
            equivalent: true,
            method: "scalar-eval".to_string(),
        }
    }

    fn emit_sample(out_root: &Path) -> BundleLock {
        let manifest = manifest_of(
            "tax-calc",
            vec![
                input_role("1_Inputs!B2", Dtype::Number, None),
                output_role("3_Outputs!B2", "out_answer"),
            ],
        );
        let hash = sha256_hex(b"workbook-content");
        emit_bundle(
            "tax-calc",
            "1.0.0",
            &sample_ir(),
            &manifest,
            &sample_layout(&hash),
            &sample_changelog("1.0.0"),
            &sample_parser_equiv(),
            hash,
            out_root,
        )
        .expect("emit bundle")
    }

    #[test]
    fn emit_seven_members() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        emit_sample(dir.path());
        let bundle = dir.path().join("tax-calc@1.0.0");
        for member in [
            "executable.ir.json",
            "manifest.json",
            "cell_map.json",
            "layout.json",
            "BUNDLE.lock",
            "evidence/changelog.json",
            "evidence/parser_equivalence.json",
        ] {
            assert!(
                bundle.join(member).exists(),
                "seven-member bundle must contain {member}"
            );
        }
    }

    #[test]
    fn bundle_lock_uses_bundle_id() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        emit_sample(dir.path());
        let lock_bytes =
            std::fs::read_to_string(dir.path().join("tax-calc@1.0.0/BUNDLE.lock")).expect("read");
        assert!(
            lock_bytes.contains("\"bundle_id\""),
            "emitted BUNDLE.lock carries bundle_id (D-17): {lock_bytes}"
        );
        assert!(
            !lock_bytes.contains("\"workflow\""),
            "emitted BUNDLE.lock does NOT carry the lighthouse workflow field"
        );
    }

    #[test]
    fn bundle_lock_via_runtime_helpers() {
        // The lock's combined hash is the runtime's build_bundle_lock output over
        // the emitted bytes — recomputing it from the on-disk members reproduces it.
        let dir = tempfile::TempDir::new().expect("tempdir");
        let lock = emit_sample(dir.path());
        let bundle = dir.path().join("tax-calc@1.0.0");
        let ir_json = std::fs::read_to_string(bundle.join("executable.ir.json")).expect("ir");
        let manifest_json =
            std::fs::read_to_string(bundle.join("manifest.json")).expect("manifest");
        let recomputed = build_bundle_lock(
            &lock.bundle_id,
            &lock.version,
            lock.workbook_hash.clone(),
            &ir_json,
            &manifest_json,
            &lock.artifacts.evidence,
        );
        assert_eq!(
            recomputed.combined, lock.combined,
            "the combined hash is the runtime build_bundle_lock output (not hand-rolled)"
        );
    }

    #[test]
    fn serialize_is_deterministic() {
        // Two full emits of the same content produce byte-identical members.
        let d1 = tempfile::TempDir::new().expect("tempdir 1");
        let d2 = tempfile::TempDir::new().expect("tempdir 2");
        let l1 = emit_sample(d1.path());
        let l2 = emit_sample(d2.path());
        assert_eq!(l1, l2, "two emits produce an identical BundleLock");
        for member in [
            "executable.ir.json",
            "manifest.json",
            "cell_map.json",
            "layout.json",
            "BUNDLE.lock",
            "evidence/changelog.json",
            "evidence/parser_equivalence.json",
        ] {
            let a = std::fs::read(d1.path().join("tax-calc@1.0.0").join(member)).expect("a");
            let b = std::fs::read(d2.path().join("tax-calc@1.0.0").join(member)).expect("b");
            assert_eq!(a, b, "{member} is byte-identical across emits");
        }
    }

    /// Load the COMMITTED Phase 92 golden manifest (NOT the in-memory builder) so
    /// the WR-01 skip is verified against the real served contract.
    fn committed_golden_manifest() -> Manifest {
        // The golden lives in the toolkit crate's test fixtures; resolve it
        // relative to THIS crate (workspace sibling).
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/manifest.json"
        );
        let bytes = std::fs::read(path).expect("read committed golden manifest.json");
        serde_json::from_slice(&bytes).expect("parse golden manifest")
    }

    #[test]
    fn ratify_skips_frozen_enum_inputs() {
        // WR-01 / WBGV-07: verified against the COMMITTED Phase 92 golden manifest.
        // The golden carries a frozen-enum input (`allowed_values: Some`); the
        // emitter authored it WITH a tier. The WR-01 invariant is the SKIP: an
        // UNTIERED frozen-enum input must NEVER be auto-seeded a default-bearing
        // tier (an out-of-enum dtype default would bypass the membership gate,
        // threat T-93-05-ENUM). We take the golden's REAL frozen-enum cell (its
        // actual `allowed_values` domain), reset its tier to None to model the
        // pre-ratification candidate, and assert ratify_tiers leaves it untiered —
        // while a plain (non-enum) untiered input IS tiered to Variable.
        let mut golden = committed_golden_manifest();

        // Locate the golden's frozen-enum input (its committed allowed_values domain).
        let frozen_cell = golden
            .cells
            .iter()
            .find(|c| matches!(c.role, Role::Input) && c.allowed_values.is_some())
            .map(|c| c.cell.clone())
            .expect("the committed golden carries a frozen-enum input (WR-01 subject)");

        // Reset EVERY input's tier to None to model the candidate manifest the
        // emitter ratifies (pre-ratification state).
        for cell in &mut golden.cells {
            if matches!(cell.role, Role::Input) {
                cell.tier = None;
            }
        }

        let ratified = ratify_tiers(&golden).expect("ratify golden");

        // The frozen-enum input STAYS untiered (WR-01 skip).
        let frozen = ratified
            .cells
            .iter()
            .find(|c| c.cell == frozen_cell)
            .expect("frozen cell present");
        assert!(
            frozen.allowed_values.is_some(),
            "the located cell is the frozen-enum input"
        );
        assert!(
            frozen.tier.is_none(),
            "a frozen-enum input ({frozen_cell}) is NEVER ratified to a default-seeding tier (WR-01)"
        );

        // A plain (non-enum) input IS tiered to Variable — the complement.
        let plain_tiered = ratified.cells.iter().any(|c| {
            matches!(c.role, Role::Input)
                && c.allowed_values.is_none()
                && matches!(c.tier, Some(InputTier::Variable { .. }))
        });
        assert!(
            plain_tiered,
            "a plain untiered input is ratified to Variable (the WR-01 complement)"
        );
    }

    #[test]
    fn ratify_tiers_plain_input_to_variable() {
        // The complement: a plain untiered input IS ratified to Variable.
        let m = manifest_of(
            "tax-calc",
            vec![
                input_role("1_Inputs!B2", Dtype::Number, None),
                output_role("3_Outputs!B2", "out_answer"),
            ],
        );
        let ratified = ratify_tiers(&m).expect("ratify");
        let plain = ratified
            .cells
            .iter()
            .find(|c| c.cell == "1_Inputs!B2")
            .expect("input present");
        assert!(
            matches!(plain.tier, Some(InputTier::Variable { .. })),
            "a plain untiered input is ratified to Variable"
        );
    }

    #[test]
    fn emitted_bundle_loads_via_toolkit() {
        // Direct loader round-trip: an emitted temp bundle is loaded by
        // pmcp-server-toolkit::workbook's fail-closed loader without error.
        use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource};

        let dir = tempfile::TempDir::new().expect("tempdir");
        emit_sample(dir.path());
        let bundle_dir = dir.path().join("tax-calc@1.0.0");
        let source = LocalDirSource::new(&bundle_dir);
        let bundle = load_bundle(&source).expect("emitted bundle loads via the toolkit loader");
        // The verified bundle exposes the stamp the emitter wrote.
        assert_eq!(bundle.stamp.bundle_id, "tax-calc");
        assert_eq!(bundle.stamp.version, "1.0.0");
        assert_eq!(bundle.cell_map.inputs.len(), 1);
        let output_count: usize = bundle.cell_map.tools.iter().map(|t| t.outputs.len()).sum();
        assert_eq!(output_count, 1);
    }
}
