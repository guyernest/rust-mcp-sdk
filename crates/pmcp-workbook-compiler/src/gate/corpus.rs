//! The AUTO-DERIVED regression corpus (D-09) + the fingerprint-bound
//! [`ApprovalRecord`] that closes the "a later unrelated change inherits an old
//! blanket approval" trust hole (WR-04 / T-93-06-INHERIT).
//!
//! # The BA authors NO cases (D-09) — the corpus is the prior version's behavior
//!
//! The lighthouse shipped a BA-curated checked-in case file. THIS crate derives the
//! case grid AUTOMATICALLY from the synthesized [`Manifest`] alone:
//!
//! - the DEFAULT case (every input at its declared tier default) — ALWAYS first;
//! - one case per ENUM member of every frozen-enum input (others held at default);
//! - NUMERIC-BOUNDARY cases per numeric input
//!   (`{default, default-step, default+step, declared min, declared max}`).
//!
//! The grid is replayed through the NAMED pure-Rust evaluation API
//! ([`super::super::sheet_ir::eval`] driving `run_executor`) for BOTH the prior
//! accepted version's IR and the candidate's IR. The PRIOR version's outputs ARE
//! the golden — captured automatically, never authored.
//!
//! # Why the approval is a fingerprint-bound RECORD, not a blanket bool (WR-04)
//!
//! An [`ApprovalRecord`] binds an approval to `{case_id, prev_bundle_hash,
//! candidate_workbook_hash, region_deltas, change_classes, approved_by,
//! approved_at, effective_date}`. The gate ([`super::gate`]) passes an
//! over-tolerance delta ONLY when a record reproduces THIS candidate's
//! [`candidate_fingerprint`]. A LATER, UNRELATED change produces a different
//! candidate hash and/or different region deltas → a different fingerprint → no
//! match → the gate blocks again. The approval can NEVER be inherited by a change
//! it did not approve (T-93-06-INHERIT).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;

use pmcp_workbook_runtime::{update_field, ChangeClass};

use crate::sheet_ir::{eval, Cell, CellEnv, CellValue, Dag};
use pmcp_workbook_runtime::{
    is_computed, manifest_model::InputTier, manifest_model::Role, CellRole, Manifest,
};

/// The grid cap (O-4 / Pitfall 6): the auto-derived corpus is BOUNDED — never a
/// combinatorial product across inputs. Generation is deterministic (default
/// FIRST, then enum members, then numeric boundaries) and truncated here.
pub const MAX_CORPUS_CASES: usize = 50;

/// The numeric-boundary STEP heuristic when the manifest declares no increment:
/// `1` unit for an integer-valued default, else `1%` of `|default|` (floored at
/// this minimum so a near-zero default still yields a distinct boundary case).
pub const MIN_FLOAT_STEP: f64 = 0.01;

/// A typed corpus error carrying owned `String` detail — no foreign type
/// (`io::Error`/`serde_json::Error`) crosses the public boundary (the crate's
/// owned-error-at-boundary idiom).
#[derive(Debug, thiserror::Error)]
pub enum CorpusError {
    /// The corpus dir/file could not be read or written.
    #[error("corpus I/O failed for {path}: {detail}")]
    Io {
        /// The path that failed.
        path: String,
        /// The underlying I/O error rendered as text.
        detail: String,
    },
    /// The corpus JSON could not be (de)serialized.
    #[error("corpus serde failed for {what}: {detail}")]
    Serde {
        /// What was being (de)serialized.
        what: String,
        /// The underlying serde error rendered as text.
        detail: String,
    },
    /// Replaying a case through the pure-Rust executor surfaced a located finding
    /// (e.g. a `dag/cycle`) — the candidate cannot be graded.
    #[error("corpus replay failed for {case_id}: {detail}")]
    Replay {
        /// The case that failed to replay.
        case_id: String,
        /// The underlying executor finding rendered as text.
        detail: String,
    },
}

