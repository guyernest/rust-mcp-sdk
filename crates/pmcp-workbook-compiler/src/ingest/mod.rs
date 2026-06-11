//! Ingest stage — the umya-isolated `.xlsx` reader.
//!
//! This is the ONE module where the Excel reader (`umya-spreadsheet`) is used.
//! It lifts the workbook into an owned `WorkbookMap`/`CellRecord` model so no
//! umya type leaks across the crate boundary (the served tree never links the
//! reader — the Makefile purity gate enforces this). Wave 1 ships a typed stub;
//! Plan 02 fills the body.

use crate::error::CompileError;

/// Read and normalize the workbook at `path` into the owned cell model.
///
/// Wave 1 stub: returns [`CompileError::NotImplemented`]. Plan 02 wires the umya
/// read + owned-`WorkbookMap` projection here.
///
/// # Errors
/// Returns [`CompileError::NotImplemented`] until the ingest body is wired.
pub fn ingest(_path: &std::path::Path) -> Result<(), CompileError> {
    Err(CompileError::NotImplemented("ingest::ingest"))
}
