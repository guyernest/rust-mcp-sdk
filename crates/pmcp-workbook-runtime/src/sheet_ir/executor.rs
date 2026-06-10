//! The interpreted typed-IR executor (CMP-05, D-01): the SERVE-time topo-ordered
//! driver that walks the per-cell [`Dag`] in dependency order, fills a
//! [`CellEnv`], and evaluates each [`CellExpr::Formula`] through a RECURSIVE
//! `eval_expr`.
//!
//! Phase 11 Plan 05 boundary: this is the RUNTIME `run()` path — it re-runs an
//! ALREADY-built+expanded `Dag` (the server deserializes a pre-built IR + DAG).
//! Leaf arithmetic routes through the PURE-RUST [`super::scalar_eval`] via
//! [`eval_leaf`] (no SWC/JS kernel); function dispatch routes through
//! [`semantics::apply`]; range member-keys + 2-D shape come from the public
//! [`resolve::expand_range`]. The DAG-BUILD path (`build_dag`/`rebase`/loop
//! unroll, `run_with_loop`) STAYS in `workbook-compiler` and calls THIS `run()`.
//!
//! Order comes from [`toposort`] ONLY — NEVER calcChain (RESEARCH Pitfall 3). A
//! `toposort` cycle surfaces as ONE located `dag/cycle` [`LintFinding`].

use std::collections::HashMap;

use serde::Serialize;

use crate::dag::{toposort, Dag};
use crate::excel_error::ExcelError;
use crate::finding::{LintFinding, Severity};
use crate::formula::{BinOp, Expr, UnOp};
use crate::resolve;

use super::eval_bridge::{eval_leaf, from_json, percent, powf, to_json, CellEnv};
use super::eval_value::EvalValue;
use super::semantics;
use super::value::CellValue;
use super::{Cell, CellExpr};

/// A per-cell evidence record the executor emits as it computes — the classifier
/// consumes it as the deciding evidence for a mismatch. Owned and
/// serde/schemars-clean.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema)]
pub struct EvalTrace {
    /// The `cell_key` (`sheet!addr`) this trace describes.
    pub cell: String,
    /// The serialized [`Expr`] formula (a debug render); `None` for a literal.
    pub formula: Option<String>,
    /// Each resolved ref/range-member key + the [`CellValue`] it carried in env.
    pub resolved_refs: Vec<(String, CellValue)>,
    /// The [`semantics::apply`] function name dispatched (when the cell is a Call).
    pub dispatched_fn: Option<String>,
    /// The materialized scalar/range-flattened operands fed to the function/leaf.
    pub operand_values: Vec<CellValue>,
    /// Human-readable coercion notes.
    pub coercions: Vec<String>,
    /// The preflight error this cell short-circuited on, if any (D-04).
    pub short_circuited: Option<ExcelError>,
}

impl EvalTrace {
    /// A fresh trace located at `cell`.
    fn new(cell: &str) -> Self {
        EvalTrace {
            cell: cell.to_string(),
            ..Default::default()
        }
    }
}

/// The result of an executor [`run`]: the computed cell map + the per-cell
/// evidence traces. Owned, serde/schemars-clean.
#[derive(Debug, Clone, Default, Serialize, schemars::JsonSchema)]
pub struct RunResult {
    /// `cell_key -> computed CellValue` for every cell the walk evaluated.
    pub computed: HashMap<String, CellValue>,
    /// `cell_key -> EvalTrace` evidence record for every formula cell.
    pub traces: HashMap<String, EvalTrace>,
}

