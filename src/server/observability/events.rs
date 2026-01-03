//! Observability events emitted by the middleware.
//!
//! Events are emitted at key points in request processing:
//! - `McpRequestEvent` - When a request is received
//! - `McpResponseEvent` - When a response is sent
//! - `McpMetric` - For metric data points
//!
//! # User Identity
//!
//! User identity (`user_id`, `tenant_id`) is captured from `AuthContext` at event
//! creation time. This is the SINGLE SOURCE OF TRUTH for user identity.
//! The observability types (`TraceContext`, `RequestMetadata`) do not duplicate
//! this information.

use super::types::{McpOperationDetails, RequestMetadata, TraceContext};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

/// Event emitted when an MCP request is received.
///
/// This event is recorded at the start of request processing, before
/// the tool/resource/prompt handler is invoked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequestEvent {
    /// Trace context for correlation.
    pub trace: TraceContext,

    /// Server name (e.g., "advanced-mcp-course").
    pub server_name: String,

    /// Operation details (method, `tool_name`, etc.).
    pub operation: McpOperationDetails,

    /// Request metadata (`client_type`, `session_id` - NOT user identity).
    pub metadata: RequestMetadata,

    /// User ID from `AuthContext` (single source of truth).
    /// This is the ONLY user identity field - sourced from `AuthContext`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Tenant ID from `AuthContext` (for multi-tenant servers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    /// Timestamp when request was received.
    pub timestamp: DateTime<Utc>,
}

impl McpRequestEvent {
    /// Create a new request event.
    pub fn new(
        trace: TraceContext,
        server_name: impl Into<String>,
        operation: McpOperationDetails,
    ) -> Self {
        Self {
            trace,
            server_name: server_name.into(),
            operation,
            metadata: RequestMetadata::default(),
            user_id: None,
            tenant_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Set the request metadata.
    pub fn with_metadata(mut self, metadata: RequestMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set the user ID from `AuthContext`.
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set the tenant ID from `AuthContext`.
    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }
}

/// Event emitted when an MCP response is sent.
///
/// This event is recorded after request processing completes, including
/// duration, success/failure status, and error details if applicable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponseEvent {
    /// Trace context for correlation.
    pub trace: TraceContext,

    /// Server name.
    pub server_name: String,

    /// Operation details.
    pub operation: McpOperationDetails,

    /// Request metadata (`client_type`, `session_id` - NOT user identity).
    pub metadata: RequestMetadata,

    /// User ID from `AuthContext` (single source of truth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Tenant ID from `AuthContext`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    /// Request duration in milliseconds.
    pub duration_ms: u64,

    /// Whether the request succeeded.
    pub success: bool,

    /// Error code if failed (MCP error code).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<i32>,

    /// Error message if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,

    /// Response payload size in bytes (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_size: Option<usize>,

    /// Timestamp when response was sent.
    pub timestamp: DateTime<Utc>,
}

impl McpResponseEvent {
    /// Create a new response event for a successful request.
    pub fn success(
        trace: TraceContext,
        server_name: impl Into<String>,
        operation: McpOperationDetails,
        duration_ms: u64,
    ) -> Self {
        Self {
            trace,
            server_name: server_name.into(),
            operation,
            metadata: RequestMetadata::default(),
            user_id: None,
            tenant_id: None,
            duration_ms,
            success: true,
            error_code: None,
            error_message: None,
            response_size: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a new response event for a failed request.
    pub fn failure(
        trace: TraceContext,
        server_name: impl Into<String>,
        operation: McpOperationDetails,
        duration_ms: u64,
        error_code: i32,
        error_message: impl Into<String>,
    ) -> Self {
        Self {
            trace,
            server_name: server_name.into(),
            operation,
            metadata: RequestMetadata::default(),
            user_id: None,
            tenant_id: None,
            duration_ms,
            success: false,
            error_code: Some(error_code),
            error_message: Some(error_message.into()),
            response_size: None,
            timestamp: Utc::now(),
        }
    }

    /// Set the request metadata.
    pub fn with_metadata(mut self, metadata: RequestMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set the user ID from `AuthContext`.
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set the tenant ID from `AuthContext`.
    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        self.tenant_id = Some(tenant_id.into());
        self
    }

    /// Set the response size.
    pub fn with_response_size(mut self, size: usize) -> Self {
        self.response_size = Some(size);
        self
    }
}

/// Metric unit for observability metrics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MetricUnit {
    /// Time in milliseconds.
    Milliseconds,
    /// Count of items.
    Count,
    /// Size in bytes.
    Bytes,
    /// Percentage (0-100).
    Percent,
    /// No unit (dimensionless).
    None,
}

impl MetricUnit {
    /// Get the unit name as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Milliseconds => "Milliseconds",
            Self::Count => "Count",
            Self::Bytes => "Bytes",
            Self::Percent => "Percent",
            Self::None => "None",
        }
    }
}

/// A metric data point for observability.
///
/// Metrics can be counters, gauges, or histograms depending on how
/// they're used by the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMetric {
    /// Metric name (e.g., "mcp.request.duration").
    pub name: String,

    /// Metric value.
    pub value: f64,

    /// Metric unit.
    pub unit: MetricUnit,

    /// Dimensions for grouping/filtering.
    pub dimensions: HashMap<String, String>,

    /// Timestamp when the metric was recorded.
    pub timestamp: DateTime<Utc>,
}

