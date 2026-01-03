# Middleware Architecture

PMCP's middleware system provides extensible hooks for request/response processing. This section covers building custom middleware, understanding priority ordering, and implementing common observability patterns.

## What is Middleware?

If you're new to middleware, think of it as a series of checkpoints that every request passes through before reaching your actual business logic (and every response passes through on the way back). It's like airport security—passengers (requests) go through multiple screening stations, each with a specific purpose.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    The Middleware Mental Model                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Without Middleware:                 With Middleware:                   │
│  ═══════════════════                 ════════════════                   │
│                                                                         │
│  Client → Tool Handler → Response    Client                             │
│                                          │                              │
│  Every handler must:                     ▼                              │
│  • Validate requests                 ┌────────────┐                     │
│  • Log operations                    │ Validation │ ← Check request     │
│  • Track timing                      └─────┬──────┘                     │
│  • Handle rate limits                      │                            │
│  • Manage authentication                   ▼                            │
│  • Record metrics                    ┌────────────┐                     │
│  • ...for every single tool!         │ Auth Check │ ← Verify identity   │
│                                      └─────┬──────┘                     │
│  Problems:                                 │                            │
│  • Duplicated code everywhere              ▼                            │
│  • Easy to forget steps               ┌────────────┐                    │
│  • Inconsistent behavior              │ Rate Limit │ ← Control traffic  │
│  • Hard to change globally            └─────┬──────┘                    │
│                                             │                           │
│                                             ▼                           │
│                                        ┌────────────┐                   │
│                                        │   Your     │                   │
│                                        │  Handler   │ ← Business logic  │
│                                        └─────┬──────┘   ONLY            │
│                                              │                          │
│                                              ▼                          │
│                                        ┌────────────┐                   │
│                                        │  Logging   │ ← Record result   │
│                                        └─────┬──────┘                   │
│                                              │                          │
│                                              ▼                          │
│                                          Response                       │
│                                                                         │
│  Benefits:                                                              │
│  ✓ Write validation ONCE, apply to ALL requests                        │
│  ✓ Handlers focus purely on business logic                             │
│  ✓ Consistent behavior across all tools                                │
│  ✓ Easy to add/remove cross-cutting concerns                           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Cross-Cutting Concerns

Middleware handles "cross-cutting concerns"—functionality that applies across your entire application rather than to specific features:

| Concern | Without Middleware | With Middleware |
|---------|-------------------|-----------------|
| **Logging** | Add log statements to every handler | Single logging middleware logs all requests |
| **Authentication** | Check auth in every handler | Auth middleware validates once, sets context |
| **Rate limiting** | Implement counters in each handler | Rate limit middleware protects everything |
| **Metrics** | Record timing in every handler | Metrics middleware measures automatically |
| **Error handling** | Try-catch in every handler | Error middleware provides consistent responses |

### The Pipeline Pattern

Middleware forms a **pipeline** where each piece processes the request, optionally modifies it, and passes it to the next piece. This pattern is common across web frameworks (Express.js, Django, Axum) and enterprise systems.

```rust
// Each middleware can:
// 1. Inspect the request
// 2. Modify the request
// 3. Short-circuit (return early without calling the next middleware)
// 4. Pass to the next middleware
// 5. Inspect/modify the response on the way back
```

## The AdvancedMiddleware Trait

PMCP's enhanced middleware system uses the `AdvancedMiddleware` trait:

```rust
use async_trait::async_trait;
use pmcp::shared::{AdvancedMiddleware, MiddlewareContext, MiddlewarePriority};
use pmcp::types::{JSONRPCRequest, JSONRPCResponse};
use pmcp::Result;

#[async_trait]
pub trait AdvancedMiddleware: Send + Sync {
    /// Execution priority (lower = runs first)
    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    /// Middleware name for identification
    fn name(&self) -> &'static str {
        "unknown"
    }

    /// Conditional execution check
    async fn should_execute(&self, context: &MiddlewareContext) -> bool {
        true
    }

    /// Process outgoing request
    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        Ok(())
    }

    /// Process incoming response
    async fn on_response_with_context(
        &self,
        response: &mut JSONRPCResponse,
        context: &MiddlewareContext,
    ) -> Result<()> {
        Ok(())
    }
}
```

