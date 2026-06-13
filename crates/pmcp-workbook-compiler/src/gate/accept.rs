//! `accept` — the BA approval that flips an over-tolerance numeric delta from BLOCK
//! to PASS (WBGV-05), plus the CR-02 versioned non-overwriting promote (WBGV-06).
//!
//! # accept = re-baseline the corpus golden + a fingerprint-bound ApprovalRecord
//!
//! [`accept`] does two TIED things, then persists them ATOMICALLY:
//!
//! 1. **Re-baseline** — UPDATE the case's `expected_outputs` to the candidate
//!    `computed` values (the new golden becomes the computed value) — no stale
//!    expected target is left behind a pass flag.
//! 2. **Bind** — write a fingerprint-bound [`ApprovalRecord`]
//!    ([`governed_artifact::write_approval`], atomic temp→rename) so the gate
//!    ([`super::gate`]) passes the delta ONLY because a record reproduces THIS
//!    candidate's [`super::corpus::candidate_fingerprint`] (T-93-06-INHERIT).
//!
//! # CR-02 versioned non-overwriting promote (WBGV-06)
//!
//! [`promote`] writes the candidate bundle into a NEW `{bundle_id}@{version}/` dir
//! via a staging-dir → atomic rename ([`governed_artifact::atomic_promote_dir`]),
//! refusing to overwrite an existing baseline — so a promote can NEVER destroy the
//! prior version's audit trail (T-93-06-DESTROY). An [`EmitLane`] enforces the
//! changelog `from_version` shape: a malformed lane writes ZERO bytes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use pmcp_workbook_runtime::sheet_ir::Cell;
use pmcp_workbook_runtime::{ChangeClass, LayoutDescriptor, Manifest, VersionChangelog};

use super::corpus::{ApprovalCase, ApprovalRecord, CorpusError, RegionDelta};
use super::governed_artifact::{atomic_promote_dir, write_approval};
use crate::artifact::{emit_bundle, ParserEquivalence};
use crate::BundleLock;

/// An accept / promote failure — owned `String` detail only (no foreign type at the
/// boundary).
#[derive(Debug, thiserror::Error)]
pub enum AcceptError {
    /// The approval / corpus could not be persisted.
    #[error("approval store failed: {0}")]
    Store(String),
    /// The promote emit failed.
    #[error("promote emit failed: {0}")]
    Emit(String),
    /// The emit lane was malformed (wrong `from_version` shape) — ZERO bytes written.
    #[error("malformed emit lane: {0}")]
    MalformedLane(String),
}

impl From<CorpusError> for AcceptError {
    fn from(e: CorpusError) -> Self {
        AcceptError::Store(e.to_string())
    }
}

/// `accept`: RE-BASELINE the corpus case `case` to the candidate `computed` values
/// AND write a fingerprint-bound [`ApprovalRecord`] under
/// `<out_root>/<bundle_id>/approvals/<fingerprint>.json` (atomic), returning the
/// re-baselined case + the recorded approval.
///
/// `candidate_workbook_hash` is the candidate's content anchor; `change_classes`
/// the auto-derived classes.
///
/// Steps (TIED):
/// 1. Build `region_deltas` = old (current golden) → new (candidate computed).
/// 2. RE-BASELINE: clone the case with `expected_outputs` set to the candidate
///    `computed` values — no stale target left behind a pass flag.
/// 3. BIND: write a fingerprint-bound [`ApprovalRecord`] (atomic).
///
/// # Errors
/// Returns [`AcceptError::Store`] on a persist failure.
// The arguments ARE the irreducible approval binding (case + both transition
// anchors + computed + classes + approver + effective-date). Bundling them into a
// struct would only relocate the same fields.
#[allow(clippy::too_many_arguments)]
pub fn accept(
    case: &ApprovalCase,
    out_root: &Path,
    bundle_id: &str,
    computed: &BTreeMap<String, f64>,
    candidate_workbook_hash: &str,
    prev_bundle_hash: &str,
    change_classes: Vec<ChangeClass>,
    approver: &str,
    effective_date: &str,
) -> Result<(ApprovalCase, ApprovalRecord), AcceptError> {
    // (1) region_deltas (old = current golden, new = candidate computed).
    let mut region_deltas: BTreeMap<String, RegionDelta> = BTreeMap::new();
    for (region, &old) in &case.expected_outputs {
        let new = computed.get(region).copied().unwrap_or(old);
        region_deltas.insert(region.clone(), RegionDelta { old, new });
    }

    // (2) RE-BASELINE: a fresh case whose golden IS the candidate computed value.
    let mut rebaselined = case.clone();
    for (region, delta) in &region_deltas {
        rebaselined
            .expected_outputs
            .insert(region.clone(), delta.new);
    }

    // (3) BIND: a fingerprint-bound ApprovalRecord, persisted atomically.
    let record = ApprovalRecord {
        case_id: case.case_id.clone(),
        prev_bundle_hash: prev_bundle_hash.to_string(),
        candidate_workbook_hash: candidate_workbook_hash.to_string(),
        region_deltas,
        change_classes,
        approved_by: approver.to_string(),
        approved_at: chrono::Utc::now().to_rfc3339(),
        effective_date: effective_date.to_string(),
    };
    write_approval(out_root, bundle_id, &record)?;

    Ok((rebaselined, record))
}

