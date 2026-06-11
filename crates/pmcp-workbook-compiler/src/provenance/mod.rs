//! Provenance stage — quarantined raw-parts identity reader.
//!
//! Reads the workbook's raw OOXML parts (via quick-xml/zip, confined here as a
//! `pub(crate)` reader that never enters the served tree) to assert the file was
//! authored by Excel. The anchored identity check uses `starts_with("Microsoft
//! Excel")` (not `.contains`), and a NET-NEW upgrade (WBCO-07) refuses umya's
//! FABRICATED identity (e.g. the `calcId == 122211` sentinel) with
//! `oracle/non-excel-app`. Wave 1 ships a typed stub; Plan 02 fills the body.

use crate::error::CompileError;

/// Assert the workbook's raw OOXML provenance is a genuine Excel author.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 02 wires the
/// quick-xml/zip raw-parts gate + umya-fabricated refusal here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the provenance gate is wired.
// Why: the raw-parts provenance gate is deliberately `pub(crate)` (the
// quick-xml/zip reader is never re-exported — purity boundary), so the Wave 1
// stub has no in-crate caller yet. Plan 02 wires `compile_workbook` to call it.
#[allow(dead_code)]
pub(crate) fn gate() -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("provenance::gate"))
}
