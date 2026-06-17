//! The compiler's typed error surface.
//!
//! [`CompileError`] is the single error type the offline pipeline returns. It is
//! `thiserror`-derived so each stage (ingest → lint → synth → DAG → reconcile →
//! emit → gate) maps its failure onto a named variant rather than a panic — the
//! crate is `#![deny(clippy::panic)]`, so stub bodies and value paths return an
//! `Err(CompileError::…)` instead of the panicking placeholder stub macros
//! (which would trip the panic-deny lint).
//!
//! The [`CompileError::NotImplemented`] variant is the Wave 1 stub sentinel:
//! every module stub and the `compile_workbook` driver return it until a
//! downstream plan fills the body. It carries a `&'static str` naming the
//! unfinished site so a caller sees *which* stage is not yet wired.

use thiserror::Error;

/// The offline compiler pipeline's error type.
///
/// Each variant corresponds to a pipeline stage so a failure is attributable to
/// the boundary it crossed. Downstream plans fill the per-stage `#[from]`/data
/// payloads; Wave 1 ships them as message-only carriers plus the
/// [`NotImplemented`](CompileError::NotImplemented) stub sentinel.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompileError {
    /// A Wave 1 stub returned because the stage is not yet implemented. The
    /// payload names the unfinished site (module or driver). This is the
    /// panic-free replacement for the placeholder stub macros under the
    /// crate's `#![deny(clippy::panic)]` posture.
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),

    /// An I/O failure reading the workbook bytes or writing bundle artifacts.
    #[error("io: {0}")]
    Io(String),

    /// The quarantined raw-parts provenance reader (quick-xml/zip) rejected the
    /// workbook (e.g. umya-fabricated identity refused — WBCO-07).
    #[error("read provenance: {0}")]
    ReadProvenance(String),

    /// The umya ingest pass failed to read or normalize the workbook.
    #[error("ingest: {0}")]
    Ingest(String),

    /// The dialect linter reported a blocking finding (whitelist/colour/layer).
    #[error("lint: {0}")]
    Lint(String),

    /// Penny-reconciliation found a named-output mismatch against the oracle.
    #[error("reconcile: {0}")]
    Reconcile(String),

    /// Bundle artifact emission failed (manifest/IR/cell-map/layout/lock).
    #[error("emit: {0}")]
    Emit(String),

    /// The promote-time governance gate refused the candidate.
    #[error("gate: {0}")]
    Gate(String),
}

impl From<std::io::Error> for CompileError {
    fn from(e: std::io::Error) -> Self {
        CompileError::Io(e.to_string())
    }
}
