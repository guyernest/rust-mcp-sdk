//! Compiler-side eval bridge — drives the runtime's SERVE-time executor.
//!
//! The compiler grades its compiled IR against the cached oracle by RUNNING that
//! IR through the SAME executor the served binary uses ([`pmcp_workbook_runtime`]'s
//! `run`, re-exported here as [`run_executor`]). The executor types — `Cell`,
//! `CellExpr`, `CellEnv`, `EvalTrace`, `RunResult`, `Dag`, and the rounding helpers
//! — are RE-EXPORTED from the runtime, NEVER re-declared (a second executor would
//! defeat the whole "one definition" keystone, and the O-1 parity proof depends on
//! the compiler and the server reconciling through the IDENTICAL pure-Rust path).
//!
//! # O-1 — no SWC/JS oracle
//!
//! The runtime executor lowers leaf arithmetic through the PURE-RUST `scalar_eval`
//! (no SWC, no `pmcp-code-mode`). The compiler therefore reconciles its IR with NO
//! JS kernel on any path — the named `o1_parity_suite` ([`super::mod`] tests) is the
//! proof, not a summary note.

// Re-export the runtime executor surface the compiler-side reconcile drives. These
// are the SAME types the served binary runs — re-export, NEVER re-declare.
pub use pmcp_workbook_runtime::sheet_ir::rounding::{excel_ceiling, excel_round, excel_roundup};
pub use pmcp_workbook_runtime::{
    build_dag, run_executor, Cell, CellEnv, CellExpr, CellValue, Dag, EvalTrace, ExcelError, Expr,
    RunResult,
};

/// Run `ir` through the runtime executor with `seed` pre-loaded, returning the
/// computed `{cell_key -> CellValue}` map + per-cell [`EvalTrace`] evidence (the
/// oracle the reconcile stage grades against). A dependency cycle surfaces as the
/// runtime's located `dag/cycle` finding (boxed) — never a panic.
///
/// This is a thin alias over [`run_executor`]: it gives the compiler a stable
/// `sheet_ir::eval` entry point while keeping the executor itself in the runtime.
///
/// # Errors
/// Propagates the runtime executor's `Box<LintFinding>` (a `dag/cycle`) unchanged.
pub fn eval(
    ir: &std::collections::HashMap<String, Cell>,
    dag: &Dag,
    seed: &CellEnv,
) -> Result<RunResult, Box<pmcp_workbook_runtime::LintFinding>> {
    run_executor(ir, dag, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn eval_runs_the_runtime_executor_on_a_tiny_ir() {
        // A one-cell IR: CEILING(700, 50) = 700, run through the SHARED executor
        // (pure-Rust, no SWC). Proves the compiler-side bridge reaches the runtime.
        let mut ir: HashMap<String, Cell> = HashMap::new();
        let mut seed = CellEnv::new();
        seed = seed.seed_cell("S!A1", &CellValue::Number(700.0));
        ir.insert(
            "S!C1".to_string(),
            Cell {
                key: "S!C1".to_string(),
                expr: CellExpr::Formula(Expr::Call {
                    name: "CEILING".to_string(),
                    args: vec![Expr::Ref("S!A1".to_string()), Expr::Number(50.0)],
                }),
            },
        );
        let mut dag = Dag::new();
        dag.add_node("S!A1");
        dag.add_edge("S!C1", "S!A1");
        let out = eval(&ir, &dag, &seed).expect("acyclic");
        assert_eq!(out.computed.get("S!C1"), Some(&CellValue::Number(700.0)));
    }

    #[test]
    fn rounding_anchors_are_the_runtime_helpers() {
        // The re-exported helpers ARE the runtime's (the classifier anchors on them).
        assert_eq!(excel_round(1594.925, 2), 1594.93);
        assert_eq!(excel_roundup(3.001, 2), 3.01);
        assert_eq!(excel_ceiling(10.0, 3.0), 12.0);
    }
}
