# Middleware

PMCP middleware intercepts requests before they reach tool/resource/prompt
handlers. Use middleware for cross-cutting concerns like logging, auth,
and rate limiting.

## Tool Middleware

Tool middleware wraps individual tool calls:

```rust
use pmcp::server::middleware::ToolMiddleware;

struct LoggingMiddleware;

#[async_trait::async_trait]
impl ToolMiddleware for LoggingMiddleware {
    async fn before_call(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> pmcp::Result<()> {
        tracing::info!(tool = tool_name, "Tool call started");
        Ok(())
    }

    async fn after_call(
        &self,
        tool_name: &str,
        result: &pmcp::CallToolResult,
    ) -> pmcp::Result<()> {
        tracing::info!(tool = tool_name, "Tool call completed");
        Ok(())
    }
}
```

## Protocol Middleware

Protocol middleware operates at the JSON-RPC message level:

```rust
use pmcp::server::middleware::ProtocolMiddleware;

struct RequestLogger;

#[async_trait::async_trait]
impl ProtocolMiddleware for RequestLogger {
    async fn on_request(
        &self,
        method: &str,
        params: &serde_json::Value,
    ) -> pmcp::Result<()> {
        tracing::debug!(method, "Incoming request");
        Ok(())
    }
}
```

## Middleware Chain Composition

Register middleware on the server builder in execution order:

```rust
server_builder
    .middleware(RequestLogger)
    .middleware(RateLimiter::new(100))
    .middleware(AuthMiddleware::new(config));
```

Middleware executes top-to-bottom for requests. The first middleware
to return an error short-circuits the chain.

## Rate Limiting

```rust
use pmcp::server::middleware::RateLimiter;

// 100 requests per minute per client
let limiter = RateLimiter::new(100).window_secs(60);
server_builder.middleware(limiter);
```

## Best Practices

- Keep middleware lightweight (avoid blocking I/O in before_call)
- Use tracing spans for structured logging
- Rate limiters should be the outermost middleware
- Auth middleware should run before business logic middleware
