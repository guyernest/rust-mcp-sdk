//! server-common template generator
//!
//! Generates the shared HTTP bootstrap crate (~80 LOC) used by all servers.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate server-common crate
pub fn generate(workspace_dir: &Path) -> Result<()> {
    let server_common_dir = workspace_dir.join("crates/server-common");
    fs::create_dir_all(&server_common_dir).context("Failed to create server-common directory")?;

    generate_cargo_toml(&server_common_dir)?;
    generate_lib_rs(&server_common_dir)?;

    println!("  {} Generated server-common crate", "✓".green());
    Ok(())
}

fn generate_cargo_toml(server_common_dir: &Path) -> Result<()> {
    let content = r#"[package]
name = "server-common"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
pmcp = { workspace = true }
axum = { workspace = true }
tokio = { workspace = true }
tower-http = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
anyhow = { workspace = true }
"#;

    fs::write(server_common_dir.join("Cargo.toml"), content)
        .context("Failed to create server-common Cargo.toml")?;

    Ok(())
}

fn generate_lib_rs(server_common_dir: &Path) -> Result<()> {
    let content = r#"//! Shared HTTP bootstrap for all MCP servers
//!
//! This module provides production-grade HTTP server setup used by all servers.
//! Binary servers just call `run_http()` with their configured server (~6 LOC).

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::Server;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Run HTTP server with production middleware and logging
///
/// This function:
/// - Sets up production logging (env filter for runtime control)
/// - Resolves port from PORT/MCP_HTTP_PORT env vars (default: 3000)
/// - Binds to 0.0.0.0 for container compatibility
/// - Starts StreamableHttpServer in stateless mode
///
/// # Example
/// ```no_run
/// use server_common::run_http;
/// use pmcp::Server;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let server = Server::builder()
///         .name("calculator")
///         .version("1.0.0")
///         .build()?;
///     run_http(server).await
/// }
/// ```
pub async fn run_http(server: Server) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize production logging
    init_logging();

    // Resolve port (PORT > MCP_HTTP_PORT > 3000)
    let port = resolve_port();
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    tracing::info!(port = port, "Starting MCP HTTP server");

    // Wrap server in Arc<Mutex<>> for sharing
    let server = Arc::new(Mutex::new(server));

    // Create stateless configuration (no session management)
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
    };

    // Create and start the HTTP server
    let http_server = StreamableHttpServer::with_config(addr, server, config);
    let (_bound_addr, server_handle) = http_server.start().await?;

    tracing::info!("Server started on {}", addr);

    // Wait for server to finish
    server_handle.await?;

    Ok(())
}

/// Initialize production logging
fn init_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Resolve HTTP port from environment variables
///
/// Priority: PORT > MCP_HTTP_PORT > 3000 (default)
fn resolve_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or_else(|| {
            std::env::var("MCP_HTTP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
        })
        .unwrap_or(3000)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Environment variable tests require sequential execution
    // Run with: cargo test --test-threads=1

    #[test]
    fn test_resolve_port_default() {
        // Test runs with whatever env vars are set
        let port = resolve_port();
        assert!(port >= 1 && port <= 65535, "Port should be valid");
    }
}
"#;

    fs::write(server_common_dir.join("src").join("lib.rs"), content)
        .or_else(|_| {
            // Create src/ directory if it doesn't exist
            fs::create_dir(server_common_dir.join("src"))?;
            fs::write(server_common_dir.join("src").join("lib.rs"), content)
        })
        .context("Failed to create server-common lib.rs")?;

    Ok(())
}
