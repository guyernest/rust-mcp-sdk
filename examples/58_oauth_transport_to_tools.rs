//! Example 58: Complete OAuth Flow - Transport → Middleware → Tools
//!
//! Demonstrates the complete OAuth token pass-through pattern:
//! 1. Transport layer receives auth credentials (e.g., HTTP Authorization header)
//! 2. AuthProvider validates OAuth token and creates AuthContext
//! 3. Transport passes AuthContext to ServerCore.handle_request()
//! 4. Tool middleware extracts token from AuthContext
//! 5. Tools use token to authenticate with backend systems
//!
//! This example shows the **production-ready pattern** for OAuth in MCP servers.
//!
//! **Note**: This is a conceptual demonstration showing the data flow.
//! For a production HTTP server example, see examples/22_streamable_http_server_stateful.rs

use async_trait::async_trait;
use pmcp::server::auth::{AuthContext, AuthProvider};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::tool_middleware::{ToolContext, ToolMiddleware};
use pmcp::types::{ClientRequest, Request as McpRequest, RequestId};
use pmcp::{Error, Result, ToolHandler};
use serde_json::{json, Value};
use std::sync::Arc;

/// Mock OAuth provider that validates Bearer tokens.
///
/// In production, this would:
/// - Validate JWT signatures
/// - Check token expiration
/// - Verify scopes/permissions
/// - Call OAuth introspection endpoint
struct MockOAuthProvider;

#[async_trait]
impl AuthProvider for MockOAuthProvider {
    async fn validate_request(&self, auth_header: Option<&str>) -> Result<Option<AuthContext>> {
        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let token = header.trim_start_matches("Bearer ").trim();

                // Mock validation - in production, verify JWT signature, expiration, etc.
                if token.starts_with("valid-token-") {
                    let user_id = token.trim_start_matches("valid-token-");

                    let mut claims = std::collections::HashMap::new();
                    claims.insert("user_id".to_string(), json!(user_id));
                    claims.insert(
                        "email".to_string(),
                        json!(format!("user{}@example.com", user_id)),
                    );
                    claims.insert("role".to_string(), json!("developer"));

                    Ok(Some(AuthContext {
                        subject: format!("user_{}", user_id),
                        scopes: vec!["read".to_string(), "write".to_string()],
                        claims,
                        token: Some(token.to_string()),
                        client_id: Some("mcp-client".to_string()),
                        expires_at: None,
                    }))
                } else {
                    Err(Error::authentication("Invalid OAuth token"))
                }
            },
            Some(_) => Err(Error::authentication(
                "Invalid Authorization header format - expected 'Bearer <token>'",
            )),
            None => Ok(None), // No auth provided - allow unauthenticated access
        }
    }
}

/// OAuth middleware that extracts token from auth_context and injects into metadata.
///
/// This is the **recommended pattern** for OAuth in MCP servers:
/// - Transport validates OAuth → sets auth_context
/// - Middleware extracts from auth_context → injects into metadata
/// - Tools read from metadata → use for backend auth
struct OAuthTokenMiddleware;

#[async_trait]
impl ToolMiddleware for OAuthTokenMiddleware {
    async fn on_request(
        &self,
        tool_name: &str,
        _args: &mut Value,
        extra: &mut RequestHandlerExtra,
        _context: &ToolContext,
    ) -> Result<()> {
        // Extract token from auth_context (set by transport layer)
        let (token, user_id) = if let Some(auth_ctx) = &extra.auth_context {
            if let Some(token) = &auth_ctx.token {
                tracing::info!(
                    "OAuth Middleware: Injecting token for tool '{}' (user: {})",
                    tool_name,
                    auth_ctx.subject
                );
                (Some(token.clone()), Some(auth_ctx.subject.clone()))
            } else {
                tracing::warn!("OAuth Middleware: auth_context present but no token");
                (None, None)
            }
        } else {
            tracing::info!("OAuth Middleware: No auth_context - unauthenticated request");
            (None, None)
        };

        // Inject into metadata
        if let Some(token) = token {
            extra.set_metadata("oauth_token".to_string(), token);
        }
        if let Some(user_id) = user_id {
            extra.set_metadata("user_id".to_string(), user_id);
        }

        Ok(())
    }

