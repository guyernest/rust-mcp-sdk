//! Stage-1 composed pass — the collect-all lint + synth + freshness + drift pass.
//!
//! A single composed pass that runs the early pipeline (lint + manifest synth +
//! freshness + drift) over the ingested workbook, collecting all findings rather
//! than failing on the first. It produces the oracle the reconcile stage grades
//! against. Wave 1 ships a typed stub; Plan 06 fills the body.

use crate::error::CompileError;

/// Run the composed stage-1 pass over the ingested workbook.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 06 wires the
/// composed lint+synth+freshness+drift pass here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until stage 1 is wired.
pub fn run_stage1() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("stage1::run_stage1"))
}
