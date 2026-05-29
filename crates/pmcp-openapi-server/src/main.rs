//! Thin `#[tokio::main]` shim for the `pmcp-openapi-server` binary.
//!
//! All assembly + serving logic lives in the library
//! ([`pmcp_openapi_server::run`]) so it stays unit-testable. This shim parses
//! [`pmcp_openapi_server::Args`] from the CLI and delegates — no business logic,
//! no `process::exit`.

use clap::Parser;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = pmcp_openapi_server::Args::parse();
    pmcp_openapi_server::run(args).await?;
    Ok(())
}
