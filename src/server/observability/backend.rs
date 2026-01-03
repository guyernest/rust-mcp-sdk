//! Observability backend implementations.
//!
//! This module provides the `ObservabilityBackend` trait and several
//! implementations for different observability platforms:
//!
//! - `ConsoleBackend` - Development/debugging output
//! - `CloudWatchBackend` - AWS `CloudWatch` with EMF support
//! - `CompositeBackend` - Fan-out to multiple backends
//!
//! # Implementing Custom Backends
//!
//! To create a custom backend, implement the `ObservabilityBackend` trait:
//!
//! ```rust,ignore
//! use pmcp::server::observability::{ObservabilityBackend, McpRequestEvent, McpResponseEvent, McpMetric};
//! use async_trait::async_trait;
//!
//! struct MyBackend { /* ... */ }
//!
//! #[async_trait]
//! impl ObservabilityBackend for MyBackend {
//!     async fn record_request(&self, event: &McpRequestEvent) {
//!         // Send to your observability platform
//!     }
//!
//!     async fn record_response(&self, event: &McpResponseEvent) {
//!         // Send to your observability platform
//!     }
//!
//!     async fn emit_metric(&self, metric: &McpMetric) {
//!         // Send to your metrics platform
//!     }
//!
//!     async fn flush(&self) {
//!         // Flush any buffered data
//!     }
//!
//!     fn name(&self) -> &'static str {
//!         "my-backend"
//!     }
//! }
//! ```

use super::events::{McpMetric, McpRequestEvent, McpResponseEvent, MetricUnit, StandardMetrics};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

/// Backend trait for observability data export.
///
/// Implementations handle the actual export to observability platforms.
/// All methods are async and should be non-blocking to avoid impacting
/// request latency.
///
/// # Error Handling
///
/// Backend methods do not return errors intentionally. Observability
/// should never fail a request. Implementations should log errors
/// internally and continue operation.
#[async_trait]
pub trait ObservabilityBackend: Send + Sync + 'static {
    /// Record a request event (called when request is received).
    async fn record_request(&self, event: &McpRequestEvent);

    /// Record a response event (called when response is sent).
    async fn record_response(&self, event: &McpResponseEvent);

    /// Emit a metric data point.
    async fn emit_metric(&self, metric: &McpMetric);

    /// Flush pending data (called on shutdown or periodically).
    async fn flush(&self);

    /// Backend name for diagnostics.
    fn name(&self) -> &'static str;

    /// Check if the backend is enabled.
    ///
    /// Override to implement conditional enablement.
    fn is_enabled(&self) -> bool {
        true
    }
}

/// Composite backend that sends to multiple backends.
///
/// Events and metrics are fanned out to all backends concurrently.
/// If any backend fails, others continue to receive data.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::observability::{CompositeBackend, ConsoleBackend, CloudWatchBackend};
/// use std::sync::Arc;
///
/// let composite = CompositeBackend::new(vec![
///     Arc::new(ConsoleBackend::new(true)),
///     Arc::new(CloudWatchBackend::new(CloudWatchConfig::default())),
/// ]);
/// ```
pub struct CompositeBackend {
    backends: Vec<Arc<dyn ObservabilityBackend>>,
}

impl std::fmt::Debug for CompositeBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeBackend")
            .field("backends_count", &self.backends.len())
            .finish()
    }
}

impl CompositeBackend {
    /// Create a new composite backend.
    pub fn new(backends: Vec<Arc<dyn ObservabilityBackend>>) -> Self {
        Self { backends }
    }

    /// Add a backend to the composite.
    pub fn add(&mut self, backend: Arc<dyn ObservabilityBackend>) {
        self.backends.push(backend);
    }

    /// Get the number of backends.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Check if there are no backends.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

#[async_trait]
impl ObservabilityBackend for CompositeBackend {
    async fn record_request(&self, event: &McpRequestEvent) {
        let futures: Vec<_> = self
            .backends
            .iter()
            .filter(|b| b.is_enabled())
            .map(|b| b.record_request(event))
            .collect();
        futures::future::join_all(futures).await;
    }

