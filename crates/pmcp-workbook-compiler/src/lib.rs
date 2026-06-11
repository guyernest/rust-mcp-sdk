//! `pmcp-workbook-compiler` — the OFFLINE Excel→MCP compiler.
//!
//! This crate runs the build-time pipeline that turns a governed Excel workbook
//! into a tested, versioned, deterministic served bundle:
//! ingest → lint → manifest synth → formula parse → DAG compile →
//! penny-reconcile → artifact emit → promote-time gate.
//!
//! # The purity boundary (the milestone's #1 trap)
//!
//! This is the ONE crate in the workspace where the Excel reader
//! (`umya-spreadsheet`, plus its transitive `quick-xml`/`zip`) is allowed. The
//! reader is confined to the [`ingest`] and [`provenance`] modules; no umya type
//! leaks across the crate boundary, and the served-tree crates
//! (`pmcp-workbook-runtime`, `pmcp-workbook-dialect`, `pmcp-server-toolkit`)
//! NEVER link it. The Makefile `purity-check` gate POSITIVELY asserts umya IS
//! here and is ABSENT everywhere served.
//!
//! # Re-export, don't re-declare (the keystone)
//!
//! Every shared model/IR/hash/changelog/finding/rounding type is re-exported
//! from [`pmcp_workbook_runtime`] (and the dialect contract from
//! [`pmcp_workbook_dialect`]) — NEVER re-declared. A second copy of
//! `Manifest`/`ChangeClass`/`WHITELIST` would make the served loader and the
//! `diff_version` tool read a DIFFERENT definition than the compiler emits.

// Compiler/clippy-enforced panic-freedom on the library value path (copied
// verbatim from pmcp-workbook-runtime). Test code constructs fixtures freely.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

use std::path::Path;

// ---- Pipeline modules (each a compilable Wave 1 stub; downstream plans fill bodies) ----

/// The compiler's typed error surface ([`CompileError`]).
pub mod error;

/// Ingest stage — the umya-isolated `.xlsx` reader (Plan 02).
pub mod ingest;

/// Dialect linter stage — runs against the SDK dialect contract (Plan 03).
pub mod dialect;

/// Manifest synthesis stage — colour/Guide/header heuristics → roles (Plan 04).
pub mod manifest;

/// Formula parse stage — tokenize + parse to the runtime's owned `Expr` (Plan 05).
pub mod formula;

/// DAG compile stage — build the dependency graph + toposort (Plan 05).
pub mod dag;

/// Sheet-IR eval bridge — drives the runtime's SERVE-time executor (Plan 05).
pub mod sheet_ir;

/// Reconcile stage — grade computed outputs against the cached oracle (Plan 06).
pub mod reconcile;

/// Provenance stage — quarantined raw-parts identity reader (Plan 02).
pub mod provenance;

/// Artifact emission stage — write the served bundle (Plan 07).
pub mod artifact;

/// Change-classification stage — diff a candidate vs the prior baseline (Plan 07).
pub mod change_class;

/// Promote-time governance gate — the build-time approval boundary (Plan 07).
pub mod gate;

/// Stage-1 composed pass — collect-all lint + synth + freshness + drift (Plan 06).
pub mod stage1;

pub use error::CompileError;

// ---- Curated re-export surface (re-export the runtime/dialect shared types) ----
//
// These mirror the names `pmcp-workbook-runtime` exports so downstream consumers
// (and Plan 07's driver wiring) get one definition of every shared type. NEVER
// re-declare any of these as a fresh struct/enum here.

// Formula AST + Excel error value set + DAG container + executor value type.
pub use pmcp_workbook_runtime::{toposort, BinOp, CellValue, Dag, ExcelError, Expr, UnOp};

// The version-changelog module (reach `changelog::Severity` via its module path
// to preserve the runtime's `changelog::Severity` vs `finding::Severity`
// collision rule — the bare `Severity` re-exported below stays the lint tier).
pub use pmcp_workbook_runtime::changelog;
pub use pmcp_workbook_runtime::{ChangeClass, OutputDelta, OutputMeta, VersionChangelog};

// The logical manifest projection model the compiler EMITS (lives in the
// runtime; re-export, never re-declare).
pub use pmcp_workbook_runtime::{AnnotationDecl, CellRole, Dtype, InputTier, Manifest, Role};

// The collect-all located lint findings the linter emits (bare `Severity` here
// is the lint-finding tier — the changelog tier stays module-path-only above).
pub use pmcp_workbook_runtime::{LintFinding, LintReport, Severity};

// The bundle artifact model + hashing helpers (NEVER hand-roll the combined
// hash — the served loader recomputes with these).
pub use pmcp_workbook_runtime::{build_bundle_lock, fold_evidence_hash, sha256_hex, BundleLock};

// The Excel rounding helpers the reconcile classifier anchors on.
pub use pmcp_workbook_runtime::sheet_ir::rounding::{excel_ceiling, excel_round, excel_roundup};

// The dialect contract the linter runs against (re-export, never re-declare — a
// second WHITELIST copy would defeat the dialect crate's spec-binding test).
pub use pmcp_workbook_dialect::{CandidateRole, DialectRules, WHITELIST};

/// Compile a governed Excel workbook into a served bundle, promote-gating the
/// result against the prior accepted version.
///
/// This is the GENERIC driver that replaces the lighthouse's hardcoded
/// reference-manifest builder (the one surviving §5 gap — WBCO-02): the manifest
/// comes SOLELY from `manifest::synthesize` → `manifest::ratify`, never from a
/// hand-built customer-specific literal, and there is no hardcoded
/// reference-workbook-path / workflow-name const.
///
/// The shape Plan 07 wires: ingest → stage1 (lint+synth+freshness+drift) →
/// parse+DAG → reconcile (named-out=ERROR, helper=WARN) → change-class gate vs
/// the prior baseline → emit on a clean gate, writing into a new
/// `{name}@{next_version}/` dir.
///
/// # Arguments
/// * `workbook_path` — the source `.xlsx` to compile.
/// * `out_root` — the bundle output root (one `{name}@{version}/` dir per promote).
/// * `approver` — the human approver recorded in the manifest sign-off + gate.
///
/// # Errors
/// Wave 1 stub: always returns [`CompileError::NotImplemented`]. Plan 07 wires
/// the full pipeline; thereafter this returns the per-stage `CompileError`
/// variants (`Ingest`/`Lint`/`Reconcile`/`Emit`/`Gate`/…) on failure.
pub fn compile_workbook(
    _workbook_path: &Path,
    _out_root: &Path,
    _approver: &str,
) -> Result<BundleLock, CompileError> {
    Err(CompileError::NotImplemented("compile_workbook"))
}