/// One AUTO-DERIVED corpus case: an `input` (cell-key → seed value) and the
/// NAMED-REGION `expected_outputs` map the prior version reproduced (the golden,
/// captured automatically — NOT BA-authored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApprovalCase {
    /// The stable derived case identifier (e.g. `"default"`, `"enum:1_In!B3=married"`).
    pub case_id: String,
    /// The auto-derived case input as a JSON object `{cell_key: value}` (the driver
    /// seeds it into the executor). A `BTreeMap`-backed object for determinism.
    pub input: Value,
    /// The named output regions keyed `cell_key` → expected value
    /// (e.g. `{"3_Out!B2": 1594.93}`), captured from the PRIOR version. A
    /// `BTreeMap` for deterministic serialization.
    pub expected_outputs: BTreeMap<String, f64>,
}

/// The old → new values of a single named output region at the moment of approval
/// (the binding payload — distinguishes a re-baseline from an unrelated delta).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RegionDelta {
    /// The expected value BEFORE the change (the prior golden).
    pub old: f64,
    /// The candidate computed value the approval re-baselines TO.
    pub new: f64,
}

/// A fingerprint-bound approval (WR-04 — the trust anchor, NOT a blanket bool).
/// The gate passes an over-tolerance delta only when a record's
/// [`candidate_fingerprint`] matches THIS candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ApprovalRecord {
    /// The case this approval is bound to.
    pub case_id: String,
    /// The PREVIOUS bundle's combined hash (the transition's left anchor).
    pub prev_bundle_hash: String,
    /// The candidate workbook content hash (the transition's right anchor).
    pub candidate_workbook_hash: String,
    /// The per-region old → new deltas this approval covers (keyed `cell_key`).
    /// A `BTreeMap` so the fingerprint is order-independent and deterministic.
    pub region_deltas: BTreeMap<String, RegionDelta>,
    /// The auto-derived change classes (the shared 6-variant enum, NOT a string).
    pub change_classes: Vec<ChangeClass>,
    /// The approver identity (audit).
    pub approved_by: String,
    /// When the approval was recorded (RFC3339; audit).
    pub approved_at: String,
    /// The effective date the re-baseline takes effect (provenance).
    pub effective_date: String,
}

/// The deterministic candidate fingerprint binding an approval to the EXACT
/// candidate transition it approved: a length-prefixed sha256 over BOTH transition
/// anchors — the `prev_bundle_hash` (LEFT anchor) and the
/// `candidate_workbook_hash` (RIGHT anchor) — plus EACH `(region_id, old, new)`
/// delta in sorted-region order (WR-04).
///
/// A different `prev_bundle_hash` OR a different `candidate_workbook_hash` OR a
/// different region delta set yields a different fingerprint — so a later unrelated
/// change (even one re-promoting the SAME candidate content from a DIFFERENT
/// baseline) can NEVER reproduce a prior approval's fingerprint (T-93-06-INHERIT).
#[must_use]
pub fn candidate_fingerprint(
    prev_bundle_hash: &str,
    candidate_workbook_hash: &str,
    region_deltas: &BTreeMap<String, RegionDelta>,
) -> String {
    let mut hasher = Sha256::new();
    update_field(&mut hasher, b"prev-hash", prev_bundle_hash.as_bytes());
    update_field(
        &mut hasher,
        b"candidate-hash",
        candidate_workbook_hash.as_bytes(),
    );
    // BTreeMap iterates in sorted-key order, so the fold is order-independent.
    for (region, delta) in region_deltas {
        update_field(&mut hasher, b"region", region.as_bytes());
        update_field(
            &mut hasher,
            b"old",
            delta.old.to_bits().to_le_bytes().as_ref(),
        );
        update_field(
            &mut hasher,
            b"new",
            delta.new.to_bits().to_le_bytes().as_ref(),
        );
    }
    // Sha2 0.11 returns the digest via the Digest trait; hex-encode the bytes.
    use sha2::Digest;
    hex::encode(hasher.finalize())
}

