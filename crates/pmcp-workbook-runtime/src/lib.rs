//! `workbook-runtime` — the RUNTIME-only leaf of the v0.5.0 compiler pipeline
//! (Phase 11, Plan 05 — Codex HIGH #2: the runtime/compiler LINK boundary).
//!
//! This crate holds the owned IR types (`Expr`/`BinOp`/`UnOp`/`RangeRef`/`Cell`/
//! `CellExpr`/`CellValue`), the dependency `Dag` container + `toposort`, the
//! SERVE-time topo-ordered [`run`] executor, the manifest projection model, and a
//! PURE-RUST scalar leaf evaluator ([`scalar_eval`]) that REPLACES the
//! `pmcp-code-mode` (SWC/JS) kernel on the served-binary path.
//!
//! It depends on NEITHER `umya` NOR SWC NOR `pmcp-code-mode` NOR `quick-xml` NOR
//! `zip` — making the Ph10 D-01 boundary a cargo-tree-PROVABLE LINK boundary. The
//! served binary (Plan 04) depends ONLY on this crate; `workbook-compiler`
//! re-exports these types FROM here so its public surface (and Plan 03) keeps
//! compiling unchanged.

// Compiler/clippy-enforced panic-freedom on the library value path (mirrors
// workbook-compiler). Test code constructs fixtures freely.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

/// The shared Excel error-value set.
pub mod excel_error;

/// The owned A1 range reference + the canonical `cell_key` helper.
pub mod range_ref;

/// The owned formula AST (Expr/BinOp/UnOp).
pub mod formula;

/// The located, collect-all lint-finding types (LintFinding/Severity/LintReport).
pub mod finding;

/// The pure owned dependency `Dag` container + Kahn's `toposort`.
pub mod dag;

/// Range/reference resolution PRIMITIVES (expand_range/parse_a1/split_ref).
pub mod resolve;

/// The logical manifest projection model (Manifest/CellRole/Role/InputTier/…).
pub mod manifest_model;

/// The RUNTIME-safe bundle artifact model + hashing (CellMap/BundleLock + the
/// integrity hash helpers shared by the emitter and the served integrity check).
pub mod artifact_model;

/// The dumb-byte `BundleSource` trait (local-dir + feature-gated embedded impls)
/// — raw-byte access only, so no source can bypass the shared loader's integrity
/// gate (WBSV-08/WBSV-09).
pub mod bundle_source;

/// The PURE-RUST scalar leaf evaluator that replaces the pmcp-code-mode kernel.
pub mod scalar_eval;

/// The `sheet_ir` value/eval layer + the SERVE-time executor.
pub mod sheet_ir;

/// The serve-side render layer (Phase 12): the shared, versioned
/// `LayoutDescriptor` serde shape (Plan 01) the offline emitter and the
/// writer-only serve path (Plan 02) both use.
pub mod render;

/// The shared version-changelog model (Phase 13, Plan 01): the owned serde +
/// `JsonSchema` records (`ChangeClass`/`Severity`/`OutputMeta`/`OutputDelta`/
/// `VersionChangelog`) the offline promote gate RECORDS and the served
/// `diff_version` tool SERVES. Defined HERE (not in `workbook-compiler`) because
/// the served binary deserializes it — the crate-purity invariant.
pub mod changelog;

// ---- Curated re-export surface (matches the names workbook-compiler exported) ----

pub use excel_error::ExcelError;
pub use finding::{LintFinding, LintReport, Severity};
pub use formula::{BinOp, Expr, UnOp};
pub use range_ref::{cell_key, RangeRef};

pub use dag::{toposort, Dag};
pub use resolve::{
    a1_to_zero_indexed_row_col, expand_range, parse_a1, split_ref, RangeShape, ResolveError,
    MAX_RANGE_CELLS,
};

pub use manifest_model::{
    is_strict_constant, plot3_key, CapabilityDecl, CellRole, ChangelogEntry, Dtype, GovernedDatum,
    InputTier, LoopDecl, Manifest, Role,
};

pub use artifact_model::{
    build_bundle_lock, sha256_hex, update_field, ArtifactHashes, BundleLock, CellEntry, CellMap,
};

pub use bundle_source::{BundleSource, BundleSourceError, LocalDirSource};
#[cfg(feature = "embedded")]
pub use bundle_source::EmbeddedSource;

pub use render::{CellLayout, LayoutDescriptor, SheetLayout, LAYOUT_DESCRIPTOR_VERSION};

// NOTE: `changelog::Severity` is INTENTIONALLY not re-exported at the crate root —
// `finding::Severity` (the lint-finding tier) already occupies the bare `Severity`
// name (line ~60). The changelog severity is a DISTINCT type; reach it via its
// module path `pmcp_workbook_runtime::changelog::Severity` (Rule 3 — blocking name
// collision resolved without aliasing the historical lint `Severity`).
pub use changelog::{ChangeClass, OutputDelta, OutputMeta, VersionChangelog};

pub use scalar_eval::eval_scalar;

pub use sheet_ir::eval_bridge::{env_key, from_json, percent, powf, preflight_error, to_json};
pub use sheet_ir::{
    build_dag, run as run_executor, Cell, CellEnv, CellExpr, CellValue, EvalTrace, EvalValue,
    RunResult,
};
