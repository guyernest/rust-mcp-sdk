//! Builder-scoped middleware executor for workflow registration.
//!
//! This module provides a `MiddlewareExecutor` implementation that works during
//! the builder phase, allowing workflows to be registered with middleware support
//! before the server is fully built.

use crate::error::Result;
use crate::server::cancellation::RequestHandlerExtra;
use crate::server::middleware_executor::MiddlewareExecutor;
use crate::server::tool_middleware::{ToolContext, ToolMiddleware};
use crate::server::ToolHandler;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Middleware executor that captures builder state for workflow tool execution.
///
/// This executor is created during the builder phase and captures:
/// - The current set of registered tools
/// - The current middleware chain
///
/// When workflows execute tools, this executor applies the middleware chain
/// exactly as the final `ServerCore` would, ensuring consistent behavior.
///
/// # Architecture
///
/// ```text
/// ServerBuilder
///   ├── tools: HashMap<String, Arc<dyn ToolHandler>>
///   ├── tool_middlewares: Vec<Arc<dyn ToolMiddleware>>
///   └── .prompt_workflow()
///         ↓
///   Creates BuilderMiddlewareExecutor (captures tools + middleware)
///         ↓
///   WorkflowPromptHandler::with_middleware_executor()
///         ↓
///   Workflow executes → Middleware applied ✅
/// ```
///
/// # Example
///
/// This type is used internally by `ServerBuilder::prompt_workflow()`:
///
/// ```rust,ignore
/// let server = Server::builder()
///     .tool("my_tool", MyTool)
///     .tool_middleware(Arc::new(OAuthMiddleware))
///     .prompt_workflow(my_workflow)?  // ← Uses BuilderMiddlewareExecutor
///     .build()?;
/// ```
#[derive(Clone)]
pub struct BuilderMiddlewareExecutor {
    /// Registered tool handlers (captured from builder)
    #[allow(dead_code)] // Tools are accessed via get() method
    tools: HashMap<String, Arc<dyn ToolHandler>>,
    /// Middleware chain (captured from builder)
    middlewares: Vec<Arc<dyn ToolMiddleware>>,
}

impl std::fmt::Debug for BuilderMiddlewareExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuilderMiddlewareExecutor")
            .field("tools", &self.tools.keys().collect::<Vec<_>>())
            .field("middlewares_count", &self.middlewares.len())
            .finish()
    }
}

impl BuilderMiddlewareExecutor {
    /// Create a new builder-scoped middleware executor.
    ///
    /// Captures the current state of tools and middleware from the builder.
    pub fn new(
        tools: HashMap<String, Arc<dyn ToolHandler>>,
        middlewares: Vec<Arc<dyn ToolMiddleware>>,
    ) -> Self {
        Self { tools, middlewares }
    }
}

