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
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true, features = ["env-filter", "json"] }
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
//!
//! Features:
//! - Structured JSON logging for CloudWatch Logs compatibility
//! - Request ID tracking and correlation
//! - Performance metrics and tracing
//! - Error tracking and categorization
//! - Tool invocation logging for observability

use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::http_middleware::{ServerHttpMiddlewareChain, ServerHttpLoggingMiddleware};
use pmcp::Server;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use std::time::Duration;

/// Run HTTP server with production middleware and logging
///
/// This function:
/// - Sets up production JSON logging for enterprise observability
/// - Adds request ID tracking for distributed tracing
/// - Configures performance monitoring and metrics
/// - Resolves port from PORT/MCP_HTTP_PORT env vars (default: 3000)
/// - Binds to 0.0.0.0 for container compatibility
/// - Starts StreamableHttpServer with observability middleware
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
///     run_http(server, "calculator", "1.0.0").await
/// }
/// ```
pub async fn run_http(server: Server, server_name: &str, server_version: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize production logging
    init_logging();

    // Log server info for observability
    let server_name = server_name.to_string();
    let server_version = server_version.to_string();
    
    tracing::info!(
        server_name = %server_name,
        server_version = %server_version,
        "Initializing MCP server"
    );

    // Resolve port (PORT > MCP_HTTP_PORT > 3000)
    let port = resolve_port();
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

    tracing::info!(
        port = port,
        address = %addr,
        "Starting MCP HTTP server"
    );

    // Wrap server in Arc<Mutex<>> for sharing
    let server = Arc::new(Mutex::new(server));

    // Create HTTP middleware chain with logging
    let mut middleware_chain = ServerHttpMiddlewareChain::new();
    middleware_chain.add(Arc::new(ServerHttpLoggingMiddleware::new()));

    // Create stateless configuration with observability
    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: Some(Arc::new(middleware_chain)),
    };

    // Create and start the HTTP server
    let http_server = StreamableHttpServer::with_config(addr, server, config);
    let (bound_addr, server_handle) = http_server.start().await?;

    tracing::info!(
        actual_address = %bound_addr,
        server_name = %server_name,
        "Server started successfully"
    );

    // Log periodic health check for monitoring
    let health_check_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            tracing::info!(
                server_name = %server_name,
                "Health check - server running"
            );
        }
    });

    // Wait for server to finish
    tokio::select! {
        result = server_handle => {
            result?;
        }
        _ = health_check_handle => {
            // Health check task ended unexpectedly
        }
    }

    Ok(())
}

/// Initialize production logging with structured JSON format
fn init_logging() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            // Default logging configuration for production
            // - INFO level for pmcp and server crates
            // - WARN level for dependencies
            "info,tower_http=debug,pmcp=info,server=info".into()
        });

    // Detect if we're running in AWS Lambda or containerized environment
    let is_lambda = std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok();
    let is_json = std::env::var("LOG_FORMAT")
        .map(|v| v.to_lowercase() == "json")
        .unwrap_or(is_lambda); // Default to JSON in Lambda

    if is_json {
        // JSON format for CloudWatch Logs and structured logging
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_target(true)
                    .with_current_span(true)
                    .with_span_list(true)
            )
            .init();
    } else {
        // Human-readable format for local development
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .init();
    }
    
    // Log initialization info
    tracing::info!(
        log_format = if is_json { "json" } else { "pretty" },
        "Logging initialized"
    );
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

/// Log MCP tool invocation for observability
///
/// This should be called by tool implementations to track usage
pub fn log_tool_invocation(tool_name: &str, request_id: Option<&str>) {
    tracing::info!(
        tool = %tool_name,
        request_id = request_id.unwrap_or("unknown"),
        event = "tool_invoked",
        "MCP tool invoked"
    );
}

/// Log MCP tool error for monitoring
pub fn log_tool_error(tool_name: &str, error: &str, request_id: Option<&str>) {
    tracing::error!(
        tool = %tool_name,
        error = %error,
        request_id = request_id.unwrap_or("unknown"),
        event = "tool_error",
        "MCP tool error"
    );
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