/// Whether an [`ApprovalRecord`] exists for `case_id` whose
/// [`candidate_fingerprint`] equals `fingerprint` — the gate's approval check.
///
/// Returns `true` ONLY for a record bound to THIS candidate; an unrelated change
/// produces a different `fingerprint` and finds no match (WR-04).
#[must_use]
pub fn approval_matches(records: &[ApprovalRecord], case_id: &str, fingerprint: &str) -> bool {
    records.iter().any(|r| {
        r.case_id == case_id
            && candidate_fingerprint(
                &r.prev_bundle_hash,
                &r.candidate_workbook_hash,
                &r.region_deltas,
            ) == fingerprint
    })
}

/// The numeric-boundary STEP for an input default (D-09 documented heuristic):
/// a manifest-declared increment is not modeled in the in-repo `InputTier`, so the
/// step is `1` for an integer-valued default, else `1%` of `|default|` floored at
/// [`MIN_FLOAT_STEP`].
#[must_use]
pub fn numeric_step(default: f64) -> f64 {
    if default.fract() == 0.0 {
        1.0
    } else {
        (default.abs() * 0.01).max(MIN_FLOAT_STEP)
    }
}

/// The default value of an input cell role, if it declares a numeric/typed tier.
fn input_default(role: &CellRole) -> Option<&CellValue> {
    match role.tier.as_ref()? {
        InputTier::Variable { default } | InputTier::BoundedVariable { default, .. } => {
            Some(default)
        },
    }
}

/// The declared `[min, max]` of a bounded-variable input, if present.
fn input_bounds(role: &CellRole) -> Option<(&CellValue, &CellValue)> {
    match role.tier.as_ref()? {
        InputTier::BoundedVariable { min, max, .. } => Some((min, max)),
        InputTier::Variable { .. } => None,
    }
}

/// Extract the numeric scalar of a [`CellValue::Number`], else `None`.
fn as_number(value: &CellValue) -> Option<f64> {
    match value {
        CellValue::Number(n) if n.is_finite() => Some(*n),
        _ => None,
    }
}

/// The base (default) seed map: every overridable input at its declared default.
///
/// Frozen-enum inputs (WR-01) carry NO tier default (they are advertised-only), so
/// the base seed omits them; the enum cases below supply each member explicitly.
fn base_seed(manifest: &Manifest) -> BTreeMap<String, CellValue> {
    let mut seed = BTreeMap::new();
    for role in &manifest.cells {
        if matches!(role.role, Role::Input) {
            if let Some(default) = input_default(role) {
                seed.insert(role.cell.clone(), default.clone());
            }
        }
    }
    seed
}

/// Build the auto-derived case GRID (D-09) from the manifest alone, deterministic
/// and BOUNDED at [`MAX_CORPUS_CASES`]: the default case FIRST, then one case per
/// enum member (in workbook order, others at default), then numeric-boundary cases
/// (`{default-step, default+step, min, max}`) per numeric input.
///
/// The grid is NEVER a combinatorial product — each non-default case varies exactly
/// ONE input off the base seed.
#[must_use]
pub fn derive_case_grid(manifest: &Manifest) -> Vec<(String, BTreeMap<String, CellValue>)> {
    let base = base_seed(manifest);
    let mut grid: Vec<(String, BTreeMap<String, CellValue>)> = Vec::new();

    // (1) The default case — ALWAYS first.
    grid.push(("default".to_string(), base.clone()));

    // Inputs in workbook (manifest) order for deterministic case ordering.
    for role in &manifest.cells {
        if !matches!(role.role, Role::Input) {
            continue;
        }

        // (2) One case per ENUM member (others held at default).
        if let Some(values) = role.allowed_values.as_ref() {
            for member in values {
                let mut seed = base.clone();
                seed.insert(role.cell.clone(), CellValue::Text(member.clone()));
                grid.push((format!("enum:{}={}", role.cell, member), seed));
            }
        }

        // (3) NUMERIC-BOUNDARY cases: {default-step, default+step, min, max}.
        if let Some(default_v) = input_default(role) {
            if let Some(default) = as_number(default_v) {
                let step = numeric_step(default);
                let mut boundaries: Vec<(String, f64)> = vec![
                    (format!("num:{}=default-step", role.cell), default - step),
                    (format!("num:{}=default+step", role.cell), default + step),
                ];
                if let Some((min, max)) = input_bounds(role) {
                    if let Some(min_n) = as_number(min) {
                        boundaries.push((format!("num:{}=min", role.cell), min_n));
                    }
                    if let Some(max_n) = as_number(max) {
                        boundaries.push((format!("num:{}=max", role.cell), max_n));
                    }
                }
                for (case_id, value) in boundaries {
                    let mut seed = base.clone();
                    seed.insert(role.cell.clone(), CellValue::Number(value));
                    grid.push((case_id, seed));
                }
            }
        }
    }

    // Deterministic truncation: the default is index 0 so it always survives.
    grid.truncate(MAX_CORPUS_CASES);
    grid
}

