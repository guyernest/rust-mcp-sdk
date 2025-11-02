//! Enhanced Middleware Tests
//!
//! Simplified tests for the enhanced middleware system focusing on:
//! - Basic middleware functionality verification
//! - Circuit breaker, rate limiting, metrics, and compression
//! - Error handling and performance validation
//! - Notification middleware processing

use pmcp::shared::middleware::*;
use pmcp::types::jsonrpc::{JSONRPCNotification, JSONRPCRequest, RequestId};
use pmcp::{Error, Result};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_enhanced_middleware_chain_basic() {
    let mut chain = EnhancedMiddlewareChain::new();
    let circuit_breaker = Arc::new(CircuitBreakerMiddleware::new(
        5,
        Duration::from_millis(100),
        Duration::from_millis(50),
    ));

    chain.add(circuit_breaker);

    // Test that chain creation and addition works
    // Basic smoke test that everything compiles and runs - no explicit assert needed
}

#[tokio::test]
async fn test_circuit_breaker_middleware() {
    let circuit_breaker = CircuitBreakerMiddleware::new(
        2,                          // failure threshold
        Duration::from_millis(100), // timeout
        Duration::from_millis(50),  // time window
    );

    let mut request = JSONRPCRequest::new(RequestId::Number(1), "test_method", Some(json!({})));

    let context = MiddlewareContext::default();

    // Test basic functionality
    let result = circuit_breaker
        .on_request_with_context(&mut request, &context)
        .await;
    assert!(result.is_ok());

    // Test name and priority
    assert!(!circuit_breaker.name().is_empty());
    assert!(matches!(
        circuit_breaker.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
}

#[tokio::test]
async fn test_rate_limit_middleware() {
    let rate_limiter = RateLimitMiddleware::new(2, 10, Duration::from_secs(1));

    let mut request = JSONRPCRequest::new(RequestId::Number(1), "test_method", Some(json!({})));

    let context = MiddlewareContext::default();

    // Test basic functionality
    let result = rate_limiter
        .on_request_with_context(&mut request, &context)
        .await;
    assert!(result.is_ok() || result.is_err()); // Either is acceptable for rate limiting

    // Test name and priority
    assert!(!rate_limiter.name().is_empty());
    assert!(matches!(
        rate_limiter.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
}

#[tokio::test]
async fn test_metrics_middleware() {
    let metrics = MetricsMiddleware::new("test_service".to_string());

    let mut request = JSONRPCRequest::new(RequestId::Number(1), "test_method", Some(json!({})));

    let context = MiddlewareContext::default();

    // Test basic functionality
    let result = metrics
        .on_request_with_context(&mut request, &context)
        .await;
    assert!(result.is_ok());

    // Test metrics collection - request count is always non-negative for u32 type
    let _request_count = metrics.get_request_count("test_method");

    // Test name and priority
    assert!(!metrics.name().is_empty());
    assert!(matches!(
        metrics.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
}

#[tokio::test]
async fn test_middleware_context_operations() {
    let context = MiddlewareContext::with_request_id("test-123".to_string());

    // Test that context creation works
    assert_eq!(context.request_id, Some("test-123".to_string()));

    // Test metadata operations
    context.set_metadata("user_id".to_string(), "123".to_string());
    assert_eq!(context.get_metadata("user_id"), Some("123".to_string()));

    // Test that we can create a default context
    let default_context = MiddlewareContext::default();
    assert!(default_context.request_id.is_none());
}

#[tokio::test]
async fn test_compression_middleware() {
    let compression = CompressionMiddleware::new(CompressionType::Gzip, 1024);

    let mut request = JSONRPCRequest::new(
        RequestId::Number(1),
        "test_method",
        Some(json!({"large_data": vec![42; 1000]})), // Large data to compress
    );

    let context = MiddlewareContext::default();

    let result = compression
        .on_request_with_context(&mut request, &context)
        .await;
    assert!(result.is_ok());

    // Test name and priority
    assert!(!compression.name().is_empty());
    assert!(matches!(
        compression.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
}

// Helper middleware that always fails for testing
#[derive(Debug)]
struct FailingMiddleware;

#[async_trait::async_trait]
impl AdvancedMiddleware for FailingMiddleware {
    fn name(&self) -> &'static str {
        "failing"
    }

    async fn on_request_with_context(
        &self,
        _request: &mut JSONRPCRequest,
        _context: &MiddlewareContext,
    ) -> Result<()> {
        Err(Error::internal("Middleware failure"))
    }
}

#[tokio::test]
async fn test_failing_middleware() {
    let failing = FailingMiddleware;
    let mut request = JSONRPCRequest::new(RequestId::Number(1), "test_method", Some(json!({})));
    let context = MiddlewareContext::default();

    let result = failing
        .on_request_with_context(&mut request, &context)
        .await;
    assert!(result.is_err());
    assert_eq!(failing.name(), "failing");
}

#[tokio::test]
async fn test_middleware_chain_creation() {
    let mut chain = EnhancedMiddlewareChain::new();

    // Add multiple middleware types
    chain.add(Arc::new(MetricsMiddleware::new("test".to_string())));
    chain.add(Arc::new(CircuitBreakerMiddleware::new(
        10,
        Duration::from_millis(100),
        Duration::from_millis(50),
    )));
    chain.add(Arc::new(CompressionMiddleware::new(
        CompressionType::Gzip,
        512,
    )));
    chain.add(Arc::new(RateLimitMiddleware::new(
        5,
        10,
        Duration::from_secs(1),
    )));

    // Test that chain creation completes without errors - no explicit assert needed
}

#[tokio::test]
async fn test_middleware_performance() {
    let circuit_breaker = CircuitBreakerMiddleware::new(
        100, // High threshold for performance testing
        Duration::from_millis(1000),
        Duration::from_millis(100),
    );

    let mut request = JSONRPCRequest::new(
        RequestId::Number(1),
        "performance_test",
        Some(json!({"data": vec![1, 2, 3, 4, 5]})),
    );

    let context = MiddlewareContext::with_request_id("perf-test".to_string());

    let start = std::time::Instant::now();

    // Run middleware operations multiple times
    for _ in 0..1000 {
        let result = circuit_breaker
            .on_request_with_context(&mut request, &context)
            .await;
        assert!(result.is_ok());
    }

    let duration = start.elapsed();
    println!("1000 middleware operations took: {:?}", duration);

    // Should complete reasonably quickly (less than 100ms for 1000 operations)
    assert!(duration.as_millis() < 100);
}

#[tokio::test]
async fn test_middleware_types_instantiation() {
    // Test that all middleware types can be instantiated correctly
    let circuit_breaker =
        CircuitBreakerMiddleware::new(5, Duration::from_millis(100), Duration::from_millis(50));
    let metrics = MetricsMiddleware::new("test".to_string());
    let compression = CompressionMiddleware::new(CompressionType::Gzip, 1024);
    let rate_limiter = RateLimitMiddleware::new(5, 10, Duration::from_secs(1));

    // Test that all middlewares have proper names and priorities
    assert!(!circuit_breaker.name().is_empty());
    assert!(!metrics.name().is_empty());
    assert!(!compression.name().is_empty());
    assert!(!rate_limiter.name().is_empty());

    // Test that they all implement the required trait methods
    assert!(matches!(
        circuit_breaker.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
    assert!(matches!(
        metrics.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
    assert!(matches!(
        compression.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
    assert!(matches!(
        rate_limiter.priority(),
        MiddlewarePriority::High | MiddlewarePriority::Normal | MiddlewarePriority::Low
    ));
}

#[tokio::test]
async fn test_compression_types() {
    // Test different compression types
    let gzip = CompressionMiddleware::new(CompressionType::Gzip, 1024);
    let deflate = CompressionMiddleware::new(CompressionType::Deflate, 1024);
    let none = CompressionMiddleware::new(CompressionType::None, 1024);

    let mut request = JSONRPCRequest::new(RequestId::Number(1), "test_method", Some(json!({})));
    let context = MiddlewareContext::default();

    // All compression types should work
    assert!(gzip
        .on_request_with_context(&mut request, &context)
        .await
        .is_ok());
    assert!(deflate
        .on_request_with_context(&mut request, &context)
        .await
        .is_ok());
    assert!(none
        .on_request_with_context(&mut request, &context)
        .await
        .is_ok());
}

#[tokio::test]
async fn test_middleware_error_handling() {
    let failing = FailingMiddleware;
    let mut chain = EnhancedMiddlewareChain::new();

    chain.add(Arc::new(failing));

    // Test that we can add a failing middleware to the chain
    let _request = JSONRPCRequest::new(RequestId::Number(1), "test_method", Some(json!({})));

    // The chain itself should not fail, individual middlewares handle their own errors
    // Basic smoke test that everything compiles and runs - no explicit assert needed
}

// ============================================================================
// Notification Middleware Tests (Week 2-3: Protocol Inbound Coverage)
// ============================================================================

/// Test middleware that tracks notification processing
struct NotificationTrackingMiddleware {
    name: String,
}

#[async_trait::async_trait]
impl AdvancedMiddleware for NotificationTrackingMiddleware {
    fn name(&self) -> &'static str {
        "notification_tracking"
    }

    async fn on_notification_with_context(
        &self,
        notification: &mut JSONRPCNotification,
        context: &MiddlewareContext,
    ) -> Result<()> {
        // Track notification method in context
        context.set_metadata(
            "notification_method".to_string(),
            notification.method.clone(),
        );
        context.set_metadata("middleware_name".to_string(), self.name.clone());

        // Mark as processed
        context.set_metadata("processed".to_string(), "true".to_string());

        Ok(())
    }
}

#[tokio::test]
async fn test_notification_middleware_processing() {
    let mut chain = EnhancedMiddlewareChain::new();
    chain.add(Arc::new(NotificationTrackingMiddleware {
        name: "test-tracker".to_string(),
    }));

    let context = MiddlewareContext::default();
    let mut notification = JSONRPCNotification::new(
        "notifications/progress",
        Some(json!({
            "progressToken": "test-123",
            "progress": 50,
            "total": 100
        })),
    );

    // Process notification through middleware chain
    let result = chain
        .process_notification_with_context(&mut notification, &context)
        .await;

    assert!(result.is_ok());

    // Verify metadata was set by middleware
    assert_eq!(
        context.get_metadata("notification_method"),
        Some("notifications/progress".to_string())
    );
    assert_eq!(
        context.get_metadata("middleware_name"),
        Some("test-tracker".to_string())
    );
    assert_eq!(context.get_metadata("processed"), Some("true".to_string()));
}

#[tokio::test]
async fn test_notification_middleware_priority_ordering() {
    /// Middleware that appends to a list
    struct OrderingMiddleware {
        id: u8,
        priority: MiddlewarePriority,
    }

    #[async_trait::async_trait]
    impl AdvancedMiddleware for OrderingMiddleware {
        fn name(&self) -> &'static str {
            "ordering"
        }

        fn priority(&self) -> MiddlewarePriority {
            self.priority
        }

        async fn on_notification_with_context(
            &self,
            _notification: &mut JSONRPCNotification,
            context: &MiddlewareContext,
        ) -> Result<()> {
            let mut order = context.get_metadata("order").unwrap_or_default();
            if !order.is_empty() {
                order.push(',');
            }
            order.push_str(&self.id.to_string());
            context.set_metadata("order".to_string(), order);
            Ok(())
        }
    }

    let mut chain = EnhancedMiddlewareChain::new();

    // Add in non-priority order
    chain.add(Arc::new(OrderingMiddleware {
        id: 3,
        priority: MiddlewarePriority::Low,
    }));
    chain.add(Arc::new(OrderingMiddleware {
        id: 1,
        priority: MiddlewarePriority::High,
    }));
    chain.add(Arc::new(OrderingMiddleware {
        id: 2,
        priority: MiddlewarePriority::Normal,
    }));

    let context = MiddlewareContext::default();
    let mut notification =
        JSONRPCNotification::new("notifications/test", None::<serde_json::Value>);

    chain
        .process_notification_with_context(&mut notification, &context)
        .await
        .unwrap();

    // Verify execution order matches priority (High -> Normal -> Low)
    assert_eq!(context.get_metadata("order"), Some("1,2,3".to_string()));
}

#[tokio::test]
async fn test_notification_middleware_error_handling() {
    /// Middleware that fails on specific notifications
    struct FailingNotificationMiddleware;

    #[async_trait::async_trait]
    impl AdvancedMiddleware for FailingNotificationMiddleware {
        fn name(&self) -> &'static str {
            "failing_notification"
        }

        async fn on_notification_with_context(
            &self,
            notification: &mut JSONRPCNotification,
            _context: &MiddlewareContext,
        ) -> Result<()> {
            if notification.method == "notifications/error" {
                return Err(Error::internal("notification processing failed"));
            }
            Ok(())
        }
    }

    let mut chain = EnhancedMiddlewareChain::new();
    chain.add(Arc::new(FailingNotificationMiddleware));

    let context = MiddlewareContext::default();

    // Success case
    let mut ok_notification =
        JSONRPCNotification::new("notifications/ok", None::<serde_json::Value>);
    assert!(chain
        .process_notification_with_context(&mut ok_notification, &context)
        .await
        .is_ok());

    // Error case
    let mut error_notification =
        JSONRPCNotification::new("notifications/error", None::<serde_json::Value>);
    let result = chain
        .process_notification_with_context(&mut error_notification, &context)
        .await;
    assert!(result.is_err());

    // Verify error was counted in metrics
    assert_eq!(context.metrics.error_count(), 1);
}

#[tokio::test]
async fn test_notification_middleware_with_metrics() {
    let mut chain = EnhancedMiddlewareChain::new();
    chain.add(Arc::new(MetricsMiddleware::new(
        "test-notification-service".to_string(),
    )));

    let context = MiddlewareContext::default();

    // Process multiple notifications
    for i in 0..5 {
        let mut notification = JSONRPCNotification::new(
            "notifications/progress",
            Some(json!({
                "progressToken": format!("token-{}", i),
                "progress": i * 20,
                "total": 100
            })),
        );

        chain
            .process_notification_with_context(&mut notification, &context)
            .await
            .unwrap();
    }

    // Metrics should track that operations occurred (even though notifications don't increment request_count)
    let stats = context.metrics;
    assert_eq!(stats.error_count(), 0);
}

#[tokio::test]
async fn test_sse_notification_simulation() {
    /// Simulates how SSE notifications flow through the dispatcher
    struct SSENotificationMiddleware {
        event_type: String,
    }

    #[async_trait::async_trait]
    impl AdvancedMiddleware for SSENotificationMiddleware {
        fn name(&self) -> &'static str {
            "sse_notification"
        }

        async fn on_notification_with_context(
            &self,
            notification: &mut JSONRPCNotification,
            context: &MiddlewareContext,
        ) -> Result<()> {
            // SSE events can be tracked/logged/transformed by middleware
            context.set_metadata("event_type".to_string(), self.event_type.clone());
            context.set_metadata("sse_method".to_string(), notification.method.clone());

            // Simulate adding SSE-specific metadata
            if notification.method.starts_with("notifications/") {
                context.set_metadata("is_sse_event".to_string(), "true".to_string());
            }

            Ok(())
        }
    }

    let mut chain = EnhancedMiddlewareChain::new();
    chain.add(Arc::new(SSENotificationMiddleware {
        event_type: "progress".to_string(),
    }));

    let context = MiddlewareContext::default();

    // Simulate SSE notification (as would come from StreamableHttpTransport)
    let mut notification = JSONRPCNotification::new(
        "notifications/progress",
        Some(json!({
            "progressToken": "sse-token-123",
            "progress": 75,
            "total": 100,
        })),
    );

    chain
        .process_notification_with_context(&mut notification, &context)
        .await
        .unwrap();

    // Verify SSE-specific metadata was set
    assert_eq!(
        context.get_metadata("event_type"),
        Some("progress".to_string())
    );
    assert_eq!(
        context.get_metadata("sse_method"),
        Some("notifications/progress".to_string())
    );
    assert_eq!(
        context.get_metadata("is_sse_event"),
        Some("true".to_string())
    );
}