/// Walk the per-cell [`Dag`] in toposort order, filling `env` from `seed`, and
/// compute every cell in `ir`. Returns the computed `{cell_key -> CellValue}` map
/// together with per-cell [`EvalTrace`] evidence, or ONE located `dag/cycle`
/// [`LintFinding`] when the DAG is cyclic.
///
/// `seed` carries pre-loaded `Role::Input` cells + per-quote plot details (cells
/// ABSENT from `ir`) so the leaf inputs resolve before the walk (D-01/D-06).
pub fn run(
    ir: &HashMap<String, Cell>,
    dag: &Dag,
    seed: &CellEnv,
) -> Result<RunResult, Box<LintFinding>> {
    let order = toposort(dag).map_err(|residual| {
        let (sheet, cell) = residual
            .first()
            .and_then(|k| k.split_once('!'))
            .map(|(s, a)| (s.to_string(), Some(a.to_string())))
            .unwrap_or_default();
        Box::new(LintFinding::new(
            Severity::Error,
            "dag/cycle",
            sheet,
            cell,
            format!("dependency cycle through cells: {}", residual.join(" → ")),
            "break the cycle by removing one of the circular references",
        ))
    })?;

    let mut env = seed.clone();
    let mut errs: HashMap<String, ExcelError> = HashMap::new();
    let mut computed: HashMap<String, CellValue> = HashMap::new();
    let mut traces: HashMap<String, EvalTrace> = HashMap::new();

    for key in order {
        match ir.get(&key) {
            Some(Cell {
                expr: CellExpr::Literal(v),
                ..
            }) => {
                env = env.seed_cell(&key, v);
                if let CellValue::Error(err) = v {
                    errs.insert(key.clone(), *err);
                }
                computed.insert(key.clone(), v.clone());
            }
            Some(Cell {
                expr: CellExpr::Formula(e),
                ..
            }) => {
                let mut trace = EvalTrace::new(&key);
                let result = eval_expr(e, &env, &errs, &mut trace);
                if let CellValue::Error(err) = &result {
                    errs.insert(key.clone(), *err);
                    trace.short_circuited.get_or_insert(*err);
                }
                if let Some(j) = to_json(&result) {
                    env = env.with_value(&key, j);
                }
                computed.insert(key.clone(), result);
                traces.insert(key.clone(), trace);
            }
            // A cell in the DAG but absent from `ir` is a pre-seeded input.
            None => {}
        }
    }

    Ok(RunResult { computed, traces })
}

/// Recursively evaluate the WHOLE [`Expr`] tree to a scalar [`CellValue`].
fn eval_expr(
    e: &Expr,
    env: &CellEnv,
    errs: &HashMap<String, ExcelError>,
    trace: &mut EvalTrace,
) -> CellValue {
    trace.formula.get_or_insert_with(|| format!("{e:?}"));
    match e {
        Expr::Call { name, args } => {
            let vals: Vec<EvalValue> = args
                .iter()
                .map(|a| materialize_arg(a, env, errs, trace))
                .collect();
            trace.dispatched_fn = Some(name.clone());
            for v in &vals {
                match v {
                    EvalValue::Scalar(cv) => trace.operand_values.push(cv.clone()),
                    EvalValue::Range(rows) => {
                        for row in rows {
                            for cv in row {
                                trace.operand_values.push(cv.clone());
                            }
                        }
                    }
                }
            }
            semantics::apply(name, &vals)
        }
        Expr::BinaryOp { left, op, right } => {
            if matches!(op, BinOp::Pow) {
                let lv = eval_expr(left, env, errs, trace);
                let rv = eval_expr(right, env, errs, trace);
                let l = semantics::to_number(&lv);
                let r = semantics::to_number(&rv);
                return match (l, r) {
                    (Ok(b), Ok(x)) => finite_or_num(powf(b, x)),
                    (Err(e), _) | (_, Err(e)) => CellValue::Error(e),
                };
            }
            if is_leaf_lowerable(left) && is_leaf_lowerable(right) {
                record_refs(e, env, trace);
                return eval_leaf(e, env, errs);
            }
            let l = eval_expr(left, env, errs, trace);
            let r = eval_expr(right, env, errs, trace);
            let lowered = Expr::BinaryOp {
                left: Box::new(scalar_to_leaf(&l)),
                op: *op,
                right: Box::new(scalar_to_leaf(&r)),
            };
            eval_leaf(&lowered, env, errs)
        }
        Expr::UnaryOp { op, operand } => {
            if matches!(op, UnOp::Percent) {
                let v = eval_expr(operand, env, errs, trace);
                return match semantics::to_number(&v) {
                    Ok(x) => finite_or_num(percent(x)),
                    Err(e) => CellValue::Error(e),
                };
            }
            if is_leaf_lowerable(operand) {
                record_refs(e, env, trace);
                return eval_leaf(e, env, errs);
            }
            let v = eval_expr(operand, env, errs, trace);
            let lowered = Expr::UnaryOp {
                op: *op,
                operand: Box::new(scalar_to_leaf(&v)),
            };
            eval_leaf(&lowered, env, errs)
        }
        other => {
            record_refs(other, env, trace);
            eval_leaf(other, env, errs)
        }
    }
}