/// The set of declared output regions (`Role::Output`) of a manifest — the named
/// outputs the corpus captures and the gate compares.
fn output_regions(manifest: &Manifest) -> Vec<String> {
    manifest
        .cells
        .iter()
        .filter(|r| matches!(r.role, Role::Output))
        .map(|r| r.cell.clone())
        .collect()
}

/// Replay ONE seed through the pure-Rust executor and read back the declared
/// output regions as `{region -> f64}`. A non-finite / non-numeric output region
/// is OMITTED from the captured map (the gate separately blocks a missing output).
fn replay_outputs(
    case_id: &str,
    ir: &std::collections::HashMap<String, Cell>,
    dag: &Dag,
    seed: &BTreeMap<String, CellValue>,
    regions: &[String],
) -> Result<BTreeMap<String, f64>, CorpusError> {
    let mut env = CellEnv::new();
    for (key, value) in seed {
        env = env.seed_cell(key.clone(), value);
    }
    let run = eval(ir, dag, &env).map_err(|e| CorpusError::Replay {
        case_id: case_id.to_string(),
        detail: format!("{e:?}"),
    })?;
    let mut outputs = BTreeMap::new();
    for region in regions {
        if let Some(CellValue::Number(n)) = run.computed.get(region) {
            if n.is_finite() {
                outputs.insert(region.clone(), *n);
            }
        }
    }
    Ok(outputs)
}

/// AUTO-DERIVE the regression corpus (D-09): build the bounded case grid from
/// `manifest`, then replay each case through the PRIOR version's IR to capture the
/// golden `expected_outputs`. The BA authors NO cases — the prior version's own
/// behavior IS the golden.
///
/// `prior_ir` / `prior_dag` are the prior accepted version's compiled IR; the
/// captured outputs become each [`ApprovalCase::expected_outputs`].
///
/// # Errors
/// Returns [`CorpusError::Replay`] if a case cannot be graded through the executor.
pub fn derive_corpus(
    manifest: &Manifest,
    prior_ir: &std::collections::HashMap<String, Cell>,
    prior_dag: &Dag,
) -> Result<Vec<ApprovalCase>, CorpusError> {
    let regions = output_regions(manifest);
    let grid = derive_case_grid(manifest);
    let mut cases = Vec::with_capacity(grid.len());
    for (case_id, seed) in grid {
        let expected_outputs = replay_outputs(&case_id, prior_ir, prior_dag, &seed, &regions)?;
        let input = seed_to_json(&seed);
        cases.push(ApprovalCase {
            case_id,
            input,
            expected_outputs,
        });
    }
    Ok(cases)
}

