//! Shared entry-point scaffolding for the four dev server binaries
//! (`team-fs`, `mem-mcp`, `approval-mcp`, `team-mcp`).
//!
//! Every binary loads its own config and builds its own [`pmcp::Server`], but
//! the transport-selection boilerplate — tracing init, the HTTP-first-vs-stdio
//! decision, and the streamable-HTTP construction — is identical (only the log
//! label and, for `team-mcp`, an HTTP edge-middleware config differ). Keeping it
//! here means one place owns "HTTP-first by default, stdio on `--stdio` or a
//! non-`http` build".

/// Initialize the `tracing` subscriber from `RUST_LOG` (defaulting to `info`).
///
/// Shared by every dev binary's `main`.
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}

/// Serve `server` over stdio, labelling logs with `server_name`.
///
/// # Errors
/// Propagates any transport error from [`pmcp::Server::run`].
pub async fn serve_stdio(
    server: pmcp::Server,
    server_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(server = server_name, "serving over stdio");
    server.run(pmcp::StdioTransport::new()).await?;
    Ok(())
}

/// Serve `server` over streamable HTTP on `127.0.0.1:port`, labelling logs with
/// `server_name`. Pass [`StreamableHttpServerConfig::default()`] for the plain
/// SDK stack, or a config carrying an HTTP edge-middleware chain (as `team-mcp`
/// does for its `x-pmcp-team-depth` → `_meta` map).
///
/// [`StreamableHttpServerConfig::default()`]: pmcp::server::streamable_http_server::StreamableHttpServerConfig
///
/// # Errors
/// Propagates a bind/serve error from the streamable-HTTP server.
#[cfg(feature = "http")]
pub async fn serve_streamable_http(
    server: pmcp::Server,
    server_name: &str,
    port: u16,
    config: pmcp::server::streamable_http_server::StreamableHttpServerConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use pmcp::server::streamable_http_server::StreamableHttpServer;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let shared = std::sync::Arc::new(tokio::sync::Mutex::new(server));
    let http = StreamableHttpServer::with_config(addr, shared, config);
    let (bound, handle) = http.start().await?;
    tracing::info!(%bound, server = server_name, "serving streamable HTTP");
    handle.await?;
    Ok(())
}
