//! Example 57: Tool Middleware with OAuth Token Injection
//!
//! Demonstrates using tool middleware for cross-cutting concerns, with a focus
//! on OAuth token pass-through from MCP server authentication to backend data systems.
//!
//! **Use Case**: Many MCP servers authenticate users with OAuth, then need to pass
//! those tokens to backend data systems. Without middleware, every tool must manually
//! extract and pass tokens. With middleware, token injection is centralized.
//!
//! **Before**: Repetitive token extraction in every tool (100+ lines)
//! ```ignore
//! impl ToolHandler for DatabaseQueryTool {
//!     async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
//!         let token = extract_oauth_token(&extra)?; // Repeated everywhere
//!         self.db_client.with_auth(token).query(args).await
//!     }
//! }
//! ```
//!
//! **After**: Centralized OAuth injection via middleware (clean and DRY)
//! ```ignore
//! impl ToolHandler for DatabaseQueryTool {
//!     async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
//!         let token = extra.get_metadata("oauth_token").unwrap(); // Injected by middleware
//!         self.db_client.with_auth(token).query(args).await
//!     }
//! }
//! ```

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::tool_middleware::{ToolContext, ToolMiddleware};
use pmcp::{Result, ToolHandler};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

/// OAuth middleware that injects tokens into tool execution context.
///
/// **Best Practice**: Extract OAuth token from `auth_context` (set by transport layer
/// during authentication) rather than maintaining a separate token store. This example
/// shows both approaches for educational purposes:
///
/// 1. **Recommended**: Extract from `extra.auth_context.token` (source of truth)
/// 2. **Fallback**: Session-based token store (for demo/testing)
///
/// In production:
/// - Transport layer validates OAuth and sets `auth_context` in `RequestHandlerExtra`
/// - Middleware extracts token from `auth_context` and injects into metadata
/// - Tools read from metadata for backend system authentication
/// - No separate token store needed (reduces duplication)
struct OAuthInjectionMiddleware {
    /// Fallback token store for demo purposes (session ID → OAuth token)
    /// In production, prefer extracting from auth_context instead
    token_store: Arc<parking_lot::RwLock<HashMap<String, String>>>,
}