    async fn record_response(&self, event: &McpResponseEvent) {
        let futures: Vec<_> = self
            .backends
            .iter()
            .filter(|b| b.is_enabled())
            .map(|b| b.record_response(event))
            .collect();
        futures::future::join_all(futures).await;
    }

    async fn emit_metric(&self, metric: &McpMetric) {
        let futures: Vec<_> = self
            .backends
            .iter()
            .filter(|b| b.is_enabled())
            .map(|b| b.emit_metric(metric))
            .collect();
        futures::future::join_all(futures).await;
    }

    async fn flush(&self) {
        let futures: Vec<_> = self.backends.iter().map(|b| b.flush()).collect();
        futures::future::join_all(futures).await;
    }

    fn name(&self) -> &'static str {
        "composite"
    }
}

/// Console backend for local development.
///
/// Outputs events and metrics to stdout in a human-readable or JSON format.
/// Useful for development and debugging.
///
/// # Example
///
/// ```rust
/// use pmcp::server::observability::ConsoleBackend;
///
/// // Pretty format for development
/// let backend = ConsoleBackend::new(true);
///
/// // JSON format for log parsing
/// let backend = ConsoleBackend::json();
/// ```
#[derive(Debug)]
pub struct ConsoleBackend {
    /// Whether to use pretty (human-readable) output.
    pretty: bool,
    /// Whether to include full event details (verbose mode).
    verbose: bool,
}

impl ConsoleBackend {
    /// Create a new console backend.
    ///
    /// # Arguments
    ///
    /// * `pretty` - If true, use human-readable format; otherwise use JSON.
    pub fn new(pretty: bool) -> Self {
        Self {
            pretty,
            verbose: false,
        }
    }

    /// Create a JSON-output console backend.
    pub fn json() -> Self {
        Self {
            pretty: false,
            verbose: false,
        }
    }

    /// Enable verbose mode (include full event details).
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}

impl Default for ConsoleBackend {
    fn default() -> Self {
        Self::new(true)
    }
}

#[async_trait]
impl ObservabilityBackend for ConsoleBackend {
    async fn record_request(&self, event: &McpRequestEvent) {
        if self.pretty {
            println!(
                "[{}] {} {} {} (user: {})",
                event.trace.short_trace_id(),
                event.server_name,
                event.operation.method,
                event.operation.operation_name().unwrap_or("-"),
                event.user_id.as_deref().unwrap_or("anonymous"),
            );
        } else if let Ok(json) = serde_json::to_string(&event) {
            println!("{json}");
        }
    }

    async fn record_response(&self, event: &McpResponseEvent) {
        if self.pretty {
            let status = if event.success { "OK" } else { "ERROR" };
            let error_info = if event.success {
                String::new()
            } else {
                format!(
                    " - {} ({})",
                    event.error_message.as_deref().unwrap_or("unknown"),
                    event.error_code.unwrap_or(0)
                )
            };

            println!(
                "[{}] {} {} {} ({}ms) {}{error_info}",
                event.trace.short_trace_id(),
                event.server_name,
                event.operation.method,
                event.operation.operation_name().unwrap_or("-"),
                event.duration_ms,
                status,
            );
        } else if let Ok(json) = serde_json::to_string(&event) {
            println!("{json}");
        }
    }

    async fn emit_metric(&self, metric: &McpMetric) {
        if self.verbose {
            if self.pretty {
                println!(
                    "[METRIC] {} = {} {}",
                    metric.name,
                    metric.value,
                    metric.unit.as_str()
                );
            } else if let Ok(json) = serde_json::to_string(&metric) {
                println!("{json}");
            }
        }
    }

    async fn flush(&self) {
        // Console output is immediate, no buffering
    }

    fn name(&self) -> &'static str {
        "console"
    }
}

/// Configuration for `CloudWatch` backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CloudWatchConfig {
    /// `CloudWatch` namespace for metrics.
    pub namespace: String,
    /// Enable EMF (Embedded Metric Format) for automatic metric extraction.
    pub emf_enabled: bool,
    /// Log group pattern (`{server_name}` is replaced).
    pub log_group_pattern: String,
}

impl Default for CloudWatchConfig {
    fn default() -> Self {
        Self {
            namespace: "PMCP/Servers".to_string(),
            emf_enabled: true,
            log_group_pattern: "/aws/lambda/{server_name}".to_string(),
        }
    }
}

