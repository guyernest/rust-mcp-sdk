//! Shape A pure-config OpenAPI MCP server (`pmcp-openapi-server`).
//!
//! Scaffold module surface — the `dispatch`/`assemble`/`run` pipeline is filled
//! by Tasks 2 and 3.

pub mod cli;

pub use cli::Args;

/// Scaffold entry point — replaced by the full pipeline in Task 3.
///
/// # Errors
///
/// Currently infallible; Task 3 returns the real `RunError` surface.
#[doc(hidden)]
pub async fn run(_args: Args) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