impl OAuthInjectionMiddleware {
    fn new() -> Self {
        Self {
            token_store: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    /// Register a token for a session (simulates OAuth flow for demo)
    /// In production, the transport layer sets auth_context with the token
    fn register_token(&self, session_id: impl Into<String>, token: impl Into<String>) {
        self.token_store
            .write()
            .insert(session_id.into(), token.into());
    }
}

#[async_trait]
impl ToolMiddleware for OAuthInjectionMiddleware {
    async fn on_request(
        &self,
        tool_name: &str,
        _args: &mut Value,
        extra: &mut RequestHandlerExtra,
        context: &ToolContext,
    ) -> Result<()> {
        tracing::info!(
            "OAuthMiddleware: Injecting token for tool '{}' (request: {})",
            tool_name,
            context.request_id
        );

        // **Best Practice**: Extract token from auth_context (source of truth)
        // The transport layer validates OAuth and sets auth_context with the token
        let token = if let Some(auth_ctx) = &extra.auth_context {
            if let Some(token) = &auth_ctx.token {
                tracing::info!(
                    "OAuthMiddleware: Token extracted from auth_context (subject: {})",
                    auth_ctx.subject
                );
                Some(token.clone())
            } else {
                tracing::warn!("OAuthMiddleware: auth_context present but no token");
                None
            }
        } else {
            // Fallback: Look up token in session store (for demo/testing)
            let session_id = context.session_id.as_deref().unwrap_or("default-session");
            if let Some(token) = self.token_store.read().get(session_id) {
                tracing::info!(
                    "OAuthMiddleware: Token retrieved from fallback store (session: {})",
                    session_id
                );
                Some(token.clone())
            } else {
                tracing::warn!(
                    "OAuthMiddleware: No token in auth_context or store (session: {})",
                    session_id
                );
                None
            }
        };

        // Inject token into metadata for tools to consume
        if let Some(token) = token {
            extra.set_metadata("oauth_token".to_string(), token);
        }

        Ok(())
    }

    fn priority(&self) -> i32 {
        // Run early (low priority number) so token is available for other middleware
        10
    }
}

/// Logging middleware that tracks tool execution.
///
/// Demonstrates multiple middleware working together.
struct ToolLoggingMiddleware;

#[async_trait]
impl ToolMiddleware for ToolLoggingMiddleware {
    async fn on_request(
        &self,
        tool_name: &str,
        args: &mut Value,
        _extra: &mut RequestHandlerExtra,
        context: &ToolContext,
    ) -> Result<()> {
        tracing::info!(
            "ToolLogger: Tool '{}' called (request: {})\n  Args: {}",
            tool_name,
            context.request_id,
            serde_json::to_string_pretty(args).unwrap_or_else(|_| "invalid".to_string())
        );
        Ok(())
    }

    async fn on_response(
        &self,
        tool_name: &str,
        result: &mut Result<Value>,
        context: &ToolContext,
    ) -> Result<()> {
        match result {
            Ok(value) => {
                tracing::info!(
                    "ToolLogger: Tool '{}' succeeded (request: {})\n  Result: {}",
                    tool_name,
                    context.request_id,
                    serde_json::to_string_pretty(value).unwrap_or_else(|_| "invalid".to_string())
                );
            },
            Err(e) => {
                tracing::error!(
                    "ToolLogger: Tool '{}' failed (request: {})\n  Error: {}",
                    tool_name,
                    context.request_id,
                    e
                );
            },
        }
        Ok(())
    }

    async fn on_error(
        &self,
        tool_name: &str,
        error: &pmcp::Error,
        context: &ToolContext,
    ) -> Result<()> {
        tracing::error!(
            "ToolLogger: Tool '{}' error handler (request: {})\n  Error: {}",
            tool_name,
            context.request_id,
            error
        );
        Ok(())
    }

    fn priority(&self) -> i32 {
        // Run late (high priority number) so we log after other middleware
        90
    }
}

/// Database query tool that uses injected OAuth token.
///
/// This tool demonstrates how to consume the OAuth token injected by middleware.
struct DatabaseQueryTool {
    // In a real application, this would be a database client
    database_name: String,
}

impl DatabaseQueryTool {
    fn new(database_name: impl Into<String>) -> Self {
        Self {
            database_name: database_name.into(),
        }
    }
}

#[async_trait]
impl ToolHandler for DatabaseQueryTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Extract the OAuth token injected by middleware
        let token = extra.get_metadata("oauth_token").ok_or_else(|| {
            pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                "OAuth token not found - authentication required",
            )
        })?;

        // Extract query from args
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("SELECT * FROM users");

        // In production: Use token to authenticate with backend database
        // db_client.with_auth(token).query(query).await
        tracing::info!(
            "DatabaseQueryTool: Executing query on '{}' with OAuth token '{}'",
            self.database_name,
            token
        );

        // Simulate database response
        Ok(json!({
            "database": self.database_name,
            "query": query,
            "authenticated_with": token,
            "results": [
                {"id": 1, "name": "Alice", "role": "admin"},
                {"id": 2, "name": "Bob", "role": "user"},
            ],
            "row_count": 2
        }))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        Some(pmcp::types::ToolInfo::new(
            "query_database",
            Some(format!(
                "Query the {} database using OAuth authentication",
                self.database_name
            )),
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "SQL query to execute"
                    }
                },
                "required": ["query"]
            }),
        ))
    }
}

/// API call tool that uses injected OAuth token.
///
/// Demonstrates token reuse across different tool types.
struct ApiCallTool {
    api_name: String,
}

impl ApiCallTool {
    fn new(api_name: impl Into<String>) -> Self {
        Self {
            api_name: api_name.into(),
        }
    }
}

