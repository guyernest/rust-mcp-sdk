//! Sheet-IR eval bridge stage — drives the runtime's SERVE-time executor.
//!
//! Bridges the compiled DAG into the runtime's `sheet_ir` executor (re-used from
//! `pmcp-workbook-runtime`, including the `rounding` helpers; NEVER re-declared)
//! to produce the oracle values the reconcile stage grades against. Wave 1 ships
//! a typed stub; Plan 05 fills the body.

use crate::error::CompileError;

/// Evaluate the compiled IR via the runtime executor to produce oracle values.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 05 wires the
/// eval bridge here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the eval bridge is wired.
pub fn eval() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("sheet_ir::eval"))
}