/// Normalize an `f64` result of an off-evaluator helper (`^`/`%`) to a typed
/// Excel error when it is non-finite (CR-02). `NaN` → `#DIV/0!`; `±inf` →
/// `#NUM!`; a finite value is wrapped unchanged.
fn finite_or_num(n: f64) -> CellValue {
    if n.is_nan() {
        CellValue::Error(ExcelError::DivZero)
    } else if !n.is_finite() {
        CellValue::Error(ExcelError::Num)
    } else {
        CellValue::Number(n)
    }
}

/// True iff `e` is a node the scalar evaluator can lower WHOLE — it contains NO
/// `Call`/`Range`/`Name` (and no `^`/`%`).
fn is_leaf_lowerable(e: &Expr) -> bool {
    match e {
        Expr::Ref(_) | Expr::Number(_) | Expr::Str(_) | Expr::Bool(_) | Expr::ErrorLit(_) => true,
        Expr::Range(_) | Expr::Name(_) | Expr::Call { .. } => false,
        Expr::BinaryOp { left, op, right } => {
            !matches!(op, BinOp::Pow) && is_leaf_lowerable(left) && is_leaf_lowerable(right)
        }
        Expr::UnaryOp { op, operand } => !matches!(op, UnOp::Percent) && is_leaf_lowerable(operand),
    }
}

/// Substitute an already-evaluated scalar [`CellValue`] back into an [`Expr`] leaf.
fn scalar_to_leaf(cv: &CellValue) -> Expr {
    match cv {
        CellValue::Number(n) => Expr::Number(*n),
        CellValue::Text(s) => Expr::Str(s.clone()),
        CellValue::Bool(b) => Expr::Bool(*b),
        CellValue::Empty => Expr::Number(0.0), // empty-cell-as-0
        CellValue::Error(e) => Expr::ErrorLit(*e),
    }
}

/// Materialize a function argument into an [`EvalValue`].
fn materialize_arg(
    a: &Expr,
    env: &CellEnv,
    errs: &HashMap<String, ExcelError>,
    trace: &mut EvalTrace,
) -> EvalValue {
    match a {
        Expr::Range(range) => {
            let current_sheet = if range.sheet.is_empty() {
                ""
            } else {
                &range.sheet
            };
            match resolve::expand_range(range, current_sheet) {
                Ok((keys, shape)) => build_range(&keys, shape, env, trace),
                Err(_) => {
                    trace.short_circuited.get_or_insert(ExcelError::Ref);
                    EvalValue::Scalar(CellValue::Error(ExcelError::Ref))
                }
            }
        }
        Expr::Name(_) => EvalValue::Scalar(CellValue::Error(ExcelError::Name)),
        scalar => EvalValue::Scalar(eval_expr(scalar, env, errs, trace)),
    }
}

/// Rebuild a shape-correct 2-D `Vec<Vec<CellValue>>` from column-major member
/// `keys` + the [`resolve::RangeShape`]. An ABSENT member is the Pitfall-5 HARD
/// error.
fn build_range(
    keys: &[String],
    shape: resolve::RangeShape,
    env: &CellEnv,
    trace: &mut EvalTrace,
) -> EvalValue {
    let rows = shape.rows as usize;
    let cols = shape.cols as usize;
    let mut out: Vec<Vec<CellValue>> = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut row_cells: Vec<CellValue> = Vec::with_capacity(cols);
        for c in 0..cols {
            let key = &keys[c * rows + r];
            let cv = match env_lookup(env, key) {
                Some(cv) => cv,
                None => {
                    trace.short_circuited.get_or_insert(ExcelError::Ref);
                    CellValue::Error(ExcelError::Ref)
                }
            };
            trace.resolved_refs.push((key.clone(), cv.clone()));
            row_cells.push(cv);
        }
        out.push(row_cells);
    }
    EvalValue::Range(out)
}

