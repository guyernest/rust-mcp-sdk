//! Promote-time governance gate — the build-time approval boundary.
//!
//! The build-time governance dir (reviewable on disk, never served): a
//! candidate fingerprint binds prev-hash + candidate-hash + region deltas
//! ([`corpus::candidate_fingerprint`]), an [`corpus::ApprovalRecord`] /
//! [`corpus::ApprovalCase`] corpus replays both versions over an AUTO-DERIVED case
//! grid (D-09: manifest defaults + enum domains + numeric boundaries, capped at
//! [`corpus::MAX_CORPUS_CASES`] — replaces the lighthouse's BA-curated checked-in
//! case file). The fingerprint-bound approval is stored atomically by
//! [`governed_artifact`]. The gate decision + accept/promote machinery lands in
//! Task 2. First version is a no-op baseline (D-12).

pub mod corpus;
pub mod governed_artifact;

use crate::error::CompileError;

/// Run the promote-time governance gate on a candidate bundle.
///
/// Task 1 wires the auto-corpus + fingerprint + atomic approval storage in
/// [`corpus`] / [`governed_artifact`]; Task 2 fills the gate decision + accept flow
/// here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the gate decision is wired.
pub fn run_gate() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("gate::run_gate"))
}
