//! Dialect linter stage — runs against the SDK dialect contract (WBDL-03).
//!
//! The linter executes against the `WHITELIST`/`DialectRules`/`CandidateRole`
//! contract owned by `pmcp-workbook-dialect` (Phase 91) and emits the runtime's
//! collect-all `LintFinding`/`LintReport` types — it re-uses both contracts
//! rather than re-declaring a second copy (a second `WHITELIST` would defeat the
//! dialect crate's spec-binding drift test).
//!
//! # The [`CellSource`] seam
//!
//! The linter reads a synthetic, reader-free [`CellSource`] abstraction — NOT
//! 93-02's umya-produced owned cell model — so this plan stays parallel with
//! 93-02. The real owned model implements [`CellSource`] in the Plan 04 wiring;
//! tests here drive a hand-built `TestCells` double.

/// The collect-all, located lint pass over a [`CellSource`] against
/// [`DialectRules`] (WBDL-03).
pub mod linter;

// The dialect contract the linter runs against — re-exported from the dialect
// crate, NEVER re-declared (a second WHITELIST copy would defeat the dialect
// crate's spec-binding drift test).
pub use pmcp_workbook_dialect::{CandidateRole, DialectRules, WHITELIST};

// The collect-all located lint findings the linter emits — re-exported from the
// runtime, NEVER re-declared.
pub use pmcp_workbook_runtime::{LintFinding, LintReport, Severity};

// The running linter surface.
pub use linter::{
    lint, lint_colour_evidence, lint_workbook_metadata, CellSource, CellView, DefinedName,
    SheetView,
};
