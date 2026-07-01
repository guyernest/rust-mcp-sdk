//! Reference-input reconciliation (WBVER-03): re-run the served executor at the
//! workbook's REFERENCE inputs and diff each tool output against its authored
//! `Tool.oracle` within `TOL`.
//!
//! This makes the compile-time penny-reconcile RUNTIME-inspectable: a
//! [`ReconcileReport`] attests, per output cell, that the engine reproduces
//! Excel's authored value AT THE REFERENCE INPUTS (the manifest tier defaults —
//! VERIFIED the oracle was computed there). It is a HONEST, narrow attestation:
//! it does NOT attest arbitrary inputs (the downloadable formula workbook, with
//! Excel as the oracle, covers those).
//!
//! Purity (reader-free leaf): this module composes ONLY the executor
//! ([`crate::sheet_ir::run`]) + the manifest/artifact model + `serde`/`schemars`.
//! It imports NO reader (`umya`/`quick-xml`/`calamine`) and is callable WITHOUT a
//! toolkit dependency — the runtime carries the tier defaults natively
//! ([`crate::manifest_model::InputTier`]), so [`seed_reference_inputs`] never
//! re-opens the source workbook nor reaches across the layering fence.
//!
//! Panic-freedom: every fn on the value path is TOTAL — `?`/`get`/`match`, never
//! `unwrap`/`expect`/`panic` (the crate-level `deny`). [`compare_output`] is a
//! total comparison over every [`CellValue`] variant (numeric, Text, Bool,
//! Empty, Error, type-mismatch) and never yields a `NaN`/unspecified delta.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::artifact_model::Tool;
use crate::dag::Dag;
use crate::finding::LintFinding;
use crate::manifest_model::{InputTier, Manifest, Role};
use crate::sheet_ir::value::CellValue;
use crate::sheet_ir::{run as run_executor, Cell, CellEnv};

/// The default reconciliation tolerance (±0.01), mirroring the compiler's
/// `reconcile::TOL` and the runtime [`crate::scalar_eval`] `TOL` so a numeric
/// output is graded WITHIN the SAME float-boundary slack the penny-reconcile used.
pub const TOL: f64 = 0.01;

/// One reconciled output cell: the per-key diff of the engine's recomputed value
/// against the authored oracle.
///
/// `cell` is the D-01 sheet-qualified A1 address (e.g. `"3_Outputs!B3"`) of the
/// source cell, filled from the matching [`crate::artifact_model::CellEntry`]
/// `seed_coord`. It is [`None`] (D-02) ONLY when an `oracle` key has no matching
/// `outputs` entry (a malformed bundle) — the row still reports its deltas.
///
/// Derive note: `Eq` is dropped because `abs_delta` is an `f64` (the
/// [`crate::artifact_model::Tool`] precedent).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OutputRow {
    /// The output's LLM-facing json key.
    pub key: String,
    /// The D-01 sheet-qualified A1 source address; [`None`] (D-02) only when an
    /// oracle key has no matching output entry.
    pub cell: Option<String>,
    /// The engine's recomputed value at the reference inputs.
    pub server_value: Option<CellValue>,
    /// The authored oracle value (Excel's cached `<v>`).
    pub oracle_value: Option<CellValue>,
    /// The absolute delta: `|server − oracle|` for numbers; `0.0` (equal) or `1.0`
    /// (not equal / type mismatch / Empty / Error) for the discrete types —
    /// DETERMINISTIC, never `NaN`/unspecified.
    pub abs_delta: f64,
    /// `true` iff this output reconciles within `TOL`.
    pub within_tol: bool,
}

/// The per-tool reconciliation report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolReport {
    /// The tool name.
    pub tool: String,
    /// `true` iff every checked output in this tool is within `TOL`. A tool with
    /// an empty oracle is vacuously `true` (D-04).
    pub all_within_tol: bool,
    /// One [`OutputRow`] per oracle/output key. Empty for an empty-oracle tool
    /// (D-04).
    pub outputs: Vec<OutputRow>,
}

/// The full reconciliation report — the `verify_accuracy` payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReconcileReport {
    /// The tolerance the report was graded at.
    pub tolerance: f64,
    /// `true` iff every checked output across every reported tool is within `TOL`.
    pub all_within_tol: bool,
    /// The number of output rows that were actually COMPARED (an empty-oracle tool
    /// contributes 0; D-04).
    pub cells_checked: u32,
    /// One [`ToolReport`] per reconciled tool.
    pub tools: Vec<ToolReport>,
}

