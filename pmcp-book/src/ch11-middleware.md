# Chapter 11: Middleware

Middleware in PMCP provides a powerful way to intercept, modify, and extend request/response processing. This chapter covers both the basic `Middleware` trait and the enhanced `AdvancedMiddleware` system with priority ordering, context propagation, and advanced patterns.

## Table of Contents

- [Understanding Middleware](#understanding-middleware)
- [Basic Middleware](#basic-middleware)
- [Advanced Middleware](#advanced-middleware)
- [Built-in Middleware](#built-in-middleware)
- [Custom Middleware](#custom-middleware)
- [Middleware Ordering](#middleware-ordering)
- [Performance Considerations](#performance-considerations)
- [Examples](#examples)

---

## Understanding Middleware

Middleware operates as a chain of interceptors that process messages bidirectionally:

```
Client                        Middleware Chain                      Server
   |                                                                    |
   |---- Request ---> [MW1] -> [MW2] -> [MW3] -> [Transport] --------->|
   |                    ↓        ↓        ↓                             |
   |<--- Response --- [MW1] <- [MW2] <- [MW3] <- [Transport] ----------|
   |                                                                    |
```

### When to Use Middleware

- **Cross-cutting concerns**: Logging, metrics, tracing
- **Request modification**: Authentication, compression, validation
- **Error handling**: Retry logic, circuit breakers
- **Performance optimization**: Caching, rate limiting
- **Observability**: Request tracking, performance monitoring

---

## Basic Middleware

The `Middleware` trait provides the foundation for request/response interception.

###  Trait Definition

```rust
use pmcp::shared::Middleware;
use async_trait::async_trait;

#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before a request is sent
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()>;

    /// Called after a response is received
    async fn on_response(&self, response: &mut JSONRPCResponse) -> Result<()>;

    /// Called when a message is sent (any type)
    async fn on_send(&self, message: &TransportMessage) -> Result<()>;

    /// Called when a message is received (any type)
    async fn on_receive(&self, message: &TransportMessage) -> Result<()>;
}
```

### Basic Example

```rust
use pmcp::shared::{Middleware, TransportMessage};
use pmcp::types::{JSONRPCRequest, JSONRPCResponse};
use async_trait::async_trait;
use std::time::Instant;

/// Custom middleware that tracks request timing
struct TimingMiddleware {
    start_times: dashmap::DashMap<String, Instant>,
}

impl TimingMiddleware {
    fn new() -> Self {
        Self {
            start_times: dashmap::DashMap::new(),
        }
    }
}

#[async_trait]
impl Middleware for TimingMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> pmcp::Result<()> {
        // Track start time
        self.start_times.insert(
            request.id.to_string(),
            Instant::now()
        );

        tracing::info!("Request started: {}", request.method);
        Ok(())
    }

    async fn on_response(&self, response: &mut JSONRPCResponse) -> pmcp::Result<()> {
        // Calculate elapsed time
        if let Some((_, start)) = self.start_times.remove(&response.id.to_string()) {
            let elapsed = start.elapsed();
            tracing::info!("Response for {} took {:?}", response.id, elapsed);
        }
        Ok(())
    }
}
```

### MiddlewareChain

Chain multiple middleware together for sequential processing:

```rust
use pmcp::shared::{MiddlewareChain, LoggingMiddleware};
use std::sync::Arc;
use tracing::Level;

// Create middleware chain
let mut chain = MiddlewareChain::new();

// Add middleware in order
chain.add(Arc::new(LoggingMiddleware::new(Level::INFO)));
chain.add(Arc::new(TimingMiddleware::new()));
chain.add(Arc::new(CustomMiddleware));

// Process request through all middleware
chain.process_request(&mut request).await?;

// Process response through all middleware
chain.process_response(&mut response).await?;
```

---

## Advanced Middleware

The `AdvancedMiddleware` trait adds priority ordering, context propagation, conditional execution, and lifecycle hooks.

### Trait Definition

```rust
use pmcp::shared::{AdvancedMiddleware, MiddlewareContext, MiddlewarePriority};

#[async_trait]
pub trait AdvancedMiddleware: Send + Sync {
    /// Get middleware priority for execution ordering
    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    /// Get middleware name for identification
    fn name(&self) -> &'static str;

    /// Check if middleware should execute for this context
    async fn should_execute(&self, context: &MiddlewareContext) -> bool {
        true
    }

    /// Called before a request is sent with context
    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()>;

    /// Called after a response is received with context
    async fn on_response_with_context(
        &self,
        response: &mut JSONRPCResponse,
        context: &MiddlewareContext,
    ) -> Result<()>;

    /// Lifecycle hooks
    async fn on_chain_start(&self, context: &MiddlewareContext) -> Result<()>;
    async fn on_chain_complete(&self, context: &MiddlewareContext) -> Result<()>;
    async fn on_error(&self, error: &Error, context: &MiddlewareContext) -> Result<()>;
}
```

### MiddlewarePriority

Control execution order with priority levels:

```rust
use pmcp::shared::MiddlewarePriority;

pub enum MiddlewarePriority {
    Critical = 0,  // Validation, security - executed first
    High = 1,      // Authentication, rate limiting
    Normal = 2,    // Business logic, transformation
    Low = 3,       // Logging, metrics
    Lowest = 4,    // Cleanup, finalization
}
```

**Execution order**: Higher priority (lower number) executes first for requests, last for responses.

### MiddlewareContext

Share data and metrics across middleware layers:

```rust
use pmcp::shared::MiddlewareContext;

let context = MiddlewareContext::with_request_id("req-123".to_string());

// Set metadata
context.set_metadata("user_id".to_string(), "user-456".to_string());

// Get metadata
if let Some(user_id) = context.get_metadata("user_id") {
    tracing::info!("User ID: {}", user_id);
}

// Record metrics
context.record_metric("processing_time_ms".to_string(), 123.45);

// Get elapsed time
let elapsed = context.elapsed();
```

### EnhancedMiddlewareChain

Automatic priority ordering and context support:

```rust
use pmcp::shared::{EnhancedMiddlewareChain, MiddlewareContext};
use std::sync::Arc;

// Create enhanced chain with auto-sorting
let mut chain = EnhancedMiddlewareChain::new();

// Add middleware (auto-sorted by priority)
chain.add(Arc::new(ValidationMiddleware));      // Critical
chain.add(Arc::new(RateLimitMiddleware::new(10, 20, Duration::from_secs(1))));  // High
chain.add(Arc::new(MetricsMiddleware::new("my-service".to_string())));  // Low

// Create context
let context = MiddlewareContext::with_request_id("req-001".to_string());

// Process with context
chain.process_request_with_context(&mut request, &context).await?;
chain.process_response_with_context(&mut response, &context).await?;
```

---

## Built-in Middleware

PMCP provides several production-ready middleware implementations.

### LoggingMiddleware

Logs all requests and responses at configurable levels:

```rust
use pmcp::shared::LoggingMiddleware;
use tracing::Level;

// Create logging middleware
let logger = LoggingMiddleware::new(Level::INFO);

// Or use default (DEBUG level)
let default_logger = LoggingMiddleware::default();
```

**Use cases**: Request/response visibility, debugging, audit trails.

### AuthMiddleware

Adds authentication to requests:

```rust
use pmcp::shared::AuthMiddleware;

let auth = AuthMiddleware::new("Bearer api-token-12345".to_string());
```

**Note**: This is a basic implementation. For production, implement custom auth middleware with your authentication scheme.

### RetryMiddleware

Configures retry behavior for failed requests:

```rust
use pmcp::shared::RetryMiddleware;

// Custom retry settings
let retry = RetryMiddleware::new(
    5,      // max_retries
    1000,   // initial_delay_ms
    30000   // max_delay_ms (exponential backoff cap)
);

// Or use defaults (3 retries, 1s initial, 30s max)
let default_retry = RetryMiddleware::default();
```

**Use cases**: Network resilience, transient failure handling.

### RateLimitMiddleware (Advanced)

Token bucket rate limiting with automatic refill:

```rust
use pmcp::shared::RateLimitMiddleware;
use std::time::Duration;

// 10 requests per second, burst of 20
let rate_limiter = RateLimitMiddleware::new(
    10,                        // max_requests per refill_duration
    20,                        // bucket_size (burst capacity)
    Duration::from_secs(1)     // refill_duration
);
```

**Features**:
- High priority (MiddlewarePriority::High)
- Automatic token refill based on time
- Thread-safe with atomic operations
- Records rate limit metrics in context

**Use cases**: API rate limiting, resource protection, QoS enforcement.

### CircuitBreakerMiddleware (Advanced)

Fault tolerance with automatic failure detection:

```rust
use pmcp::shared::CircuitBreakerMiddleware;
use std::time::Duration;

// Open circuit after 5 failures in 60s window, timeout for 30s
let circuit_breaker = CircuitBreakerMiddleware::new(
    5,                         // failure_threshold
    Duration::from_secs(60),   // time_window
    Duration::from_secs(30),   // timeout_duration
);
```

**States**:
- **Closed**: Normal operation, requests pass through
- **Open**: Too many failures, requests fail fast
- **Half-Open**: Testing if service recovered, limited requests allowed

**Features**:
- High priority (MiddlewarePriority::High)
- Automatic state transitions
- Records circuit breaker state in metrics

**Use cases**: Cascading failure prevention, service degradation, fault isolation.

### MetricsMiddleware (Advanced)

Collects performance and usage metrics:

```rust
use pmcp::shared::MetricsMiddleware;

let metrics = MetricsMiddleware::new("my-service".to_string());

// Query metrics
let request_count = metrics.get_request_count("tools/call");
let error_count = metrics.get_error_count("tools/call");
let avg_duration = metrics.get_average_duration("tools/call");  // in microseconds

tracing::info!(
    "Method: tools/call, Requests: {}, Errors: {}, Avg: {}μs",
    request_count,
    error_count,
    avg_duration
);
```

**Collected metrics**:
- Request counts per method
- Error counts per method
- Average processing time per method
- Total processing time

**Use cases**: Observability, performance monitoring, capacity planning.

### CompressionMiddleware (Advanced)

Compresses large messages to reduce network usage:

```rust
use pmcp::shared::{CompressionMiddleware, CompressionType};

// Gzip compression for messages larger than 1KB
let compression = CompressionMiddleware::new(
    CompressionType::Gzip,
    1024  // min_size in bytes
);

// Compression types
pub enum CompressionType {
    None,
    Gzip,
    Deflate,
}
```

**Features**:
- Normal priority (MiddlewarePriority::Normal)
- Size threshold to avoid compressing small messages
- Records compression metrics (original size, compression type)

**Use cases**: Large payload optimization, bandwidth reduction.

---

## Custom Middleware

### Basic Custom Middleware

```rust
use pmcp::shared::Middleware;
use pmcp::types::{JSONRPCRequest, JSONRPCResponse};
use async_trait::async_trait;

struct MetadataMiddleware {
    client_id: String,
}

#[async_trait]
impl Middleware for MetadataMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> pmcp::Result<()> {
        tracing::info!("Client {} sending request: {}", self.client_id, request.method);
        // Could add client_id to request params here
        Ok(())
    }

    async fn on_response(&self, response: &mut JSONRPCResponse) -> pmcp::Result<()> {
        tracing::info!("Client {} received response for: {:?}", self.client_id, response.id);
        Ok(())
    }
}
```

### Advanced Custom Middleware

```rust
use pmcp::shared::{AdvancedMiddleware, MiddlewareContext, MiddlewarePriority};
use pmcp::types::JSONRPCRequest;
use async_trait::async_trait;

struct ValidationMiddleware {
    strict_mode: bool,
}

#[async_trait]
impl AdvancedMiddleware for ValidationMiddleware {
    fn name(&self) -> &'static str {
        "validation"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Critical  // Run first
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
    ) -> pmcp::Result<()> {
        // Validate request
        if request.method.is_empty() {
            context.record_metric("validation_failures".to_string(), 1.0);
            return Err(pmcp::Error::Validation("Empty method name".to_string()));
        }

        if request.jsonrpc != "2.0" {
            context.record_metric("validation_failures".to_string(), 1.0);
            return Err(pmcp::Error::Validation("Invalid JSON-RPC version".to_string()));
        }

        context.set_metadata("method".to_string(), request.method.clone());
        context.record_metric("validation_passed".to_string(), 1.0);
        Ok(())
    }
}
```

---

## Middleware Ordering

### Recommended Order

```rust
use pmcp::shared::EnhancedMiddlewareChain;
use std::sync::Arc;

let mut chain = EnhancedMiddlewareChain::new();

// 1. Critical: Validation, security (first in, last out)
chain.add(Arc::new(ValidationMiddleware::new()));

// 2. High: Rate limiting, circuit breaker (protect downstream)
chain.add(Arc::new(RateLimitMiddleware::new(10, 20, Duration::from_secs(1))));
chain.add(Arc::new(CircuitBreakerMiddleware::new(
    5,
    Duration::from_secs(60),
    Duration::from_secs(30)
)));

// 3. Normal: Business logic, compression, transformation
chain.add(Arc::new(CompressionMiddleware::new(CompressionType::Gzip, 1024)));
chain.add(Arc::new(CustomBusinessLogic));

// 4. Low: Metrics, logging (observe everything)
chain.add(Arc::new(MetricsMiddleware::new("my-service".to_string())));
chain.add(Arc::new(LoggingMiddleware::new(Level::INFO)));
```

### Ordering Principles

1. **Validation First**: Reject invalid requests before doing expensive work
2. **Protection Before Processing**: Rate limit and circuit break early
3. **Transform in the Middle**: Business logic and compression
4. **Observe Everything**: Logging and metrics wrap all operations

### Manual Ordering (No Auto-Sort)

```rust
// Disable automatic priority sorting
let mut chain = EnhancedMiddlewareChain::new_no_sort();

// Add in explicit order
chain.add(Arc::new(FirstMiddleware));
chain.add(Arc::new(SecondMiddleware));
chain.add(Arc::new(ThirdMiddleware));

// Manual sort by priority if needed
chain.sort_by_priority();
```

---

## Performance Considerations

### Minimizing Overhead

```rust
// ✅ Good: Lightweight check
async fn on_request_with_context(
    &self,
    request: &mut JSONRPCRequest,
    context: &MiddlewareContext,
) -> pmcp::Result<()> {
    // Quick validation
    if !request.method.starts_with("tools/") {
        return Ok(());  // Skip early
    }

    // Expensive operation only when needed
    self.expensive_validation(request).await
}

// ❌ Bad: Always does expensive work
async fn on_request_with_context(
    &self,
    request: &mut JSONRPCRequest,
    context: &MiddlewareContext,
) -> pmcp::Result<()> {
    // Always expensive, even when unnecessary
    self.expensive_validation(request).await
}
```

### Async Best Practices

```rust
// ✅ Good: Non-blocking
async fn on_request_with_context(
    &self,
    request: &mut JSONRPCRequest,
    context: &MiddlewareContext,
) -> pmcp::Result<()> {
    // Async I/O is fine
    let user = self.user_service.get_user(&request.user_id).await?;
    context.set_metadata("user_name".to_string(), user.name);
    Ok(())
}

// ❌ Bad: Blocking in async
async fn on_request_with_context(
    &self,
    request: &mut JSONRPCRequest,
    context: &MiddlewareContext,
) -> pmcp::Result<()> {
    // Blocks the executor!
    let data = std::fs::read_to_string("config.json")?;
    Ok(())
}
```

### Conditional Execution

```rust
impl AdvancedMiddleware for ExpensiveMiddleware {
    async fn should_execute(&self, context: &MiddlewareContext) -> bool {
        // Only run for specific methods
        context.get_metadata("method")
            .map(|m| m.starts_with("tools/"))
            .unwrap_or(false)
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> pmcp::Result<()> {
        // This only runs if should_execute returned true
        self.expensive_operation(request).await
    }
}
```

### Performance Monitoring

```rust
use pmcp::shared::PerformanceMetrics;

let context = MiddlewareContext::default();

// Metrics are automatically collected
chain.process_request_with_context(&mut request, &context).await?;

// Access metrics
let metrics = context.metrics;
tracing::info!(
    "Requests: {}, Errors: {}, Avg time: {:?}",
    metrics.request_count(),
    metrics.error_count(),
    metrics.average_time()
);
```

---

## Examples

### Example 1: Basic Middleware Chain

See `examples/15_middleware.rs`:

```rust
use pmcp::shared::{MiddlewareChain, LoggingMiddleware};
use std::sync::Arc;
use tracing::Level;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    tracing_subscriber::fmt::init();

    // Create middleware chain
    let mut middleware = MiddlewareChain::new();
    middleware.add(Arc::new(LoggingMiddleware::new(Level::DEBUG)));
    middleware.add(Arc::new(TimingMiddleware::new()));

    // Use with transport/client
    // (middleware integration is transport-specific)

    Ok(())
}
```

### Example 2: Enhanced Middleware System

See `examples/30_enhanced_middleware.rs`:

```rust
use pmcp::shared::{
    EnhancedMiddlewareChain,
    MiddlewareContext,
    RateLimitMiddleware,
    CircuitBreakerMiddleware,
    MetricsMiddleware,
    CompressionMiddleware,
    CompressionType,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    tracing_subscriber::fmt().init();

    // Create enhanced chain
    let mut chain = EnhancedMiddlewareChain::new();

    // Add middleware (auto-sorted by priority)
    chain.add(Arc::new(ValidationMiddleware::new(false)));
    chain.add(Arc::new(RateLimitMiddleware::new(5, 10, Duration::from_secs(1))));
    chain.add(Arc::new(CircuitBreakerMiddleware::new(
        3,
        Duration::from_secs(10),
        Duration::from_secs(5)
    )));
    chain.add(Arc::new(MetricsMiddleware::new("my-service".to_string())));
    chain.add(Arc::new(CompressionMiddleware::new(CompressionType::Gzip, 1024)));

    tracing::info!("Middleware chain configured with {} middleware", chain.len());

    // Create context
    let context = MiddlewareContext::with_request_id("req-001".to_string());

    // Process requests
    let mut request = create_test_request();
    chain.process_request_with_context(&mut request, &context).await?;

    Ok(())
}
```

### Example 3: Custom Validation Middleware

```rust
use pmcp::shared::{AdvancedMiddleware, MiddlewareContext, MiddlewarePriority};
use async_trait::async_trait;

// Uses your preferred JSON Schema library (e.g., jsonschema)
struct SchemaValidationMiddleware {
    schemas: Arc<HashMap<String, JsonSchema>>,
}

#[async_trait]
impl AdvancedMiddleware for SchemaValidationMiddleware {
    fn name(&self) -> &'static str {
        "schema_validation"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Critical
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> pmcp::Result<()> {
        // Get schema for this method
        let schema = self.schemas.get(&request.method)
            .ok_or_else(|| pmcp::Error::Validation(
                format!("No schema for method: {}", request.method)
            ))?;

        // Validate params against schema
        if let Some(ref params) = request.params {
            schema.validate(params).map_err(|e| {
                context.record_metric("schema_validation_failed".to_string(), 1.0);
                pmcp::Error::Validation(format!("Schema validation failed: {}", e))
            })?;
        }

        context.record_metric("schema_validation_passed".to_string(), 1.0);
        Ok(())
    }
}
```

---

## Summary

### Key Takeaways

1. **Two Middleware Systems**: Basic `Middleware` for simple cases, `AdvancedMiddleware` for production
2. **Priority Ordering**: Control execution order with `MiddlewarePriority`
3. **Context Propagation**: Share data and metrics with `MiddlewareContext`
4. **Built-in Patterns**: Rate limiting, circuit breakers, metrics, compression
5. **Conditional Execution**: `should_execute()` for selective middleware
6. **Performance**: Use `should_execute()`, async operations, and metrics tracking

### When to Use Each System

**Basic Middleware (`MiddlewareChain`)**:
- Simple logging or tracing
- Development and debugging
- Lightweight request modification

**Advanced Middleware (`EnhancedMiddlewareChain`)**:
- Production deployments
- Complex ordering requirements
- Performance monitoring
- Fault tolerance patterns (rate limiting, circuit breakers)
- Context-dependent behavior

### Best Practices

1. **Keep Middleware Focused**: Single responsibility per middleware
2. **Order Matters**: Validation → Protection → Logic → Observation
3. **Use Priorities**: Let `EnhancedMiddlewareChain` auto-sort
4. **Conditional Execution**: Skip expensive operations when possible
5. **Monitor Performance**: Use `PerformanceMetrics` and context
6. **Handle Errors Gracefully**: Implement `on_error()` for cleanup
7. **Test in Isolation**: Unit test middleware independently

### Examples Reference

- `examples/15_middleware.rs`: Basic middleware chain
- `examples/30_enhanced_middleware.rs`: Advanced patterns with built-in middleware
- Inline doctests in `src/shared/middleware.rs` demonstrate each middleware

---

## HTTP-Level Middleware

HTTP-level middleware operates at the HTTP transport layer, before MCP protocol processing. This is useful for header injection, authentication, compression, and other HTTP-specific concerns.

### Architecture: Two-Layer Middleware System

PMCP has two distinct middleware layers:

```
┌─────────────────────────────────────────────────────────────┐
│  Client Application                                         │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  Protocol-Level Middleware (AdvancedMiddleware)             │
│  - Operates on JSONRPCRequest/JSONRPCResponse               │
│  - LoggingMiddleware, MetricsMiddleware, ValidationMiddleware│
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  HTTP-Level Middleware (HttpMiddleware)                     │
│  - Operates on HTTP request/response                        │
│  - OAuthClientMiddleware, header injection, compression     │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│  HTTP Transport (StreamableHttpTransport)                   │
└─────────────────────────────────────────────────────────────┘
```

**Key principle**: Middleware doesn't run twice. Protocol-level operates on JSON-RPC messages, HTTP-level operates on HTTP requests.

### HttpMiddleware Trait

```rust
use pmcp::client::http_middleware::{HttpMiddleware, HttpRequest, HttpResponse, HttpMiddlewareContext};
use async_trait::async_trait;

#[async_trait]
pub trait HttpMiddleware: Send + Sync {
    /// Called before HTTP request is sent
    async fn on_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        Ok(())
    }

    /// Called after HTTP response is received
    async fn on_response(
        &self,
        response: &mut HttpResponse,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        Ok(())
    }

    /// Called when an error occurs
    async fn on_error(
        &self,
        error: &pmcp::Error,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        Ok(())
    }

    /// Priority for ordering (lower runs first)
    fn priority(&self) -> i32 {
        50 // Default priority
    }

    /// Should this middleware execute for this context?
    async fn should_execute(&self, _context: &HttpMiddlewareContext) -> bool {
        true
    }
}
```

### HttpRequest and HttpResponse

Simplified HTTP representations for middleware:

```rust
pub struct HttpRequest {
    pub method: String,           // "GET", "POST", etc.
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

pub struct HttpResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}
```

### HttpMiddlewareContext

Context for HTTP middleware execution:

```rust
pub struct HttpMiddlewareContext {
    pub request_id: Option<String>,
    pub url: String,
    pub method: String,
    pub attempt: u32,
    pub metadata: Arc<RwLock<HashMap<String, String>>>,
}

// Usage
let context = HttpMiddlewareContext::new(url.to_string(), "POST".to_string());
context.set_metadata("user_id".to_string(), "user-123".to_string());
let user_id = context.get_metadata("user_id");
```

### OAuthClientMiddleware

Built-in OAuth middleware for automatic token injection:

```rust
use pmcp::client::oauth_middleware::{OAuthClientMiddleware, BearerToken};
use std::time::Duration;

// Create bearer token
let token = BearerToken::new("my-api-token".to_string());

// Or with expiration
let token = BearerToken::with_expiry(
    "my-api-token".to_string(),
    Duration::from_secs(3600) // 1 hour
);

// Create OAuth middleware
let oauth_middleware = OAuthClientMiddleware::new(token);

// Add to HttpMiddlewareChain
let mut http_chain = HttpMiddlewareChain::new();
http_chain.add(Arc::new(oauth_middleware));
```

**Features**:
- Automatic token injection into `Authorization` header
- Token expiry checking before each request
- 401/403 detection for token refresh triggers
- OAuth precedence policy (respects transport auth_provider)

### OAuth Precedence Policy

To avoid duplicate authentication, OAuth middleware follows this precedence:

```
1. Transport auth_provider (highest priority)
   ↓
2. HttpMiddleware OAuth (OAuthClientMiddleware)
   ↓
3. Extra headers from config (lowest priority)
```

The middleware checks `auth_already_set` metadata to skip injection when transport auth is configured:

```rust
// OAuth middleware checks metadata
if context.get_metadata("auth_already_set").is_some() {
    // Skip - transport auth_provider takes precedence
    return Ok(());
}

// Also skips if Authorization header already present
if request.has_header("Authorization") {
    // Warn about duplicate auth configuration
    return Ok(());
}
```

### Example: Custom HTTP Middleware

```rust
use pmcp::client::http_middleware::{HttpMiddleware, HttpRequest, HttpMiddlewareContext};
use async_trait::async_trait;

/// Adds custom correlation headers
struct CorrelationHeaderMiddleware {
    service_name: String,
}

#[async_trait]
impl HttpMiddleware for CorrelationHeaderMiddleware {
    async fn on_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        // Add service name header
        request.add_header("X-Service-Name".to_string(), self.service_name.clone());

        // Add timestamp header
        request.add_header(
            "X-Request-Timestamp".to_string(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string(),
        );

        // Add request ID if available
        if let Some(req_id) = &context.request_id {
            request.add_header("X-Request-ID".to_string(), req_id.clone());
        }

        Ok(())
    }

    fn priority(&self) -> i32 {
        20 // Run after OAuth (priority 10)
    }
}
```

### Integration with ClientBuilder

Use `ClientBuilder::with_middleware()` for protocol-level middleware:

```rust
use pmcp::ClientBuilder;
use pmcp::shared::{MetricsMiddleware, LoggingMiddleware};
use std::sync::Arc;

let transport = /* your transport */;

let client = ClientBuilder::new(transport)
    .with_middleware(Arc::new(MetricsMiddleware::new("my-client".to_string())))
    .with_middleware(Arc::new(LoggingMiddleware::default()))
    .build();
```

**Note**: HTTP middleware is configured separately on the transport (StreamableHttpTransport), not via ClientBuilder.

### Integration with StreamableHttpTransport

Configure HTTP middleware when creating the transport:

```rust
use pmcp::shared::{StreamableHttpTransport, StreamableHttpConfig};
use pmcp::client::http_middleware::HttpMiddlewareChain;
use pmcp::client::oauth_middleware::{OAuthClientMiddleware, BearerToken};
use std::sync::Arc;
use std::time::Duration;

// Create HTTP middleware chain
let mut http_chain = HttpMiddlewareChain::new();

// Add OAuth middleware
let token = BearerToken::with_expiry(
    "api-token-12345".to_string(),
    Duration::from_secs(3600)
);
http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));

// Add custom middleware
http_chain.add(Arc::new(CorrelationHeaderMiddleware {
    service_name: "my-client".to_string(),
}));

// Create transport config with HTTP middleware
let config = StreamableHttpConfig::new("https://api.example.com".to_string())
    .with_http_middleware(Arc::new(http_chain));

let transport = StreamableHttpTransport::with_config(config).await?;
```

### Complete Example: OAuth + Protocol Middleware

```rust
use pmcp::{ClientBuilder, ClientCapabilities};
use pmcp::shared::{StreamableHttpTransport, StreamableHttpConfig, MetricsMiddleware};
use pmcp::client::http_middleware::HttpMiddlewareChain;
use pmcp::client::oauth_middleware::{OAuthClientMiddleware, BearerToken};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // 1. Create HTTP middleware chain
    let mut http_chain = HttpMiddlewareChain::new();

    // Add OAuth (priority 10 - runs first)
    let token = BearerToken::with_expiry(
        std::env::var("API_TOKEN")?,
        Duration::from_secs(3600)
    );
    http_chain.add(Arc::new(OAuthClientMiddleware::new(token)));

    // Add correlation headers (priority 20 - runs after OAuth)
    http_chain.add(Arc::new(CorrelationHeaderMiddleware {
        service_name: "my-service".to_string(),
    }));

    // 2. Create transport with HTTP middleware
    let config = StreamableHttpConfig::new("https://api.example.com".to_string())
        .with_http_middleware(Arc::new(http_chain));

    let transport = StreamableHttpTransport::with_config(config).await?;

    // 3. Create client with protocol middleware
    let client = ClientBuilder::new(transport)
        .with_middleware(Arc::new(MetricsMiddleware::new("my-client".to_string())))
        .build();

    // 4. Use client - both middleware layers automatically apply
    let mut client = client;
    let init_result = client.initialize(ClientCapabilities::minimal()).await?;

    println!("Connected: {}", init_result.server_info.name);

    Ok(())
}
```

### Middleware Execution Flow

For a typical HTTP POST request:

```
1. Client.call_tool(name, args)
   ↓
2. Protocol Middleware (Request):
   - MetricsMiddleware::on_request()
   - LoggingMiddleware::on_request()
   ↓
3. Transport serialization (JSON-RPC → bytes)
   ↓
4. HTTP Middleware (Request):
   - OAuthClientMiddleware::on_request() → Add Authorization header
   - CorrelationHeaderMiddleware::on_request() → Add X-Service-Name, X-Request-ID
   ↓
5. HTTP Transport sends POST request
   ↓
6. HTTP Transport receives response
   ↓
7. HTTP Middleware (Response) [reverse order]:
   - CorrelationHeaderMiddleware::on_response()
   - OAuthClientMiddleware::on_response() → Check for 401
   ↓
8. Transport deserialization (bytes → JSON-RPC)
   ↓
9. Protocol Middleware (Response) [reverse order]:
   - LoggingMiddleware::on_response()
   - MetricsMiddleware::on_response()
   ↓
10. Client receives result
```

### Error Handling

Both middleware layers support error hooks:

```rust
#[async_trait]
impl HttpMiddleware for MyMiddleware {
    async fn on_error(
        &self,
        error: &pmcp::Error,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()> {
        // Log error with context
        tracing::error!(
            "HTTP error for {} {}: {}",
            context.method,
            context.url,
            error
        );

        // Clean up resources if needed
        self.cleanup().await;

        Ok(())
    }
}
```

**Short-circuit behavior**: If middleware returns an error:
1. Processing stops immediately
2. `on_error()` is called for ALL middleware in the chain
3. Original error is returned to caller

### Middleware Priority Reference

**HTTP Middleware**:
- `0-9`: Reserved for critical security middleware
- `10`: OAuthClientMiddleware (default)
- `20-49`: Custom authentication/authorization
- `50`: Default priority
- `51-99`: Logging, metrics, headers

**Protocol Middleware**:
- `Critical (0)`: Validation, security
- `High (1)`: Rate limiting, circuit breakers
- `Normal (2)`: Business logic, compression
- `Low (3)`: Metrics, logging
- `Lowest (4)`: Cleanup

### Further Reading

- Repository docs: `docs/advanced/middleware-composition.md`
- Advanced Middleware API: https://docs.rs/pmcp/latest/pmcp/shared/middleware/
- Performance Metrics API: https://docs.rs/pmcp/latest/pmcp/shared/middleware/struct.PerformanceMetrics.html
- Example: `examples/40_middleware_demo.rs` - Complete two-layer middleware demonstration
