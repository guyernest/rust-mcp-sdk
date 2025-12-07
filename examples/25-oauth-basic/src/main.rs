//! Basic OAuth authentication example for MCP servers.
//!
//! This example demonstrates:
//! - Using NoOpAuthProvider for development
//! - Protecting specific tools with authentication
//! - Handling authenticated and unauthenticated requests

use anyhow::Result;
use async_trait::async_trait;
#[cfg(feature = "streamable-http")]
use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::{
    auth::{NoOpAuthProvider, ScopeBasedAuthorizer},
    RequestHandlerExtra, Server, ServerCapabilities, ToolHandler,
};
use serde_json::{json, Value};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A public tool that doesn't require authentication
struct PublicTool;

#[async_trait]
impl ToolHandler for PublicTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "message": "This is a public tool - no authentication required",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// A protected tool that requires authentication
struct ProtectedTool;

#[async_trait]
impl ToolHandler for ProtectedTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // The auth context is available in extra when authentication is enabled
        let message = if let Some(auth_context) = extra.auth_context() {
            format!(
                "Hello authenticated user: {} with scopes: {:?}",
                auth_context.subject, auth_context.scopes
            )
        } else {
            "Authentication required but not provided".to_string()
        };

        Ok(json!({
            "message": message,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

/// An admin tool that requires specific scopes
struct AdminTool;

#[async_trait]
impl ToolHandler for AdminTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // This tool requires admin scope (configured in main)
        if let Some(auth_context) = extra.auth_context() {
            Ok(json!({
                "message": format!("Admin action executed by: {}", auth_context.subject),
                "action": args.get("action").unwrap_or(&json!("default")),
                "timestamp": chrono::Utc::now().to_rfc3339()
            }))
        } else {
            Err(pmcp::Error::protocol(
                pmcp::ErrorCode::AUTHENTICATION_REQUIRED,
                "Admin access required",
            ))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Only initialize logging if not in stdio mode to avoid interfering with protocol
    let args: Vec<String> = std::env::args().collect();
    let is_stdio = args.get(1).map(|s| s == "stdio").unwrap_or(false);

    if !is_stdio {
        // Initialize logging to stderr for non-stdio modes
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .with_env_filter(
                tracing_subscriber::EnvFilter::from_default_env()
                    .add_directive("oauth_basic=info".parse()?)
                    .add_directive("pmcp=info".parse()?),
            )
            .init();
    }

    // Create a simple auth provider for development
    // In production, you would use ProxyProvider or a custom implementation
    let auth_provider = NoOpAuthProvider;

    // Create a scope-based authorizer that defines access rules
    let authorizer = ScopeBasedAuthorizer::new()
        // Public tool - no scopes required (anyone can access)
        .require_scopes("public_info", Vec::<String>::new())
        // Protected tool - requires basic read scope
        .require_scopes("protected_data", vec!["read".to_string()])
        // Admin tool - requires admin scope
        .require_scopes("admin_action", vec!["admin".to_string()])
        // Default for any other tools
        .default_scopes(vec!["read".to_string()]);

    // Build the server with authentication
    let server = Server::builder()
        .name("oauth-basic-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        // Add the authentication provider
        .auth_provider(auth_provider)
        // Add the tool authorizer for fine-grained access control
        .tool_authorizer(authorizer)
        // Register tools
        .tool("public_info", PublicTool)
        .tool("protected_data", ProtectedTool)
        .tool("admin_action", AdminTool)
        .build()?;

    // Run based on command line arguments
    match args.get(1).map(|s| s.as_str()) {
        Some("stdio") => {
            server.run_stdio().await?;
        },
        Some("http") => {
            let port = args
                .get(2)
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or(8080);

            eprintln!("Starting OAuth MCP server on HTTP port {}", port);
            eprintln!("Server endpoint: http://0.0.0.0:{}/", port);
            eprintln!("");
            eprintln!("Connect with MCP client using:");
            eprintln!("  URL: http://localhost:{}/", port);
            eprintln!("");
            eprintln!("Available tools:");
            eprintln!("  - public_info: No authentication required");
            eprintln!("  - protected_data: Requires 'read' scope (NoOpAuthProvider grants all)");
            eprintln!("  - admin_action: Requires 'admin' scope (NoOpAuthProvider grants all)");

            #[cfg(feature = "streamable-http")]
            {
                // Wrap server in Arc<Mutex<>> for HTTP transport
                let server = Arc::new(Mutex::new(server));

                // Configure the HTTP server address
                let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);

                // Create the streamable HTTP server
                let http_server = StreamableHttpServer::new(addr, server);

                // Start the server
                let (bound_addr, server_handle) = http_server.start().await?;

                eprintln!("Server successfully started on: http://{}", bound_addr);
                eprintln!("");
                eprintln!("For remote access, use your public IP:");
                eprintln!("  http://<your-public-ip>:{}/", port);

                // Wait for the server to complete
                server_handle.await?;
            }

            #[cfg(not(feature = "streamable-http"))]
            {
                eprintln!("Error: This binary was not compiled with streamable-http support");
                eprintln!("Please rebuild with: cargo build --package oauth-basic --features streamable-http");
                std::process::exit(1);
            }
        },
        _ => {
            eprintln!("Usage: {} <stdio|http> [port]", args[0]);
            eprintln!("");
            eprintln!("Examples:");
            eprintln!("  {} stdio              # Run on stdio", args[0]);
            eprintln!("  {} http               # Run on HTTP port 8080", args[0]);
            eprintln!("  {} http 3000          # Run on HTTP port 3000", args[0]);
            std::process::exit(1);
        },
    }

    Ok(())
}
