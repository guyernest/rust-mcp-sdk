//! Enhanced Middleware System Example
//!
//! PMCP-4004: Demonstrates the enhanced middleware system with advanced capabilities:
//! - Priority-based middleware ordering
//! - Rate limiting and circuit breaker patterns
//! - Metrics collection and performance monitoring
//! - Conditional middleware execution
//! - Context propagation across middleware layers
//!
//! Run with: cargo run --example 30_enhanced_middleware --features full

use async_trait::async_trait;
use pmcp::shared::{
    AdvancedMiddleware, CircuitBreakerMiddleware, CompressionMiddleware, CompressionType,
    EnhancedMiddlewareChain, MetricsMiddleware, MiddlewareContext, MiddlewarePriority,
    RateLimitMiddleware, Transport, TransportMessage,
};
use pmcp::types::{
    JSONRPCRequest, JSONRPCResponse, Notification, ProgressNotification, ProgressToken, RequestId,
};
use pmcp::Result;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, Level};

/// Custom middleware for request validation
#[derive(Debug)]
struct ValidationMiddleware {
    strict_mode: bool,
}

impl ValidationMiddleware {
    fn new(strict_mode: bool) -> Self {
        Self { strict_mode }
    }
}

#[async_trait]
impl AdvancedMiddleware for ValidationMiddleware {
    fn name(&self) -> &'static str {
        "validation"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Critical
    }

    async fn should_execute(&self, context: &MiddlewareContext) -> bool {
        // Only execute for high-priority requests in strict mode
        if self.strict_mode {
            matches!(
                context.priority,
                Some(pmcp::shared::transport::MessagePriority::High)
            )
        } else {
            true
        }
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        // Validate request format
        if request.method.is_empty() {
            context.record_metric("validation_failures".to_string(), 1.0);
            return Err(pmcp::Error::Validation("Empty method name".to_string()));
        }

        if request.jsonrpc != "2.0" {
            context.record_metric("validation_failures".to_string(), 1.0);
            return Err(pmcp::Error::Validation(
                "Invalid JSON-RPC version".to_string(),
            ));
        }

        context.record_metric("validation_passed".to_string(), 1.0);
        context.set_metadata("method".to_string(), request.method.clone());
        info!("Request validation passed for method: {}", request.method);
        Ok(())
    }
}

/// Mock transport for demonstration
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MockTransport {
    id: u32,
}

impl MockTransport {
    #[allow(dead_code)]
    fn new(id: u32) -> Self {
        Self { id }
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&mut self, _message: TransportMessage) -> Result<()> {
        info!("MockTransport {} sending message", self.id);
        tokio::time::sleep(Duration::from_millis(10)).await;
        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(TransportMessage::Notification(Notification::Progress(
            ProgressNotification {
                progress_token: ProgressToken::String(format!("mock-{}", self.id)),
                progress: 50.0,
                message: Some(format!("Mock message from transport {}", self.id)),
            },
        )))
    }

    async fn close(&mut self) -> Result<()> {
        info!("MockTransport {} closed", self.id);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        true
    }

