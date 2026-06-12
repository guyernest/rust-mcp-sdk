//! Reconcile stage — grade compiled IR against the cached oracle (WBCO-04).
//!
//! When the executor ([`crate::sheet_ir`], the runtime's pure-Rust SERVE-time
//! evaluator — NO SWC/JS oracle) computes a cell whose value differs from the
//! workbook's CACHED `<v>` value, the [`classifier`] decides — from EVIDENCE, never
//! from a blanket gap tolerance — which mismatch class the divergence belongs to, or
//! refuses (`Unclassified` = HARD FAIL).
//!
//! # The D-03 severity split (the keystone)
//!
//! [`reconcile`] diffs EVERY comparison cell against its cached target within
//! [`TOL`] (NEVER exact-float), classifies every out-of-tolerance delta, and grades
//! it via [`drift::mismatch_severity`]: a **named-output** mismatch is an **ERROR**
//! that blocks emit ([`ReconcileReport::has_errors`]); a **helper-cell** mismatch is
//! a located **WARNING**. An `Unclassified` delta hard-fails regardless of where it
//! sits ([`ReconcileReport::is_hard_fail`]).
//!
//! # Non-numeric cached cells (Codex MEDIUM)
//!
//! [`within_tol`] compares NUMBERS via a penny tolerance and every other shape
//! (text/bool/blank/error) STRUCTURALLY — a cached non-numeric value never reaches a
//! numeric branch, so the reconcile never panics on a non-money cell.

pub mod classifier;
pub mod drift;

use std::collections::HashMap;

use pmcp_workbook_runtime::{Cell, CellExpr, CellValue, EvalTrace, Expr, Manifest, Severity};

use classifier::{classify, classify_absent, MismatchClass, MismatchEvidence};

/// The money reconciliation tolerance — ±0.01 (one penny). EVERY numeric compare in
/// the reconciliation goes through this; an exact-float `==` on money is FORBIDDEN.
pub const TOL: f64 = 0.01;

/// True iff `computed` reconciles to `target` within [`TOL`]. NUMBERS compare via
/// the magnitude-free `(a - b).abs() <= TOL` (NEVER exact-float `==`); a non-finite
/// Number is ALWAYS out of tolerance (so `inf == inf` never spuriously reconciles);
/// every non-numeric pair (text/bool/blank/error) reconciles iff STRUCTURALLY equal
/// (Codex MEDIUM — a cached text/bool/error never enters a numeric branch).
#[must_use]
pub fn within_tol(computed: &CellValue, target: &CellValue) -> bool {
    match (computed, target) {
        (CellValue::Number(a), CellValue::Number(b)) if a.is_finite() && b.is_finite() => {
            (a - b).abs() <= TOL
        },
        // A non-finite Number on either side is never reconciled structurally.
        (CellValue::Number(n), _) | (_, CellValue::Number(n)) if !n.is_finite() => false,
        // A non-numeric value has no penny tolerance — it reconciles iff identical.
        (a, b) => a == b,
    }
}

/// One comparison cell: the compiled `cell_key` and its cached-oracle `target`.
#[derive(Debug, Clone)]
pub struct ComparisonCell {
    /// The fully-qualified compiled cell key (`sheet!addr`).
    pub cell_key: String,
    /// The reconciliation target value (the workbook's cached `<v>` oracle).
    pub target: CellValue,
}

/// The comparison-cell map: `cell_key -> cached target`. Keyed in insertion order.
#[derive(Debug, Clone, Default)]
pub struct ComparisonMap {
    cells: Vec<ComparisonCell>,
}

impl ComparisonMap {
    /// An empty map ready to accumulate comparison cells.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one comparison cell with a numeric cached target.
    #[must_use]
    pub fn with(mut self, cell_key: &str, target: f64) -> Self {
        self.cells.push(ComparisonCell {
            cell_key: cell_key.to_string(),
            target: CellValue::Number(target),
        });
        self
    }

    /// Add one comparison cell with an arbitrary (numeric OR non-numeric) cached
    /// target — used for the text/bool/blank/error cached-cell paths.
    #[must_use]
    pub fn with_value(mut self, cell_key: &str, target: CellValue) -> Self {
        self.cells.push(ComparisonCell {
            cell_key: cell_key.to_string(),
            target,
        });
        self
    }

    /// The comparison cells, in insertion order.
    #[must_use]
    pub fn cells(&self) -> &[ComparisonCell] {
        &self.cells
    }
}