/// Walk an [`Expr`] tree and collect every cell key it DEPENDS ON, in
/// left-to-right encounter order. An [`Expr::Ref`] contributes its single key; an
/// [`Expr::Range`] contributes EVERY expanded member key (via the shared
/// [`resolve::expand_range`]) — a range edge is NOT dropped. A range that fails to
/// expand contributes nothing (it surfaces as a `#REF!` at eval time).
///
/// This is the SINGLE ref-walk shared by [`build_dag`] (which needs the dependency
/// keys to build edges) and [`record_refs`] (which additionally reads each key's
/// current env value into the trace) — so the two cannot disagree on what a cell
/// depends on.
fn collect_ref_keys(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Ref(addr) => out.push(addr.clone()),
        Expr::Range(range) => {
            let current_sheet = if range.sheet.is_empty() {
                ""
            } else {
                &range.sheet
            };
            if let Ok((keys, _shape)) = resolve::expand_range(range, current_sheet) {
                out.extend(keys);
            }
        }
        Expr::BinaryOp { left, right, .. } => {
            collect_ref_keys(left, out);
            collect_ref_keys(right, out);
        }
        Expr::UnaryOp { operand, .. } => collect_ref_keys(operand, out),
        Expr::Call { args, .. } => {
            for a in args {
                collect_ref_keys(a, out);
            }
        }
        Expr::Number(_) | Expr::Str(_) | Expr::Bool(_) | Expr::Name(_) | Expr::ErrorLit(_) => {}
    }
}

/// Record every ref the leaf-lowered [`Expr`] `e` depends on + its current env
/// value into the trace, sharing the single [`collect_ref_keys`] walk.
fn record_refs(e: &Expr, env: &CellEnv, trace: &mut EvalTrace) {
    let mut keys = Vec::new();
    collect_ref_keys(e, &mut keys);
    for key in keys {
        let cv = env_lookup(env, &key).unwrap_or(CellValue::Empty);
        trace.resolved_refs.push((key, cv));
    }
}

/// Build the per-cell dependency [`Dag`] from a pre-built IR (the served binary
/// deserializes a pre-built IR and reconstructs the DAG ONCE at load).
///
/// For each cell it adds a node, and one `add_edge(cell, dep)` per dependency key
/// the cell's formula references — collected via the SAME [`collect_ref_keys`]
/// walk the executor's trace uses (so the DAG edges and the eval-time ref-walk
/// agree, and `Range` edges are correctly included via `expand_range`). A literal
/// cell is a zero-dependency node. Absent dependency endpoints are registered as
/// nodes by [`Dag::add_edge`].
pub fn build_dag(ir: &HashMap<String, Cell>) -> Dag {
    let mut dag = Dag::new();
    for (key, cell) in ir {
        dag.add_node(key);
        if let CellExpr::Formula(e) = &cell.expr {
            let mut deps = Vec::new();
            collect_ref_keys(e, &mut deps);
            for dep in deps {
                dag.add_edge(key, &dep);
            }
        }
    }
    dag
}