impl McpMetric {
    /// Create a new metric.
    pub fn new(name: impl Into<String>, value: f64, unit: MetricUnit) -> Self {
        Self {
            name: name.into(),
            value,
            unit,
            dimensions: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Add a dimension.
    pub fn with_dimension(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.dimensions.insert(key.into(), value.into());
        self
    }

    /// Add multiple dimensions.
    pub fn with_dimensions(mut self, dimensions: HashMap<String, String>) -> Self {
        self.dimensions.extend(dimensions);
        self
    }

    /// Create a duration metric.
    pub fn duration(name: impl Into<String>, duration_ms: u64) -> Self {
        Self::new(name, duration_ms as f64, MetricUnit::Milliseconds)
    }

    /// Create a count metric.
    pub fn count(name: impl Into<String>, count: u64) -> Self {
        Self::new(name, count as f64, MetricUnit::Count)
    }

    /// Create a size metric.
    pub fn bytes(name: impl Into<String>, bytes: usize) -> Self {
        Self::new(name, bytes as f64, MetricUnit::Bytes)
    }
}

/// Standard MCP metrics emitted by the observability middleware.
#[derive(Debug, Clone, Copy)]
pub struct StandardMetrics;

impl StandardMetrics {
    /// Metric name for request duration.
    pub const REQUEST_DURATION: &'static str = "mcp.request.duration";

    /// Metric name for request count.
    pub const REQUEST_COUNT: &'static str = "mcp.request.count";

    /// Metric name for error count.
    pub const REQUEST_ERRORS: &'static str = "mcp.request.errors";

    /// Metric name for response size.
    pub const RESPONSE_SIZE: &'static str = "mcp.response.size";

    /// Metric name for composition depth.
    pub const COMPOSITION_DEPTH: &'static str = "mcp.composition.depth";
}

/// Tracks the start time of a request for duration calculation.
///
/// This is stored in request context and used to calculate duration
/// when the response is sent.
#[derive(Debug, Clone)]
pub struct RequestStart {
    /// The instant when the request started.
    pub instant: Instant,

    /// The trace context for this request.
    pub trace: TraceContext,

    /// The operation details.
    pub operation: McpOperationDetails,

    /// The request metadata.
    pub metadata: RequestMetadata,
}

impl RequestStart {
    /// Create a new request start marker.
    pub fn new(trace: TraceContext, operation: McpOperationDetails) -> Self {
        Self {
            instant: Instant::now(),
            trace,
            operation,
            metadata: RequestMetadata::default(),
        }
    }

    /// Calculate elapsed time in milliseconds.
    pub fn elapsed_ms(&self) -> u64 {
        self.instant.elapsed().as_millis() as u64
    }

    /// Set the request metadata.
    pub fn with_metadata(mut self, metadata: RequestMetadata) -> Self {
        self.metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_event_creation() {
        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("get_weather");

        let event = McpRequestEvent::new(trace.clone(), "test-server", operation)
            .with_user_id("user-123")
            .with_tenant_id("tenant-456");

        assert_eq!(event.server_name, "test-server");
        assert_eq!(event.user_id, Some("user-123".to_string()));
        assert_eq!(event.tenant_id, Some("tenant-456".to_string()));
        assert_eq!(event.trace.trace_id, trace.trace_id);
    }

    #[test]
    fn test_response_event_success() {
        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("get_weather");

        let event = McpResponseEvent::success(trace.clone(), "test-server", operation, 150)
            .with_response_size(1024);

        assert!(event.success);
        assert_eq!(event.duration_ms, 150);
        assert_eq!(event.response_size, Some(1024));
        assert!(event.error_code.is_none());
    }

    #[test]
    fn test_response_event_failure() {
        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("get_weather");

        let event = McpResponseEvent::failure(
            trace.clone(),
            "test-server",
            operation,
            50,
            -32600,
            "Invalid request",
        );

        assert!(!event.success);
        assert_eq!(event.error_code, Some(-32600));
        assert_eq!(event.error_message, Some("Invalid request".to_string()));
    }

    #[test]
    fn test_metric_creation() {
        let metric = McpMetric::duration("mcp.request.duration", 150)
            .with_dimension("server", "test-server")
            .with_dimension("method", "tools/call");

        assert_eq!(metric.name, "mcp.request.duration");
        assert!((metric.value - 150.0).abs() < f64::EPSILON);
        assert_eq!(metric.unit, MetricUnit::Milliseconds);
        assert_eq!(
            metric.dimensions.get("server"),
            Some(&"test-server".to_string())
        );
    }

    #[test]
    fn test_request_start_elapsed() {
        let trace = TraceContext::new_root();
        let operation = McpOperationDetails::tool_call("test");
        let start = RequestStart::new(trace, operation);

        // Just verify it doesn't panic and returns something reasonable
        let elapsed = start.elapsed_ms();
        assert!(elapsed < 1000); // Should complete in less than a second
    }

    #[test]
    fn test_metric_unit_as_str() {
        assert_eq!(MetricUnit::Milliseconds.as_str(), "Milliseconds");
        assert_eq!(MetricUnit::Count.as_str(), "Count");
        assert_eq!(MetricUnit::Bytes.as_str(), "Bytes");
        assert_eq!(MetricUnit::Percent.as_str(), "Percent");
        assert_eq!(MetricUnit::None.as_str(), "None");
    }
}
