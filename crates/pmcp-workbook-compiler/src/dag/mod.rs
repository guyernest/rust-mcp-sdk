//! DAG compile stage — build the dependency graph + toposort.
//!
//! Builds the runtime's owned `Dag` and orders it with `toposort` (both
//! re-exported from `pmcp-workbook-runtime`; NEVER re-declared). Wave 1 ships a
//! typed stub; Plan 05 fills the body.

use crate::error::CompileError;

/// Build the dependency DAG from the parsed formula set.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 05 wires the
/// graph build + toposort here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the DAG build is wired.
pub fn build_dag() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("dag::build_dag"))
}
