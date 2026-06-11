//! Promote-time governance gate — the build-time approval boundary.
//!
//! The build-time governance dir (reviewable on disk, never served): a
//! candidate fingerprint binds prev-hash + candidate-hash + region deltas
//! (`candidate_fingerprint`), an `ApprovalRecord`/`ApprovalCase` corpus replays
//! both versions over an AUTO-DERIVED case grid (D-09: manifest defaults + enum
//! domains + numeric boundaries, capped at small N — replaces the lighthouse's
//! BA-curated `cases.json`), and `accept` promotes into a NEW `{name}@{version}/`
//! dir without overwriting the baseline (CR-02). First version is a no-op
//! baseline (D-12). Wave 1 ships a typed stub; Plan 07 fills the body.

use crate::error::CompileError;

/// Run the promote-time governance gate on a candidate bundle.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 07 wires the
/// auto-corpus + fingerprint + accept flow here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the gate is wired.
pub fn run_gate() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("gate::run_gate"))
}