/// Replay a single case's seed through the CANDIDATE version's IR, returning the
/// candidate's `{region -> f64}` output map (the gate grades these against the
/// prior golden).
///
/// # Errors
/// Returns [`CorpusError::Replay`] if the candidate cannot be graded.
pub fn replay_candidate(
    case: &ApprovalCase,
    manifest: &Manifest,
    candidate_ir: &std::collections::HashMap<String, Cell>,
    candidate_dag: &Dag,
) -> Result<BTreeMap<String, f64>, CorpusError> {
    let regions = output_regions(manifest);
    let seed = json_to_seed(&case.input);
    replay_outputs(&case.case_id, candidate_ir, candidate_dag, &seed, &regions)
}

/// Lower a seed map to a deterministic JSON object (sorted keys via `BTreeMap`).
fn seed_to_json(seed: &BTreeMap<String, CellValue>) -> Value {
    let map: serde_json::Map<String, Value> = seed
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::to_value(v).unwrap_or(Value::Null)))
        .collect();
    Value::Object(map)
}

/// Lift a JSON object back to a seed map; a malformed value is skipped (the
/// executor seeds only well-formed `CellValue`s).
fn json_to_seed(input: &Value) -> BTreeMap<String, CellValue> {
    let mut seed = BTreeMap::new();
    if let Value::Object(map) = input {
        for (k, v) in map {
            if let Ok(cv) = serde_json::from_value::<CellValue>(v.clone()) {
                seed.insert(k.clone(), cv);
            }
        }
    }
    seed
}

/// Build the per-region old (expected golden) → new (candidate computed) deltas the
/// gate fingerprints and the block report renders. A region absent from `computed`
/// re-baselines to its `old` (mirrors `accept`), keeping the fingerprint stable;
/// the gate separately HARD-BLOCKS a missing output.
#[must_use]
pub fn region_deltas(
    case: &ApprovalCase,
    computed: &BTreeMap<String, f64>,
) -> BTreeMap<String, RegionDelta> {
    let mut deltas = BTreeMap::new();
    for (region, &old) in &case.expected_outputs {
        let new = computed.get(region).copied().unwrap_or(old);
        deltas.insert(region.clone(), RegionDelta { old, new });
    }
    deltas
}

