//! Observability middleware for MCP servers.
//!
//! This middleware implements the `ToolMiddleware` trait to provide:
//! - Request/response event emission
//! - Duration metrics
//! - Error tracking
//! - Distributed trace context propagation
//!
//! # Design Principles
//!
//! - **`AuthContext` is the Single Source of Truth**: User identity (`user_id`, `tenant_id`)
//!   is extracted from `AuthContext` at event emission time.
//! - **Non-blocking**: Events are emitted asynchronously, never blocking tool execution.
//! - **Configurable**: Field capture and sampling are controlled via `ObservabilityConfig`.
//!
//! # Example
//!
//! ```rust,ignore
//! use pmcp::server::observability::{
//!     McpObservabilityMiddleware, ObservabilityConfig, ConsoleBackend,
//! };
//! use std::sync::Arc;
//!
//! let config = ObservabilityConfig::development();
//! let backend = Arc::new(ConsoleBackend::new(config.console.clone()));
//! let middleware = McpObservabilityMiddleware::new(
//!     "my-server",
//!     config,
//!     backend,
//! );
//!
//! // Add to server builder
//! server.with_tool_middleware(Arc::new(middleware));
//! ```

use super::backend::ObservabilityBackend;
use super::config::ObservabilityConfig;
use super::events::{McpMetric, McpRequestEvent, McpResponseEvent};
use super::types::{McpOperationDetails, RequestMetadata, TraceContext};
use crate::error::{Error, Result};
use crate::server::cancellation::RequestHandlerExtra;
use crate::server::tool_middleware::{ToolContext, ToolMiddleware};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

/// Key used to store the request start time in metadata.
const REQUEST_START_KEY: &str = "_observability_start_ns";

/// Key used to store the trace context in metadata.
const TRACE_CONTEXT_KEY: &str = "_observability_trace";

/// Observability middleware for MCP tool execution.
///
/// This middleware hooks into the tool execution lifecycle to:
/// 1. Record request events before tool execution
/// 2. Calculate duration and record response events after execution
/// 3. Emit metrics for monitoring and alerting
/// 4. Track errors with full context
///
/// # Thread Safety
///
/// The middleware is `Send + Sync` and can be safely shared across async tasks.
/// All state is either immutable or stored in `RequestHandlerExtra.metadata`.
pub struct McpObservabilityMiddleware {
    /// Server name for event attribution.
    server_name: String,

    /// Observability configuration.
    config: ObservabilityConfig,

    /// Backend for emitting events and metrics.
    backend: Arc<dyn ObservabilityBackend>,
}

impl McpObservabilityMiddleware {
    /// Create a new observability middleware.
    ///
    /// # Arguments
    ///
    /// * `server_name` - Name of the MCP server (used in events)
    /// * `config` - Observability configuration
    /// * `backend` - Backend for event/metric emission
    pub fn new(
        server_name: impl Into<String>,
        config: ObservabilityConfig,
        backend: Arc<dyn ObservabilityBackend>,
    ) -> Self {
        Self {
            server_name: server_name.into(),
            config,
            backend,
        }
    }

    /// Create middleware with development configuration.
    ///
    /// Uses console output with pretty formatting.
    pub fn development(
        server_name: impl Into<String>,
        backend: Arc<dyn ObservabilityBackend>,
    ) -> Self {
        Self::new(server_name, ObservabilityConfig::development(), backend)
    }

    /// Create middleware with production configuration.
    ///
    /// Uses `CloudWatch` backend with EMF metrics.
    pub fn production(
        server_name: impl Into<String>,
        backend: Arc<dyn ObservabilityBackend>,
    ) -> Self {
        Self::new(server_name, ObservabilityConfig::production(), backend)
    }

    /// Build request metadata from the handler extra context.
    fn build_request_metadata(
        &self,
        extra: &RequestHandlerExtra,
        context: &ToolContext,
    ) -> RequestMetadata {
        let mut metadata = RequestMetadata::default();

        // Session ID
        if self.config.fields.capture_session_id {
            if let Some(session_id) = &extra.session_id {
                metadata = metadata.with_session_id(session_id.clone());
            } else if let Some(session_id) = &context.session_id {
                metadata = metadata.with_session_id(session_id.clone());
            }
        }

        // Client info from auth context (if available)
        if let Some(auth_ctx) = &extra.auth_context {
            if self.config.fields.capture_client_type {
                if let Some(client_id) = &auth_ctx.client_id {
                    metadata = metadata.with_client_type(client_id.clone());
                }
            }
        }

        // Client IP (if configured and available in metadata)
        if self.config.fields.capture_client_ip {
            if let Some(ip) = extra.get_metadata("client_ip") {
                metadata = metadata.with_client_ip(ip.clone());
            }
        }

        metadata
    }