impl CloudWatchConfig {
    /// Create a new `CloudWatch` configuration with custom namespace.
    pub fn with_namespace(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            ..Default::default()
        }
    }
}

/// `CloudWatch` backend with EMF (Embedded Metric Format) support.
///
/// When EMF is enabled, metrics are embedded in log messages and
/// automatically extracted by `CloudWatch`. This is more efficient
/// than separate `PutMetric` API calls.
///
/// # Example
///
/// ```rust
/// use pmcp::server::observability::{CloudWatchBackend, CloudWatchConfig};
///
/// let config = CloudWatchConfig::default();
/// let backend = CloudWatchBackend::new(config);
/// ```
#[derive(Debug)]
pub struct CloudWatchBackend {
    config: CloudWatchConfig,
}

impl CloudWatchBackend {
    /// Create a new `CloudWatch` backend.
    pub fn new(config: CloudWatchConfig) -> Self {
        Self { config }
    }

    /// Create a `CloudWatch` backend with default configuration.
    pub fn default_config() -> Self {
        Self::new(CloudWatchConfig::default())
    }

    /// Format response event as `CloudWatch` EMF.
    fn format_emf(&self, event: &McpResponseEvent) -> serde_json::Value {
        json!({
            // EMF metadata
            "_aws": {
                "Timestamp": event.timestamp.timestamp_millis(),
                "CloudWatchMetrics": [{
                    "Namespace": &self.config.namespace,
                    "Dimensions": [
                        ["ServerName"],
                        ["ServerName", "Method"],
                        ["ServerName", "Method", "Operation"]
                    ],
                    "Metrics": [
                        {"Name": "Duration", "Unit": "Milliseconds"},
                        {"Name": "RequestCount", "Unit": "Count"},
                        {"Name": "ErrorCount", "Unit": "Count"}
                    ]
                }]
            },
            // Dimensions
            "ServerName": event.server_name,
            "Method": event.operation.method,
            "Operation": event.operation.operation_name().unwrap_or("none"),
            // Trace context (for correlation)
            "TraceId": event.trace.trace_id,
            "SpanId": event.trace.span_id,
            "ParentSpanId": event.trace.parent_span_id,
            "Depth": event.trace.depth,
            // User identity (from AuthContext - single source of truth)
            "UserId": event.user_id,
            "TenantId": event.tenant_id,
            // Request metadata (analytics, not user identity)
            "ClientType": event.metadata.client_type,
            "SessionId": event.metadata.session_id,
            // Metrics
            "Duration": event.duration_ms,
            "RequestCount": 1,
            "ErrorCount": i32::from(!event.success),
            // Additional fields
            "Success": event.success,
            "ErrorCode": event.error_code,
            "ErrorMessage": event.error_message,
            "ResponseSize": event.response_size,
        })
    }
}

#[async_trait]
impl ObservabilityBackend for CloudWatchBackend {
    async fn record_request(&self, event: &McpRequestEvent) {
        // Log request start using tracing
        tracing::info!(
            target: "mcp.observability",
            trace_id = %event.trace.trace_id,
            span_id = %event.trace.span_id,
            depth = event.trace.depth,
            server = %event.server_name,
            method = %event.operation.method,
            operation = ?event.operation.operation_name(),
            user_id = ?event.user_id,
            tenant_id = ?event.tenant_id,
            client_type = ?event.metadata.client_type,
            "MCP request started"
        );
    }

    async fn record_response(&self, event: &McpResponseEvent) {
        if self.config.emf_enabled {
            // EMF format - CloudWatch automatically extracts metrics
            let emf = self.format_emf(event);
            if let Ok(emf_str) = serde_json::to_string(&emf) {
                // Use println for EMF as it needs to be a standalone JSON line
                println!("{emf_str}");
            }
        } else {
            // Standard structured logging
            tracing::info!(
                target: "mcp.observability",
                trace_id = %event.trace.trace_id,
                span_id = %event.trace.span_id,
                parent_span_id = ?event.trace.parent_span_id,
                depth = event.trace.depth,
                server = %event.server_name,
                method = %event.operation.method,
                operation = ?event.operation.operation_name(),
                user_id = ?event.user_id,
                tenant_id = ?event.tenant_id,
                client_type = ?event.metadata.client_type,
                duration_ms = event.duration_ms,
                success = event.success,
                error_code = ?event.error_code,
                error_message = ?event.error_message,
                response_size = ?event.response_size,
                "MCP request completed"
            );
        }
    }

