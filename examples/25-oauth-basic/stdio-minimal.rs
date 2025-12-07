use async_trait::async_trait;
use pmcp::server::auth::{NoOpAuthProvider, ScopeBasedAuthorizer};
use pmcp::{RequestHandlerExtra, Server, ServerCapabilities, ToolHandler};
use serde_json::{json, Value};

/// Public tool - no authentication required
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

/// Protected tool - requires 'read' scope
struct ProtectedTool;

#[async_trait]
impl ToolHandler for ProtectedTool {
    async fn handle(&self, _args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
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

/// Admin tool - requires 'admin' scope
struct AdminTool;

#[async_trait]
impl ToolHandler for AdminTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("default_action");

        let message = if let Some(auth_context) = extra.auth_context() {
            format!("Admin action executed by: {}", auth_context.subject)
        } else {
            "Admin access required".to_string()
        };

        Ok(json!({
            "message": message,
            "action": action,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create scope-based authorizer
    let authorizer = ScopeBasedAuthorizer::new()
        .require_scopes("public_info", Vec::<String>::new())  // No scopes required
        .require_scopes("protected_data", vec!["read".to_string()])  // Read scope required
        .require_scopes("admin_action", vec!["admin".to_string()])   // Admin scope required
        .default_scopes(vec!["mcp:tools:use".to_string()]); // Default for other tools

    // Build OAuth-enabled server
    let server = Server::builder()
        .name("oauth-basic-minimal")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .auth_provider(NoOpAuthProvider)
        .tool_authorizer(authorizer)
        .tool("public_info", PublicTool)
        .tool("protected_data", ProtectedTool)
        .tool("admin_action", AdminTool)
        .build()?;

    // Run on stdio
    server.run_stdio().await?;
    Ok(())
}
