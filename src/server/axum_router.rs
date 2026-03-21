//! Axum Router convenience function for MCP servers.
//!
//! Provides [`router()`] and [`router_with_config()`] that return a
//! fully-configured [`axum::Router`] with DNS rebinding protection,
//! security response headers, and origin-locked CORS applied as Tower
//! Layers.
//!
//! # Example
//!
//! ```rust,no_run
//! use pmcp::server::axum_router::{router, AllowedOrigins, RouterConfig};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let server = pmcp::Server::builder()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .build()?;
//!
//! let server = Arc::new(tokio::sync::Mutex::new(server));
//!
//! // One-liner: secure MCP server with localhost protection
//! let app = router(server.clone());
//!
//! let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
//! axum::serve(listener, app).await?;
//! # Ok(())
//! # }
//! ```

use crate::server::streamable_http_server::{
    build_mcp_router, make_server_state, StreamableHttpServerConfig,
};
use crate::server::tower_layers::{DnsRebindingLayer, SecurityHeadersLayer};
use crate::server::Server;
use axum::Router;
use http::Method;
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;

// Re-export for convenience so users can import from pmcp::axum::*
pub use crate::server::tower_layers::AllowedOrigins;

/// Configuration for the MCP Axum Router.
///
/// Controls allowed origins, security headers, and underlying streamable
/// HTTP settings (sessions, middleware, etc.).
#[derive(Debug)]
pub struct RouterConfig {
    /// Allowed origins for DNS rebinding protection and CORS.
    /// Defaults to localhost aliases when `None`.
    pub allowed_origins: Option<AllowedOrigins>,
    /// Security headers configuration. Defaults to all enabled.
    pub security_headers: SecurityHeadersLayer,
    /// Streamable HTTP server configuration (sessions, middleware, etc.)
    pub server_config: StreamableHttpServerConfig,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            allowed_origins: None,
            security_headers: SecurityHeadersLayer::default(),
            server_config: StreamableHttpServerConfig::default(),
        }
    }
}

/// Create a secure MCP Axum Router with default localhost protection.
///
/// Returns an [`axum::Router`] with:
/// - DNS rebinding protection (Host + Origin validation)
/// - Security response headers (nosniff, DENY, no-store)
/// - Origin-locked CORS (no wildcard `*`)
///
/// Bind a listener and serve:
///
/// ```rust,no_run
/// # async fn example(app: axum::Router) {
/// let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
/// axum::serve(listener, app).await.unwrap();
/// # }
/// ```
pub fn router(server: Arc<tokio::sync::Mutex<Server>>) -> Router {
    router_with_config(server, RouterConfig::default())
}

/// Create a secure MCP Axum Router with explicit configuration.
///
/// Use this when deploying to production with specific allowed origins:
///
/// ```rust,no_run
/// # use pmcp::server::axum_router::*;
/// # fn example(server: std::sync::Arc<tokio::sync::Mutex<pmcp::Server>>) {
/// let app = router_with_config(server, RouterConfig {
///     allowed_origins: Some(AllowedOrigins::explicit(vec![
///         "https://myapp.example.com".to_string(),
///     ])),
///     ..Default::default()
/// });
/// # }
/// ```
pub fn router_with_config(
    server: Arc<tokio::sync::Mutex<Server>>,
    config: RouterConfig,
) -> Router {
    let allowed = config
        .allowed_origins
        .unwrap_or_else(AllowedOrigins::localhost);

    let state = make_server_state(server, config.server_config);
    let base_router = build_mcp_router(state);

    let cors = CorsLayer::new()
        .allow_origin(allowed.to_cors_allow_origin())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            "content-type".parse().expect("valid header name"),
            "accept".parse().expect("valid header name"),
            "mcp-session-id".parse().expect("valid header name"),
            "mcp-protocol-version".parse().expect("valid header name"),
            "last-event-id".parse().expect("valid header name"),
        ])
        .expose_headers([
            "mcp-session-id".parse().expect("valid header name"),
            "mcp-protocol-version".parse().expect("valid header name"),
        ])
        .max_age(Duration::from_secs(86400));

    // Layer ordering with Router::layer():
    //   Last .layer() call runs FIRST on incoming requests.
    //   Request flow: CORS (outermost, handles preflight) ->
    //                 DnsRebindingLayer (validates Host/Origin) ->
    //                 SecurityHeadersLayer (innermost, adds response headers) ->
    //                 handler
    base_router
        .layer(config.security_headers)
        .layer(DnsRebindingLayer::new(allowed))
        .layer(cors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_returns_router() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = Server::builder()
                .name("test")
                .version("0.1.0")
                .build()
                .unwrap();
            let server = Arc::new(tokio::sync::Mutex::new(server));
            let _app = router(server);
        });
    }

    #[test]
    fn test_router_with_explicit_origins() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let server = Server::builder()
                .name("test")
                .version("0.1.0")
                .build()
                .unwrap();
            let server = Arc::new(tokio::sync::Mutex::new(server));
            let _app = router_with_config(
                server,
                RouterConfig {
                    allowed_origins: Some(AllowedOrigins::explicit(vec![
                        "https://example.com".to_string(),
                    ])),
                    ..Default::default()
                },
            );
        });
    }

    #[test]
    fn test_router_config_default() {
        let config = RouterConfig::default();
        assert!(config.allowed_origins.is_none());
    }
}