/// The emission lane (WBGV-06): whether this write SEEDS the first baseline (no
/// prior version, D-12) or is a GATED UPDATE over a prior version. The lane carries
/// the changelog whose `from_version` shape it enforces — a malformed lane (a
/// `GatedUpdate` whose changelog `from_version` is empty, or a `Seed` whose
/// `from_version` is NON-empty) is rejected and writes ZERO bytes (T-93-06-PARTIAL).
#[derive(Debug, Clone)]
pub enum EmitLane {
    /// The FIRST version: no prior baseline. The changelog's `from_version` MUST be
    /// empty (there is nothing to transition from).
    Seed,
    /// A GATED UPDATE over a prior version. The changelog's `from_version` MUST be
    /// the prior version (non-empty).
    GatedUpdate {
        /// The prior accepted version this update transitions FROM.
        prior_version: String,
    },
}

impl EmitLane {
    /// Validate the lane against the changelog's `from_version` shape. Returns
    /// [`AcceptError::MalformedLane`] (so the caller writes ZERO bytes) on a
    /// mismatch.
    fn validate(&self, changelog: &VersionChangelog) -> Result<(), AcceptError> {
        match self {
            EmitLane::Seed => {
                if !changelog.from_version.is_empty() {
                    return Err(AcceptError::MalformedLane(format!(
                        "Seed lane requires an empty changelog from_version, got `{}`",
                        changelog.from_version
                    )));
                }
            },
            EmitLane::GatedUpdate { prior_version } => {
                if changelog.from_version.is_empty() {
                    return Err(AcceptError::MalformedLane(
                        "GatedUpdate lane requires a non-empty changelog from_version".to_string(),
                    ));
                }
                if &changelog.from_version != prior_version {
                    return Err(AcceptError::MalformedLane(format!(
                        "GatedUpdate from_version `{}` != prior_version `{prior_version}`",
                        changelog.from_version
                    )));
                }
            },
        }
        Ok(())
    }
}

/// All inputs the CR-02 promote emits — bundled to keep [`promote`] within the
/// argument budget while threading the irreducible emit surface.
pub struct PromoteInputs<'a> {
    /// The bundle id (`{bundle_id}@{version}/` dir name).
    pub bundle_id: &'a str,
    /// The workbook-declared version (D-11; `BUNDLE.lock` version == `to_version`).
    pub version: &'a str,
    /// The compiled IR to emit.
    pub ir: &'a std::collections::HashMap<String, Cell>,
    /// The ratified manifest.
    pub manifest: &'a Manifest,
    /// The captured workbook layout.
    pub layout: &'a LayoutDescriptor,
    /// The recorded prev→current changelog (its `from_version` shape is lane-checked).
    pub changelog: &'a VersionChangelog,
    /// The parser-equivalence evidence record.
    pub parser_equivalence: &'a ParserEquivalence,
    /// The canonical workbook content hash.
    pub workbook_hash: String,
}