    async fn emit_metric(&self, metric: &McpMetric) {
        // When using EMF, metrics are extracted automatically from logs
        // This is a fallback for explicit metric emission
        if !self.config.emf_enabled {
            tracing::info!(
                target: "mcp.metrics",
                metric_name = %metric.name,
                metric_value = metric.value,
                metric_unit = %metric.unit.as_str(),
                dimensions = ?metric.dimensions,
                "Metric emitted"
            );
        }
    }

    async fn flush(&self) {
        // CloudWatch logs are flushed automatically by the Lambda runtime
        // or the tracing subscriber
    }

    fn name(&self) -> &'static str {
        "cloudwatch"
    }
}

/// No-op backend that discards all events.
///
/// Useful for testing or when observability is disabled.
#[derive(Debug, Clone, Copy)]
pub struct NullBackend;

impl NullBackend {
    /// Create a new null backend.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ObservabilityBackend for NullBackend {
    async fn record_request(&self, _event: &McpRequestEvent) {}
    async fn record_response(&self, _event: &McpResponseEvent) {}
    async fn emit_metric(&self, _metric: &McpMetric) {}
    async fn flush(&self) {}

    fn name(&self) -> &'static str {
        "null"
    }
}

/// Helper to emit standard metrics from a response event.
///
/// This is used by the middleware to emit the standard set of metrics
/// after each request.
#[allow(dead_code)]
async fn emit_standard_metrics(
    backend: &dyn ObservabilityBackend,
    event: &McpResponseEvent,
    prefix: &str,
) {
    let mut dimensions = std::collections::HashMap::new();
    dimensions.insert("ServerName".to_string(), event.server_name.clone());
    dimensions.insert("Method".to_string(), event.operation.method.clone());

    if let Some(op_name) = event.operation.operation_name() {
        dimensions.insert("Operation".to_string(), op_name.to_string());
    }

    // Duration histogram
    let duration_name = if prefix.is_empty() {
        StandardMetrics::REQUEST_DURATION.to_string()
    } else {
        format!("{prefix}.request.duration")
    };
    backend
        .emit_metric(
            &McpMetric::new(
                duration_name,
                event.duration_ms as f64,
                MetricUnit::Milliseconds,
            )
            .with_dimensions(dimensions.clone()),
        )
        .await;

    // Request count
    let count_name = if prefix.is_empty() {
        StandardMetrics::REQUEST_COUNT.to_string()
    } else {
        format!("{prefix}.request.count")
    };
    backend
        .emit_metric(
            &McpMetric::new(count_name, 1.0, MetricUnit::Count).with_dimensions(dimensions.clone()),
        )
        .await;

    // Error count (if failed)
    if !event.success {
        let mut error_dims = dimensions.clone();
        if let Some(code) = event.error_code {
            error_dims.insert("ErrorCode".to_string(), code.to_string());
        }

        let error_name = if prefix.is_empty() {
            StandardMetrics::REQUEST_ERRORS.to_string()
        } else {
            format!("{prefix}.request.errors")
        };
        backend
            .emit_metric(
                &McpMetric::new(error_name, 1.0, MetricUnit::Count).with_dimensions(error_dims),
            )
            .await;
    }

    // Response size (if captured)
    if let Some(size) = event.response_size {
        let size_name = if prefix.is_empty() {
            StandardMetrics::RESPONSE_SIZE.to_string()
        } else {
            format!("{prefix}.response.size")
        };
        backend
            .emit_metric(
                &McpMetric::new(size_name, size as f64, MetricUnit::Bytes)
                    .with_dimensions(dimensions),
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::observability::types::{McpOperationDetails, TraceContext};
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Test backend that counts events.
    #[allow(clippy::struct_field_names)]
    struct CountingBackend {
        request_count: AtomicUsize,
        response_count: AtomicUsize,
        metric_count: AtomicUsize,
        flush_count: AtomicUsize,
    }

    impl CountingBackend {
        fn new() -> Self {
            Self {
                request_count: AtomicUsize::new(0),
                response_count: AtomicUsize::new(0),
                metric_count: AtomicUsize::new(0),
                flush_count: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ObservabilityBackend for CountingBackend {
        async fn record_request(&self, _event: &McpRequestEvent) {
            self.request_count.fetch_add(1, Ordering::SeqCst);
        }

        async fn record_response(&self, _event: &McpResponseEvent) {
            self.response_count.fetch_add(1, Ordering::SeqCst);
        }

        async fn emit_metric(&self, _metric: &McpMetric) {
            self.metric_count.fetch_add(1, Ordering::SeqCst);
        }

        async fn flush(&self) {
            self.flush_count.fetch_add(1, Ordering::SeqCst);
        }

        fn name(&self) -> &'static str {
            "counting"
        }
    }

    #[tokio::test]
    async fn test_composite_backend() {
        let backend1 = Arc::new(CountingBackend::new());
        let backend2 = Arc::new(CountingBackend::new());

        let composite = CompositeBackend::new(vec![backend1.clone(), backend2.clone()]);

        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("test");
        let request_event = McpRequestEvent::new(trace.clone(), "test-server", operation.clone());
        let response_event = McpResponseEvent::success(trace, "test-server", operation, 100);

        composite.record_request(&request_event).await;
        composite.record_response(&response_event).await;
        composite.flush().await;

        // Both backends should receive events
        assert_eq!(backend1.request_count.load(Ordering::SeqCst), 1);
        assert_eq!(backend2.request_count.load(Ordering::SeqCst), 1);
        assert_eq!(backend1.response_count.load(Ordering::SeqCst), 1);
        assert_eq!(backend2.response_count.load(Ordering::SeqCst), 1);
        assert_eq!(backend1.flush_count.load(Ordering::SeqCst), 1);
        assert_eq!(backend2.flush_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_console_backend_pretty() {
        let backend = ConsoleBackend::new(true);

        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("get_weather");
        let event = McpResponseEvent::success(trace, "test-server", operation, 150);

        // Just verify it doesn't panic
        backend.record_response(&event).await;
    }

    #[tokio::test]
    async fn test_null_backend() {
        let backend = NullBackend::new();

        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("test");
        let request_event = McpRequestEvent::new(trace.clone(), "test", operation.clone());
        let response_event = McpResponseEvent::success(trace, "test", operation, 100);

        // Should not panic
        backend.record_request(&request_event).await;
        backend.record_response(&response_event).await;
        backend.emit_metric(&McpMetric::count("test", 1)).await;
        backend.flush().await;
    }

    #[test]
    fn test_cloudwatch_emf_format() {
        let backend = CloudWatchBackend::new(CloudWatchConfig::default());

        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("get_weather");
        let event = McpResponseEvent::success(trace, "test-server", operation, 150)
            .with_user_id("user-123");

        let emf = backend.format_emf(&event);

        // Check EMF structure
        assert!(emf.get("_aws").is_some());
        assert_eq!(emf.get("ServerName").unwrap(), "test-server");
        assert_eq!(emf.get("Duration").unwrap(), 150);
        assert_eq!(emf.get("RequestCount").unwrap(), 1);
        assert_eq!(emf.get("ErrorCount").unwrap(), 0);
        assert_eq!(emf.get("UserId").unwrap(), "user-123");
    }

    #[tokio::test]
    async fn test_emit_standard_metrics() {
        let backend = Arc::new(CountingBackend::new());

        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("get_weather");
        let event = McpResponseEvent::success(trace, "test-server", operation, 150)
            .with_response_size(1024);

        emit_standard_metrics(backend.as_ref(), &event, "mcp").await;

        // Should emit: duration, count, response_size (no error since success)
        assert_eq!(backend.metric_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_emit_standard_metrics_with_error() {
        let backend = Arc::new(CountingBackend::new());

        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("get_weather");
        let event = McpResponseEvent::failure(trace, "test-server", operation, 50, -32600, "Error");

        emit_standard_metrics(backend.as_ref(), &event, "mcp").await;

        // Should emit: duration, count, errors
        assert_eq!(backend.metric_count.load(Ordering::SeqCst), 3);
    }
}
