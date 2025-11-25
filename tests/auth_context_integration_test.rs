//! Integration test for OAuth `auth_context` pass-through from transport to tools.
//!
//! This test verifies the complete data flow:
//! 1. Transport layer validates OAuth and creates `AuthContext`
//! 2. `AuthContext` is passed to `ServerCore.handle_request()`
//! 3. Tool middleware extracts token from `AuthContext`
//! 4. Middleware injects token into `RequestHandlerExtra` metadata
//! 5. Tools consume token from metadata

use async_trait::async_trait;
use pmcp::server::auth::{AuthContext, AuthProvider};
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::cancellation::RequestHandlerExtra;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::tool_middleware::{ToolContext, ToolMiddleware};
use pmcp::types::{CallToolParams, ClientRequest, Request, RequestId};
use pmcp::{Error, Result, ToolHandler};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock OAuth provider for testing.
struct TestOAuthProvider;

#[async_trait]
impl AuthProvider for TestOAuthProvider {
    async fn validate_request(&self, auth_header: Option<&str>) -> Result<Option<AuthContext>> {
        match auth_header {
            Some(header) if header.starts_with("Bearer ") => {
                let token = header.trim_start_matches("Bearer ");

                if token.starts_with("test-token-") {
                    let user_id = token.trim_start_matches("test-token-");

                    let mut claims = HashMap::new();
                    claims.insert("user_id".to_string(), json!(user_id));

                    Ok(Some(AuthContext {
                        subject: format!("test_user_{}", user_id),
                        scopes: vec!["read".to_string()],
                        claims,
                        token: Some(token.to_string()),
                        client_id: Some("test-client".to_string()),
                        expires_at: None,
                    }))
                } else {
                    Err(Error::authentication("Invalid test token"))
                }
            },
            _ => Ok(None),
        }
    }
}

/// Test middleware that extracts token from `auth_context` and tracks execution.
#[derive(Clone)]
struct TestAuthMiddleware {
    extraction_log: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ToolMiddleware for TestAuthMiddleware {
    async fn on_request(
        &self,
        tool_name: &str,
        _args: &mut Value,
        extra: &mut RequestHandlerExtra,
        _context: &ToolContext,
    ) -> Result<()> {
        let (token, user_id) = if let Some(auth_ctx) = &extra.auth_context {
            if let Some(token) = &auth_ctx.token {
                // Log that we extracted the token
                self.extraction_log.lock().await.push(format!(
                    "tool={}, user={}, token={}",
                    tool_name, auth_ctx.subject, token
                ));

                (Some(token.clone()), Some(auth_ctx.subject.clone()))
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Inject into metadata for tools
        if let Some(token) = token {
            extra.set_metadata("auth_token".to_string(), token);
        }
        if let Some(user_id) = user_id {
            extra.set_metadata("user_id".to_string(), user_id);
        }

        Ok(())
    }

    fn priority(&self) -> i32 {
        10
    }
}

/// Test tool that consumes OAuth token from metadata.
#[derive(Clone)]
struct TestAuthenticatedTool {
    tool_name: String,
    execution_log: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl ToolHandler for TestAuthenticatedTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        let token = extra
            .get_metadata("auth_token")
            .ok_or_else(|| Error::protocol(pmcp::ErrorCode::INVALID_PARAMS, "No auth token"))?;

        let user_id = extra
            .get_metadata("user_id")
            .map_or("unknown", |s| s.as_str());

        // Log execution
        self.execution_log.lock().await.push(format!(
            "tool={}, user={}, token={}",
            self.tool_name, user_id, token
        ));

        Ok(json!({
            "tool": self.tool_name,
            "user": user_id,
            "token": token,
            "args": args
        }))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        Some(pmcp::types::ToolInfo::new(
            self.tool_name.clone(),
            Some("Test authenticated tool".to_string()),
            json!({}),
        ))
    }
}

#[tokio::test]
async fn test_auth_context_flows_from_transport_to_tools() {
    // Setup
    let middleware_log = Arc::new(Mutex::new(Vec::new()));
    let tool_log = Arc::new(Mutex::new(Vec::new()));

    let middleware = TestAuthMiddleware {
        extraction_log: middleware_log.clone(),
    };

    let tool = TestAuthenticatedTool {
        tool_name: "test_tool".to_string(),
        execution_log: tool_log.clone(),
    };

    let server = ServerCoreBuilder::new()
        .name("test-server")
        .version("1.0.0")
        .tool("test_tool", tool)
        .tool_middleware(Arc::new(middleware))
        .build()
        .unwrap();

    let auth_provider = TestOAuthProvider;

    // Initialize server
    let init_request = Request::Client(Box::new(ClientRequest::Initialize(
        pmcp::types::InitializeParams {
            protocol_version: pmcp::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: pmcp::types::ClientCapabilities::default(),
            client_info: pmcp::types::Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        },
    )));

    server
        .handle_request(RequestId::from(0i64), init_request, None)
        .await;

    // Simulate transport layer validating OAuth
    let auth_header = "Bearer test-token-alice";
    let auth_context = auth_provider
        .validate_request(Some(auth_header))
        .await
        .unwrap();

    // Call tool with auth_context
    let tool_request = Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
        name: "test_tool".to_string(),
        arguments: json!({"action": "query"}),
        _meta: None,
    })));

    let response = server
        .handle_request(RequestId::from(1i64), tool_request, auth_context)
        .await;

    // Verify response succeeded
    match response.payload {
        pmcp::types::jsonrpc::ResponsePayload::Result(result) => {
            let tool_result: pmcp::types::CallToolResult = serde_json::from_value(result).unwrap();
            assert!(!tool_result.is_error);

            // Verify tool received the token
            let content = &tool_result.content[0];
            if let pmcp::types::Content::Text { text } = content {
                let data: Value = serde_json::from_str(text).unwrap();
                assert_eq!(data["user"], "test_user_alice");
                assert_eq!(data["token"], "test-token-alice");
            } else {
                panic!("Expected text content");
            }
        },
        pmcp::types::jsonrpc::ResponsePayload::Error(_) => panic!("Expected successful result"),
    }

    // Verify middleware extracted token
    let middleware_executions = middleware_log.lock().await;
    assert_eq!(middleware_executions.len(), 1);
    assert!(middleware_executions[0].contains("test_user_alice"));
    assert!(middleware_executions[0].contains("test-token-alice"));

    // Verify tool received and used token
    let tool_executions = tool_log.lock().await;
    assert_eq!(tool_executions.len(), 1);
    assert!(tool_executions[0].contains("test_user_alice"));
    assert!(tool_executions[0].contains("test-token-alice"));
}

