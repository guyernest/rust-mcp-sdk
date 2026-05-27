//! Shape A pure-config SQL MCP server (`pmcp-sql-server`).
//!
//! This crate is the **Shape A** delivery of the v2.2 "Configuration-Only MCP
//! Servers" milestone: a standalone binary that an operator points at a
//! `config.toml` (the `[[tools]]` / `[database]` / `[code_mode]` declarations)
//! plus a schema file and runs as a production MCP server — **without writing
//! any Rust**. It assembles a [`pmcp::Server`] entirely from
//! `pmcp-server-toolkit` config primitives and the per-backend connector crates
//! (`pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`, plus
//! the `sqlite` feature's `SqliteConnector`).
//!
//! # Crate layout (lib + bin split)
//!
//! The testable assembly entry point lives here in the library ([`run`]); the
//! `pmcp-sql-server` binary (`src/main.rs`) is a thin `#[tokio::main]` shim that
//! parses CLI/env arguments and delegates to [`run`]. This split keeps the
//! server-construction logic unit-testable without spawning a process.
//!
//! # Status
//!
//! Wave 1 (Plan 85-03) scaffolds this crate so later waves have a place to
//! build. [`run`] is a documented placeholder that succeeds without starting a
//! server; Wave 2 (Plan 85-04) replaces its body with the real config-load →
//! connector-select → `pmcp::Server` assembly → transport-serve pipeline.
//!
//! # Wave 2 seams (Plan 85-04)
//!
//! - [`cli`]: the clap [`Args`] surface (`--config` / `--schema` / `--http`).

pub mod cli;

pub use cli::Args;

/// Runtime configuration for the Shape A server.
///
/// Wave 2 (Plan 85-04) expands this into the parsed CLI/env surface (config
/// path, `--schema` path, bind address, transport selection). For the Wave 1
/// scaffold it is intentionally empty so the lib/bin split compiles and the
/// binary has a concrete type to construct and pass to [`run`].
#[derive(Debug, Default, Clone)]
#[non_exhaustive]
pub struct RunConfig {}

impl RunConfig {
    /// Construct an empty [`RunConfig`].
    ///
    /// Wave 2 replaces this with parsing from CLI/env arguments.
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

/// Assemble and serve the Shape A SQL MCP server from configuration.
///
/// # Wave 1 scaffold behaviour
///
/// This is a documented placeholder: it accepts the [`RunConfig`] and returns
/// `Ok(())` without starting a server. Wave 2 (Plan 85-04) replaces the body
/// with the real pipeline — load `config.toml`, select the connector for the
/// configured backend, synthesize tools/resources/prompts via the toolkit, and
/// serve over the chosen transport.
///
/// # Errors
///
/// Returns a boxed error once Wave 2 wires in config loading and transport
/// startup (parse failures, connector construction errors, bind failures). The
/// Wave 1 scaffold never errors.
pub async fn run(_config: RunConfig) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{run, RunConfig};

    #[test]
    fn run_config_new_is_default() {
        // RunConfig is the Wave 1 scaffold surface; constructing it must not panic.
        let _cfg = RunConfig::new();
        let _default = RunConfig::default();
    }

    #[tokio::test]
    async fn run_scaffold_succeeds() {
        // Wave 1 placeholder returns Ok; Wave 2 replaces with real server startup.
        let result = run(RunConfig::new()).await;
        assert!(result.is_ok(), "Wave 1 scaffold run() must succeed");
    }
}