#[async_trait]
impl ToolHandler for ApiCallTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // Extract the OAuth token injected by middleware
        let token = extra.get_metadata("oauth_token").ok_or_else(|| {
            pmcp::Error::protocol(
                pmcp::ErrorCode::INVALID_PARAMS,
                "OAuth token not found - authentication required",
            )
        })?;

        // Extract endpoint from args
        let endpoint = args
            .get("endpoint")
            .and_then(|v| v.as_str())
            .unwrap_or("/users");

        // In production: Use token to call backend API
        // api_client.with_auth(token).get(endpoint).await
        tracing::info!(
            "ApiCallTool: Calling {} API endpoint '{}' with OAuth token '{}'",
            self.api_name,
            endpoint,
            token
        );

        // Simulate API response
        Ok(json!({
            "api": self.api_name,
            "endpoint": endpoint,
            "authenticated_with": token,
            "response": {
                "status": "success",
                "data": {
                    "users": ["alice", "bob", "charlie"]
                }
            }
        }))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        Some(pmcp::types::ToolInfo::new(
            "call_api",
            Some(format!(
                "Call the {} API using OAuth authentication",
                self.api_name
            )),
            json!({
                "type": "object",
                "properties": {
                    "endpoint": {
                        "type": "string",
                        "description": "API endpoint to call"
                    }
                },
                "required": ["endpoint"]
            }),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Tool Middleware with OAuth Token Injection Example ===\n");

    // Create OAuth middleware and register a token
    let oauth_middleware = Arc::new(OAuthInjectionMiddleware::new());
    oauth_middleware.register_token("default-session", "oauth-token-abc123");

    // Create logging middleware
    let logging_middleware = Arc::new(ToolLoggingMiddleware);

    // Build server with tool middleware
    let server = ServerCoreBuilder::new()
        .name("oauth-demo-server")
        .version("1.0.0")
        // Add tools that consume OAuth tokens
        .tool("query_database", DatabaseQueryTool::new("production_db"))
        .tool("call_api", ApiCallTool::new("UserManagement"))
        // Add middleware (will be sorted by priority during build)
        .tool_middleware(oauth_middleware.clone())
        .tool_middleware(logging_middleware)
        .build()?;

    println!("✅ Server created with tool middleware!");
    println!("\n📊 Middleware Chain (execution order by priority):");
    println!("   1. OAuthInjectionMiddleware (priority: 10) - Injects OAuth tokens");
    println!("   2. ToolLoggingMiddleware (priority: 90) - Logs tool calls\n");

    println!("🔧 Available Tools:");
    println!("   - query_database: Query database with OAuth authentication");
    println!("   - call_api: Call API with OAuth authentication\n");

    println!("🔐 OAuth Token Flow (Best Practice):");
    println!("   1. User authenticates with MCP server via OAuth");
    println!("   2. Transport layer validates token and sets auth_context in RequestHandlerExtra");
    println!("   3. On tool execution, middleware extracts token from auth_context");
    println!("   4. Middleware injects token into RequestHandlerExtra metadata");
    println!("   5. Tools extract token from metadata (no manual auth code needed)");
    println!("   6. Tools use token to authenticate with backend systems");
    println!("\n   **Note**: This example uses fallback token store for demo purposes.");
    println!("   In production, prefer auth_context as the source of truth.\n");

    println!("💡 Benefits:");
    println!("   ✅ DRY: No repetitive token extraction in every tool");
    println!("   ✅ Centralized: OAuth logic in one place");
    println!("   ✅ Secure: Tokens never exposed in tool arguments");
    println!("   ✅ Flexible: Easy to add token refresh, validation, etc.");
    println!("   ✅ Composable: Multiple middleware can work together\n");

    println!("🎯 Example Tool Call (conceptual):");
    println!(
        r#"   tools/call {{ "name": "query_database", "arguments": {{ "query": "SELECT * FROM users" }} }}"#
    );
    println!("\n📝 Execution Flow:");
    println!("   1. Request arrives at ServerCore");
    println!("   2. OAuthInjectionMiddleware.on_request() injects token");
    println!("   3. ToolLoggingMiddleware.on_request() logs the call");
    println!("   4. DatabaseQueryTool.handle() executes with token");
    println!("   5. ToolLoggingMiddleware.on_response() logs the result");

    // Show server info
    println!("\n🚀 Server Info:");
    println!("   Name: {}", server.info().name);
    println!("   Version: {}", server.info().version);
    println!(
        "   Capabilities: {:?}",
        server.capabilities().tools.is_some()
    );

    Ok(())
}
