//! Reconcile stage — grade computed outputs against the cached oracle.
//!
//! Penny-reconciles the executor's outputs against the workbook's cached cell
//! values. Anchored on the runtime's `rounding` helpers (re-used from
//! `pmcp-workbook-runtime`): a `RoundingBoundary` classification fires ONLY when
//! the deciding cell's `Expr` contains a `ROUND`/`ROUNDUP`/`CEILING` call AND the
//! operand sits within epsilon — a `delta.abs()` short-circuit is forbidden and
//! grep-gated. Named-output mismatch = ERROR; helper-cell mismatch = WARNING
//! (D-03). Wave 1 ships a typed stub; Plan 06 fills the body.

use crate::error::CompileError;

/// Reconcile computed outputs against the cached oracle.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 06 wires the
/// operand-anchored classifier here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until reconcile is wired.
pub fn reconcile() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("reconcile::reconcile"))
}
