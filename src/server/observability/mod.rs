//! Observability infrastructure for MCP servers.
//!
//! This module provides comprehensive observability for MCP servers including:
//! - **Distributed Tracing**: Trace context propagation across composed servers
//! - **Event Logging**: Structured request/response events
//! - **Metrics**: Duration, count, and error rate metrics
//! - **Multi-Backend Support**: Console, CloudWatch, or custom backends
//!
//! # Design Principles
//!
//! 1. **AuthContext as Single Source of Truth**: User identity (user_id, tenant_id)
//!    is ONLY extracted from `AuthContext`. The observability types (`TraceContext`,
//!    `RequestMetadata`) contain NO user identity fields.
//!
//! 2. **Non-Blocking**: All event emission is asynchronous and non-blocking.
//!
//! 3. **Configurable**: Field capture, sampling, and backends are all configurable.
//!
//! 4. **Privacy-Conscious**: Sensitive data capture (IP, full arguments) is opt-in.
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use pmcp::server::observability::{
//!     McpObservabilityMiddleware, ObservabilityConfig, ConsoleBackend,
//! };
//! use std::sync::Arc;
//!
//! // Create development configuration with console output
//! let config = ObservabilityConfig::development();
//! let backend = Arc::new(ConsoleBackend::new(config.console.clone()));
//!
//! // Create middleware
//! let middleware = McpObservabilityMiddleware::new(
//!     "my-server",
//!     config,
//!     backend,
//! );
//!
//! // Add to your MCP server
//! server_builder.with_tool_middleware(Arc::new(middleware));
//! ```
//!
//! # Configuration
//!
//! Configuration can be loaded from:
//! 1. TOML file (`.pmcp-config.toml`)
//! 2. Environment variables (with `PMCP_OBSERVABILITY_` prefix)
//!
//! ```toml
//! [observability]
//! enabled = true
//! backend = "console"  # or "cloudwatch"
//! sample_rate = 1.0
//!
//! [observability.fields]
//! capture_tool_name = true
//! capture_user_id = true
//! capture_arguments_hash = false  # Privacy-sensitive
//!
//! [observability.metrics]
//! request_count = true
//! request_duration = true
//! tool_usage = true
//! ```
//!
//! # Backends
//!
//! ## Console Backend
//!
//! For local development with pretty-printed output:
//!
//! ```rust,ignore
//! use pmcp::server::observability::{ConsoleBackend, ConsoleConfig};
//!
//! let config = ConsoleConfig { pretty: true, verbose: false };
//! let backend = ConsoleBackend::new(config);
//! ```
//!
//! ## CloudWatch Backend
//!
//! For production with EMF metrics:
//!
//! ```rust,ignore
//! use pmcp::server::observability::{CloudWatchBackend, CloudWatchConfig};
//!
//! let config = CloudWatchConfig::default();
//! let backend = CloudWatchBackend::new(config);
//! ```
//!
//! ## Composite Backend
//!
//! Combine multiple backends:
//!
//! ```rust,ignore
//! use pmcp::server::observability::CompositeBackend;
//!
//! let composite = CompositeBackend::new()
//!     .with(console_backend)
//!     .with(cloudwatch_backend);
//! ```
//!
//! # Trace Propagation
//!
//! The middleware automatically propagates trace context through:
//! - Request metadata for composed MCP calls
//! - HTTP headers for external service calls
//!
//! ```rust,ignore
//! use pmcp::server::observability::TraceContext;
//!
//! // Create root trace at entry point
//! let root = TraceContext::new_root();
//!
//! // Create child trace for downstream call
//! let child = root.child();
//! assert_eq!(root.trace_id, child.trace_id);
//! assert_eq!(child.depth, 1);
//! ```

mod backend;
mod config;
mod events;
mod middleware;
mod types;

// Re-export public types
pub use backend::{
    CloudWatchBackend, CloudWatchConfig, CompositeBackend, ConsoleBackend, NullBackend,
    ObservabilityBackend,
};
pub use config::{
    ConfigError, ConsoleConfig, FieldsConfig, MetricsConfig, ObservabilityConfig, TracingConfig,
};
pub use events::{
    McpMetric, McpRequestEvent, McpResponseEvent, MetricUnit, RequestStart, StandardMetrics,
};
pub use middleware::McpObservabilityMiddleware;
pub use types::{hash_value, McpOperationDetails, RequestMetadata, TraceContext};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_end_to_end_observability() {
        // Create configuration
        let config = ObservabilityConfig::development();

        // Create console backend (pretty = true for development)
        let backend = Arc::new(ConsoleBackend::new(config.console.pretty));

        // Create middleware
        let middleware = McpObservabilityMiddleware::new("test-server", config, backend);

        // Verify middleware is properly constructed
        assert!(format!("{:?}", middleware).contains("test-server"));
    }

    #[test]
    fn test_trace_context_propagation() {
        // Root trace
        let root = TraceContext::new_root();
        assert_eq!(root.depth, 0);
        assert!(root.parent_span_id.is_none());

        // First hop
        let child1 = root.child();
        assert_eq!(child1.depth, 1);
        assert_eq!(child1.trace_id, root.trace_id);
        assert_eq!(child1.parent_span_id, Some(root.span_id.clone()));

        // Second hop
        let child2 = child1.child();
        assert_eq!(child2.depth, 2);
        assert_eq!(child2.trace_id, root.trace_id);
        assert_eq!(child2.parent_span_id, Some(child1.span_id.clone()));
    }

    #[test]
    fn test_config_loading_from_toml() {
        let toml = r#"
            [observability]
            enabled = true
            backend = "cloudwatch"
            sample_rate = 0.5

            [observability.fields]
            capture_tool_name = true
            capture_user_id = true

            [observability.cloudwatch]
            namespace = "TestApp/MCP"
        "#;

        let config = ObservabilityConfig::from_toml(toml).unwrap();

        assert!(config.enabled);
        assert_eq!(config.backend, "cloudwatch");
        assert!((config.sample_rate - 0.5).abs() < f64::EPSILON);
        assert!(config.fields.capture_tool_name);
        assert_eq!(config.cloudwatch.namespace, "TestApp/MCP");
    }

    #[test]
    fn test_operation_details_tool_call() {
        let details = McpOperationDetails::tool_call("get_weather");

        assert_eq!(details.method, "tools/call");
        assert_eq!(details.tool_name, Some("get_weather".to_string()));
        assert_eq!(details.operation_name(), Some("get_weather"));
    }

    #[test]
    fn test_request_metadata_builder() {
        let metadata = RequestMetadata::default()
            .with_client_type("claude-desktop")
            .with_client_version("1.2.3")
            .with_session_id("session-123");

        assert_eq!(metadata.client_type, Some("claude-desktop".to_string()));
        assert_eq!(metadata.client_version, Some("1.2.3".to_string()));
        assert_eq!(metadata.session_id, Some("session-123".to_string()));
    }

    #[tokio::test]
    async fn test_composite_backend() {
        let null1 = Arc::new(NullBackend);
        let null2 = Arc::new(NullBackend);

        let composite = CompositeBackend::new(vec![null1, null2]);

        // Should not panic
        composite.flush().await;
        assert_eq!(composite.len(), 2);
    }
}