    fn priority(&self) -> i32 {
        10 // Run early so token is available
    }
}

/// Backend API tool that uses OAuth token.
struct BackendApiTool {
    api_name: String,
}

#[async_trait]
impl ToolHandler for BackendApiTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let endpoint = args
            .get("endpoint")
            .and_then(|v| v.as_str())
            .unwrap_or("/api/data");

        // Extract OAuth token from metadata (injected by middleware)
        let response = if let Some(token) = extra.get_metadata("oauth_token") {
            let user_id = extra
                .get_metadata("user_id")
                .map(|s| s.as_str())
                .unwrap_or("unknown");

            tracing::info!(
                "BackendApiTool: Calling {} with OAuth (user: {}, endpoint: {})",
                self.api_name,
                user_id,
                endpoint
            );

            // In production: Use token to call backend API
            // let resp = reqwest::Client::new()
            //     .get(&format!("https://backend.example.com{}", endpoint))
            //     .bearer_auth(token)
            //     .send()
            //     .await?;

            json!({
                "api": self.api_name,
                "endpoint": endpoint,
                "authenticated": true,
                "user": user_id,
                "token_used": token,
                "data": {
                    "status": "success",
                    "message": "Data retrieved successfully with OAuth"
                }
            })
        } else {
            tracing::warn!("BackendApiTool: No OAuth token - returning limited data");

            json!({
                "api": self.api_name,
                "endpoint": endpoint,
                "authenticated": false,
                "data": {
                    "status": "limited",
                    "message": "Public data only (no authentication)"
                }
            })
        };

        Ok(response)
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        Some(pmcp::types::ToolInfo {
            name: "call_backend_api".to_string(),
            description: Some(format!(
                "Call {} API with OAuth authentication",
                self.api_name
            )),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "endpoint": {
                        "type": "string",
                        "description": "API endpoint to call"
                    }
                },
                "required": ["endpoint"]
            }),
        })
    }
}