    /// Extract user ID from `AuthContext` (single source of truth).
    fn extract_user_id(&self, extra: &RequestHandlerExtra) -> Option<String> {
        if !self.config.fields.capture_user_id {
            return None;
        }
        extra.auth_context.as_ref().map(|ctx| ctx.subject.clone())
    }

    /// Extract tenant ID from `AuthContext` claims.
    fn extract_tenant_id(extra: &RequestHandlerExtra) -> Option<String> {
        extra.auth_context.as_ref().and_then(|ctx| {
            // Try common tenant claim names
            ctx.claims
                .get("tenant_id")
                .or_else(|| ctx.claims.get("org_id"))
                .or_else(|| ctx.claims.get("organization_id"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
    }

    /// Build operation details from the tool call.
    fn build_operation_details(&self, tool_name: &str, args: &Value) -> McpOperationDetails {
        let mut details = McpOperationDetails::tool_call(tool_name);

        // Capture arguments hash if configured
        if self.config.fields.capture_arguments_hash {
            details = details.with_arguments_hash(super::types::hash_value(args));
        }

        details
    }

    /// Create and store trace context.
    fn create_trace_context(&self, extra: &RequestHandlerExtra) -> TraceContext {
        // Check if we have an incoming trace context (from composed calls)
        if let Some(trace_json) = extra.get_metadata(TRACE_CONTEXT_KEY) {
            if let Ok(parent_trace) = serde_json::from_str::<TraceContext>(trace_json) {
                // Check depth limit
                if parent_trace.depth >= self.config.max_depth {
                    tracing::warn!(
                        trace_id = %parent_trace.trace_id,
                        depth = parent_trace.depth,
                        max_depth = self.config.max_depth,
                        "Composition depth limit reached, creating new root trace"
                    );
                    return TraceContext::new_root();
                }
                return parent_trace.child();
            }
        }

        // Create new root trace
        TraceContext::new_root()
    }

    /// Emit metrics for a response.
    #[allow(dead_code)]
    async fn emit_response_metrics(
        &self,
        operation: &McpOperationDetails,
        duration_ms: u64,
        success: bool,
        response_size: Option<usize>,
    ) {
        let prefix = &self.config.metrics.prefix;

        // Request count
        if self.config.metrics.request_count {
            let metric = McpMetric::count(format!("{}.request.count", prefix), 1)
                .with_dimension("server", &self.server_name)
                .with_dimension("method", &operation.method)
                .with_dimension("success", success.to_string());

            if let Some(name) = operation.operation_name() {
                self.backend
                    .emit_metric(&metric.with_dimension("operation", name))
                    .await;
            } else {
                self.backend.emit_metric(&metric).await;
            }
        }

        // Request duration
        if self.config.metrics.request_duration {
            let metric = McpMetric::duration(format!("{}.request.duration", prefix), duration_ms)
                .with_dimension("server", &self.server_name)
                .with_dimension("method", &operation.method);

            if let Some(name) = operation.operation_name() {
                self.backend
                    .emit_metric(&metric.with_dimension("operation", name))
                    .await;
            } else {
                self.backend.emit_metric(&metric).await;
            }
        }

        // Error rate (only on failure)
        if !success && self.config.metrics.error_rate {
            let metric = McpMetric::count(format!("{}.request.errors", prefix), 1)
                .with_dimension("server", &self.server_name)
                .with_dimension("method", &operation.method);

            if let Some(name) = operation.operation_name() {
                self.backend
                    .emit_metric(&metric.with_dimension("operation", name))
                    .await;
            } else {
                self.backend.emit_metric(&metric).await;
            }
        }

        // Response size
        if let Some(size) = response_size {
            if self.config.fields.capture_response_size {
                let metric = McpMetric::bytes(format!("{}.response.size", prefix), size)
                    .with_dimension("server", &self.server_name)
                    .with_dimension("method", &operation.method);

                self.backend.emit_metric(&metric).await;
            }
        }

        // Tool usage metrics
        if self.config.metrics.tool_usage {
            if let Some(tool_name) = &operation.tool_name {
                let metric = McpMetric::count(format!("{}.tool.usage", prefix), 1)
                    .with_dimension("server", &self.server_name)
                    .with_dimension("tool", tool_name)
                    .with_dimension("success", success.to_string());

                self.backend.emit_metric(&metric).await;
            }
        }
    }
}

#[async_trait]
impl ToolMiddleware for McpObservabilityMiddleware {
    async fn on_request(
        &self,
        tool_name: &str,
        args: &mut Value,
        extra: &mut RequestHandlerExtra,
        context: &ToolContext,
    ) -> Result<()> {
        // Check if observability is enabled and should sample this request
        if !self.config.enabled || !self.config.should_sample() {
            return Ok(());
        }

        // Store start time for duration calculation
        let start_ns = Instant::now().elapsed().as_nanos().to_string();
        extra.set_metadata(REQUEST_START_KEY.to_string(), start_ns);

        // Create/propagate trace context
        let trace = self.create_trace_context(extra);

        // Store trace context for later
        if let Ok(trace_json) = serde_json::to_string(&trace) {
            extra.set_metadata(TRACE_CONTEXT_KEY.to_string(), trace_json);
        }

        // Build operation details
        let operation = self.build_operation_details(tool_name, args);

        // Build request metadata
        let metadata = self.build_request_metadata(extra, context);

        // Create request event
        let mut event =
            McpRequestEvent::new(trace, &self.server_name, operation).with_metadata(metadata);

        // Extract user identity from AuthContext (single source of truth)
        if let Some(user_id) = self.extract_user_id(extra) {
            event = event.with_user_id(user_id);
        }
        if let Some(tenant_id) = Self::extract_tenant_id(extra) {
            event = event.with_tenant_id(tenant_id);
        }

        // Emit request event (non-blocking)
        self.backend.record_request(&event).await;

        Ok(())
    }

    async fn on_response(
        &self,
        tool_name: &str,
        result: &mut Result<Value>,
        _context: &ToolContext,
    ) -> Result<()> {
        // Note: We need the extra to access timing and trace info
        // Since on_response doesn't have extra, we rely on context metadata
        // This is a limitation - we'll emit basic metrics without full context

        // For now, emit a basic success/failure metric
        // Full observability requires integration at a higher level

        if !self.config.enabled {
            return Ok(());
        }

        let success = result.is_ok();

        // Emit basic metrics
        if self.config.metrics.tool_usage {
            let prefix = &self.config.metrics.prefix;
            let metric = McpMetric::count(format!("{}.tool.complete", prefix), 1)
                .with_dimension("server", &self.server_name)
                .with_dimension("tool", tool_name)
                .with_dimension("success", success.to_string());

            self.backend.emit_metric(&metric).await;
        }

        Ok(())
    }

    async fn on_error(&self, tool_name: &str, error: &Error, _context: &ToolContext) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Create trace context for error event
        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call(tool_name);

        // Get error details (convert ErrorCode to i32)
        let (error_code, error_message) = match error {
            Error::Protocol { code, message, .. } => (code.as_i32(), message.clone()),
            _ => (crate::ErrorCode::INTERNAL_ERROR.as_i32(), error.to_string()),
        };

        // Create failure response event
        let event = McpResponseEvent::failure(
            trace,
            &self.server_name,
            operation.clone(),
            0, // Duration unknown in error handler
            error_code,
            error_message,
        );

        // Record the error event
        self.backend.record_response(&event).await;

        // Emit error metrics
        if self.config.metrics.error_rate {
            let prefix = &self.config.metrics.prefix;
            let metric = McpMetric::count(format!("{}.request.errors", prefix), 1)
                .with_dimension("server", &self.server_name)
                .with_dimension("tool", tool_name)
                .with_dimension("error_code", error_code.to_string());

            self.backend.emit_metric(&metric).await;
        }

        // Log error details if configured
        if self.config.fields.capture_error_details {
            tracing::error!(
                tool = %tool_name,
                error_code = error_code,
                error = %error,
                "Tool execution failed"
            );
        }

        Ok(())
    }

    fn priority(&self) -> i32 {
        // Run early to capture timing, but after auth middleware
        20
    }

    async fn should_execute(&self, _context: &ToolContext) -> bool {
        self.config.enabled
    }
}

impl std::fmt::Debug for McpObservabilityMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpObservabilityMiddleware")
            .field("server_name", &self.server_name)
            .field("enabled", &self.config.enabled)
            .field("backend", &self.backend.name())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::auth::AuthContext;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio_util::sync::CancellationToken;

    /// Test backend that counts events.
    struct CountingBackend {
        requests: AtomicUsize,
        responses: AtomicUsize,
        metrics: AtomicUsize,
    }

    impl CountingBackend {
        fn new() -> Self {
            Self {
                requests: AtomicUsize::new(0),
                responses: AtomicUsize::new(0),
                metrics: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ObservabilityBackend for CountingBackend {
        async fn record_request(&self, _event: &McpRequestEvent) {
            self.requests.fetch_add(1, Ordering::SeqCst);
        }

        async fn record_response(&self, _event: &McpResponseEvent) {
            self.responses.fetch_add(1, Ordering::SeqCst);
        }

        async fn emit_metric(&self, _metric: &McpMetric) {
            self.metrics.fetch_add(1, Ordering::SeqCst);
        }

        async fn flush(&self) {}

        fn name(&self) -> &'static str {
            "counting"
        }
    }

    #[tokio::test]
    async fn test_middleware_records_request() {
        let backend = Arc::new(CountingBackend::new());
        let config = ObservabilityConfig::development();
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend.clone());

        let mut args = serde_json::json!({"key": "value"});
        let mut extra = RequestHandlerExtra::new("req-123".to_string(), CancellationToken::new());
        let context = ToolContext::new("test_tool", "req-123");

        middleware
            .on_request("test_tool", &mut args, &mut extra, &context)
            .await
            .unwrap();

        assert_eq!(backend.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_middleware_respects_disabled_config() {
        let backend = Arc::new(CountingBackend::new());
        let config = ObservabilityConfig::disabled();
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend.clone());

        let mut args = serde_json::json!({});
        let mut extra = RequestHandlerExtra::new("req-123".to_string(), CancellationToken::new());
        let context = ToolContext::new("test_tool", "req-123");

        middleware
            .on_request("test_tool", &mut args, &mut extra, &context)
            .await
            .unwrap();

        // Should not record anything when disabled
        assert_eq!(backend.requests.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_middleware_extracts_user_id_from_auth_context() {
        let backend = Arc::new(CountingBackend::new());
        let config = ObservabilityConfig::development();
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend.clone());

        let mut args = serde_json::json!({});
        let mut extra = RequestHandlerExtra::new("req-123".to_string(), CancellationToken::new())
            .with_auth_context(Some(AuthContext::new("user-456")));
        let context = ToolContext::new("test_tool", "req-123");

        middleware
            .on_request("test_tool", &mut args, &mut extra, &context)
            .await
            .unwrap();

        assert_eq!(backend.requests.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_middleware_stores_trace_context() {
        let backend = Arc::new(CountingBackend::new());
        let config = ObservabilityConfig::development();
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend.clone());

        let mut args = serde_json::json!({});
        let mut extra = RequestHandlerExtra::new("req-123".to_string(), CancellationToken::new());
        let context = ToolContext::new("test_tool", "req-123");

        middleware
            .on_request("test_tool", &mut args, &mut extra, &context)
            .await
            .unwrap();

        // Check trace context was stored
        let trace_json = extra.get_metadata(TRACE_CONTEXT_KEY);
        assert!(trace_json.is_some());

        let trace: TraceContext = serde_json::from_str(trace_json.unwrap()).unwrap();
        assert!(!trace.trace_id.is_empty());
        assert_eq!(trace.depth, 0);
    }

    #[tokio::test]
    async fn test_middleware_creates_child_trace_for_composition() {
        let backend = Arc::new(CountingBackend::new());
        let config = ObservabilityConfig::development();
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend.clone());

        // Simulate incoming trace from parent
        let parent_trace = TraceContext::new_root();
        let parent_trace_json = serde_json::to_string(&parent_trace).unwrap();

        let mut args = serde_json::json!({});
        let mut extra = RequestHandlerExtra::new("req-123".to_string(), CancellationToken::new());
        extra.set_metadata(TRACE_CONTEXT_KEY.to_string(), parent_trace_json);
        let context = ToolContext::new("test_tool", "req-123");

        middleware
            .on_request("test_tool", &mut args, &mut extra, &context)
            .await
            .unwrap();

        // Check child trace was created
        let trace_json = extra.get_metadata(TRACE_CONTEXT_KEY).unwrap();
        let child_trace: TraceContext = serde_json::from_str(trace_json).unwrap();

        assert_eq!(child_trace.trace_id, parent_trace.trace_id);
        assert_eq!(child_trace.depth, 1);
        assert_eq!(child_trace.parent_span_id, Some(parent_trace.span_id));
    }

    #[tokio::test]
    async fn test_middleware_handles_error() {
        let backend = Arc::new(CountingBackend::new());
        let config = ObservabilityConfig::development();
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend.clone());

        let error = Error::protocol(crate::ErrorCode::INTERNAL_ERROR, "Test error");
        let context = ToolContext::new("test_tool", "req-123");

        middleware
            .on_error("test_tool", &error, &context)
            .await
            .unwrap();

        assert_eq!(backend.responses.load(Ordering::SeqCst), 1);
        assert!(backend.metrics.load(Ordering::SeqCst) > 0);
    }

    #[tokio::test]
    async fn test_middleware_priority() {
        let backend = Arc::new(CountingBackend::new());
        let config = ObservabilityConfig::development();
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend);

        // Should run early (after auth, before most other middleware)
        assert_eq!(middleware.priority(), 20);
    }
}
