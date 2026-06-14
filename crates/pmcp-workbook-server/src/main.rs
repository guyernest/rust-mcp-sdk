//! Thin `#[tokio::main]` shim for the `pmcp-workbook-server` binary.
//!
//! All assembly + serving logic lives in the library ([`pmcp_workbook_server::run`])
//! so it stays unit-testable. This shim parses [`pmcp_workbook_server::Args`] from
//! the CLI and delegates — no business logic, no `process::exit`. The non-zero
//! exit + the legible stderr error come from returning `run()`'s [`RunError`]
//! through `#[tokio::main]`'s `Result<(), Box<dyn Error>>`, exactly as
//! `pmcp-sql-server` does.
//!
//! [`RunError`]: pmcp_workbook_server::RunError

use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = pmcp_workbook_server::Args::parse();
    pmcp_workbook_server::run(args).await?;
    Ok(())
}