/// One graded mismatch: the classifier's [`MismatchEvidence`] + the D-03 severity.
#[derive(Debug, Clone)]
pub struct GradedMismatch {
    /// The classified evidence.
    pub evidence: MismatchEvidence,
    /// The D-03 severity: `Error` for a named output, `Warning` for a helper.
    pub severity: Severity,
}

/// The collect-all aggregate of one reconciliation pass. The driver accumulates
/// EVERY out-of-tolerance delta's graded evidence; [`ReconcileReport::has_errors`]
/// (a named-output mismatch) and [`ReconcileReport::is_hard_fail`] (any
/// `Unclassified`) are the two emit gates.
#[derive(Debug, Clone, Default)]
pub struct ReconcileReport {
    /// EVERY out-of-tolerance delta's graded evidence, in comparison order.
    pub mismatches: Vec<GradedMismatch>,
    /// The count of comparison cells that reconciled within [`TOL`].
    pub reconciled: usize,
}

impl ReconcileReport {
    /// The D-03 emit gate: `true` iff ANY mismatch is on a NAMED OUTPUT
    /// (`Severity::Error`) — a wrong PUBLISHED answer blocks emit.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.mismatches
            .iter()
            .any(|m| m.severity == Severity::Error)
    }

    /// The hard-fail gate: `true` iff ANY mismatch is [`MismatchClass::Unclassified`]
    /// — a real logic bug is never silently fudged, regardless of cell role.
    #[must_use]
    pub fn is_hard_fail(&self) -> bool {
        self.mismatches
            .iter()
            .any(|m| m.evidence.class == MismatchClass::Unclassified)
    }

    /// `true` iff EVERY comparison cell reconciled within [`TOL`] (no mismatches).
    #[must_use]
    pub fn all_reconciled(&self) -> bool {
        self.mismatches.is_empty()
    }

    /// The located WARNING mismatches (helper-cell drift — advisory, never blocking).
    #[must_use]
    pub fn warnings(&self) -> Vec<&GradedMismatch> {
        self.mismatches
            .iter()
            .filter(|m| m.severity == Severity::Warning)
            .collect()
    }
}

/// Drive the reconciliation: diff `computed` against the `comparison` map within
/// [`TOL`], classify every out-of-tolerance (or absent) delta threading the per-cell
/// [`EvalTrace`], grade each by the D-03 named-output/helper split, and accumulate a
/// collect-all [`ReconcileReport`] — never stopping at the first delta. A comparison
/// cell ABSENT from `computed` is an `Unclassified` `#REF!`-target mismatch. Value
/// path: no panic, no `.unwrap()`.
#[must_use]
pub fn reconcile(
    computed: &HashMap<String, CellValue>,
    traces: &HashMap<String, EvalTrace>,
    ir: &HashMap<String, Cell>,
    comparison: &ComparisonMap,
    manifest: &Manifest,
) -> ReconcileReport {
    let mut report = ReconcileReport::default();
    let empty_trace = EvalTrace::default();

    for cell in comparison.cells() {
        let key = &cell.cell_key;
        let target = &cell.target;
        match computed.get(key) {
            Some(c) if within_tol(c, target) => report.reconciled += 1,
            maybe => {
                let absent = maybe.is_none();
                let computed_val = maybe
                    .cloned()
                    .unwrap_or(CellValue::Error(pmcp_workbook_runtime::ExcelError::Ref));
                let expr = deciding_expr(ir, key);
                let trace = traces.get(key).unwrap_or(&empty_trace);
                let evidence = if absent {
                    classify_absent(key, &expr, target, manifest)
                } else {
                    classify(key, &expr, &computed_val, target, trace, manifest)
                };
                let severity = drift::mismatch_severity(key, manifest);
                report
                    .mismatches
                    .push(GradedMismatch { evidence, severity });
            },
        }
    }

    report
}