    fn transport_type(&self) -> &'static str {
        "mock"
    }
}

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("🚀 Starting Enhanced Middleware System Example");

    // Create enhanced middleware chain
    let mut chain = EnhancedMiddlewareChain::new();

    info!("🔧 Setting up middleware chain with various middleware types...");

    // Add different middleware types with various priorities
    chain.add(Arc::new(ValidationMiddleware::new(false)));
    chain.add(Arc::new(RateLimitMiddleware::new(
        5,
        10,
        Duration::from_secs(1),
    )));
    chain.add(Arc::new(CircuitBreakerMiddleware::new(
        3,
        Duration::from_secs(10),
        Duration::from_secs(5),
    )));
    chain.add(Arc::new(MetricsMiddleware::new(
        "enhanced_middleware_example".to_string(),
    )));
    chain.add(Arc::new(CompressionMiddleware::new(
        CompressionType::Gzip,
        1024,
    )));

    info!(
        "✅ Middleware chain configured with {} middleware",
        chain.len()
    );
    info!("  • Priority ordering: Critical → High → Normal → Low → Lowest");
    info!("  • Validation (Critical priority)");
    info!("  • Rate Limiting (High priority): 5 req/sec, burst of 10");
    info!("  • Circuit Breaker (High priority): 3 failures in 10s window");
    info!("  • Metrics Collection (Low priority)");
    info!("  • Compression (Normal priority): Gzip for messages >1KB");

    // Create contexts with different priorities
    let contexts = [
        MiddlewareContext {
            request_id: Some("req-001".to_string()),
            priority: Some(pmcp::shared::transport::MessagePriority::High),
            ..Default::default()
        },
        MiddlewareContext {
            request_id: Some("req-002".to_string()),
            priority: Some(pmcp::shared::transport::MessagePriority::Normal),
            ..Default::default()
        },
        MiddlewareContext {
            request_id: Some("req-003".to_string()),
            priority: Some(pmcp::shared::transport::MessagePriority::Low),
            ..Default::default()
        },
    ];

    info!("🎯 Testing middleware chain with different priority contexts...");

    // Test requests with different priorities
    for (i, context) in contexts.iter().enumerate() {
        info!(
            "Testing request {} with priority {:?}",
            i + 1,
            context.priority
        );

        let mut request = JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: format!("test.method_{}", i + 1),
            params: Some(serde_json::json!({
                "data": format!("test data for request {}", i + 1),
                "timestamp": chrono::Utc::now().to_rfc3339(),
            })),
            id: RequestId::from(i as i64 + 1),
        };

        // Process request through middleware chain
        match chain
            .process_request_with_context(&mut request, context)
            .await
        {
            Ok(()) => {
                info!("  ✓ Request {} processed successfully", i + 1);

                // Create a mock response
                let mut response = JSONRPCResponse {
                    jsonrpc: "2.0".to_string(),
                    id: request.id.clone(),
                    payload: pmcp::types::jsonrpc::ResponsePayload::Result(
                        serde_json::json!({"status": "success", "request_id": i + 1}),
                    ),
                };

                // Process response through middleware chain
                if let Err(e) = chain
                    .process_response_with_context(&mut response, context)
                    .await
                {
                    info!("  ⚠ Response processing failed: {}", e);
                } else {
                    info!("  ✓ Response {} processed successfully", i + 1);
                }
            },
            Err(e) => {
                info!("  ❌ Request {} failed: {}", i + 1, e);
            },
        }

        // Test message processing
        let test_message =
            TransportMessage::Notification(Notification::Progress(ProgressNotification {
                progress_token: ProgressToken::String(format!("progress-{}", i + 1)),
                progress: 25.0 * (i + 1) as f64,
                message: Some(format!("Processing request {}", i + 1)),
            }));

        if let Err(e) = chain
            .process_send_with_context(&test_message, context)
            .await
        {
            info!("  ⚠ Message send processing failed: {}", e);
        } else {
            info!("  ✓ Message send processed");
        }

        // Add delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Demonstrate rate limiting by sending multiple requests rapidly
    info!("🚦 Testing rate limiting with rapid requests...");
    let rate_limit_context = MiddlewareContext::with_request_id("rate-test".to_string());

    for i in 0..12 {
        let mut request = JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "rapid.test".to_string(),
            params: Some(serde_json::json!({"request_number": i})),
            id: RequestId::from((i + 100) as i64),
        };

        match chain
            .process_request_with_context(&mut request, &rate_limit_context)
            .await
        {
            Ok(()) => info!("  ✓ Rapid request {} allowed", i + 1),
            Err(pmcp::Error::RateLimited) => info!("  🛑 Rapid request {} rate limited", i + 1),
            Err(e) => info!("  ❌ Rapid request {} failed: {}", i + 1, e),
        }

        // Small delay to demonstrate burst behavior
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // Demonstrate conditional middleware execution
    info!("🎛️ Testing conditional middleware execution...");
    let strict_chain = {
        let mut chain = EnhancedMiddlewareChain::new();
        chain.add(Arc::new(ValidationMiddleware::new(true))); // Strict mode
        chain
    };

    let test_contexts = vec![
        (
            "High priority",
            MiddlewareContext {
                request_id: Some("conditional-high".to_string()),
                priority: Some(pmcp::shared::transport::MessagePriority::High),
                ..Default::default()
            },
        ),
        (
            "Normal priority",
            MiddlewareContext {
                request_id: Some("conditional-normal".to_string()),
                priority: Some(pmcp::shared::transport::MessagePriority::Normal),
                ..Default::default()
            },
        ),
    ];

    for (name, context) in test_contexts {
        let mut request = JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            method: "conditional.test".to_string(),
            params: None,
            id: RequestId::from(200i64),
        };

        match strict_chain
            .process_request_with_context(&mut request, &context)
            .await
        {
            Ok(()) => info!("  ✓ {} request processed (validation executed)", name),
            Err(e) => info!("  ❌ {} request failed: {}", name, e),
        }
    }

    // Display performance metrics
    info!("📊 Performance and context features:");
    info!("  • Context propagation: Metadata and metrics passed between middleware");
    info!("  • Priority-based ordering: Middleware sorted by importance");
    info!("  • Conditional execution: Middleware can be selectively enabled");
    info!("  • Error handling: Failed middleware notifies all other middleware");
    info!("  • Performance tracking: Built-in timing and metrics collection");

    info!("🔄 Enhanced middleware system benefits:");
    info!("  • Automatic priority-based middleware ordering");
    info!("  • Rich context propagation across middleware layers");
    info!("  • Built-in performance monitoring and metrics");
    info!("  • Conditional middleware execution based on context");
    info!("  • Advanced patterns: rate limiting, circuit breaker, compression");
    info!("  • Comprehensive error handling and recovery");

    info!("👋 Enhanced middleware system demonstration complete");

    Ok(())
}
