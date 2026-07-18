//! `mem-mcp` — the dev-grade in-memory memory MCP server binary (`mem__*` tools).
//!
//! HTTP-first (D-02/D-03): built with the `http` feature it serves streamable
//! HTTP by default (the SDK owns the DNS-rebinding/CORS/security-headers stack)
//! or stdio when `--stdio` is passed; built WITHOUT `http` it serves stdio only.
//!
//! Configuration (same shape as `team-fs`):
//! - `--package` (primary): the captured [`TeamPackage`] providing roster
//!   context. A `TeamPackage` carries NO per-server memory settings.
//! - `--data-dir` / `PMCP_MEM_MCP_DATA_DIR`: reserved for a future persistent
//!   backend. The dev backend is purely in-memory, so this is logged but unused.
//! - `--port`: HTTP bind port (HTTP builds only).
//! - `--stdio`: force stdio even on an `http` build.
//!
//! The production backend uses the random-UUID id seam; conformance/examples
//! construct the backend with the deterministic (`mem-001`, …) seam directly.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use pmcp_package::TeamPackage;
use pmcp_team_servers::mem::backend::{InMemoryMemoryBackend, TeamMemoryBackend};
use pmcp_team_servers::mem::server::build_mem_mcp_server;

/// CLI arguments for the `mem-mcp` server binary.
#[derive(Debug, Parser)]
#[command(name = "mem-mcp", about = "Dev-grade in-memory memory MCP server")]
struct Args {
    /// Path to the captured TeamPackage JSON (primary config; roster context).
    #[arg(long)]
    package: PathBuf,

    /// Reserved for a future persistent backend (the dev backend is in-memory).
    #[arg(long, env = "PMCP_MEM_MCP_DATA_DIR", default_value = "./mem-mcp-data")]
    data_dir: PathBuf,

    /// HTTP bind port (ignored for a stdio build / `--stdio`).
    #[arg(long, default_value_t = 8080)]
    port: u16,

    /// Force stdio transport (default is HTTP when built with the `http` feature).
    #[arg(long, default_value_t = false)]
    stdio: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    // Load the TeamPackage for roster context (memory settings are NOT sourced here).
    let package_bytes = std::fs::read(&args.package)?;
    let package: TeamPackage = serde_json::from_slice(&package_bytes)?;
    tracing::info!(
        team = %package.name,
        version = %package.version,
        members = package.members.len(),
        data_dir = %args.data_dir.display(),
        "loaded TeamPackage roster context (mem-mcp backend is in-memory)"
    );

    // Production uses the random-UUID id seam.
    let backend = Arc::new(InMemoryMemoryBackend::new()) as Arc<dyn TeamMemoryBackend>;
    let server = build_mem_mcp_server(backend)?;

    serve(server, &args).await
}

/// Serve over HTTP by default, or stdio when requested / when built without HTTP.
#[cfg(feature = "http")]
async fn serve(server: pmcp::Server, args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    if args.stdio {
        return serve_stdio(server).await;
    }
    use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));
    let shared = Arc::new(tokio::sync::Mutex::new(server));
    let http =
        StreamableHttpServer::with_config(addr, shared, StreamableHttpServerConfig::default());
    let (bound, handle) = http.start().await?;
    tracing::info!(%bound, "mem-mcp serving streamable HTTP");
    handle.await?;
    Ok(())
}

/// Stdio-only build: the `http` feature is absent, so serve stdio unconditionally.
#[cfg(not(feature = "http"))]
async fn serve(server: pmcp::Server, _args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    serve_stdio(server).await
}

async fn serve_stdio(server: pmcp::Server) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("mem-mcp serving over stdio");
    server.run(pmcp::StdioTransport::new()).await?;
    Ok(())
}