#[async_trait]
impl MiddlewareExecutor for BuilderMiddlewareExecutor {
    async fn execute_tool_with_middleware(
        &self,
        tool_name: &str,
        mut args: Value,
        mut extra: RequestHandlerExtra,
    ) -> Result<Value> {
        // Debug: Check auth_context at middleware executor entry
        tracing::debug!(
            "BuilderMiddlewareExecutor.execute_tool_with_middleware() - Entry: auth_context present: {}, has_token: {}, tool: {}",
            extra.auth_context.is_some(),
            extra.auth_context.as_ref().and_then(|ctx| ctx.token.as_ref()).is_some(),
            tool_name
        );

        // Get the tool handler
        let handler = self
            .tools
            .get(tool_name)
            .ok_or_else(|| crate::Error::internal(format!("Tool '{}' not found", tool_name)))?;

        // Create tool context for middleware
        let context = ToolContext::new(tool_name, &extra.request_id);

        // Process request through middleware chain
        for middleware in &self.middlewares {
            // Debug: Log before middleware processing
            tracing::debug!(
                "BuilderMiddlewareExecutor - Before middleware: auth_context present: {}, has_token: {}",
                extra.auth_context.is_some(),
                extra.auth_context.as_ref().and_then(|ctx| ctx.token.as_ref()).is_some()
            );

            middleware
                .on_request(tool_name, &mut args, &mut extra, &context)
                .await?;

            // Debug: Log after middleware processing
            tracing::debug!(
                "BuilderMiddlewareExecutor - After middleware: auth_context present: {}, has_token: {}",
                extra.auth_context.is_some(),
                extra.auth_context.as_ref().and_then(|ctx| ctx.token.as_ref()).is_some()
            );
        }

        // Execute the tool
        let mut result = handler.handle(args, extra.clone()).await;

        // Process response through middleware chain
        for middleware in &self.middlewares {
            if let Err(e) = middleware
                .on_response(tool_name, &mut result, &context)
                .await
            {
                // Log error but continue with original result
                tracing::warn!("Tool response middleware processing failed: {}", e);
            }
        }

        // If tool execution failed, call on_error hooks
        if let Err(ref e) = result {
            for middleware in &self.middlewares {
                let _ = middleware.on_error(tool_name, e, &context).await;
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    struct MockTool;

    #[async_trait]
    impl ToolHandler for MockTool {
        async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(json!({"echo": args}))
        }
    }

    struct CounterMiddleware {
        counter: Arc<std::sync::Mutex<i32>>,
    }

    #[async_trait]
    impl ToolMiddleware for CounterMiddleware {
        async fn on_request(
            &self,
            _tool_name: &str,
            args: &mut Value,
            _extra: &mut RequestHandlerExtra,
            _context: &ToolContext,
        ) -> Result<()> {
            *self.counter.lock().unwrap() += 1;
            // Add metadata to args
            args.as_object_mut()
                .unwrap()
                .insert("middleware_applied".to_string(), json!(true));
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_builder_middleware_executor() {
        let counter = Arc::new(std::sync::Mutex::new(0));

        // Build executor with tool and middleware
        let mut tools = HashMap::new();
        tools.insert(
            "mock_tool".to_string(),
            Arc::new(MockTool) as Arc<dyn ToolHandler>,
        );

        let middleware = Arc::new(CounterMiddleware {
            counter: counter.clone(),
        }) as Arc<dyn ToolMiddleware>;

        let executor = BuilderMiddlewareExecutor::new(tools, vec![middleware]);

        // Execute tool through middleware
        let extra = RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new());
        let result = executor
            .execute_tool_with_middleware("mock_tool", json!({"input": "test"}), extra)
            .await
            .unwrap();

        // Verify middleware was applied
        assert_eq!(*counter.lock().unwrap(), 1);
        assert_eq!(
            result,
            json!({"echo": {"input": "test", "middleware_applied": true}})
        );
    }

    // Test that auth_context is preserved through clone and middleware execution
    struct AuthCheckingMiddleware {
        auth_checked: Arc<std::sync::Mutex<bool>>,
    }

    #[async_trait]
    impl ToolMiddleware for AuthCheckingMiddleware {
        async fn on_request(
            &self,
            _tool_name: &str,
            _args: &mut Value,
            extra: &mut RequestHandlerExtra,
            _context: &ToolContext,
        ) -> Result<()> {
            // Check if auth_context is present and has token
            if let Some(auth_ctx) = &extra.auth_context {
                if let Some(token) = &auth_ctx.token {
                    if token == "test-token-123" {
                        *self.auth_checked.lock().unwrap() = true;
                        // Inject token into metadata (like OAuth middleware would)
                        extra.set_metadata("oauth_token".to_string(), token.clone());
                        return Ok(());
                    }
                }
            }
            Err(crate::Error::authentication(
                "OAuth authentication required - auth_context missing or invalid".to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn test_auth_context_preserved_through_middleware() {
        let auth_checked = Arc::new(std::sync::Mutex::new(false));

        // Build executor with tool and auth-checking middleware
        let mut tools = HashMap::new();
        tools.insert(
            "mock_tool".to_string(),
            Arc::new(MockTool) as Arc<dyn ToolHandler>,
        );

        let middleware = Arc::new(AuthCheckingMiddleware {
            auth_checked: auth_checked.clone(),
        }) as Arc<dyn ToolMiddleware>;

        let executor = BuilderMiddlewareExecutor::new(tools, vec![middleware]);

        // Create auth context with token
        let auth_context = crate::server::auth::AuthContext {
            subject: "user-123".to_string(),
            scopes: vec!["openid".to_string()],
            claims: std::collections::HashMap::new(),
            token: Some("test-token-123".to_string()),
            client_id: Some("client-456".to_string()),
            expires_at: None,
            authenticated: true,
        };

        // Create extra with auth_context
        let extra = RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new())
            .with_auth_context(Some(auth_context));

        // Execute tool through middleware (with clone)
        let result = executor
            .execute_tool_with_middleware("mock_tool", json!({"input": "test"}), extra)
            .await
            .unwrap();

        // Verify middleware saw the auth_context
        assert!(
            *auth_checked.lock().unwrap(),
            "Middleware should have seen auth_context with token"
        );

        // Verify tool executed successfully
        assert_eq!(result, json!({"echo": {"input": "test"}}));
    }
}
