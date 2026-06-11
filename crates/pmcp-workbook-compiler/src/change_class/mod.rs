//! Change-classification stage — diff a candidate against the prior baseline.
//!
//! Classifies the diff between a candidate and the prior accepted version into
//! the runtime's owned `ChangeClass` (re-exported from `pmcp-workbook-runtime`;
//! NEVER re-declared — the served `diff_version` tool reads the SAME enum).
//! CR-01: the classifier is symmetric (assumption involvement on EITHER side →
//! Assumption; role-flips away from Input/Output are schema changes). WBGV-02:
//! `effective_policy` derives the strongest gate policy. WBGV-03: subdag-hash IR
//! identity. Wave 1 ships a typed stub; Plan 07 fills the body.

use crate::error::CompileError;

/// Classify the diff between a candidate and the prior baseline.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 07 wires the
/// symmetric classifier + effective-policy derivation here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until classification is wired.
pub fn classify() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("change_class::classify"))
}
