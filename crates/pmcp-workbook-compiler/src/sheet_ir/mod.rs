//! Sheet-IR eval bridge stage — drives the runtime's SERVE-time executor.
//!
//! Bridges the compiled DAG into the runtime's `sheet_ir` executor (re-used from
//! [`pmcp_workbook_runtime`], including the `rounding` helpers; NEVER re-declared)
//! to produce the oracle values the [`crate::reconcile`] stage grades against.
//!
//! The executor + rounding TYPES come from the runtime — this module only adds the
//! compiler-side [`eval_bridge`] entry point (`eval`) and re-exports the surface the
//! reconcile classifier anchors on. `loop_exec.rs` / `RoomAggregator` are
//! DEFERRED — not lifted here.

pub mod eval_bridge;

// The compiler-side run entry point + the re-exported runtime executor/rounding
// surface the reconcile stage consumes (re-export, never re-declare).
pub use eval_bridge::{
    build_dag, eval, excel_ceiling, excel_round, excel_roundup, run_executor, Cell, CellEnv,
    CellExpr, CellValue, Dag, EvalTrace, ExcelError, Expr, RunResult,
};
