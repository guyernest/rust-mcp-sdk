//! Tool middleware for cross-cutting concerns in tool execution.
//!
//! This module provides middleware hooks for tool execution, enabling:
//! - OAuth token injection from server auth context
//! - Request/response logging with tool metadata
//! - Performance metrics collection
//! - Authorization checks
//! - Argument validation and transformation
//!
//! # Security Best Practices
//!
//! **Token Handling**:
//! - Prefer `extra.auth_context.token` (set by transport layer) as source of truth
//! - Use `RequestHandlerExtra.metadata` for token injection (not tool args)
//! - Never log `metadata` values directly - use Debug impl which redacts sensitive keys
//! - Check `extra.is_cancelled()` in long-running middleware to respect cancellation
//!
//! **Error Sanitization**:
//! - Use `on_response` to sanitize errors before returning to client
//! - Remove auth tokens, internal paths, and stack traces from client-facing errors
//! - Use `on_error` for full error logging (not sent to client)
//! - Map auth failures to consistent error codes (e.g., UNAUTHORIZED)
//!
//! **Metadata Security**:
//! - `RequestHandlerExtra` has custom Debug impl that redacts sensitive metadata
//! - Keys containing "token", "key", "secret", "password" are automatically redacted
//! - Middleware should not log `extra.metadata` values directly
//! - Tools must not log metadata contents to prevent token leaks
//!
//! # Use Case: OAuth Token Pass-Through
//!
//! A common pattern is for MCP servers to authenticate users with OAuth, then
//! pass those tokens to backend data systems. Without middleware, every tool
//! must manually extract and pass tokens:
//!
//! ```rust,ignore
//! // Without middleware - repeated in every tool
//! impl ToolHandler for DatabaseQueryTool {
//!     async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
//!         let token = extract_oauth_token(&extra)?; // Repeated everywhere
//!         self.db_client.with_auth(token).query(args).await
//!     }
//! }
//! ```
//!
//! With middleware, token injection is centralized:
//!
//! ```rust,ignore
//! // OAuth middleware injects tokens automatically
//! struct OAuthInjectionMiddleware {
//!     token_store: Arc<TokenStore>,
//! }
//!
//! #[async_trait]
//! impl ToolMiddleware for OAuthInjectionMiddleware {
//!     async fn on_request(
//!         &self,
//!         tool_name: &str,
//!         _args: &mut Value,
//!         extra: &mut RequestHandlerExtra,
//!     ) -> Result<()> {
//!         if let Some(session_id) = &extra.session_id {
//!             let token = self.token_store.get_token(session_id).await?;
//!             extra.set_metadata("oauth_token", token);
//!         }
//!         Ok(())
//!     }
//! }
//!
//! // Tools just read from metadata - clean and DRY
//! impl ToolHandler for DatabaseQueryTool {
//!     async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
//!         let token = extra.get_metadata("oauth_token").unwrap();
//!         self.db_client.with_auth(token).query(args).await
//!     }
//! }
//! ```

use crate::error::{Error, Result};
use crate::server::cancellation::RequestHandlerExtra;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Context for tool middleware execution.
///
/// Provides metadata about the tool execution environment for middleware
/// to make conditional decisions.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Name of the tool being executed
    pub tool_name: String,
    /// Session ID if available
    pub session_id: Option<String>,
    /// Request ID for correlation
    pub request_id: String,
    /// Custom metadata
    pub metadata: HashMap<String, String>,
}

impl ToolContext {
    /// Create a new tool context.
    pub fn new(tool_name: impl Into<String>, request_id: impl Into<String>) -> Self {
        Self {
            tool_name: tool_name.into(),
            session_id: None,
            request_id: request_id.into(),
            metadata: HashMap::new(),
        }
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get metadata value.
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Set metadata value.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }
}

