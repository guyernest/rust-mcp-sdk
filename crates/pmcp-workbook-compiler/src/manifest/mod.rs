//! Manifest synthesis stage — colour/Guide/header heuristics → roles.
//!
//! Synthesizes the runtime's owned `Manifest` (re-exported from
//! `pmcp-workbook-runtime`; NEVER re-declared here) from the ingested cell
//! model, then ratifies it (D-04 sign-off fields + the in-repo `annotations`
//! field). Wave 1 ships typed stubs; Plan 04 fills the bodies. Every hand-built
//! `Manifest { … }` literal added downstream MUST populate `annotations`
//! (`vec![]` if none) and the `ratified*` fields, or the byte-identical re-emit
//! test fails.

use crate::error::CompileError;

/// Synthesize a manifest from the ingested cell model.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 04 wires the
/// colour/Guide/header synthesis here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until synthesis is wired.
pub fn synthesize() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("manifest::synthesize"))
}

/// Ratify a synthesized manifest (sign-off + tiering).
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 04 wires the
/// ratify pass here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until ratify is wired.
pub fn ratify() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("manifest::ratify"))
}