### Execution Order

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Middleware Execution Flow                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│                           REQUEST PATH                                  │
│                           ════════════                                  │
│                                                                         │
│  Client Request                                                         │
│       │                                                                 │
│       ▼                                                                 │
│  ┌───────────────┐                                                      │
│  │ Critical (0)  │  ← Validation, security checks                       │
│  └───────┬───────┘                                                      │
│          │                                                              │
│          ▼                                                              │
│  ┌───────────────┐                                                      │
│  │ High (1)      │  ← Rate limiting, authentication                     │
│  └───────┬───────┘                                                      │
│          │                                                              │
│          ▼                                                              │
│  ┌───────────────┐                                                      │
│  │ Normal (2)    │  ← Business logic transforms                         │
│  └───────┬───────┘                                                      │
│          │                                                              │
│          ▼                                                              │
│  ┌───────────────┐                                                      │
│  │ Low (3)       │  ← Logging, metrics recording                        │
│  └───────┬───────┘                                                      │
│          │                                                              │
│          ▼                                                              │
│  ┌───────────────┐                                                      │
│  │ Lowest (4)    │  ← Cleanup, finalization                             │
│  └───────┬───────┘                                                      │
│          │                                                              │
│          ▼                                                              │
│     Tool Handler                                                        │
│          │                                                              │
│          │                                                              │
│                           RESPONSE PATH                                 │
│                           ═════════════                                 │
│          │                                                              │
│          ▼                                                              │
│  ┌───────────────┐                                                      │
│  │ Lowest (4)    │  ← Response timing recorded                          │
│  └───────┬───────┘                                                      │
│          │                                                              │
│          ▼                                                              │
│  ┌───────────────┐                                                      │
│  │ Low (3)       │  ← Response logged                                   │
│  └───────┬───────┘                                                      │
│          │                                                              │
│          ▼                                                              │
│  ... (continues up to Critical)                                         │
│          │                                                              │
│          ▼                                                              │
│  Client Response                                                        │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Building Custom Middleware

### Request Timing Middleware

Track how long requests take:

```rust
use async_trait::async_trait;
use pmcp::shared::{AdvancedMiddleware, MiddlewareContext, MiddlewarePriority};
use pmcp::types::{JSONRPCRequest, JSONRPCResponse};
use pmcp::Result;
use std::sync::Arc;
use dashmap::DashMap;
use std::time::Instant;

pub struct TimingMiddleware {
    start_times: DashMap<String, Instant>,
}

impl TimingMiddleware {
    pub fn new() -> Self {
        Self {
            start_times: DashMap::new(),
        }
    }
}

#[async_trait]
impl AdvancedMiddleware for TimingMiddleware {
    fn name(&self) -> &'static str {
        "timing"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Low  // Run late so we time everything
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        // Record start time keyed by request ID
        if let Some(ref request_id) = context.request_id {
            self.start_times.insert(request_id.clone(), Instant::now());
        }

        tracing::debug!(
            method = %request.method,
            request_id = ?context.request_id,
            "Request started"
        );

        Ok(())
    }

    async fn on_response_with_context(
        &self,
        response: &mut JSONRPCResponse,
        context: &MiddlewareContext,
    ) -> Result<()> {
        // Calculate duration
        if let Some(ref request_id) = context.request_id {
            if let Some((_, start)) = self.start_times.remove(request_id) {
                let duration = start.elapsed();

                // Record in context metrics
                context.record_metric(
                    "request_duration_ms".to_string(),
                    duration.as_millis() as f64
                );

                tracing::info!(
                    request_id = %request_id,
                    duration_ms = %duration.as_millis(),
                    "Request completed"
                );
            }
        }

        Ok(())
    }
}
```

### Validation Middleware

