//! Shape A pure-config workbook MCP server (`pmcp-workbook-server`).
//!
//! Scaffold form — the [`RunError`] enum and the [`build_server`] assemble seam
//! are in place (Task 2); the `run`/`serve`/`run_serving` pipeline lands in
//! Task 3. This split keeps the server-construction logic unit-testable without
//! spawning a process, mirroring `pmcp-sql-server`.

pub mod assemble;
pub mod cli;

pub use assemble::build_server;
pub use cli::Args;

/// Error from the Shape A pipeline.
///
/// No `Io` variant: the binary reads no config/schema files (D-03 pure CLI
/// args), so an `Io` variant would have no producer (Codex MEDIUM #5). The
/// bundle integrity-load I/O is wrapped fail-closed by the toolkit's
/// [`pmcp_server_toolkit::ToolkitError`] → [`RunError::Bundle`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RunError {
    /// Loading / integrity-verifying the bundle through the toolkit failed
    /// (fail-closed boot gate). Carries the toolkit's own error for diagnostics.
    #[error("workbook bundle load/verify failed: {0}")]
    Bundle(#[from] pmcp_server_toolkit::ToolkitError),

    /// The `--bundle-id` assertion did not match the loaded bundle's identity
    /// (D-01 fail-closed). Echoes ONLY the operator-typed `bundle_id` strings
    /// (credential-free, mirrors `pmcp-sql-server`'s `DispatchError`) — never the
    /// raw `--bundle-dir` filesystem path.
    #[error("--bundle-id '{expected}' does not match the loaded bundle id '{actual}'")]
    BundleIdMismatch {
        /// The operator-supplied expected bundle id.
        expected: String,
        /// The actual bundle id read from the loaded `BUNDLE.lock`.
        actual: String,
    },

    /// The `--http` bind address could not be parsed as a `SocketAddr`.
    #[error("invalid --http bind address '{addr}': {source}")]
    Addr {
        /// The offending address string.
        addr: String,
        /// Parse error.
        source: std::net::AddrParseError,
    },

    /// Binding / starting the streamable-HTTP listener failed.
    #[error("streamable-HTTP server failed to start: {0}")]
    Serve(#[source] pmcp::Error),

    /// The serving task ended abnormally (panic / abort).
    ///
    /// A discarded `JoinError` would hide a crashed listener — the process looks
    /// healthy while serving nothing. Propagating it here surfaces as a non-zero
    /// process exit so a supervisor restarts the binary (threat T-85-10-02).
    #[error("streamable-HTTP serving task failed: {0}")]
    Serving(#[source] tokio::task::JoinError),
}

/// Scaffold entry point — replaced by the real `run` pipeline in Task 3.
///
/// # Errors
///
/// Returns any [`RunError`] from [`build_server`] in this scaffold form.
pub async fn run(args: Args) -> Result<(), RunError> {
    let _server = build_server(&args)?;
    Ok(())
}