/// Look a `cell_key` up in `env`, mapping the stored `JsonValue` back to a
/// [`CellValue`] via [`from_json`]. `None` iff the key is ABSENT.
fn env_lookup(env: &CellEnv, key: &str) -> Option<CellValue> {
    env.get(key).map(from_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formula::BinOp;
    use crate::range_ref::RangeRef;

    fn lit(key: &str, n: f64) -> (String, Cell) {
        (
            key.to_string(),
            Cell {
                key: key.to_string(),
                expr: CellExpr::Literal(CellValue::Number(n)),
            },
        )
    }

    fn formula(key: &str, e: Expr) -> (String, Cell) {
        (
            key.to_string(),
            Cell {
                key: key.to_string(),
                expr: CellExpr::Formula(e),
            },
        )
    }

    fn dag_of(edges: &[(&str, &[&str])]) -> Dag {
        let mut dag = Dag::new();
        for (cell, deps) in edges {
            dag.add_node(cell);
            for d in *deps {
                dag.add_edge(cell, d);
            }
        }
        dag
    }

    fn range(sheet: &str, start: &str, end: &str) -> Expr {
        Expr::Range(RangeRef {
            sheet: sheet.to_string(),
            start: start.to_string(),
            end: end.to_string(),
        })
    }

    fn call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::Call {
            name: name.to_string(),
            args,
        }
    }

    #[test]
    fn literal_is_seeded_and_readable_downstream() {
        let ir: HashMap<String, Cell> = [
            lit("S!A1", 3.0),
            formula("S!B1", Expr::Ref("S!A1".to_string())),
        ]
        .into_iter()
        .collect();
        let dag = dag_of(&[("S!A1", &[]), ("S!B1", &["S!A1"])]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        assert_eq!(out.computed.get("S!B1"), Some(&CellValue::Number(3.0)));
    }

    #[test]
    fn leaf_arithmetic_computes_via_pure_rust() {
        let ir: HashMap<String, Cell> = [
            lit("S!A1", 10.0),
            lit("S!A2", 5.0),
            formula(
                "S!C1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op: BinOp::Add,
                    right: Box::new(Expr::Ref("S!A2".to_string())),
                },
            ),
        ]
        .into_iter()
        .collect();
        let dag = dag_of(&[("S!A1", &[]), ("S!A2", &[]), ("S!C1", &["S!A1", "S!A2"])]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        assert_eq!(out.computed.get("S!C1"), Some(&CellValue::Number(15.0)));
    }

    #[test]
    fn cycle_is_one_located_finding_not_a_panic() {
        let ir: HashMap<String, Cell> = [
            formula("S!A1", Expr::Ref("S!B1".to_string())),
            formula("S!B1", Expr::Ref("S!A1".to_string())),
        ]
        .into_iter()
        .collect();
        let mut dag = Dag::new();
        dag.add_edge("S!A1", "S!B1");
        dag.add_edge("S!B1", "S!A1");
        let err = run(&ir, &dag, &CellEnv::new()).expect_err("a cycle must be Err");
        assert_eq!(err.rule, "dag/cycle");
        assert_eq!(err.severity, Severity::Error);
    }

    #[test]
    fn eval_trace_records_resolved_refs() {
        let ir: HashMap<String, Cell> = [
            lit("S!A1", 10.0),
            lit("S!A2", 5.0),
            formula(
                "S!C1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op: BinOp::Add,
                    right: Box::new(Expr::Ref("S!A2".to_string())),
                },
            ),
        ]
        .into_iter()
        .collect();
        let dag = dag_of(&[("S!A1", &[]), ("S!A2", &[]), ("S!C1", &["S!A1", "S!A2"])]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        let trace = out.traces.get("S!C1").expect("a trace for C1");
        assert_eq!(
            trace.resolved_refs,
            vec![
                ("S!A1".to_string(), CellValue::Number(10.0)),
                ("S!A2".to_string(), CellValue::Number(5.0)),
            ]
        );
    }

    #[test]
    fn error_cell_short_circuits_downstream() {
        let ir: HashMap<String, Cell> = [
            (
                "S!A1".to_string(),
                Cell {
                    key: "S!A1".to_string(),
                    expr: CellExpr::Literal(CellValue::Error(ExcelError::Ref)),
                },
            ),
            formula(
                "S!B1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op: BinOp::Add,
                    right: Box::new(Expr::Number(1.0)),
                },
            ),
        ]
        .into_iter()
        .collect();
        let dag = dag_of(&[("S!A1", &[]), ("S!B1", &["S!A1"])]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        assert_eq!(
            out.computed.get("S!B1"),
            Some(&CellValue::Error(ExcelError::Ref))
        );
    }

    #[test]
    fn sum_range_1d_column_major() {
        let ir: HashMap<String, Cell> = [
            lit("S!B2", 10.0),
            lit("S!B3", 20.0),
            lit("S!B4", 30.0),
            formula("S!C1", call("SUM", vec![range("S", "B2", "B4")])),
        ]
        .into_iter()
        .collect();
        let dag = dag_of(&[
            ("S!B2", &[]),
            ("S!B3", &[]),
            ("S!B4", &[]),
            ("S!C1", &["S!B2", "S!B3", "S!B4"]),
        ]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        assert_eq!(out.computed.get("S!C1"), Some(&CellValue::Number(60.0)));
    }

    #[test]
    fn nested_call_in_binary_op() {
        let ir: HashMap<String, Cell> = [
            lit("S!A1", 1.0),
            lit("S!A2", 2.0),
            lit("S!A3", 3.0),
            lit("S!B1", 4.567),
            formula(
                "S!C1",
                Expr::BinaryOp {
                    left: Box::new(call("SUM", vec![range("S", "A1", "A3")])),
                    op: BinOp::Add,
                    right: Box::new(call(
                        "ROUND",
                        vec![Expr::Ref("S!B1".to_string()), Expr::Number(2.0)],
                    )),
                },
            ),
        ]
        .into_iter()
        .collect();
        let dag = dag_of(&[
            ("S!A1", &[]),
            ("S!A2", &[]),
            ("S!A3", &[]),
            ("S!B1", &[]),
            ("S!C1", &["S!A1", "S!A2", "S!A3", "S!B1"]),
        ]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        match out.computed.get("S!C1") {
            Some(CellValue::Number(n)) => assert!((n - 10.57).abs() < 1e-9, "got {n}"),
            other => panic!("expected Number(10.57), got {other:?}"),
        }
    }

    #[test]
    fn pow_and_percent_are_not_in_the_evaluator() {
        let ir: HashMap<String, Cell> = [
            formula(
                "S!C1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Number(2.0)),
                    op: BinOp::Pow,
                    right: Box::new(Expr::Number(3.0)),
                },
            ),
            formula(
                "S!D1",
                Expr::UnaryOp {
                    op: UnOp::Percent,
                    operand: Box::new(Expr::Number(50.0)),
                },
            ),
        ]
        .into_iter()
        .collect();
        let dag = dag_of(&[("S!C1", &[]), ("S!D1", &[])]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        assert_eq!(out.computed.get("S!C1"), Some(&CellValue::Number(8.0)));
        assert_eq!(out.computed.get("S!D1"), Some(&CellValue::Number(0.5)));
    }

    #[test]
    fn build_dag_includes_range_member_edges() {
        // SUM over a range must produce one DAG edge PER expanded member (the
        // server-side hand-rolled walk used to DROP range edges).
        let ir: HashMap<String, Cell> = [
            lit("S!B2", 10.0),
            lit("S!B3", 20.0),
            lit("S!B4", 30.0),
            formula("S!C1", call("SUM", vec![range("S", "B2", "B4")])),
        ]
        .into_iter()
        .collect();
        let dag = build_dag(&ir);
        let mut deps = dag.dependencies_of("S!C1").to_vec();
        deps.sort();
        assert_eq!(
            deps,
            vec!["S!B2".to_string(), "S!B3".to_string(), "S!B4".to_string()],
            "every range member is a DAG edge (not dropped)"
        );
        // The built DAG drives a correct topo run.
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        assert_eq!(out.computed.get("S!C1"), Some(&CellValue::Number(60.0)));
    }

    #[test]
    fn build_dag_ref_edges_drive_a_correct_run() {
        let ir: HashMap<String, Cell> = [
            lit("S!A1", 3.0),
            formula(
                "S!B1",
                Expr::BinaryOp {
                    left: Box::new(Expr::Ref("S!A1".to_string())),
                    op: BinOp::Add,
                    right: Box::new(Expr::Number(1.0)),
                },
            ),
        ]
        .into_iter()
        .collect();
        let dag = build_dag(&ir);
        assert_eq!(dag.dependencies_of("S!B1"), &["S!A1".to_string()]);
        let out = run(&ir, &dag, &CellEnv::new()).expect("no cycle");
        assert_eq!(out.computed.get("S!B1"), Some(&CellValue::Number(4.0)));
    }

    #[test]
    fn coil_band_ceiling_reconciles_700() {
        // CEILING(C6 * C17, C18): C6=666, C17=1.05, C18=50 → 700.
        let ir: HashMap<String, Cell> = [
            formula(
                "5_Quantities!C8",
                call(
                    "CEILING",
                    vec![
                        Expr::BinaryOp {
                            left: Box::new(Expr::Ref("5_Quantities!C6".to_string())),
                            op: BinOp::Mul,
                            right: Box::new(Expr::Ref("2_Constants!C17".to_string())),
                        },
                        Expr::Ref("2_Constants!C18".to_string()),
                    ],
                ),
            ),
            lit("2_Constants!C17", 1.05),
            lit("2_Constants!C18", 50.0),
        ]
        .into_iter()
        .collect();
        let mut dag = Dag::new();
        dag.add_node("5_Quantities!C6");
        dag.add_node("2_Constants!C17");
        dag.add_node("2_Constants!C18");
        dag.add_edge("5_Quantities!C8", "5_Quantities!C6");
        dag.add_edge("5_Quantities!C8", "2_Constants!C17");
        dag.add_edge("5_Quantities!C8", "2_Constants!C18");
        let seed = CellEnv::new().seed_cell("5_Quantities!C6", &CellValue::Number(666.0));
        let out = run(&ir, &dag, &seed).expect("no cycle");
        assert_eq!(
            out.computed.get("5_Quantities!C8"),
            Some(&CellValue::Number(700.0))
        );
    }
}