/// Build the REFERENCE-input seed map natively from the manifest tier defaults.
///
/// Iterates `manifest.cells`, keeps each [`Role::Input`], and reads its
/// [`InputTier`] default as a runtime-native [`CellValue`] — the
/// [`InputTier::Variable`] / [`InputTier::BoundedVariable`] `default`. An input
/// whose `tier` is [`None`] contributes NO seed (mirroring the toolkit's
/// `tier_default` `Some`-guard); the executor then resolves it from the IR.
///
/// This is a runtime-native mirror of the TOOLKIT-private `seed_tier_defaults`
/// (which returns `serde_json::Value`) at the manifest-tier level — returning the
/// runtime [`CellValue`] WITHOUT a toolkit dependency and WITHOUT re-implementing
/// the toolkit's dtype/enum input validation (reconcile needs only the reference
/// values, not input validation).
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use pmcp_workbook_runtime::reconcile::seed_reference_inputs;
/// use pmcp_workbook_runtime::{CellValue, InputTier, Manifest, Role};
/// use pmcp_workbook_runtime::manifest_model::{CellRole, Dtype};
///
/// let manifest = Manifest {
///     schema_version: 1,
///     workflow: "demo".into(),
///     workbook_hash: None,
///     ratified: true,
///     ratified_by: None,
///     ratified_at: None,
///     cells: vec![CellRole {
///         cell: "1_Inputs!B2".into(),
///         role: Role::Input,
///         name: Some("in_x".into()),
///         unit: None,
///         meaning: None,
///         dtype: Dtype::Number,
///         colour_evidence: None,
///         source: "test".into(),
///         notes: None,
///         tier: Some(InputTier::Variable { default: CellValue::Number(42.0) }),
///         allowed_values: None,
///     }],
///     loop_block: None,
///     governed_data: vec![],
///     changelog: vec![],
///     capability_calls: vec![],
///     annotations: vec![],
/// };
///
/// let seeds = seed_reference_inputs(&manifest);
/// assert_eq!(seeds.get("1_Inputs!B2"), Some(&CellValue::Number(42.0)));
/// ```
#[must_use]
pub fn seed_reference_inputs(manifest: &Manifest) -> BTreeMap<String, CellValue> {
    let mut seeds = BTreeMap::new();
    for role in &manifest.cells {
        if !matches!(role.role, Role::Input) {
            continue;
        }
        match &role.tier {
            Some(InputTier::Variable { default })
            | Some(InputTier::BoundedVariable { default, .. }) => {
                seeds.insert(role.cell.clone(), default.clone());
            },
            None => {},
        }
    }
    seeds
}

/// Compare one server value against its oracle, returning `(abs_delta, within_tol)`.
///
/// TOTAL over every [`CellValue`] pairing (and the missing-value cases):
/// - numeric/numeric → `abs_delta = |server − oracle|`, `within_tol` iff BOTH are
///   finite AND `abs_delta <= tol`;
/// - `Text`/`Text` or `Bool`/`Bool` → equality: `0.0` + `true` when equal, `1.0` +
///   `false` when not equal;
/// - any other pairing (`Empty`, `Error`, a type mismatch, or a missing
///   server/oracle value) → `1.0` + `false` (fail-closed).
///
/// NEVER yields a `NaN`/unspecified delta.
#[must_use]
fn compare_output(server: Option<&CellValue>, oracle: Option<&CellValue>) -> (f64, bool) {
    match (server, oracle) {
        (Some(CellValue::Number(s)), Some(CellValue::Number(o)))
            if s.is_finite() && o.is_finite() =>
        {
            let delta = (s - o).abs();
            (delta, delta <= TOL)
        },
        (Some(CellValue::Text(s)), Some(CellValue::Text(o))) => discrete_eq(s == o),
        (Some(CellValue::Bool(s)), Some(CellValue::Bool(o))) => discrete_eq(s == o),
        // Empty/Error/type-mismatch/missing → fail-closed, deterministic.
        _ => (1.0, false),
    }
}

/// The deterministic discrete-type delta: `(0.0, true)` when equal, `(1.0,
/// false)` when not (Text/Bool). Never `NaN`.
#[must_use]
fn discrete_eq(equal: bool) -> (f64, bool) {
    if equal {
        (0.0, true)
    } else {
        (1.0, false)
    }
}