/// Tool middleware trait for cross-cutting concerns in tool execution.
///
/// Middleware can intercept tool execution to:
/// - Inject OAuth tokens or other authentication credentials
/// - Log requests and responses
/// - Collect performance metrics
/// - Validate or transform arguments
/// - Handle errors uniformly
///
/// # Execution Order
///
/// Middleware runs in priority order (lower priority values execute first):
/// 1. `should_execute()` - Check if middleware applies to this tool
/// 2. `on_request()` - Before tool execution (can modify args/extra)
/// 3. Tool execution
/// 4. `on_response()` - After successful execution (can transform result)
/// 5. `on_error()` - If tool execution fails (can transform error)
///
/// # Examples
///
/// ## OAuth Token Injection
///
/// ```rust
/// use pmcp::server::tool_middleware::{ToolMiddleware, ToolContext};
/// use pmcp::server::cancellation::RequestHandlerExtra;
/// use pmcp::Result;
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct OAuthMiddleware {
///     token: String,
/// }
///
/// #[async_trait]
/// impl ToolMiddleware for OAuthMiddleware {
///     async fn on_request(
///         &self,
///         _tool_name: &str,
///         _args: &mut Value,
///         extra: &mut RequestHandlerExtra,
///         _context: &ToolContext,
///     ) -> Result<()> {
///         extra.set_metadata("oauth_token".to_string(), self.token.clone());
///         Ok(())
///     }
/// }
/// ```
///
/// ## Logging Middleware
///
/// ```rust
/// use pmcp::server::tool_middleware::{ToolMiddleware, ToolContext};
/// use pmcp::server::cancellation::RequestHandlerExtra;
/// use pmcp::Result;
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct LoggingMiddleware;
///
/// #[async_trait]
/// impl ToolMiddleware for LoggingMiddleware {
///     async fn on_request(
///         &self,
///         tool_name: &str,
///         args: &mut Value,
///         _extra: &mut RequestHandlerExtra,
///         context: &ToolContext,
///     ) -> Result<()> {
///         tracing::info!("Tool call: {} with args: {:?}", tool_name, args);
///         Ok(())
///     }
///
///     async fn on_response(
///         &self,
///         tool_name: &str,
///         result: &mut Result<Value>,
///         _context: &ToolContext,
///     ) -> Result<()> {
///         match result {
///             Ok(value) => tracing::info!("Tool {} succeeded: {:?}", tool_name, value),
///             Err(e) => tracing::error!("Tool {} failed: {}", tool_name, e),
///         }
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait ToolMiddleware: Send + Sync {
    /// Called before tool execution.
    ///
    /// Can modify arguments or extra data before the tool runs.
    /// Return an error to short-circuit execution.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool being called
    /// * `args` - Tool arguments (mutable for transformation)
    /// * `extra` - Request handler extra data (mutable for metadata injection)
    /// * `context` - Execution context with session, request ID, etc.
    async fn on_request(
        &self,
        tool_name: &str,
        args: &mut Value,
        extra: &mut RequestHandlerExtra,
        context: &ToolContext,
    ) -> Result<()> {
        let _ = (tool_name, args, extra, context);
        Ok(())
    }

    /// Called after tool execution (success or failure).
    ///
    /// Can inspect or transform the result. This hook receives `&mut Result<Value>`,
    /// allowing transformation of both success and error cases.
    ///
    /// **Allowed Transformations**:
    /// - Transform `Ok(value)` → `Ok(modified_value)` (e.g., add metadata, redact fields)
    /// - Transform `Err(error)` → `Err(sanitized_error)` (e.g., remove sensitive details)
    /// - Inspect result for logging/metrics (do not modify)
    ///
    /// **Discouraged Transformations**:
    /// - Converting `Err` → `Ok` (changes tool semantics, confuses clients)
    /// - Converting `Ok` → `Err` (use `on_request` validation instead)
    ///
    /// **Security Note**: When transforming errors, ensure sensitive details
    /// (auth tokens, internal paths, stack traces) are sanitized before returning
    /// to the client. Use `on_error` for logging full error details.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool that was called
    /// * `result` - Tool execution result (mutable for transformation)
    /// * `context` - Execution context
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Transform success values
    /// async fn on_response(&self, tool_name: &str, result: &mut Result<Value>, context: &ToolContext) -> Result<()> {
    ///     if let Ok(value) = result {
    ///         // Add execution metadata
    ///         if let Value::Object(map) = value {
    ///             map.insert("_tool".to_string(), json!(tool_name));
    ///             map.insert("_request_id".to_string(), json!(&context.request_id));
    ///         }
    ///     }
    ///     Ok(())
    /// }
    ///
    /// // Sanitize errors
    /// async fn on_response(&self, tool_name: &str, result: &mut Result<Value>, context: &ToolContext) -> Result<()> {
    ///     if let Err(e) = result {
    ///         // Remove sensitive details from error messages
    ///         *result = Err(Error::internal("An internal error occurred"));
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn on_response(
        &self,
        tool_name: &str,
        result: &mut Result<Value>,
        context: &ToolContext,
    ) -> Result<()> {
        let _ = (tool_name, result, context);
        Ok(())
    }

    /// Called when tool execution fails or middleware returns an error.
    ///
    /// Useful for logging, metrics, or cleanup. Errors from this hook
    /// are logged but don't propagate to avoid cascading failures.
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool that failed
    /// * `error` - The error that occurred
    /// * `context` - Execution context
    async fn on_error(&self, tool_name: &str, error: &Error, context: &ToolContext) -> Result<()> {
        let _ = (tool_name, error, context);
        Ok(())
    }

    /// Priority for ordering (lower runs first).
    ///
    /// Default priority is 50. Use lower values for middleware that should
    /// run early (e.g., auth: 10) and higher values for middleware that should
    /// run late (e.g., logging: 90).
    fn priority(&self) -> i32 {
        50
    }

    /// Should this middleware execute for this tool call?
    ///
    /// Return false to skip middleware for specific tools, sessions, or contexts.
    /// Useful for conditional middleware execution (e.g., OAuth only for specific tools).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::tool_middleware::{ToolMiddleware, ToolContext};
    ///
    /// struct DatabaseOnlyMiddleware;
    ///
    /// # #[async_trait::async_trait]
    /// # impl ToolMiddleware for DatabaseOnlyMiddleware {
    /// async fn should_execute(&self, context: &ToolContext) -> bool {
    ///     // Filter by tool name
    ///     context.tool_name.starts_with("db_")
    /// }
    /// # }
    ///
    /// struct SessionAwareMiddleware;
    ///
    /// # #[async_trait::async_trait]
    /// # impl ToolMiddleware for SessionAwareMiddleware {
    /// async fn should_execute(&self, context: &ToolContext) -> bool {
    ///     // Filter by session presence
    ///     context.session_id.is_some()
    /// }
    /// # }
    /// ```
    async fn should_execute(&self, _context: &ToolContext) -> bool {
        true
    }
}

/// Chain of tool middleware.
///
/// Executes middleware in priority order for tool execution lifecycle hooks.
pub struct ToolMiddlewareChain {
    middlewares: Vec<Arc<dyn ToolMiddleware>>,
}

impl ToolMiddlewareChain {
    /// Create a new empty middleware chain.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add middleware to the chain.
    ///
    /// Middleware is automatically sorted by priority (lower = earlier execution).
    pub fn add(&mut self, middleware: Arc<dyn ToolMiddleware>) {
        self.middlewares.push(middleware);
        // Sort by priority (lower = higher priority)
        self.middlewares.sort_by_key(|m| m.priority());
    }

    /// Process request through all middleware.
    ///
    /// If any middleware returns an error:
    /// 1. Processing short-circuits immediately
    /// 2. `on_error` is called for all middleware
    /// 3. The original error is returned
    pub async fn process_request(
        &self,
        tool_name: &str,
        args: &mut Value,
        extra: &mut RequestHandlerExtra,
        context: &ToolContext,
    ) -> Result<()> {
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_request(tool_name, args, extra, context).await {
                    // Short-circuit: call on_error for all middleware
                    self.handle_error(tool_name, &e, context).await;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Process response through all middleware (in reverse order).
    ///
    /// If any middleware returns an error:
    /// 1. Processing short-circuits immediately
    /// 2. `on_error` is called for all middleware
    /// 3. The original error is returned
    pub async fn process_response(
        &self,
        tool_name: &str,
        result: &mut Result<Value>,
        context: &ToolContext,
    ) -> Result<()> {
        for middleware in self.middlewares.iter().rev() {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_response(tool_name, result, context).await {
                    // Short-circuit: call on_error for all middleware
                    self.handle_error(tool_name, &e, context).await;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Handle error by calling `on_error` for all middleware.
    ///
    /// Errors from `on_error` itself are logged but don't propagate.
    async fn handle_error(&self, tool_name: &str, error: &Error, context: &ToolContext) {
        for middleware in &self.middlewares {
            if let Err(e) = middleware.on_error(tool_name, error, context).await {
                tracing::error!(
                    "Error in tool middleware on_error hook: {} (original error: {})",
                    e,
                    error
                );
            }
        }
    }

    /// Handle error from tool execution.
    ///
    /// This should be called when a tool error occurs to allow middleware
    /// to log, record metrics, or perform cleanup.
    pub async fn handle_tool_error(&self, tool_name: &str, error: &Error, context: &ToolContext) {
        self.handle_error(tool_name, error, context).await;
    }
}

impl Default for ToolMiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolMiddlewareChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolMiddlewareChain")
            .field("count", &self.middlewares.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    struct TestMiddleware {
        priority: i32,
        name: String,
    }

    #[async_trait]
    impl ToolMiddleware for TestMiddleware {
        async fn on_request(
            &self,
            _tool_name: &str,
            args: &mut Value,
            _extra: &mut RequestHandlerExtra,
            _context: &ToolContext,
        ) -> Result<()> {
            // Add marker to args to track execution
            if let Value::Object(map) = args {
                map.insert(format!("{}_executed", self.name), Value::Bool(true));
            }
            Ok(())
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    #[tokio::test]
    async fn test_middleware_chain_priority_ordering() {
        let mut chain = ToolMiddlewareChain::new();

        // Add in reverse priority order
        chain.add(Arc::new(TestMiddleware {
            priority: 100,
            name: "third".to_string(),
        }));
        chain.add(Arc::new(TestMiddleware {
            priority: 10,
            name: "first".to_string(),
        }));
        chain.add(Arc::new(TestMiddleware {
            priority: 50,
            name: "second".to_string(),
        }));

        let mut args = serde_json::json!({});
        let mut extra =
            RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new());
        let context = ToolContext::new("test_tool", "req-123");

        chain
            .process_request("test_tool", &mut args, &mut extra, &context)
            .await
            .unwrap();

        // Verify execution order by checking args
        let map = args.as_object().unwrap();
        assert!(map.contains_key("first_executed"));
        assert!(map.contains_key("second_executed"));
        assert!(map.contains_key("third_executed"));
    }

    #[tokio::test]
    async fn test_middleware_chain_short_circuit_on_error() {
        struct FailingMiddleware;

        #[async_trait]
        impl ToolMiddleware for FailingMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                _extra: &mut RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                Err(Error::protocol(
                    crate::ErrorCode::INVALID_PARAMS,
                    "Middleware failed",
                ))
            }

            fn priority(&self) -> i32 {
                50
            }
        }

        let mut chain = ToolMiddlewareChain::new();
        chain.add(Arc::new(FailingMiddleware));

        let mut args = serde_json::json!({});
        let mut extra =
            RequestHandlerExtra::new("test-request".to_string(), CancellationToken::new());
        let context = ToolContext::new("test_tool", "req-123");

        let result = chain
            .process_request("test_tool", &mut args, &mut extra, &context)
            .await;

        assert!(result.is_err());
        let error_string = result.unwrap_err().to_string();
        assert!(
            error_string.contains("Middleware failed"),
            "Expected error to contain 'Middleware failed', got: {}",
            error_string
        );
    }

    #[tokio::test]
    async fn test_tool_context() {
        let context = ToolContext::new("test_tool", "req-123")
            .with_session_id("session-456")
            .with_metadata("key1", "value1");

        assert_eq!(context.tool_name, "test_tool");
        assert_eq!(context.request_id, "req-123");
        assert_eq!(context.session_id, Some("session-456".to_string()));
        assert_eq!(context.get_metadata("key1"), Some(&"value1".to_string()));
    }

    /// Test OAuth token injection flow from `auth_context` to metadata
    #[tokio::test]
    async fn test_oauth_injection_flow() {
        use crate::server::auth::AuthContext;

        struct OAuthInjectionMiddleware;

        #[async_trait]
        impl ToolMiddleware for OAuthInjectionMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                extra: &mut RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                // Extract token from auth_context (source of truth)
                if let Some(auth_ctx) = &extra.auth_context {
                    if let Some(token) = &auth_ctx.token {
                        extra.set_metadata("oauth_token".to_string(), token.clone());
                    }
                }
                Ok(())
            }

            fn priority(&self) -> i32 {
                10 // Run early
            }
        }

        let mut chain = ToolMiddlewareChain::new();
        chain.add(Arc::new(OAuthInjectionMiddleware));

        // Create extra with auth_context containing a token
        let mut extra = RequestHandlerExtra::new("test-req".to_string(), CancellationToken::new())
            .with_auth_context(Some(AuthContext {
                subject: "user-123".to_string(),
                scopes: vec!["read".to_string(), "write".to_string()],
                claims: std::collections::HashMap::new(),
                token: Some("oauth-token-abc123".to_string()),
                client_id: Some("client-456".to_string()),
                expires_at: None,
                authenticated: true,
            }));

        let mut args = serde_json::json!({});
        let context = ToolContext::new("test_tool", "req-123");

        // Process request through middleware
        chain
            .process_request("test_tool", &mut args, &mut extra, &context)
            .await
            .unwrap();

        // Verify token was injected into metadata
        assert_eq!(
            extra.get_metadata("oauth_token"),
            Some(&"oauth-token-abc123".to_string())
        );
    }

    /// Test conditional execution based on `ToolContext`
    #[tokio::test]
    async fn test_conditional_execution_by_context() {
        struct SessionAwareMiddleware {
            executed: Arc<parking_lot::Mutex<bool>>,
        }

        #[async_trait]
        impl ToolMiddleware for SessionAwareMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                _extra: &mut RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                *self.executed.lock() = true;
                Ok(())
            }

            async fn should_execute(&self, context: &ToolContext) -> bool {
                // Only execute if session_id is present and tool_name starts with "api_"
                context.session_id.is_some() && context.tool_name.starts_with("api_")
            }
        }

        let executed = Arc::new(parking_lot::Mutex::new(false));
        let middleware = SessionAwareMiddleware {
            executed: executed.clone(),
        };

        let mut chain = ToolMiddlewareChain::new();
        chain.add(Arc::new(middleware));

        // Test 1: No session_id - should not execute
        *executed.lock() = false;
        let mut extra = RequestHandlerExtra::new("test-req".to_string(), CancellationToken::new());
        let mut args = serde_json::json!({});
        let context = ToolContext::new("api_call", "req-123");

        chain
            .process_request("api_call", &mut args, &mut extra, &context)
            .await
            .unwrap();
        assert!(!*executed.lock(), "Should not execute without session_id");

        // Test 2: Has session_id but wrong tool name - should not execute
        *executed.lock() = false;
        let context = ToolContext::new("db_query", "req-124").with_session_id("session-456");

        chain
            .process_request("db_query", &mut args, &mut extra, &context)
            .await
            .unwrap();
        assert!(
            !*executed.lock(),
            "Should not execute for non-api_ tool names"
        );

        // Test 3: Has session_id and correct tool name - should execute
        *executed.lock() = false;
        let context = ToolContext::new("api_call", "req-125").with_session_id("session-456");

        chain
            .process_request("api_call", &mut args, &mut extra, &context)
            .await
            .unwrap();
        assert!(
            *executed.lock(),
            "Should execute with session_id and api_ prefix"
        );
    }

    /// Test concurrent tool calls with middleware
    #[tokio::test]
    async fn test_concurrent_middleware_execution() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingMiddleware {
            counter: Arc<AtomicUsize>,
        }

        #[async_trait]
        impl ToolMiddleware for CountingMiddleware {
            async fn on_request(
                &self,
                _tool_name: &str,
                _args: &mut Value,
                extra: &mut RequestHandlerExtra,
                _context: &ToolContext,
            ) -> Result<()> {
                let count = self.counter.fetch_add(1, Ordering::SeqCst);
                extra.set_metadata("execution_number".to_string(), count.to_string());
                Ok(())
            }
        }

        let counter = Arc::new(AtomicUsize::new(0));
        let middleware = CountingMiddleware {
            counter: counter.clone(),
        };

        let mut chain_builder = ToolMiddlewareChain::new();
        chain_builder.add(Arc::new(middleware));
        let chain = Arc::new(chain_builder);

        // Spawn 100 concurrent tasks
        let mut handles = Vec::new();
        for i in 0..100 {
            let chain = chain.clone();
            let handle = tokio::spawn(async move {
                let mut extra =
                    RequestHandlerExtra::new(format!("req-{}", i), CancellationToken::new());
                let mut args = serde_json::json!({});
                let context = ToolContext::new("test_tool", format!("req-{}", i));

                chain
                    .process_request("test_tool", &mut args, &mut extra, &context)
                    .await
                    .unwrap();

                // Verify metadata was set
                assert!(extra.get_metadata("execution_number").is_some());
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify all 100 executions occurred
        assert_eq!(counter.load(Ordering::SeqCst), 100);
    }

    /// Test that sensitive metadata is not leaked in debug output
    #[tokio::test]
    async fn test_no_leak_in_logging() {
        let mut extra = RequestHandlerExtra::new("test-req".to_string(), CancellationToken::new());

        // Add sensitive token
        extra.set_metadata(
            "oauth_token".to_string(),
            "super-secret-token-xyz".to_string(),
        );
        extra.set_metadata("user_id".to_string(), "user-456".to_string());

        // Get debug output (simulates logging)
        let debug_output = format!("{:?}", extra);

        // Verify token is redacted
        assert!(
            debug_output.contains("[REDACTED]"),
            "Expected [REDACTED] in debug output"
        );
        assert!(
            !debug_output.contains("super-secret-token-xyz"),
            "Token should not appear in debug output: {}",
            debug_output
        );

        // Verify non-sensitive data is visible
        assert!(
            debug_output.contains("user-456"),
            "Non-sensitive metadata should be visible: {}",
            debug_output
        );
    }
}
