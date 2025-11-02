//! Middleware executor abstraction for tool execution.
//!
//! This module provides a trait that abstracts tool execution with middleware,
//! enabling workflow handlers to execute tools through the same middleware
//! chain as direct tool calls without circular dependencies.

use crate::error::Result;
use crate::server::cancellation::RequestHandlerExtra;
use async_trait::async_trait;
use serde_json::Value;

/// Trait for executing tools with full middleware chain application.
///
/// This trait provides an abstraction for tool execution that ensures
/// middleware (authentication, logging, validation, rate limiting, etc.)
/// is consistently applied regardless of whether the tool is called:
/// - Directly via `tools/call` JSON-RPC requests
/// - Server-side during workflow prompt execution
///
/// # Architecture
///
/// The trait decouples workflow execution from `ServerCore`, preventing
/// circular dependencies while ensuring consistent middleware application.
///
/// ```text
/// ┌─────────────────┐
/// │  WorkflowPrompt │
/// │    Handler      │
/// └────────┬────────┘
///          │ uses
///          ▼
/// ┌─────────────────────┐
/// │ MiddlewareExecutor  │◄─────── Trait abstraction
/// │      (trait)        │
/// └────────────────────┘
///          △
///          │ implements
///          │
/// ┌────────┴────────┐
/// │   ServerCore    │
/// │ (with middleware│
/// │     chains)     │
/// └─────────────────┘
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::middleware_executor::MiddlewareExecutor;
/// use pmcp::server::workflow::WorkflowPromptHandler;
/// use std::sync::Arc;
///
/// // ServerCore implements MiddlewareExecutor
/// let executor: Arc<dyn MiddlewareExecutor> = Arc::new(server_core);
///
/// // Workflow handler uses the abstraction
/// let handler = WorkflowPromptHandler::new(
///     workflow,
///     tool_registry,
///     executor,  // Pass as trait object
///     resource_handler,
/// );
///
/// // When workflow executes tools, middleware runs automatically
/// let result = handler.handle(args, extra).await?;
/// ```
///
/// # Benefits
///
/// 1. **Consistent Auth**: OAuth tokens injected by middleware work in workflows
/// 2. **Testability**: Easy to mock for workflow testing
/// 3. **No Circular Dependencies**: Clean separation of concerns
/// 4. **Single Source of Truth**: One middleware application path for all tools
#[async_trait]
pub trait MiddlewareExecutor: Send + Sync {
    /// Execute a tool with full middleware chain processing.
    ///
    /// This method applies the complete middleware pipeline:
    /// 1. **Request Middleware** - Inject credentials, validate permissions, log calls
    /// 2. **Tool Execution** - Call the actual tool handler
    /// 3. **Response Middleware** - Transform results, add headers, log responses
    /// 4. **Error Middleware** - Handle failures, retry logic, error transformation
    ///
    /// # Parameters
    ///
    /// * `tool_name` - Name of the tool to execute
    /// * `args` - Tool arguments (may be modified by middleware)
    /// * `extra` - Request context (auth, cancellation, metadata)
    ///
    /// # Returns
    ///
    /// The tool's result after response middleware processing.
    ///
    /// # Errors
    ///
    /// - `Error::Internal` - Tool not found or middleware processing failed
    /// - `Error::Authentication` - Authorization check failed
    /// - Tool-specific errors after middleware transformation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // In WorkflowPromptHandler::execute_tool_step
    /// let result = self.middleware_executor.execute_tool_with_middleware(
    ///     step.tool().name(),
    ///     params,
    ///     extra,
    /// ).await?;
    ///
    /// // Middleware has already:
    /// // - Injected OAuth token into extra.metadata
    /// // - Logged the tool call
    /// // - Validated permissions
    /// // - Applied rate limits
    /// ```
    async fn execute_tool_with_middleware(
        &self,
        tool_name: &str,
        args: Value,
        extra: RequestHandlerExtra,
    ) -> Result<Value>;
}