/// Reconcile ONE tool: project each oracle/output key into an [`OutputRow`] and
/// roll up the tool-level `all_within_tol` + the count of COMPARED rows.
///
/// A row is built for every `outputs` entry that has an oracle value, PLUS any
/// oracle key with NO matching `outputs` entry (D-02: `cell = None`, still graded).
/// An empty oracle yields `outputs: []` + `all_within_tol = true` (D-04, vacuous),
/// contributing 0 to the comparison count.
fn reconcile_tool(tool: &Tool, computed: &HashMap<String, CellValue>) -> (ToolReport, u32) {
    let mut rows = Vec::new();
    // Borrowed output keys we have already graded — used to skip oracle-only keys
    // in the D-02 loop below. Borrowed (`&str`) + set membership avoids a per-key
    // String clone and the O(outputs × oracle) linear scan.
    let mut matched_keys: HashSet<&str> = HashSet::new();

    // Rows for declared outputs (the common path: cell = Some(seed_coord)).
    for entry in &tool.outputs {
        let Some(oracle_value) = tool.oracle.get(&entry.json_key) else {
            continue; // an output with no authored oracle is not graded here.
        };
        matched_keys.insert(entry.json_key.as_str());
        let server_value = computed.get(&entry.seed_coord).cloned();
        let (abs_delta, within_tol) = compare_output(server_value.as_ref(), Some(oracle_value));
        rows.push(OutputRow {
            key: entry.json_key.clone(),
            cell: Some(entry.seed_coord.clone()),
            server_value,
            oracle_value: Some(oracle_value.clone()),
            abs_delta,
            within_tol,
        });
    }

    // D-02: any oracle key WITHOUT a matching outputs entry → cell = None, graded.
    for (key, oracle_value) in &tool.oracle {
        if matched_keys.contains(key.as_str()) {
            continue;
        }
        let (abs_delta, within_tol) = compare_output(None, Some(oracle_value));
        rows.push(OutputRow {
            key: key.clone(),
            cell: None,
            server_value: None,
            oracle_value: Some(oracle_value.clone()),
            abs_delta,
            within_tol,
        });
    }

    let compared = u32::try_from(rows.len()).unwrap_or(u32::MAX);
    let all_within_tol = rows.iter().all(|r| r.within_tol);
    (
        ToolReport {
            tool: tool.name.clone(),
            all_within_tol,
            outputs: rows,
        },
        compared,
    )
}

