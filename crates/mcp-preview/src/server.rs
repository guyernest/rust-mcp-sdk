//! Preview server implementation

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::handlers;
use crate::proxy::McpProxy;

/// Configuration for the preview server
#[derive(Debug, Clone)]
pub struct PreviewConfig {
    /// URL of the target MCP server
    pub mcp_url: String,
    /// Port for the preview server
    pub port: u16,
    /// Initial tool to select
    pub initial_tool: Option<String>,
    /// Initial theme (light/dark)
    pub theme: String,
    /// Initial locale
    pub locale: String,
}

impl Default for PreviewConfig {
    fn default() -> Self {
        Self {
            mcp_url: "http://localhost:3000".to_string(),
            port: 8765,
            initial_tool: None,
            theme: "light".to_string(),
            locale: "en-US".to_string(),
        }
    }
}

/// Shared application state
pub struct AppState {
    pub config: PreviewConfig,
    pub proxy: McpProxy,
}

/// MCP Preview Server
pub struct PreviewServer;

impl PreviewServer {
    /// Start the preview server
    pub async fn start(config: PreviewConfig) -> Result<()> {
        let proxy = McpProxy::new(&config.mcp_url);

        let state = Arc::new(AppState {
            config: config.clone(),
            proxy,
        });

        // Build CORS layer
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        // Build router
        let app = Router::new()
            // Main preview page
            .route("/", get(handlers::page::index))
            // API endpoints - tools
            .route("/api/config", get(handlers::api::get_config))
            .route("/api/tools", get(handlers::api::list_tools))
            .route("/api/tools/call", post(handlers::api::call_tool))
            // API endpoints - resources
            .route("/api/resources", get(handlers::api::list_resources))
            .route("/api/resources/read", get(handlers::api::read_resource))
            // API endpoints - session management
            .route("/api/reconnect", post(handlers::api::reconnect))
            .route("/api/status", get(handlers::api::status))
            // Static assets
            .route("/assets/{*path}", get(handlers::assets::serve))
            // WebSocket for live updates
            .route("/ws", get(handlers::websocket::handler))
            .layer(cors)
            .with_state(state);

        let addr = SocketAddr::from(([127, 0, 0, 1], config.port));

        println!();
        println!("\x1b[1;36m╔══════════════════════════════════════════════════╗\x1b[0m");
        println!("\x1b[1;36m║          MCP Apps Preview Server                 ║\x1b[0m");
        println!("\x1b[1;36m╠══════════════════════════════════════════════════╣\x1b[0m");
        println!(
            "\x1b[1;36m║\x1b[0m  Preview:    \x1b[1;33mhttp://localhost:{:<5}\x1b[0m             \x1b[1;36m║\x1b[0m",
            config.port
        );
        println!(
            "\x1b[1;36m║\x1b[0m  MCP Server: \x1b[1;32m{:<30}\x1b[0m   \x1b[1;36m║\x1b[0m",
            truncate_url(&config.mcp_url, 30)
        );
        println!("\x1b[1;36m╠══════════════════════════════════════════════════╣\x1b[0m");
        println!(
            "\x1b[1;36m║\x1b[0m  Press Ctrl+C to stop                           \x1b[1;36m║\x1b[0m"
        );
        println!("\x1b[1;36m╚══════════════════════════════════════════════════╝\x1b[0m");
        println!();

        info!("Preview server starting on http://{}", addr);

        let listener = TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

fn truncate_url(url: &str, max_len: usize) -> String {
    if url.len() <= max_len {
        url.to_string()
    } else {
        format!("{}...", &url[..max_len - 3])
    }
}
