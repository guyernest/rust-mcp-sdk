//! Dialect linter stage — runs against the SDK dialect contract.
//!
//! The linter executes against the `WHITELIST`/`DialectRules`/`CandidateRole`
//! contract owned by `pmcp-workbook-dialect` (Phase 91) and emits the runtime's
//! collect-all `LintFinding`/`LintReport` types — it re-uses both contracts
//! rather than re-declaring a second copy (a second `WHITELIST` would defeat the
//! dialect crate's spec-binding drift test). Wave 1 ships a typed stub; Plan 03
//! fills the body.

use crate::error::CompileError;

/// Lint the ingested workbook against the dialect contract.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 03 wires the
/// whitelist/colour/sheet-layer linter here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the linter body is wired.
pub fn lint() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("dialect::lint"))
}
