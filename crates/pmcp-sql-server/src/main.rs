//! Thin `#[tokio::main]` shim for the `pmcp-sql-server` binary.
//!
//! All assembly logic lives in the library ([`pmcp_sql_server::run`]) so it
//! stays unit-testable. Wave 2 (Plan 85-04) adds CLI/env argument parsing here
//! and constructs the [`pmcp_sql_server::RunConfig`] from it.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pmcp_sql_server::run(pmcp_sql_server::RunConfig::new()).await
}
