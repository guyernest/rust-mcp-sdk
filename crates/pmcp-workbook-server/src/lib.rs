//! Shape A pure-config workbook MCP server (`pmcp-workbook-server`).
//!
//! Scaffold placeholder — the full `run`/`serve`/`run_serving` pipeline and the
//! `RunError` enum land in Task 3. This minimal form exposes the [`Args`] surface
//! so `cli.rs`'s doctest and unit tests resolve through `pmcp_workbook_server`,
//! plus a minimal `run`/`RunError` so the thin `main.rs` shim compiles. Task 3
//! replaces these with the real pipeline.

pub mod cli;

pub use cli::Args;

/// Error from the Shape A pipeline (minimal scaffold form — fleshed out in Task 3).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// Placeholder so the shim compiles before the real pipeline lands.
    #[error("pmcp-workbook-server pipeline not yet implemented")]
    NotImplemented,
}

/// Scaffold entry point — replaced by the real `run` pipeline in Task 3.
///
/// # Errors
///
/// Always returns [`RunError::NotImplemented`] in this scaffold form.
pub async fn run(_args: Args) -> Result<(), RunError> {
    Err(RunError::NotImplemented)
}