/// Re-run the executor at the workbook's REFERENCE inputs and reconcile every
/// tool output against its authored `Tool.oracle` within `tol`.
///
/// Seeds the [`CellEnv`] natively from [`seed_reference_inputs`] (the manifest
/// tier defaults — NO toolkit dep, no serde round-trip), runs the SHARED executor
/// ([`crate::sheet_ir::run`] — no second evaluator), then projects per tool via
/// [`reconcile_tool`]. The report's `all_within_tol` is true iff EVERY compared
/// output is within `tol`; `cells_checked` counts only compared rows (an
/// empty-oracle tool contributes 0, D-04).
///
/// Panic-free: returns `Err(Box<LintFinding>)` on an executor failure (e.g. a DAG
/// cycle — impossible for a conforming bundle); never `unwrap`/`panic`.
///
/// # Errors
///
/// Returns the located [`LintFinding`] the executor surfaces (e.g. a `dag/cycle`).
///
/// # Examples
///
/// ```
/// use std::collections::HashMap;
/// use pmcp_workbook_runtime::reconcile::reconcile_reference;
/// use pmcp_workbook_runtime::{build_dag, CellMap, Manifest};
///
/// // A degenerate bundle with no tools reconciles vacuously.
/// let manifest = Manifest {
///     schema_version: 1,
///     workflow: "empty".into(),
///     workbook_hash: None,
///     ratified: true,
///     ratified_by: None,
///     ratified_at: None,
///     cells: vec![],
///     loop_block: None,
///     governed_data: vec![],
///     changelog: vec![],
///     capability_calls: vec![],
///     annotations: vec![],
/// };
/// let cell_map = CellMap { inputs: vec![], tools: vec![] };
/// let ir = HashMap::new();
/// let dag = build_dag(&ir);
/// let report = reconcile_reference(&cell_map, &manifest, &ir, &dag, 0.01).unwrap();
/// assert!(report.all_within_tol);
/// assert_eq!(report.cells_checked, 0);
/// ```
#[allow(clippy::result_large_err)]
pub fn reconcile_reference(
    cell_map: &crate::artifact_model::CellMap,
    manifest: &Manifest,
    ir: &HashMap<String, Cell>,
    dag: &Dag,
    tol: f64,
) -> Result<ReconcileReport, Box<LintFinding>> {
    // Seed the executor natively from the manifest reference (tier) defaults.
    let mut env = CellEnv::new();
    for (key, value) in seed_reference_inputs(manifest) {
        env = env.seed_cell(key, &value);
    }

    let run = run_executor(ir, dag, &env)?;

    let mut tools = Vec::with_capacity(cell_map.tools.len());
    let mut cells_checked: u32 = 0;
    let mut all_within_tol = true;
    for tool in &cell_map.tools {
        let (report, compared) = reconcile_tool(tool, &run.computed);
        cells_checked = cells_checked.saturating_add(compared);
        all_within_tol = all_within_tol && report.all_within_tol;
        tools.push(report);
    }

    Ok(ReconcileReport {
        tolerance: tol,
        all_within_tol,
        cells_checked,
        tools,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_model::{CellEntry, CellMap};
    use crate::manifest_model::{CellRole, Dtype};
    use crate::sheet_ir::{build_dag, Cell, CellExpr};

    fn input_role(cell: &str, default: CellValue) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Input,
            name: None,
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".into(),
            notes: None,
            tier: Some(InputTier::Variable { default }),
            allowed_values: None,
        }
    }

    fn manifest_with(cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "test".into(),
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

    fn output_entry(json_key: &str, seed_coord: &str) -> CellEntry {
        CellEntry {
            json_key: json_key.to_string(),
            seed_coord: seed_coord.to_string(),
            unit: None,
        }
    }

    /// A literal cell that the executor will echo into `run.computed`.
    fn literal_cell(key: &str, value: CellValue) -> Cell {
        Cell {
            key: key.to_string(),
            expr: CellExpr::Literal(value),
        }
    }

    #[test]
    fn seed_reference_inputs_reads_tier_defaults() {
        let manifest = manifest_with(vec![
            input_role("S!A1", CellValue::Number(10.0)),
            input_role("S!A2", CellValue::Text("hi".into())),
        ]);
        let seeds = seed_reference_inputs(&manifest);
        assert_eq!(seeds.get("S!A1"), Some(&CellValue::Number(10.0)));
        assert_eq!(seeds.get("S!A2"), Some(&CellValue::Text("hi".into())));
    }

    #[test]
    fn seed_reference_inputs_skips_untiered_inputs() {
        let mut role = input_role("S!A1", CellValue::Number(1.0));
        role.tier = None;
        let manifest = manifest_with(vec![role]);
        let seeds = seed_reference_inputs(&manifest);
        assert!(
            seeds.is_empty(),
            "an untiered Role::Input contributes no seed"
        );
    }

    #[test]
    fn seed_reference_inputs_skips_non_input_roles() {
        let mut role = input_role("S!A1", CellValue::Number(1.0));
        role.role = Role::Constant;
        let manifest = manifest_with(vec![role]);
        assert!(seed_reference_inputs(&manifest).is_empty());
    }

    /// A one-output bundle whose oracle matches the recomputed reference value
    /// reconciles `all_within_tol == true`, `cells_checked == 1`, `cell == coord`.
    fn one_output_tool(oracle: CellValue) -> (CellMap, Manifest, HashMap<String, Cell>, Dag) {
        let manifest = manifest_with(vec![input_role("S!A1", CellValue::Number(5.0))]);
        let mut ir = HashMap::new();
        // The output cell is a literal so the executor echoes a known value.
        ir.insert(
            "S!B1".to_string(),
            literal_cell("S!B1", CellValue::Number(5.0)),
        );
        let dag = build_dag(&ir);
        let mut oracle_map = BTreeMap::new();
        oracle_map.insert("out".to_string(), oracle);
        let cell_map = CellMap {
            inputs: vec![],
            tools: vec![Tool {
                name: "T".into(),
                description: None,
                input_keys: vec![],
                outputs: vec![output_entry("out", "S!B1")],
                oracle: oracle_map,
            }],
        };
        (cell_map, manifest, ir, dag)
    }

    #[test]
    fn golden_within_tol_reconciles_true() {
        let (cell_map, manifest, ir, dag) = one_output_tool(CellValue::Number(5.0));
        let report = reconcile_reference(&cell_map, &manifest, &ir, &dag, TOL).unwrap();
        assert!(report.all_within_tol);
        assert_eq!(report.cells_checked, 1);
        let row = &report.tools[0].outputs[0];
        assert_eq!(row.cell.as_deref(), Some("S!B1"));
        assert!(row.within_tol);
        assert!(row.abs_delta <= TOL);
    }

    #[test]
    fn perturbed_oracle_reconciles_false() {
        // Oracle deliberately wrong (5.0 computed, oracle 99.0).
        let (cell_map, manifest, ir, dag) = one_output_tool(CellValue::Number(99.0));
        let report = reconcile_reference(&cell_map, &manifest, &ir, &dag, TOL).unwrap();
        assert!(!report.all_within_tol);
        assert!(!report.tools[0].all_within_tol);
        assert!(!report.tools[0].outputs[0].within_tol);
    }

    #[test]
    fn text_abs_delta_is_deterministic() {
        let equal = compare_output(
            Some(&CellValue::Text("a".into())),
            Some(&CellValue::Text("a".into())),
        );
        assert_eq!(equal, (0.0, true));
        let differ = compare_output(
            Some(&CellValue::Text("a".into())),
            Some(&CellValue::Text("b".into())),
        );
        assert_eq!(differ, (1.0, false));
    }

    #[test]
    fn bool_abs_delta_is_deterministic() {
        assert_eq!(
            compare_output(Some(&CellValue::Bool(true)), Some(&CellValue::Bool(true))),
            (0.0, true)
        );
        assert_eq!(
            compare_output(Some(&CellValue::Bool(true)), Some(&CellValue::Bool(false))),
            (1.0, false)
        );
    }

    #[test]
    fn type_mismatch_and_missing_fail_closed() {
        // Number vs Text → fail-closed.
        assert_eq!(
            compare_output(
                Some(&CellValue::Number(1.0)),
                Some(&CellValue::Text("x".into()))
            ),
            (1.0, false)
        );
        // Missing server value → fail-closed.
        assert_eq!(
            compare_output(None, Some(&CellValue::Number(1.0))),
            (1.0, false)
        );
        // Empty → fail-closed.
        assert_eq!(
            compare_output(Some(&CellValue::Empty), Some(&CellValue::Number(0.0))),
            (1.0, false)
        );
    }

    #[test]
    fn empty_oracle_tool_is_vacuous_d04() {
        let manifest = manifest_with(vec![]);
        let ir = HashMap::new();
        let dag = build_dag(&ir);
        let cell_map = CellMap {
            inputs: vec![],
            tools: vec![Tool {
                name: "Empty".into(),
                description: None,
                input_keys: vec![],
                outputs: vec![],
                oracle: BTreeMap::new(),
            }],
        };
        let report = reconcile_reference(&cell_map, &manifest, &ir, &dag, TOL).unwrap();
        assert_eq!(report.tools[0].outputs.len(), 0);
        assert!(report.tools[0].all_within_tol);
        assert_eq!(report.cells_checked, 0);
        assert!(report.all_within_tol);
    }

    #[test]
    fn oracle_without_outputs_entry_yields_cell_none_d02() {
        let manifest = manifest_with(vec![]);
        let ir = HashMap::new();
        let dag = build_dag(&ir);
        let mut oracle = BTreeMap::new();
        oracle.insert("ghost".to_string(), CellValue::Number(1.0));
        let cell_map = CellMap {
            inputs: vec![],
            tools: vec![Tool {
                name: "T".into(),
                description: None,
                input_keys: vec![],
                outputs: vec![], // no matching entry for "ghost"
                oracle,
            }],
        };
        let report = reconcile_reference(&cell_map, &manifest, &ir, &dag, TOL).unwrap();
        let row = &report.tools[0].outputs[0];
        assert_eq!(row.key, "ghost");
        assert_eq!(row.cell, None);
        assert!(!row.within_tol); // no server value → fail-closed
    }

    proptest::proptest! {
        /// Report-level all_within_tol == AND over tool-level, and holds iff every
        /// OutputRow.within_tol is true.
        #[test]
        fn prop_all_within_tol_is_conjunction(oracle in -1000.0f64..1000.0) {
            let (cell_map, manifest, ir, dag) = one_output_tool(CellValue::Number(oracle));
            let report = reconcile_reference(&cell_map, &manifest, &ir, &dag, TOL).unwrap();
            let tool_and = report.tools.iter().all(|t| t.all_within_tol);
            proptest::prop_assert_eq!(report.all_within_tol, tool_and);
            let row_and = report
                .tools
                .iter()
                .flat_map(|t| t.outputs.iter())
                .all(|r| r.within_tol);
            proptest::prop_assert_eq!(report.all_within_tol, row_and);
        }
    }
}