/// Simulate transport layer handling a request with OAuth.
///
/// In a real HTTP transport (like StreamableHttpServer), this would:
/// 1. Extract Authorization header from HTTP request
/// 2. Validate OAuth token using AuthProvider
/// 3. Pass resulting AuthContext to ServerCore.handle_request()
async fn simulate_transport_request(
    server: &pmcp::server::core::ServerCore,
    auth_provider: &MockOAuthProvider,
    auth_header: Option<&str>,
    mcp_request: McpRequest,
    request_id: RequestId,
) -> pmcp::types::JSONRPCResponse {
    tracing::info!("=== Transport Layer: Handling Request ===");

    // Step 1: Extract and validate OAuth token
    let auth_context = match auth_provider.validate_request(auth_header).await {
        Ok(ctx) => {
            if let Some(ref auth_ctx) = ctx {
                tracing::info!(
                    "✅ OAuth validated: user={}, scopes={:?}",
                    auth_ctx.subject,
                    auth_ctx.scopes
                );
            } else {
                tracing::info!("ℹ️  No authentication provided");
            }
            ctx
        },
        Err(e) => {
            tracing::error!("❌ OAuth validation failed: {}", e);
            return pmcp::types::JSONRPCResponse {
                jsonrpc: "2.0".to_string(),
                id: request_id,
                payload: pmcp::types::jsonrpc::ResponsePayload::Error(
                    pmcp::types::jsonrpc::JSONRPCError {
                        code: -32003,
                        message: format!("Authentication failed: {}", e),
                        data: None,
                    },
                ),
            };
        },
    };

    // Step 2: Pass auth_context to ServerCore
    // **KEY STEP**: This is where OAuth flows from transport → protocol → middleware → tools
    tracing::info!("🔄 Passing request to ServerCore with auth_context");
    server
        .handle_request(request_id, mcp_request, auth_context)
        .await
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    println!("=== Complete OAuth Flow: Transport → Middleware → Tools ===\n");

    // Create OAuth provider
    let auth_provider = MockOAuthProvider;

    // Build server with OAuth middleware and tools
    let server = ServerCoreBuilder::new()
        .name("oauth-flow-demo")
        .version("1.0.0")
        .tool(
            "call_backend_api",
            BackendApiTool {
                api_name: "ProductionAPI".to_string(),
            },
        )
        .tool_middleware(Arc::new(OAuthTokenMiddleware))
        .build()?;

    println!("✅ Server created with OAuth flow!\n");
    println!("🔐 OAuth Token Flow:");
    println!("   1. Transport receives request with Authorization header");
    println!("   2. Transport validates OAuth token via AuthProvider");
    println!("   3. AuthProvider creates AuthContext with user info + token");
    println!("   4. Transport passes AuthContext to ServerCore.handle_request()");
    println!("   5. Tool middleware extracts token from AuthContext");
    println!("   6. Middleware injects token into RequestHandlerExtra metadata");
    println!("   7. Tools extract token from metadata → use for backend auth\n");

    println!("🧪 Demonstrating 3 scenarios:\n");

    // Scenario 1: Authenticated request with valid token
    println!("=== Scenario 1: Authenticated Request (Valid Token) ===\n");
    let request1 = McpRequest::Client(Box::new(ClientRequest::CallTool(
        pmcp::types::CallToolParams {
            name: "call_backend_api".to_string(),
            arguments: json!({
                "endpoint": "/api/users"
            }),
            _meta: None,
        },
    )));

    let response1 = simulate_transport_request(
        &server,
        &auth_provider,
        Some("Bearer valid-token-alice"),
        request1,
        RequestId::from(1i64),
    )
    .await;

    println!("\n📤 Response 1:");
    println!("{}\n", serde_json::to_string_pretty(&response1)?);

    // Scenario 2: Unauthenticated request (no token)
    println!("=== Scenario 2: Unauthenticated Request (No Token) ===\n");
    let request2 = McpRequest::Client(Box::new(ClientRequest::CallTool(
        pmcp::types::CallToolParams {
            name: "call_backend_api".to_string(),
            arguments: json!({
                "endpoint": "/api/public"
            }),
            _meta: None,
        },
    )));

    let response2 = simulate_transport_request(
        &server,
        &auth_provider,
        None, // No auth header
        request2,
        RequestId::from(2i64),
    )
    .await;

    println!("\n📤 Response 2:");
    println!("{}\n", serde_json::to_string_pretty(&response2)?);

    // Scenario 3: Invalid token (should fail)
    println!("=== Scenario 3: Invalid Token (Should Fail) ===\n");
    let request3 = McpRequest::Client(Box::new(ClientRequest::CallTool(
        pmcp::types::CallToolParams {
            name: "call_backend_api".to_string(),
            arguments: json!({
                "endpoint": "/api/data"
            }),
            _meta: None,
        },
    )));

    let response3 = simulate_transport_request(
        &server,
        &auth_provider,
        Some("Bearer invalid-token"),
        request3,
        RequestId::from(3i64),
    )
    .await;

    println!("\n📤 Response 3:");
    println!("{}\n", serde_json::to_string_pretty(&response3)?);

    println!("✅ Demo complete!\n");
    println!("💡 Key Takeaways:");
    println!("   1. Transport validates OAuth and creates AuthContext");
    println!("   2. AuthContext flows through ServerCore to middleware");
    println!("   3. Middleware extracts token and injects into metadata");
    println!("   4. Tools consume token from metadata (no auth logic in tools)");
    println!("   5. Invalid tokens are rejected at transport layer\n");

    Ok(())
}