/// CR-02 versioned NON-OVERWRITING promote (WBGV-06): write the candidate bundle to
/// a NEW `{bundle_id}@{version}/` dir via a staging dir → atomic rename, refusing to
/// overwrite an existing baseline. The prior baseline is left BYTE-IDENTICAL
/// (T-93-06-DESTROY). The [`EmitLane`] enforces the changelog `from_version` shape
/// — a malformed lane writes ZERO bytes (T-93-06-PARTIAL).
///
/// D-11: `version` is the workbook-declared version; the emitted `BUNDLE.lock`
/// version equals it equals `changelog.to_version` (the emit cross-checks).
///
/// # Errors
/// - [`AcceptError::MalformedLane`] — the lane's `from_version` shape is wrong
///   (ZERO bytes written).
/// - [`AcceptError::Emit`] — the bundle emit failed.
/// - [`AcceptError::Store`] — the atomic promote (rename) failed, e.g. the baseline
///   already exists (CR-02 non-overwrite).
pub fn promote(
    lane: &EmitLane,
    out_root: &Path,
    inputs: &PromoteInputs<'_>,
) -> Result<(BundleLock, PathBuf), AcceptError> {
    // Lane check FIRST — a malformed lane writes ZERO bytes (no staging dir created).
    lane.validate(inputs.changelog)?;

    // D-11 stamp binding: the declared version must equal the changelog to_version.
    if inputs.version != inputs.changelog.to_version {
        return Err(AcceptError::MalformedLane(format!(
            "declared version `{}` != changelog to_version `{}`",
            inputs.version, inputs.changelog.to_version
        )));
    }

    // The final (published) bundle dir — refuse to overwrite an existing baseline.
    let final_dir = out_root.join(format!("{}@{}", inputs.bundle_id, inputs.version));
    if final_dir.exists() {
        return Err(AcceptError::Store(format!(
            "refusing to overwrite an existing baseline {} (CR-02)",
            final_dir.display()
        )));
    }

    // Emit into a STAGING root: emit_bundle writes `{bundle_id}@{version}/` under it,
    // so the staging path mirrors the final layout and the rename is a single move.
    let staging_root = out_root.join(format!(
        ".staging-{}-{}-{}",
        inputs.bundle_id,
        inputs.version,
        std::process::id()
    ));
    std::fs::create_dir_all(&staging_root).map_err(|e| AcceptError::Emit(e.to_string()))?;

    let lock = emit_bundle(
        inputs.bundle_id,
        inputs.version,
        inputs.ir,
        inputs.manifest,
        inputs.layout,
        inputs.changelog,
        inputs.parser_equivalence,
        inputs.workbook_hash.clone(),
        &staging_root,
    )
    .map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging_root);
        AcceptError::Emit(e.to_string())
    })?;

    let staged_bundle = staging_root.join(format!("{}@{}", inputs.bundle_id, inputs.version));
    atomic_promote_dir(&staged_bundle, &final_dir).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staging_root);
        AcceptError::Store(e.to_string())
    })?;
    // Clean up the (now-empty) staging root.
    let _ = std::fs::remove_dir_all(&staging_root);

    Ok((lock, final_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::layout;
    use crate::gate::corpus::candidate_fingerprint;
    use crate::gate::governed_artifact::read_approvals;
    use pmcp_workbook_runtime::manifest_model::{Dtype, Role};
    use pmcp_workbook_runtime::sheet_ir::CellExpr;
    use pmcp_workbook_runtime::{sha256_hex, CellRole, CellValue};
    use std::collections::HashMap;

    fn tmp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        let n = C.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("wbc-accept-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("mk dir");
        dir
    }

    fn sample_case() -> ApprovalCase {
        let mut eo = BTreeMap::new();
        eo.insert("3_Out!B2".to_string(), 20.0);
        ApprovalCase {
            case_id: "default".to_string(),
            input: serde_json::json!({"1_In!B2": {"Number": 10.0}}),
            expected_outputs: eo,
        }
    }

    fn computed(v: f64) -> BTreeMap<String, f64> {
        let mut m = BTreeMap::new();
        m.insert("3_Out!B2".to_string(), v);
        m
    }

    // ---- promote (CR-02) fixtures ----

    fn output_role(cell: &str) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Output,
            name: Some(cell.to_string()),
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn input_role(cell: &str) -> CellRole {
        let mut c = output_role(cell);
        c.role = Role::Input;
        c
    }

    fn sample_manifest() -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: Some("ba".to_string()),
            ratified_at: Some("2026-06-12".to_string()),
            cells: vec![input_role("1_In!B2"), output_role("3_Out!B2")],
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
            "3_Out!B2".to_string(),
            Cell {
                key: "3_Out!B2".to_string(),
                expr: CellExpr::Literal(CellValue::Number(20.0)),
            },
        );
        ir
    }

    fn sample_layout(hash: &str) -> LayoutDescriptor {
        LayoutDescriptor {
            descriptor_version: layout::LAYOUT_DESCRIPTOR_VERSION,
            source_workbook_hash: Some(hash.to_string()),
            sheets: vec![],
        }
    }

    fn changelog(from: &str, to: &str) -> VersionChangelog {
        VersionChangelog {
            from_version: from.to_string(),
            to_version: to.to_string(),
            deltas: vec![],
            summary: format!("{from} -> {to}"),
        }
    }

    fn parser_equiv() -> ParserEquivalence {
        ParserEquivalence {
            checked_cells: 1,
            equivalent: true,
            method: "scalar-eval".to_string(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn promote_inputs<'a>(
        bundle_id: &'a str,
        version: &'a str,
        ir: &'a HashMap<String, Cell>,
        manifest: &'a Manifest,
        layout: &'a LayoutDescriptor,
        changelog: &'a VersionChangelog,
        pe: &'a ParserEquivalence,
        hash: String,
    ) -> PromoteInputs<'a> {
        PromoteInputs {
            bundle_id,
            version,
            ir,
            manifest,
            layout,
            changelog,
            parser_equivalence: pe,
            workbook_hash: hash,
        }
    }

    #[test]
    fn accept_records_and_passes() {
        // accept re-baselines the golden to the candidate value AND writes a
        // fingerprint-bound approval atomically that LETS the gate pass.
        let dir = tmp_dir();
        let case = sample_case();
        let comp = computed(31.0); // a £11 move (over-TOL)

        let (rebaselined, record) = accept(
            &case,
            &dir,
            "tax-calc",
            &comp,
            "cand-A",
            "prev",
            vec![ChangeClass::FormulaLogic],
            "ba@test",
            "2026-06-12",
        )
        .expect("accept");

        // Re-baselined golden IS the candidate computed value.
        assert_eq!(rebaselined.expected_outputs.get("3_Out!B2"), Some(&31.0));
        assert_eq!(record.candidate_workbook_hash, "cand-A");
        assert_eq!(record.approved_by, "ba@test");

        // The approval was persisted atomically and the gate now passes.
        let approvals = read_approvals(&dir, "tax-calc").expect("read approvals");
        assert_eq!(approvals.len(), 1);
        let fp = candidate_fingerprint("prev", "cand-A", &record.region_deltas);
        let decision = crate::gate::gate(
            &case, // the ORIGINAL case (golden 20) + the candidate 31
            &comp,
            "cand-A",
            "prev",
            &[ChangeClass::FormulaLogic],
            &approvals,
        );
        assert!(
            matches!(decision, crate::gate::GateDecision::Pass { .. }),
            "the bound approval lets the over-TOL candidate pass: {decision:?}"
        );
        let _ = fp;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn malformed_lane_writes_nothing() {
        // A GatedUpdate lane whose changelog from_version is EMPTY is malformed → no
        // bytes written.
        let dir = tmp_dir();
        let ir = sample_ir();
        let m = sample_manifest();
        let hash = sha256_hex(b"wb");
        let layout = sample_layout(&hash);
        let cl = changelog("", "1.0.0"); // empty from_version under GatedUpdate = bad
        let pe = parser_equiv();
        let inputs = promote_inputs("tax-calc", "1.0.0", &ir, &m, &layout, &cl, &pe, hash);
        let lane = EmitLane::GatedUpdate {
            prior_version: "0.9.0".to_string(),
        };
        let err = promote(&lane, &dir, &inputs).expect_err("malformed lane must fail");
        assert!(matches!(err, AcceptError::MalformedLane(_)));
        // ZERO bytes: no bundle dir was created.
        assert!(
            !dir.join("tax-calc@1.0.0").exists(),
            "a malformed lane writes nothing"
        );
        // No staging dir leaked either.
        let leaked: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(".staging-"))
            .collect();
        assert!(
            leaked.is_empty(),
            "no staging dir leaked on a malformed lane"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn first_version_gate_noop() {
        // D-12: on the FIRST version there is no prior baseline; the Seed lane emits
        // the baseline. (The gate is a no-op established by deriving an empty
        // approval set — proven here by a successful Seed promote with no approvals.)
        let dir = tmp_dir();
        let ir = sample_ir();
        let m = sample_manifest();
        let hash = sha256_hex(b"wb-v1");
        let layout = sample_layout(&hash);
        let cl = changelog("", "1.0.0"); // Seed: empty from_version
        let pe = parser_equiv();
        let inputs = promote_inputs("tax-calc", "1.0.0", &ir, &m, &layout, &cl, &pe, hash);
        let (lock, final_dir) = promote(&EmitLane::Seed, &dir, &inputs).expect("seed promote");
        assert_eq!(lock.version, "1.0.0");
        assert!(
            final_dir.join("BUNDLE.lock").exists(),
            "the baseline is established"
        );
        // No approvals were needed for the first version.
        assert!(read_approvals(&dir, "tax-calc").expect("read").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn promote_twice_two_dirs() {
        // CR-02 / WBGV-06: promoting twice yields two distinct @version dirs; the
        // prior baseline survives BYTE-IDENTICAL; BUNDLE.lock version == to_version.
        let dir = tmp_dir();
        let ir = sample_ir();
        let m = sample_manifest();

        // v1.0.0 (Seed).
        let h1 = sha256_hex(b"wb-v1");
        let layout1 = sample_layout(&h1);
        let cl1 = changelog("", "1.0.0");
        let pe = parser_equiv();
        let in1 = promote_inputs("tax-calc", "1.0.0", &ir, &m, &layout1, &cl1, &pe, h1);
        let (lock1, dir1) = promote(&EmitLane::Seed, &dir, &in1).expect("promote v1");
        assert_eq!(lock1.version, "1.0.0");
        let v1_lock_before = std::fs::read(dir1.join("BUNDLE.lock")).expect("v1 lock");

        // v1.1.0 (GatedUpdate from 1.0.0).
        let h2 = sha256_hex(b"wb-v2");
        let layout2 = sample_layout(&h2);
        let cl2 = changelog("1.0.0", "1.1.0");
        let in2 = promote_inputs("tax-calc", "1.1.0", &ir, &m, &layout2, &cl2, &pe, h2);
        let lane2 = EmitLane::GatedUpdate {
            prior_version: "1.0.0".to_string(),
        };
        let (lock2, dir2) = promote(&lane2, &dir, &in2).expect("promote v2");
        assert_eq!(lock2.version, "1.1.0");

        // Two DISTINCT dirs.
        assert_ne!(dir1, dir2);
        assert!(dir1.exists() && dir2.exists());

        // The prior baseline survives BYTE-IDENTICAL.
        let v1_lock_after = std::fs::read(dir1.join("BUNDLE.lock")).expect("v1 lock after");
        assert_eq!(
            v1_lock_before, v1_lock_after,
            "the prior baseline is byte-identical after the second promote (CR-02)"
        );

        // BUNDLE.lock version == changelog to_version (WBGV-06 / D-11).
        assert_eq!(lock2.version, cl2.to_version);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn promote_is_atomic() {
        // A promote uses staging→rename, so a baseline is never left partially
        // written: re-promoting the SAME version is REFUSED (no partial overwrite).
        let dir = tmp_dir();
        let ir = sample_ir();
        let m = sample_manifest();
        let h = sha256_hex(b"wb-v1");
        let layout = sample_layout(&h);
        let cl = changelog("", "1.0.0");
        let pe = parser_equiv();
        let in1 = promote_inputs("tax-calc", "1.0.0", &ir, &m, &layout, &cl, &pe, h.clone());
        promote(&EmitLane::Seed, &dir, &in1).expect("first promote");

        // A second promote into the SAME version dir is refused (no partial state).
        let in2 = promote_inputs("tax-calc", "1.0.0", &ir, &m, &layout, &cl, &pe, h);
        let err = promote(&EmitLane::Seed, &dir, &in2).expect_err("re-promote refused");
        assert!(matches!(err, AcceptError::Store(_)));
        // No staging dir leaked.
        let leaked: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(".staging-"))
            .collect();
        assert!(leaked.is_empty(), "no staging dir leaked");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn block_prints_accept_command() {
        // D-10: a block decision renders the deltas, the change class, and the exact
        // copy-pasteable --accept command.
        let case = sample_case(); // golden 20
        let comp = computed(31.0); // candidate 31 (over-TOL), no approval
        let decision = crate::gate::gate(
            &case,
            &comp,
            "cand",
            "prev",
            &[ChangeClass::FormulaLogic],
            &[],
        );
        match decision {
            crate::gate::GateDecision::Blocked(block) => {
                let rendered = block.render();
                assert!(
                    rendered.contains("--accept"),
                    "renders the --accept command"
                );
                assert!(rendered.contains("--approver"), "names the approver flag");
                assert!(
                    rendered.contains("--effective-date"),
                    "names the effective-date flag"
                );
                assert!(rendered.contains("3_Out!B2"), "names the changed output");
                assert!(
                    rendered.contains("FormulaLogic"),
                    "names the change class: {rendered}"
                );
            },
            other => panic!("expected a block, got {other:?}"),
        }
    }
}
