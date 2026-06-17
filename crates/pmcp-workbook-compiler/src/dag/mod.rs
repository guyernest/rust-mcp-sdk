//! DAG compile stage (WBCO-03): build the dependency graph + Kahn toposort.
//!
//! Builds the runtime's owned [`graph::Dag`] from the parsed [`crate::formula`]
//! `Expr` references and orders it with [`topo::toposort`] (both re-exported from
//! `pmcp-workbook-runtime`; NEVER re-declared — no petgraph). The build path
//! operates over a SYNTHETIC [`topo::ParsedCell`] slice + a synthetic
//! defined-name table (`crate::dialect::DefinedName`) — never 93-02's owned cell
//! model — so this plan stays parallel with 93-02.
//!
//! # Typed errors, NOT lint findings (Codex MEDIUM)
//!
//! The DAG build returns a typed [`resolve::DagBuildError`] (range-too-large,
//! malformed range, unknown name, cycle) — it never pushes a `LintFinding`. The
//! linter owns the cell-addressed reporting; this layer is a pure IR→graph
//! transform with a typed failure mode.

/// The pure owned dependency-graph container (re-exported from the runtime).
pub mod graph;
/// Reference resolution + the typed [`resolve::DagBuildError`].
pub mod resolve;
/// Kahn topo-sort + cycle detection + the [`topo::build_dag`] entry point.
pub mod topo;

pub use graph::Dag;
pub use resolve::{
    collect_refs, expand_range, parse_a1, split_ref, DagBuildError, RangeShape, ResolveError,
    MAX_RANGE_CELLS,
};
pub use topo::{build_dag, toposort, ParsedCell};
