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
        // Get the tool handler
        let handler = self
            .tools
            .get(tool_name)
            .ok_or_else(|| crate::Error::internal(format!("Tool '{}' not found", tool_name)))?;

        // Create tool context for middleware
        let context = ToolContext::new(tool_name, &extra.request_id);

        // Process request through middleware chain
        for middleware in &self.middlewares {
            middleware
                .on_request(tool_name, &mut args, &mut extra, &context)
                .await?;
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
}
