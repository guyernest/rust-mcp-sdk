//! The pure owned per-cell dependency-graph container [`Dag`] — re-exported from
//! `pmcp-workbook-runtime` (the served binary's executor re-runs an
//! already-built `Dag`, so the container lives runtime-side). NEVER re-declared
//! here.

pub use pmcp_workbook_runtime::Dag;