Validate requests before they reach handlers:

```rust
use pmcp::shared::{AdvancedMiddleware, MiddlewareContext, MiddlewarePriority};
use pmcp::Error;

pub struct ValidationMiddleware {
    strict_mode: bool,
}

#[async_trait]
impl AdvancedMiddleware for ValidationMiddleware {
    fn name(&self) -> &'static str {
        "validation"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Critical  // Run first - block invalid requests
    }

    async fn should_execute(&self, context: &MiddlewareContext) -> bool {
        // In non-strict mode, only validate high-priority requests
        if !self.strict_mode {
            matches!(
                context.priority,
                Some(pmcp::shared::transport::MessagePriority::High)
            )
        } else {
            true  // Always validate in strict mode
        }
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            context.record_metric("validation_failures".to_string(), 1.0);
            return Err(Error::Validation(
                "Invalid JSON-RPC version".to_string()
            ));
        }

        // Validate method not empty
        if request.method.is_empty() {
            context.record_metric("validation_failures".to_string(), 1.0);
            return Err(Error::Validation(
                "Method name cannot be empty".to_string()
            ));
        }

        // Store method in context for later middleware
        context.set_metadata("method".to_string(), request.method.clone());
        context.record_metric("validation_passed".to_string(), 1.0);

        Ok(())
    }
}
```

### Request ID Middleware

Generate correlation IDs for distributed tracing:

```rust
use uuid::Uuid;

pub struct RequestIdMiddleware;

#[async_trait]
impl AdvancedMiddleware for RequestIdMiddleware {
    fn name(&self) -> &'static str {
        "request_id"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Critical  // Run first to set ID
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let request_id = Uuid::new_v4().to_string();

        // Store in context for other middleware
        context.set_metadata("request_id".to_string(), request_id.clone());
        context.set_metadata("correlation_id".to_string(), request_id.clone());

        // Optionally inject into request params
        if let Some(params) = request.params.as_mut() {
            if let Some(obj) = params.as_object_mut() {
                obj.insert(
                    "_request_id".to_string(),
                    serde_json::json!(request_id)
                );
            }
        }

        tracing::info!(
            request_id = %request_id,
            method = %request.method,
            "Assigned request ID"
        );

        Ok(())
    }

    async fn on_response_with_context(
        &self,
        _response: &mut JSONRPCResponse,
        context: &MiddlewareContext,
    ) -> Result<()> {
        if let Some(request_id) = context.get_metadata("request_id") {
            tracing::debug!(
                request_id = %request_id,
                "Response completed for request"
            );
        }
        Ok(())
    }
}
```

## Building Middleware Chains

Combine middleware into an execution chain:

```rust
use pmcp::shared::EnhancedMiddlewareChain;
use std::sync::Arc;

fn build_observability_chain() -> EnhancedMiddlewareChain {
    let mut chain = EnhancedMiddlewareChain::new();

    // Add middleware (automatically sorted by priority)
    chain.add(Arc::new(RequestIdMiddleware));
    chain.add(Arc::new(ValidationMiddleware { strict_mode: true }));
    chain.add(Arc::new(TimingMiddleware::new()));
    chain.add(Arc::new(MetricsMiddleware::new("my-server".to_string())));

    chain
}
```

### Using Built-in Observability (Recommended)

For standard observability needs, use the built-in module instead of building custom chains:

```rust
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::observability::ObservabilityConfig;

// Using ServerCoreBuilder
let server = ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .tool("echo", EchoTool)
    .capabilities(ServerCapabilities::tools_only())
    .with_observability(ObservabilityConfig::development())
    .build()?;

// Or using Server::builder() (same API)
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .tool("echo", EchoTool)
    .with_observability(ObservabilityConfig::production())
    .build()?;
```

This adds a pre-configured `McpObservabilityMiddleware` that handles:
- Distributed tracing with `TraceContext`
- Request/response event logging
- Automatic metrics collection
- Console or CloudWatch output