/// PROPERTY guard (ALWAYS requirement): a derived corpus NEVER seeds a value
/// outside a frozen-enum input's `allowed_values` domain. Returns `true` iff every
/// enum-input seed in `cases` is a declared member (or the default, which is the
/// no-seed base for an enum input). Used by the property test.
#[must_use]
pub fn no_seeded_value_outside_allowed(manifest: &Manifest, cases: &[ApprovalCase]) -> bool {
    let enum_domains: BTreeMap<&str, &Vec<String>> = manifest
        .cells
        .iter()
        .filter(|r| matches!(r.role, Role::Input))
        .filter_map(|r| r.allowed_values.as_ref().map(|v| (r.cell.as_str(), v)))
        .collect();
    for case in cases {
        if let Value::Object(map) = &case.input {
            for (cell, value) in map {
                if let Some(domain) = enum_domains.get(cell.as_str()) {
                    if let Value::String(s) = value {
                        if !domain.contains(s) {
                            return false;
                        }
                    } else if let Ok(CellValue::Text(s)) =
                        serde_json::from_value::<CellValue>(value.clone())
                    {
                        if !domain.contains(&s) {
                            return false;
                        }
                    }
                }
            }
        }
    }
    let _ = is_computed; // re-exported predicate kept available to the gate consumer
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::manifest_model::Dtype;
    use pmcp_workbook_runtime::sheet_ir::CellExpr;
    use pmcp_workbook_runtime::{build_dag, Expr};
    use std::collections::HashMap;

    fn input_cell(cell: &str, default: f64) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Input,
            name: Some(cell.to_string()),
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: Some(InputTier::Variable {
                default: CellValue::Number(default),
            }),
            allowed_values: None,
        }
    }

    fn bounded_input(cell: &str, default: f64, min: f64, max: f64) -> CellRole {
        let mut c = input_cell(cell, default);
        c.tier = Some(InputTier::BoundedVariable {
            default: CellValue::Number(default),
            min: CellValue::Number(min),
            max: CellValue::Number(max),
        });
        c
    }

    fn enum_input(cell: &str, members: &[&str]) -> CellRole {
        let mut c = input_cell(cell, 0.0);
        c.dtype = Dtype::Text;
        c.tier = None; // frozen-enum inputs are advertised-only (WR-01)
        c.allowed_values = Some(members.iter().map(|s| (*s).to_string()).collect());
        c
    }

    fn output_cell(cell: &str) -> CellRole {
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

    fn manifest_of(cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "test".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells,
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    /// An IR that copies `in_cell` to `out_cell` (out = in * mult): exercises the
    /// pure-Rust executor on a one-hop dependency so replay produces a real output.
    fn linear_ir(in_cell: &str, out_cell: &str, mult: f64) -> HashMap<String, Cell> {
        let mut ir = HashMap::new();
        ir.insert(
            out_cell.to_string(),
            Cell {
                key: out_cell.to_string(),
                expr: CellExpr::Formula(Expr::BinaryOp {
                    op: pmcp_workbook_runtime::BinOp::Mul,
                    left: Box::new(Expr::Ref(in_cell.to_string())),
                    right: Box::new(Expr::Number(mult)),
                }),
            },
        );
        ir
    }

    fn deltas(pairs: &[(&str, f64, f64)]) -> BTreeMap<String, RegionDelta> {
        pairs
            .iter()
            .map(|(r, old, new)| {
                (
                    (*r).to_string(),
                    RegionDelta {
                        old: *old,
                        new: *new,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn corpus_auto_derives_from_manifest() {
        // The generator builds the grid from the manifest alone — no checked-in case
        // file, no BA input. A single numeric input + one output yields: default + two
        // numeric-boundary cases (default±step).
        let m = manifest_of(vec![input_cell("1_In!B2", 10.0), output_cell("3_Out!B2")]);
        let ir = linear_ir("1_In!B2", "3_Out!B2", 2.0);
        let dag = build_dag(&ir);
        let cases = derive_corpus(&m, &ir, &dag).expect("derive corpus");

        assert!(
            !cases.is_empty(),
            "corpus auto-derives at least the default case"
        );
        assert_eq!(
            cases[0].case_id, "default",
            "the default case is ALWAYS first"
        );
        // The prior version's output IS the golden: default 10 * 2 = 20.
        assert_eq!(
            cases[0].expected_outputs.get("3_Out!B2"),
            Some(&20.0),
            "the prior version's output is captured automatically as the golden"
        );
    }

    #[test]
    fn corpus_grid_is_bounded() {
        // A manifest with many inputs must NOT explode combinatorially: the grid is
        // default-first + per-enum-member + numeric-boundary, capped at 50, NEVER a
        // product across inputs.
        let mut cells: Vec<CellRole> = (0..40)
            .map(|i| input_cell(&format!("1_In!B{i}"), 5.0))
            .collect();
        cells.push(output_cell("3_Out!B2"));
        let m = manifest_of(cells);
        let grid = derive_case_grid(&m);

        // 1 default + 40 inputs * 2 boundary cases = 81 candidate cases, truncated.
        assert!(
            grid.len() <= MAX_CORPUS_CASES,
            "grid is capped at {MAX_CORPUS_CASES}"
        );
        assert_eq!(grid[0].0, "default", "deterministic order: default first");

        // Linear cardinality proof (never combinatorial): each non-default case
        // varies exactly ONE cell off the base seed.
        let base = base_seed(&m);
        for (case_id, seed) in grid.iter().skip(1) {
            let differing = seed
                .iter()
                .filter(|(k, v)| base.get(*k) != Some(*v))
                .count();
            assert!(
                differing <= 1,
                "case {case_id} varies more than one input (would be combinatorial)"
            );
        }
    }

    #[test]
    fn corpus_grid_includes_enum_members_and_numeric_boundaries() {
        let m = manifest_of(vec![
            enum_input("1_In!B3", &["single", "married"]),
            bounded_input("1_In!B4", 100.0, 0.0, 200.0),
            output_cell("3_Out!B2"),
        ]);
        let grid = derive_case_grid(&m);
        let ids: Vec<&str> = grid.iter().map(|(id, _)| id.as_str()).collect();
        assert!(ids.contains(&"enum:1_In!B3=single"));
        assert!(ids.contains(&"enum:1_In!B3=married"));
        assert!(ids.contains(&"num:1_In!B4=default-step"));
        assert!(ids.contains(&"num:1_In!B4=default+step"));
        assert!(ids.contains(&"num:1_In!B4=min"));
        assert!(ids.contains(&"num:1_In!B4=max"));
    }

    #[test]
    fn corpus_replays_via_named_evaluator() {
        // Cases replay through the named pure-Rust bundle-evaluation API (sheet_ir
        // eval → run_executor) for BOTH prior and candidate; the prior's outputs are
        // the golden, the candidate's are graded against them.
        let m = manifest_of(vec![input_cell("1_In!B2", 10.0), output_cell("3_Out!B2")]);
        let prior_ir = linear_ir("1_In!B2", "3_Out!B2", 2.0);
        let prior_dag = build_dag(&prior_ir);
        let cases = derive_corpus(&m, &prior_ir, &prior_dag).expect("derive");

        // The candidate triples instead of doubling: default 10 * 3 = 30 ≠ golden 20.
        let cand_ir = linear_ir("1_In!B2", "3_Out!B2", 3.0);
        let cand_dag = build_dag(&cand_ir);
        let default_case = cases.iter().find(|c| c.case_id == "default").unwrap();
        let candidate = replay_candidate(default_case, &m, &cand_ir, &cand_dag).expect("replay");
        assert_eq!(
            candidate.get("3_Out!B2"),
            Some(&30.0),
            "the candidate is replayed through the SAME named pure-Rust evaluator"
        );
        assert_ne!(
            candidate.get("3_Out!B2"),
            default_case.expected_outputs.get("3_Out!B2"),
            "the candidate moved off the prior golden"
        );
    }

    #[test]
    fn fingerprint_binds_content() {
        // A different candidate content yields a different fingerprint (WR-04).
        let base = deltas(&[("3_Out!B2", 20.0, 30.0)]);
        let fp_a = candidate_fingerprint("prev", "cand-A", &base);
        let fp_b = candidate_fingerprint("prev", "cand-B", &base);
        assert_ne!(
            fp_a, fp_b,
            "a different candidate hash yields a different fingerprint"
        );
    }

    #[test]
    fn approval_mismatch_on_prior_hash_change() {
        let d = deltas(&[("3_Out!B2", 20.0, 30.0)]);
        let record = ApprovalRecord {
            case_id: "default".to_string(),
            prev_bundle_hash: "prev-A".to_string(),
            candidate_workbook_hash: "cand".to_string(),
            region_deltas: d.clone(),
            change_classes: vec![ChangeClass::FormulaLogic],
            approved_by: "ba".to_string(),
            approved_at: "t".to_string(),
            effective_date: "d".to_string(),
        };
        let records = vec![record];
        let matching = candidate_fingerprint("prev-A", "cand", &d);
        assert!(approval_matches(&records, "default", &matching));
        // ONLY the prior hash changes → no match.
        let changed = candidate_fingerprint("prev-B", "cand", &d);
        assert!(!approval_matches(&records, "default", &changed));
    }

    #[test]
    fn approval_mismatch_on_candidate_hash_change() {
        let d = deltas(&[("3_Out!B2", 20.0, 30.0)]);
        let record = ApprovalRecord {
            case_id: "default".to_string(),
            prev_bundle_hash: "prev".to_string(),
            candidate_workbook_hash: "cand-A".to_string(),
            region_deltas: d.clone(),
            change_classes: vec![],
            approved_by: "ba".to_string(),
            approved_at: "t".to_string(),
            effective_date: "d".to_string(),
        };
        let records = vec![record];
        assert!(approval_matches(
            &records,
            "default",
            &candidate_fingerprint("prev", "cand-A", &d)
        ));
        // ONLY the candidate hash changes → no match.
        assert!(!approval_matches(
            &records,
            "default",
            &candidate_fingerprint("prev", "cand-B", &d)
        ));
    }

    #[test]
    fn approval_mismatch_on_region_delta_change() {
        let d = deltas(&[("3_Out!B2", 20.0, 30.0)]);
        let record = ApprovalRecord {
            case_id: "default".to_string(),
            prev_bundle_hash: "prev".to_string(),
            candidate_workbook_hash: "cand".to_string(),
            region_deltas: d.clone(),
            change_classes: vec![],
            approved_by: "ba".to_string(),
            approved_at: "t".to_string(),
            effective_date: "d".to_string(),
        };
        let records = vec![record];
        assert!(approval_matches(
            &records,
            "default",
            &candidate_fingerprint("prev", "cand", &d)
        ));
        // ONLY the region delta changes → no match.
        let changed = deltas(&[("3_Out!B2", 20.0, 31.0)]);
        assert!(!approval_matches(
            &records,
            "default",
            &candidate_fingerprint("prev", "cand", &changed)
        ));
    }

    #[test]
    fn over_tolerance_blocks() {
        // A named output that moved beyond tolerance with no covering approval must
        // surface as a block. Here the gate-level decision is modeled via the delta:
        // a 10-unit move with no matching approval record → not approved.
        let m = manifest_of(vec![input_cell("1_In!B2", 10.0), output_cell("3_Out!B2")]);
        let prior_ir = linear_ir("1_In!B2", "3_Out!B2", 2.0);
        let prior_dag = build_dag(&prior_ir);
        let cases = derive_corpus(&m, &prior_ir, &prior_dag).expect("derive");
        let default_case = cases.iter().find(|c| c.case_id == "default").unwrap();

        let cand_ir = linear_ir("1_In!B2", "3_Out!B2", 3.0);
        let cand_dag = build_dag(&cand_ir);
        let computed = replay_candidate(default_case, &m, &cand_ir, &cand_dag).expect("replay");
        let d = region_deltas(default_case, &computed);
        let delta = d.get("3_Out!B2").expect("region present");
        assert!(
            (delta.new - delta.old).abs() > 0.01,
            "moved beyond the penny tolerance"
        );

        // With no approval records, the over-tolerance move is NOT covered → block.
        let no_approvals: Vec<ApprovalRecord> = vec![];
        let fp = candidate_fingerprint("prev", "cand", &d);
        assert!(
            !approval_matches(&no_approvals, &default_case.case_id, &fp),
            "an over-tolerance delta with no covering approval blocks"
        );
    }

    #[test]
    fn no_seeded_default_outside_allowed_values_property() {
        // ALWAYS property: every enum-input seed in the derived corpus is a declared
        // member — the generator NEVER fabricates an out-of-domain value.
        let m = manifest_of(vec![
            enum_input("1_In!B3", &["single", "married", "head_of_household"]),
            output_cell("3_Out!B2"),
        ]);
        // The output IR just echoes a constant so replay never fails on text inputs.
        let mut ir = HashMap::new();
        ir.insert(
            "3_Out!B2".to_string(),
            Cell {
                key: "3_Out!B2".to_string(),
                expr: CellExpr::Literal(CellValue::Number(1.0)),
            },
        );
        let dag = build_dag(&ir);
        let cases = derive_corpus(&m, &ir, &dag).expect("derive");
        assert!(
            no_seeded_value_outside_allowed(&m, &cases),
            "no derived case seeds a value outside the frozen-enum domain"
        );
    }
}