#[tokio::test]
async fn test_missing_auth_context_fails_in_tool() {
    // Setup tool that requires auth
    let tool = TestAuthenticatedTool {
        tool_name: "secure_tool".to_string(),
        execution_log: Arc::new(Mutex::new(Vec::new())),
    };

    let server = ServerCoreBuilder::new()
        .name("test-server")
        .version("1.0.0")
        .tool("secure_tool", tool)
        .build()
        .unwrap();

    // Initialize server
    let init_request = Request::Client(Box::new(ClientRequest::Initialize(
        pmcp::types::InitializeParams {
            protocol_version: pmcp::DEFAULT_PROTOCOL_VERSION.to_string(),
            capabilities: pmcp::types::ClientCapabilities::default(),
            client_info: pmcp::types::Implementation {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            },
        },
    )));

    server
        .handle_request(RequestId::from(0i64), init_request, None)
        .await;

    // Call tool WITHOUT auth_context
    let tool_request = Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
        name: "secure_tool".to_string(),
        arguments: json!({}),
        _meta: None,
    })));

    let response = server
        .handle_request(RequestId::from(1i64), tool_request, None)
        .await;

    // Verify tool returned error due to missing auth
    match response.payload {
        pmcp::types::jsonrpc::ResponsePayload::Error(error) => {
            assert!(error.message.contains("No auth token"));
        },
        pmcp::types::jsonrpc::ResponsePayload::Result(_) => {
            panic!("Expected error due to missing auth")
        },
    }
}

#[tokio::test]
async fn test_invalid_token_rejected_at_transport() {
    // This test simulates the transport layer rejecting invalid tokens
    let auth_provider = TestOAuthProvider;

    // Try to validate an invalid token
    let result = auth_provider
        .validate_request(Some("Bearer invalid-token"))
        .await;

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Invalid test token"));
}