See the [Built-in Observability Module](./ch17-middleware.md#built-in-observability-module-recommended) section for full configuration options.

### Integrating with ClientBuilder

```rust
use pmcp::{ClientBuilder, StdioTransport};

async fn create_instrumented_client() -> pmcp::Result<Client> {
    let transport = StdioTransport::new();

    let client = ClientBuilder::new(transport)
        .with_middleware(Arc::new(RequestIdMiddleware))
        .with_middleware(Arc::new(TimingMiddleware::new()))
        .with_middleware(Arc::new(MetricsMiddleware::new("my-client".to_string())))
        .build();

    Ok(client)
}
```

### Using Middleware Presets

PMCP provides pre-configured middleware for common scenarios:

```rust
use pmcp::shared::middleware_presets::PresetConfig;
use pmcp::{ClientBuilder, StdioTransport};

// For stdio transport
let client = ClientBuilder::new(StdioTransport::new())
    .middleware_chain(PresetConfig::stdio().build_protocol_chain())
    .build();

// For HTTP transport
let http_chain = PresetConfig::http().build_protocol_chain();
```

## HTTP-Level Middleware

For HTTP transports, PMCP provides a separate middleware layer:

```rust
use async_trait::async_trait;
use pmcp::server::http_middleware::{
    ServerHttpMiddleware, ServerHttpContext, ServerHttpResponse,
};

/// CORS middleware for browser clients
#[derive(Debug, Clone)]
struct CorsMiddleware {
    allowed_origins: Vec<String>,
}

#[async_trait]
impl ServerHttpMiddleware for CorsMiddleware {
    async fn on_response(
        &self,
        response: &mut ServerHttpResponse,
        _context: &ServerHttpContext,
    ) -> pmcp::Result<()> {
        response.add_header(
            "Access-Control-Allow-Origin",
            &self.allowed_origins.join(", ")
        );
        response.add_header(
            "Access-Control-Allow-Methods",
            "GET, POST, OPTIONS"
        );
        response.add_header(
            "Access-Control-Allow-Headers",
            "Content-Type, Authorization, MCP-Session-ID"
        );
        response.add_header("Access-Control-Max-Age", "86400");

        Ok(())
    }

    fn priority(&self) -> i32 {
        90  // Run after logging
    }
}
```

### HTTP Logging with Redaction

PMCP's `ServerHttpLoggingMiddleware` provides secure logging:

```rust
use pmcp::server::http_middleware::{
    ServerHttpLoggingMiddleware,
    ServerHttpMiddlewareChain,
};

let mut http_chain = ServerHttpMiddlewareChain::new();

let logging = ServerHttpLoggingMiddleware::new()
    .with_level(tracing::Level::INFO)
    .with_redact_query(true)        // Strip query params from logs
    .with_max_body_bytes(1024);     // Limit body logging size

http_chain.add(Arc::new(logging));
```

**Automatically redacted headers**:
- `Authorization`
- `Cookie`
- `X-Api-Key`

### Complete Server Setup

```rust
use pmcp::server::streamable_http_server::{
    StreamableHttpServer,
    StreamableHttpServerConfig,
};

// Build server with HTTP middleware
let server = Server::builder()
    .name("instrumented-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities::tools_only())
    .tool("echo", EchoTool)
    .with_http_middleware(Arc::new(http_chain))
    .build()?;

// Create HTTP server config
let config = StreamableHttpServerConfig {
    http_middleware: server.http_middleware(),
    session_id_generator: Some(Box::new(|| {
        format!("session-{}", uuid::Uuid::new_v4())
    })),
    enable_json_response: true,
    ..Default::default()
};

let http_server = StreamableHttpServer::with_config(
    "0.0.0.0:8080".parse().unwrap(),
    Arc::new(Mutex::new(server)),
    config
);

let (addr, handle) = http_server.start().await?;
```

## Context Propagation

The `MiddlewareContext` enables data sharing between middleware:

```rust
#[derive(Debug, Clone)]
pub struct MiddlewareContext {
    /// Request ID for correlation
    pub request_id: Option<String>,

    /// Custom metadata (thread-safe)
    pub metadata: Arc<DashMap<String, String>>,

    /// Performance metrics
    pub metrics: Arc<PerformanceMetrics>,

    /// Request start time
    pub start_time: Instant,

    /// Priority level
    pub priority: Option<MessagePriority>,
}

impl MiddlewareContext {
    /// Store metadata for other middleware
    pub fn set_metadata(&self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Retrieve metadata from earlier middleware
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.get(key).map(|v| v.clone())
    }

    /// Record a metric value
    pub fn record_metric(&self, name: String, value: f64) {
        self.metrics.record(name, value);
    }

    /// Get elapsed time since request started
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}
```

### Context Usage Pattern

```rust
// Early middleware sets context
async fn on_request_with_context(
    &self,
    request: &mut JSONRPCRequest,
    context: &MiddlewareContext,
) -> Result<()> {
    // Set user ID from auth token
    context.set_metadata("user_id".to_string(), "user-123".to_string());
    context.set_metadata("tenant_id".to_string(), "acme-corp".to_string());
    Ok(())
}

// Later middleware reads context
async fn on_request_with_context(
    &self,
    request: &mut JSONRPCRequest,
    context: &MiddlewareContext,
) -> Result<()> {
    let user_id = context.get_metadata("user_id")
        .unwrap_or_else(|| "anonymous".to_string());

    tracing::info!(
        user_id = %user_id,
        method = %request.method,
        "Audit log: User invoked method"
    );
    Ok(())
}
```

## Resilience Patterns

Production systems fail. Networks drop connections, databases become overloaded, external APIs go down. **Resilience patterns** are defensive programming techniques that help your system survive and recover from these failures gracefully, rather than cascading into complete outages.

PMCP includes middleware implementing two critical resilience patterns: **rate limiting** and **circuit breakers**.

### Rate Limiting

#### What is Rate Limiting?

Rate limiting controls how many requests a client can make within a time window. Think of it like a bouncer at a club—only letting in a certain number of people per hour to prevent overcrowding.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Rate Limiting Visualized                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Without Rate Limiting:              With Rate Limiting:                │
│  ══════════════════════              ═══════════════════                │
│                                                                         │
│     Client A ─┐                     Client A ─┐                         │
│     Client A ─┤                     Client A ─┤  ┌──────────┐           │
│     Client A ─┤                     Client A ─┼──│   Rate   │           │
│     Client A ─┼──▶ Server 💥        Client A ─┤  │  Limiter │──▶ Server │
│     Client A ─┤    (overwhelmed)    Client A ─┘  │          │           │
│     Client A ─┘                                  │  5 req/s │           │
│                                     Client A ─┬──│          │           │
│  Result:                            Client A ─┤  └────┬─────┘           │
│  • Server crashes                   Client A ─┘       │                 │
│  • All users affected                                 ▼                 │
│  • Potential data loss                        "Rate Limited"            │
│                                               (try again later)         │
│                                                                         │
│  Result with limiting:                                                  │
│  • Server stays healthy                                                 │
│  • Fair access for all clients                                          │
│  • Excess requests get clear feedback                                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

#### Why Rate Limiting Matters

| Problem | How Rate Limiting Helps |
|---------|------------------------|
| **DoS attacks** | Prevents malicious clients from overwhelming your server |
| **Runaway AI loops** | Stops buggy AI clients from making infinite tool calls |
| **Resource exhaustion** | Protects expensive operations (database queries, API calls) |
| **Fair usage** | Ensures no single client monopolizes server capacity |
| **Cost control** | Limits calls to expensive external APIs (GPT-4, cloud services) |

#### The Token Bucket Algorithm

PMCP's rate limiter uses the **token bucket algorithm**, which provides smooth rate limiting with burst tolerance:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Token Bucket Algorithm                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────┐                                                │
│  │    Token Bucket     │      Tokens added at fixed rate                │
│  │   ┌─────────────┐   │      (e.g., 5 tokens per second)               │
│  │   │ ● ● ● ● ●   │   │◄──── Bucket has max capacity                   │
│  │   │ ● ● ●       │   │      (e.g., 10 tokens = burst capacity)        │
│  │   └──────┬──────┘   │                                                │
│  └──────────┼──────────┘                                                │
│             │                                                           │
│             ▼                                                           │
│        Each request                                                     │
│        consumes 1 token                                                 │
│             │                                                           │
│             ▼                                                           │
│  ┌──────────────────────────────────────────┐                           │
│  │ Tokens available? ─────▶ Process request │                           │
│  │                   ─────▶ Reject (429)    │                           │
│  │        No                                │                           │
│  └──────────────────────────────────────────┘                           │
│                                                                         │
│  Example: 5 req/sec rate, 10 burst capacity                             │
│                                                                         │
│  Time 0s: Bucket full (10 tokens)                                       │
│  Time 0s: 8 requests arrive → 8 processed, 2 tokens left                │
│  Time 1s: 5 tokens added → 7 tokens available                           │
│  Time 1s: 3 requests arrive → 3 processed, 4 tokens left                │
│  Time 2s: 5 tokens added → 9 tokens (capped at 10)                      │
│                                                                         │
│  Key: Burst allows brief spikes above the steady-state rate             │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

#### When to Use Rate Limiting

- **Always** for public-facing MCP servers
- **Always** when calling expensive external APIs
- When serving multiple clients with shared resources
- When you have resource constraints (memory, CPU, database connections)
- When cost per request matters (cloud API calls, AI model inference)

#### PMCP Rate Limiting Implementation

```rust
use pmcp::shared::RateLimitMiddleware;
use std::time::Duration;

// Configure the rate limiter
let rate_limiter = RateLimitMiddleware::new(
    5,                          // Requests per window (steady rate)
    10,                         // Burst capacity (max tokens in bucket)
    Duration::from_secs(1),     // Window size (token refill period)
);

// This configuration means:
// - Sustained rate: 5 requests per second
// - Burst: Up to 10 requests if bucket is full
// - After burst: Must wait for tokens to refill
```

### Circuit Breaker

#### What is a Circuit Breaker?

A circuit breaker is a pattern borrowed from electrical engineering. Just as an electrical circuit breaker trips to prevent house fires when there's too much current, a software circuit breaker "trips" to prevent cascade failures when a dependency is failing.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Circuit Breaker States                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                                                                 │    │
│  │            ┌──────────┐                                         │    │
│  │   ┌───────▶│  CLOSED  │◀───────┐                                │    │
│  │   │        │(Normal)  │        │                                │    │
│  │   │        └────┬─────┘        │                                │    │
│  │   │             │              │                                │    │
│  │   │   Failures exceed         Success in                        │    │
│  │   │   threshold               half-open state                   │    │
│  │   │             │              │                                │    │
│  │   │             ▼              │                                │    │
│  │   │        ┌──────────┐        │                                │    │
│  │   │        │   OPEN   │────────┘                                │    │
│  │   │        │(Failing) │        │                                │    │
│  │   │        └────┬─────┘        │                                │    │
│  │   │             │              │                                │    │
│  │   │   Timeout expires     Failure in                            │    │
│  │   │             │         half-open state                       │    │
│  │   │             ▼              │                                │    │
│  │   │        ┌──────────┐        │                                │    │
│  │   └────────│HALF-OPEN │────────┘                                │    │
│  │            │(Testing) │                                         │    │
│  │            └──────────┘                                         │    │
│  │                                                                 │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
│  State Behaviors:                                                       │
│  ═════════════════                                                      │
│                                                                         │
│  CLOSED (Normal):    All requests pass through to the handler           │
│                      Track failure count                                │
│                                                                         │
│  OPEN (Failing):     All requests IMMEDIATELY rejected (fail fast)      │
│                      Don't even try calling the failing service         │
│                      Wait for recovery timeout                          │
│                                                                         │
│  HALF-OPEN (Testing): Allow ONE request through to test recovery        │
│                       If success → CLOSED (service recovered!)          │
│                       If failure → OPEN (still broken)                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

#### Why Circuit Breakers Matter

Without circuit breakers, a failing dependency causes **cascade failures**:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Cascade Failure Without Circuit Breaker              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. Database becomes slow (overloaded)                                  │
│                                                                         │
│  2. MCP Server keeps trying                                             │
│     • Requests pile up waiting for database                             │
│     • Thread pool exhausted                                             │
│     • Memory fills with pending requests                                │
│                                                                         │
│  3. MCP Server stops responding                                         │
│     • AI client times out                                               │
│     • Retries make it worse                                             │
│                                                                         │
│  4. Complete outage                                                     │
│     • Even requests that don't need the database fail                   │
│     • Recovery requires restart                                         │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  With Circuit Breaker:                                                  │
│                                                                         │
│  1. Database becomes slow                                               │
│                                                                         │
│  2. After N failures, circuit OPENS                                     │
│     • Requests fail immediately (no waiting)                            │
│     • Clear error: "Service temporarily unavailable"                    │
│     • Resources freed instantly                                         │
│                                                                         │
│  3. Server stays healthy                                                │
│     • Other tools continue working                                      │
│     • No resource exhaustion                                            │
│                                                                         │
│  4. Automatic recovery testing                                          │
│     • Circuit tries HALF-OPEN periodically                              │
│     • When database recovers, circuit CLOSES automatically              │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

| Problem | How Circuit Breaker Helps |
|---------|--------------------------|
| **Cascade failures** | Stops failure from spreading to healthy components |
| **Resource exhaustion** | Frees threads/memory instead of waiting on broken services |
| **Slow failures** | Converts slow timeouts into fast failures |
| **Automatic recovery** | Detects when service recovers, no manual intervention |
| **User experience** | Fast "service unavailable" beats slow timeout |

#### When to Use Circuit Breakers

- When calling external APIs (weather services, AI models, databases)
- When a dependency failure shouldn't crash your entire server
- When you need automatic recovery detection
- When fast failure is better than slow failure (almost always!)
- When dealing with unreliable network connections

#### PMCP Circuit Breaker Implementation

```rust
use pmcp::shared::CircuitBreakerMiddleware;
use std::time::Duration;

// Configure the circuit breaker
let circuit_breaker = CircuitBreakerMiddleware::new(
    3,                          // Failure threshold (trips after 3 failures)
    Duration::from_secs(10),    // Failure window (3 failures within 10s trips)
    Duration::from_secs(5),     // Recovery timeout (wait 5s before testing)
);

// This configuration means:
// - If 3 requests fail within a 10-second window, circuit OPENS
// - While OPEN, all requests immediately fail (no actual execution)
// - After 5 seconds, circuit goes HALF-OPEN to test recovery
// - One successful request closes circuit; one failure reopens it
```

### Combining Resilience Patterns

In production, rate limiting and circuit breakers work together:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Resilience Defense in Depth                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Incoming Request                                                       │
│       │                                                                 │
│       ▼                                                                 │
│  ┌────────────────┐                                                     │
│  │  Rate Limiter  │──▶ Too many requests? → 429 "Rate Limited"          │
│  └───────┬────────┘                                                     │
│          │ OK                                                           │
│          ▼                                                              │
│  ┌────────────────┐                                                     │
│  │Circuit Breaker │──▶ Circuit open? → 503 "Service Unavailable"        │
│  └───────┬────────┘                                                     │
│          │ OK                                                           │
│          ▼                                                              │
│  ┌────────────────┐                                                     │
│  │  Tool Handler  │──▶ Actual work happens here                         │
│  └───────┬────────┘                                                     │
│          │                                                              │
│          ▼                                                              │
│  Success or failure                                                     │
│  (failure increments circuit breaker counter)                           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Combined Resilience Chain

```rust
fn build_resilient_chain() -> EnhancedMiddlewareChain {
    let mut chain = EnhancedMiddlewareChain::new();

    // Resilience middleware (High priority - runs early)
    // Rate limiter first: reject excess traffic before it hits circuit breaker
    chain.add(Arc::new(RateLimitMiddleware::new(
        100, 200, Duration::from_secs(1)
    )));
    // Circuit breaker second: fast-fail if dependencies are down
    chain.add(Arc::new(CircuitBreakerMiddleware::new(
        5, Duration::from_secs(30), Duration::from_secs(10)
    )));

    // Observability middleware (Low priority - runs late)
    chain.add(Arc::new(TimingMiddleware::new()));
    chain.add(Arc::new(MetricsMiddleware::new("my-server".to_string())));

    chain
}
```

### Choosing the Right Configuration

| Scenario | Rate Limit | Circuit Breaker |
|----------|------------|-----------------|
| **AI chatbot backend** | 10 req/s, burst 20 | 5 failures in 30s, 10s recovery |
| **Internal tool server** | 100 req/s, burst 500 | 10 failures in 60s, 30s recovery |
| **Public API** | 5 req/s per client | 3 failures in 10s, 5s recovery |
| **Database-heavy tools** | 20 req/s | 3 failures in 5s, 15s recovery |

**Guidelines:**
- **Rate limits**: Start conservative, increase based on monitoring data
- **Circuit breaker threshold**: Lower = faster failure detection, but more false positives
- **Recovery timeout**: Long enough for actual recovery, short enough to restore service promptly

## Best Practices

### 1. Use Appropriate Priorities

| Middleware Type | Priority | Reason |
|----------------|----------|--------|
| Request ID generation | Critical | Needed by all other middleware |
| Validation | Critical | Reject bad requests early |
| Rate limiting | High | Protect resources before processing |
| Circuit breaker | High | Fail fast when unhealthy |
| Business logic | Normal | After protection, before logging |
| Logging | Low | Capture complete request lifecycle |
| Metrics | Low | Record after all processing |
| Cleanup | Lowest | Final resource release |

### 2. Keep Middleware Focused

```rust
// GOOD: Single responsibility
struct TimingMiddleware;    // Only timing
struct LoggingMiddleware;   // Only logging
struct MetricsMiddleware;   // Only metrics

// BAD: Too many responsibilities
struct KitchenSinkMiddleware;  // Timing + logging + metrics + validation...
```

### 3. Make Middleware Stateless When Possible

```rust
// GOOD: Stateless (easily clonable, no synchronization)
struct ValidationMiddleware {
    strict_mode: bool,  // Configuration, not state
}

// OK: State with thread-safe access
struct TimingMiddleware {
    start_times: DashMap<String, Instant>,  // Thread-safe map
}

// BAD: Mutable state without synchronization
struct BrokenMiddleware {
    request_count: u64,  // Data race!
}
```

### 4. Handle Errors Gracefully

```rust
async fn on_request_with_context(
    &self,
    request: &mut JSONRPCRequest,
    context: &MiddlewareContext,
) -> Result<()> {
    // Log and continue if non-critical
    if let Err(e) = self.optional_check() {
        tracing::warn!(error = %e, "Optional check failed, continuing");
    }

    // Return error only for critical failures
    self.required_check()
        .map_err(|e| Error::Validation(format!("Critical check failed: {}", e)))
}
```

## Summary

PMCP's middleware architecture provides:

| Feature | Benefit |
|---------|---------|
| **Priority ordering** | Predictable execution flow |
| **Context propagation** | Share data between middleware |
| **Two-layer system** | HTTP and protocol-level hooks |
| **Built-in middleware** | Production-ready rate limiting, circuit breaker |
| **Presets** | Quick setup for common scenarios |
| **Async-first** | Works naturally with MCP's async handlers |

The middleware system enables comprehensive observability without modifying tool handlers—instrumentation is orthogonal to business logic.

---

*Continue to [Logging Best Practices](./ch17-02-logging.md) →*
