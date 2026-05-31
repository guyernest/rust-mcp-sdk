//! Thin `#[tokio::main]` shim for the `pmcp-sql-server` binary.
//!
//! All assembly + serving logic lives in the library ([`pmcp_sql_server::run`])
//! so it stays unit-testable. This shim parses [`pmcp_sql_server::Args`] from the
//! CLI and delegates — no business logic, no `process::exit`.

use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = pmcp_sql_server::Args::parse();
    pmcp_sql_server::run(args).await?;
    Ok(())
}