/// The deciding [`Expr`] for `cell_key`: the cell's parsed formula when `ir` carries
/// one, else a literal stand-in (`Expr::Number(0)`) for a pure-input/constant cell
/// with no formula AST (which cannot be a rounding boundary, so the stand-in is sound).
fn deciding_expr(ir: &HashMap<String, Cell>, cell_key: &str) -> Expr {
    match ir.get(cell_key).map(|c| &c.expr) {
        Some(CellExpr::Formula(e)) => e.clone(),
        _ => Expr::Number(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::{run_executor, BinOp, CellEnv, Dag};
    use pmcp_workbook_runtime::{CellRole, Dtype, Role};

    fn empty_manifest() -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "wf".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells: vec![],
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    fn manifest_with_output(cell: &str) -> Manifest {
        let mut m = empty_manifest();
        m.cells.push(CellRole {
            cell: cell.to_string(),
            role: Role::Output,
            name: Some("out_total".to_string()),
            unit: None,
            meaning: None,
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        });
        m
    }

    #[test]
    fn within_tol_is_penny_tolerant_never_exact_float() {
        assert!(within_tol(
            &CellValue::Number(1594.93),
            &CellValue::Number(1594.935)
        ));
        assert!(!within_tol(
            &CellValue::Number(1594.93),
            &CellValue::Number(1594.95)
        ));
    }

    #[test]
    fn within_tol_never_reconciles_a_non_finite_number() {
        assert!(!within_tol(
            &CellValue::Number(f64::INFINITY),
            &CellValue::Number(f64::INFINITY)
        ));
        assert!(!within_tol(
            &CellValue::Number(f64::NAN),
            &CellValue::Number(1.0)
        ));
    }

    #[test]
    fn within_tol_compares_non_numeric_cached_cells_structurally() {
        // Codex MEDIUM: a cached text/bool/blank reconciles iff identical — never a
        // numeric branch, never a panic.
        assert!(within_tol(
            &CellValue::Text("a".to_string()),
            &CellValue::Text("a".to_string())
        ));
        assert!(!within_tol(
            &CellValue::Text("a".to_string()),
            &CellValue::Text("b".to_string())
        ));
        assert!(within_tol(&CellValue::Bool(true), &CellValue::Bool(true)));
        assert!(within_tol(&CellValue::Empty, &CellValue::Empty));
    }

    /// A tiny CEILING IR: required input seeded, `CEILING(req*1.05, 50)` at the
    /// output cell — a representative formula reconciled via the PURE-RUST executor.
    fn ceiling_ir(required: f64) -> (HashMap<String, Cell>, Dag, CellEnv) {
        let mut ir: HashMap<String, Cell> = HashMap::new();
        let mut seed = CellEnv::new();
        seed = seed.seed_cell("S!REQ", &CellValue::Number(required));
        ir.insert(
            "S!OUT".to_string(),
            Cell {
                key: "S!OUT".to_string(),
                expr: CellExpr::Formula(Expr::Call {
                    name: "CEILING".to_string(),
                    args: vec![
                        Expr::BinaryOp {
                            left: Box::new(Expr::Ref("S!REQ".to_string())),
                            op: BinOp::Mul,
                            right: Box::new(Expr::Number(1.05)),
                        },
                        Expr::Number(50.0),
                    ],
                }),
            },
        );
        let mut dag = Dag::new();
        dag.add_node("S!REQ");
        dag.add_edge("S!OUT", "S!REQ");
        (ir, dag, seed)
    }

    #[test]
    fn reconcile_named_output_mismatch_errors() {
        // A non-rounding divergence on a NAMED OUTPUT blocks emit (D-03 ERROR).
        let (ir, dag, seed) = ceiling_ir(666.0); // CEILING(699.3, 50) = 700
        let out = run_executor(&ir, &dag, &seed).expect("acyclic");
        let m = manifest_with_output("S!OUT");
        // Target 500 is far off and NOT a one-step rounding difference.
        let map = ComparisonMap::new().with("S!OUT", 500.0);
        let report = reconcile(&out.computed, &out.traces, &ir, &map, &m);
        assert!(report.has_errors(), "a named-output mismatch blocks emit");
        assert_eq!(report.mismatches[0].severity, Severity::Error);
    }

    #[test]
    fn reconcile_helper_mismatch_warns() {
        // The SAME divergence on an intermediate/helper cell (no Output role) is a
        // located WARNING, never a block (D-03).
        let (ir, dag, seed) = ceiling_ir(666.0);
        let out = run_executor(&ir, &dag, &seed).expect("acyclic");
        let m = empty_manifest(); // S!OUT has no Output role here → helper
        let map = ComparisonMap::new().with("S!OUT", 500.0);
        let report = reconcile(&out.computed, &out.traces, &ir, &map, &m);
        assert!(!report.has_errors(), "a helper mismatch never blocks emit");
        assert_eq!(report.warnings().len(), 1);
        assert_eq!(report.mismatches[0].severity, Severity::Warning);
    }

    #[test]
    fn an_absent_required_output_hard_fails() {
        let (ir, dag, seed) = ceiling_ir(666.0);
        let out = run_executor(&ir, &dag, &seed).expect("acyclic");
        let m = manifest_with_output("S!MISSING");
        let map = ComparisonMap::new().with("S!MISSING", 1594.93); // never computed
        let report = reconcile(&out.computed, &out.traces, &ir, &map, &m);
        assert_eq!(report.reconciled, 0);
        assert!(
            report.is_hard_fail(),
            "a missing required output hard-fails"
        );
        assert_eq!(
            report.mismatches[0].evidence.deciding_rule,
            "reconcile/absent-output"
        );
    }

    #[test]
    fn reconcile_non_numeric_cached_cell_does_not_panic() {
        // A cached TEXT cell vs a computed TEXT cell reconciles structurally — the
        // driver never coerces it through a numeric branch (Codex MEDIUM).
        let mut computed: HashMap<String, CellValue> = HashMap::new();
        computed.insert("S!T".to_string(), CellValue::Text("hello".to_string()));
        let traces: HashMap<String, EvalTrace> = HashMap::new();
        let ir: HashMap<String, Cell> = HashMap::new();
        let m = empty_manifest();

        let ok = ComparisonMap::new().with_value("S!T", CellValue::Text("hello".to_string()));
        let report = reconcile(&computed, &traces, &ir, &ok, &m);
        assert!(report.all_reconciled(), "identical cached text reconciles");

        let bad = ComparisonMap::new().with_value("S!T", CellValue::Text("world".to_string()));
        let report = reconcile(&computed, &traces, &ir, &bad, &m);
        assert_eq!(
            report.mismatches.len(),
            1,
            "a differing cached text mismatches"
        );
        assert_eq!(
            report.mismatches[0].evidence.class,
            MismatchClass::NonNumericMismatch
        );
    }

    // ── O-1 PARITY SUITE (the named proof: pure-Rust reconcile, NO SWC/JS) ──────
    //
    // A representative set of dialect formulas reconciles via the runtime's
    // pure-Rust scalar_eval + sheet_ir executor with NO JS oracle. THIS suite IS
    // the O-1 parity proof — not a summary note. (No pmcp-code-mode dependency
    // exists in this crate; the AC grep enforces it.)

    /// Run one literal-args formula `expr` through the pure-Rust executor and
    /// return its single computed output value.
    fn eval_one(expr: Expr) -> CellValue {
        let mut ir: HashMap<String, Cell> = HashMap::new();
        ir.insert(
            "S!OUT".to_string(),
            Cell {
                key: "S!OUT".to_string(),
                expr: CellExpr::Formula(expr),
            },
        );
        let mut dag = Dag::new(); // no deps — all-literal formula, but the output
        dag.add_node("S!OUT"); // cell must still be a DAG node so the executor walks it
        let out = run_executor(&ir, &dag, &CellEnv::new()).expect("acyclic");
        out.computed
            .get("S!OUT")
            .cloned()
            .unwrap_or(CellValue::Empty)
    }

    fn num(name: &str, args: Vec<Expr>) -> CellValue {
        eval_one(Expr::Call {
            name: name.to_string(),
            args,
        })
    }

    #[test]
    fn o1_parity_suite() {
        // Arithmetic + the rounding family + a conditional — all pure-Rust, no SWC.
        // ROUND half-away-from-zero at the decimal boundary.
        assert!(within_tol(
            &num("ROUND", vec![Expr::Number(1594.925), Expr::Number(2.0)]),
            &CellValue::Number(1594.93),
        ));
        // ROUNDUP away from zero.
        assert!(within_tol(
            &num("ROUNDUP", vec![Expr::Number(3.001), Expr::Number(2.0)]),
            &CellValue::Number(3.01),
        ));
        // CEILING to the next multiple.
        assert!(within_tol(
            &num("CEILING", vec![Expr::Number(699.3), Expr::Number(50.0)]),
            &CellValue::Number(700.0),
        ));
        // SUM of literals.
        assert!(within_tol(
            &num("SUM", vec![Expr::Number(980.04), Expr::Number(614.89)]),
            &CellValue::Number(1594.93),
        ));
        // A bare arithmetic expression: cost / (1 - margin).
        let sell = eval_one(Expr::BinaryOp {
            left: Box::new(Expr::Number(532.66)),
            op: BinOp::Div,
            right: Box::new(Expr::BinaryOp {
                left: Box::new(Expr::Number(1.0)),
                op: BinOp::Sub,
                right: Box::new(Expr::Number(0.37)),
            }),
        });
        assert!(
            within_tol(&sell, &CellValue::Number(845.49)),
            "pure-Rust arithmetic reconciles: {sell:?}"
        );
        // IF dispatch.
        assert!(within_tol(
            &num(
                "IF",
                vec![Expr::Bool(true), Expr::Number(10.0), Expr::Number(20.0)]
            ),
            &CellValue::Number(10.0),
        ));
    }
}
