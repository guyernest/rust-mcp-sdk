# Error Handling

Error handling is one of Rust's superpowers, and PMCP leverages this strength to provide robust, predictable error management for your MCP applications. This chapter introduces error handling concepts (even if you're new to Rust) and shows you how to build resilient MCP applications.

## Why Rust's Error Handling is Different (and Better)

If you're coming from languages like JavaScript, Python, or Java, Rust's approach to errors might feel different at first—but once you understand it, you'll appreciate its power.

### No Surprises: Errors You Can See

In many languages, errors are invisible in function signatures:

```javascript
// JavaScript - can this throw? Who knows!
function processData(input) {
    return JSON.parse(input);  // Might throw, might not
}
```

In Rust, errors are **explicit and visible**:

```rust
// Rust - the Result<T, E> tells you this can fail
fn process_data(input: &str) -> Result<Value, Error> {
    serde_json::from_str(input)  // Returns Result - you must handle it
}
```

The `Result<T, E>` type means:
- **`Ok(T)`** - Success with value of type `T`
- **`Err(E)`** - Failure with error of type `E`

### Pattern Matching: Elegant Error Handling

Rust's `match` statement makes error handling explicit and exhaustive:

```rust
use tracing::{info, error};

match client.call_tool("calculator", args).await {
    Ok(result) => {
        info!("Success! Result: {}", result.content);
    }
    Err(err) => {
        error!("Failed: {}", err);
        // Handle the error appropriately
    }
}
```

The compiler **forces** you to handle both cases—no forgotten error checks!

### The `?` Operator: Concise Error Propagation

For quick error propagation, Rust provides the `?` operator:

```rust
async fn fetch_and_process() -> Result<Value, Error> {
    let result = client.call_tool("fetch_data", args).await?;  // Propagates errors
    let processed = process_result(&result)?;                   // Continues if Ok
    Ok(processed)
}
```

The `?` operator automatically:
1. Returns the error if the operation failed
2. Unwraps the success value if it succeeded
3. Converts between compatible error types

This is **much cleaner** than nested error checking in other languages!

## PMCP Error Types

PMCP provides a comprehensive error system aligned with the MCP protocol and JSON-RPC 2.0 specification.

### Core Error Categories

```rust
use pmcp::error::{Error, ErrorCode, TransportError};

// Protocol errors (JSON-RPC 2.0 standard codes)
let parse_error = Error::parse("Invalid JSON structure");
let invalid_request = Error::protocol(
    ErrorCode::INVALID_REQUEST,
    "Request missing required field 'method'".to_string()
);
let method_not_found = Error::method_not_found("tools/unknown");
let invalid_params = Error::invalid_params("Count must be positive");
let internal_error = Error::internal("Database connection failed");

// Validation errors (business logic, not protocol-level)
let validation_error = Error::validation("Email format is invalid");

// Transport errors (network, connection issues)
let timeout = Error::timeout(30_000);  // 30 second timeout
let transport_error = Error::Transport(TransportError::Request("connection timeout".into()));

// Resource errors
let not_found = Error::not_found("User with ID 123 not found");

// Rate limiting (predefined error code)
let rate_limit = Error::protocol(
    ErrorCode::RATE_LIMITED,
    "Rate limit exceeded: retry after 60s".to_string()
);

// Custom protocol errors
let custom_error = Error::protocol(
    ErrorCode::other(-32099),  // Application-defined codes
    "Custom application error".to_string()
);
```

### Error Code Reference

PMCP follows JSON-RPC 2.0 error codes with MCP-specific extensions:

| Code | Constant | When to Use |
|------|----------|-------------|
| -32700 | `PARSE_ERROR` | Invalid JSON received |
| -32600 | `INVALID_REQUEST` | Request structure is wrong |
| -32601 | `METHOD_NOT_FOUND` | Unknown method/tool name |
| -32602 | `INVALID_PARAMS` | Parameter validation failed |
| -32603 | `INTERNAL_ERROR` | Server-side failure |

**MCP-Specific Error Codes:**

| Code | Constant | When to Use |
|------|----------|-------------|
| -32001 | `REQUEST_TIMEOUT` | Request exceeded timeout |
| -32002 | `UNSUPPORTED_CAPABILITY` | Feature not supported |
| -32003 | `AUTHENTICATION_REQUIRED` | Auth needed |
| -32004 | `PERMISSION_DENIED` | User lacks permission |
| -32005 | `RATE_LIMITED` | Rate limit exceeded |
| -32006 | `CIRCUIT_BREAKER_OPEN` | Circuit breaker tripped |

**Application-Defined Codes:**
- **-32000 to -32099**: Use `ErrorCode::other(code)` for custom application errors

### Creating Meaningful Errors

Good error messages help users understand and fix problems:

```rust
// ❌ Bad: Vague error
Err(Error::validation("Invalid input"))

// ✅ Good: Specific, actionable error
Err(Error::validation(
    "Parameter 'email' must be a valid email address. Got: 'not-an-email'"
))

// ✅ Better: Include context and suggestions
Err(Error::invalid_params(format!(
    "Parameter 'count' must be between 1 and 100. Got: {}. \
     Reduce the count or use pagination.",
    count
)))
```

## Practical Error Handling Patterns

Let's explore real-world error handling patterns you'll use in PMCP applications.

### Pattern 1: Graceful Degradation with Fallbacks

When a primary operation fails, try a simpler fallback:

```rust
use tracing::warn;

// Try advanced feature, fall back to basic version
let result = match client.call_tool("advanced_search", args).await {
    Ok(result) => result,
    Err(e) => {
        warn!("Advanced search failed: {}. Trying basic search...", e);

        // Fallback to basic search
        client.call_tool("basic_search", args).await?
    }
};
```

### Pattern 2: Retry with Exponential Backoff

For transient failures (network issues, temporary unavailability), retry with increasing delays:

```rust
use pmcp::error::{Error, TransportError};
use tokio::time::{sleep, Duration};
use futures::future::BoxFuture;
use tracing::warn;

async fn retry_with_backoff<F, T>(
    mut operation: F,
    max_retries: u32,
    initial_delay: Duration,
) -> Result<T, Error>
where
    F: FnMut() -> BoxFuture<'static, Result<T, Error>>,
{
    let mut delay = initial_delay;

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                // Check if error is retryable by matching on variants
                let is_retryable = matches!(
                    e,
                    Error::Timeout(_)
                    | Error::RateLimited
                    | Error::Transport(TransportError::ConnectionClosed)
                    | Error::Transport(TransportError::Io(_))
                    | Error::Transport(TransportError::Request(_))
                );

                if !is_retryable || attempt == max_retries {
                    return Err(e);
                }

                warn!("Attempt {} failed: {}. Retrying in {:?}...", attempt + 1, e, delay);
                sleep(delay).await;
                delay *= 2;  // Exponential backoff
            }
        }
    }

    Err(Error::internal("All retry attempts failed"))
}

// Usage
let result = retry_with_backoff(
    || Box::pin(client.call_tool("unstable_api", args)),
    3,  // max_retries
    Duration::from_millis(500),  // initial_delay
).await?;
```

**Why exponential backoff?**
- Prevents overwhelming a struggling server
- Gives transient issues time to resolve
- Reduces network congestion

**Important:** Only retry **idempotent operations** (reads, GETs, safe queries). For non-idempotent operations (writes, POSTs, state changes), retrying may cause duplicate actions. Consider using:
- Request IDs to detect duplicates
- Conditional operations (e.g., "only if version matches")
- Separate retry logic for reads vs. writes

### Pattern 3: Circuit Breaker

Stop trying operations that consistently fail to prevent resource waste:

```rust
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::Mutex;
use std::future::Future;
use tracing::error;

struct CircuitBreaker {
    failures: AtomicU32,
    failure_threshold: u32,
    state: Mutex<CircuitState>,
}

enum CircuitState {
    Closed,      // Normal operation
    Open,        // Failing - reject requests
    HalfOpen,    // Testing if service recovered
}

impl CircuitBreaker {
    async fn call<F, T>(&self, operation: F) -> Result<T, Error>
    where
        F: Future<Output = Result<T, Error>>,
    {
        // Check circuit state
        let state = self.state.lock().await;
        if matches!(*state, CircuitState::Open) {
            // Use typed error internally; convert to protocol error at API boundary
            return Err(Error::CircuitBreakerOpen);
        }
        drop(state);

        // Execute operation
        match operation.await {
            Ok(result) => {
                // Success - reset failures
                self.failures.store(0, Ordering::SeqCst);
                Ok(result)
            }
            Err(e) => {
                // Increment failures
                let failures = self.failures.fetch_add(1, Ordering::SeqCst);

                if failures >= self.failure_threshold {
                    let mut state = self.state.lock().await;
                    *state = CircuitState::Open;
                    error!("Circuit breaker opened after {} failures", failures);
                }

                Err(e)
            }
        }
    }
}
```

**Note:** This example uses `Error::CircuitBreakerOpen` internally. When constructing JSON-RPC responses at API boundaries, you can also use `Error::protocol(ErrorCode::CIRCUIT_BREAKER_OPEN, "...")` to include additional metadata.

**When to use circuit breakers:**
- Calling external services that might be down
- Database operations that might fail
- Rate-limited APIs
- Any operation that could cascade failures

### Pattern 4: Timeout Protection

Prevent operations from hanging indefinitely:

```rust
use tokio::time::{timeout, Duration};
use pmcp::error::Error;
use tracing::{info, error};

async fn call_with_timeout(client: &Client, args: Value) -> Result<CallToolResult, Error> {
    // Set a timeout for any async operation
    match timeout(
        Duration::from_secs(30),
        client.call_tool("slow_operation", args)
    ).await {
        Ok(Ok(result)) => {
            info!("Success: {:?}", result);
            Ok(result)
        }
        Ok(Err(e)) => {
            error!("Operation failed: {}", e);
            Err(e)
        }
        Err(_) => {
            // Convert elapsed timeout to PMCP error
            Err(Error::timeout(30_000))  // 30,000 milliseconds
        }
    }
}
```

### Pattern 5: Batch Error Aggregation

When processing multiple operations, collect both successes and failures:

```rust
use tracing::{info, error};

let operations = vec![
    ("task1", task1_args),
    ("task2", task2_args),
    ("task3", task3_args),
];

let mut successes = Vec::new();
let mut failures = Vec::new();

for (name, args) in operations {
    match client.call_tool("processor", args).await {
        Ok(result) => successes.push((name, result)),
        Err(err) => failures.push((name, err)),
    }
}

// Report results
info!("Completed: {}/{}", successes.len(), successes.len() + failures.len());

if !failures.is_empty() {
    error!("Failed operations:");
    for (name, err) in &failures {
        error!("  - {}: {}", name, err);
    }
}

// Continue with successful results
for (name, result) in successes {
    process_result(name, result).await?;
}
```

## Input Validation and Error Messages

Proper validation prevents errors and provides clear feedback when they occur.

### Validation Best Practices

```rust
use async_trait::async_trait;
use serde_json::{json, Value};
use pmcp::{ToolHandler, RequestHandlerExtra};
use pmcp::error::Error;

#[async_trait]
impl ToolHandler for ValidatorTool {
    async fn handle(&self, arguments: Value, _extra: RequestHandlerExtra)
        -> pmcp::Result<Value>
    {
        // 1. Check required fields exist
        let input = arguments
            .get("input")
            .ok_or_else(|| Error::invalid_params(
                "Missing required parameter 'input'"
            ))?
            .as_str()
            .ok_or_else(|| Error::invalid_params(
                "Parameter 'input' must be a string"
            ))?;

        // 2. Validate input constraints
        if input.len() < 5 {
            return Err(Error::validation(
                format!("Input must be at least 5 characters. Got: {} chars", input.len())
            ));
        }

        if !input.chars().all(|c| c.is_alphanumeric()) {
            return Err(Error::validation(
                "Input must contain only alphanumeric characters"
            ));
        }

        // 3. Business logic validation
        if is_blacklisted(input) {
            return Err(Error::validation(
                format!("Input '{}' is not allowed by policy", input)
            ));
        }

        // All validations passed
        Ok(json!({
            "status": "validated",
            "input": input
        }))
    }
}
```

### Progressive Validation

Validate in order from cheapest to most expensive:

```rust
use pmcp::error::Error;

async fn validate_and_process(data: &str) -> Result<ProcessedData, Error> {
    // 1. Fast: Check syntax (no I/O)
    if !is_valid_syntax(data) {
        return Err(Error::validation("Invalid syntax"));
    }

    // 2. Medium: Check against local rules (minimal I/O)
    if !passes_local_checks(data) {
        return Err(Error::validation("Failed local validation"));
    }

    // 3. Slow: Check against external service (network I/O)
    if !check_with_service(data).await? {
        return Err(Error::validation("Failed external validation"));
    }

    // 4. Process (expensive operation)
    process_data(data).await
}
```

## Error Recovery Strategies

Different errors require different recovery strategies.

### Decision Tree for Error Handling

```
Is the error retryable?
├─ Yes (timeout, network, temporary)
│  ├─ Retry with exponential backoff
│  └─ If retries exhausted → Try fallback
│
└─ No (validation, permission, not found)
   ├─ Can we use cached/default data?
   │  ├─ Yes → Use fallback data
   │  └─ No → Propagate error to user
   │
   └─ Log error details for debugging
```

### Example: Comprehensive Error Strategy

```rust
use pmcp::error::{Error, ErrorCode};
use tokio::time::Duration;
use tracing::warn;

async fn fetch_user_data(user_id: &str) -> Result<UserData, Error> {
    // Try primary source with retries
    let primary_result = retry_with_backoff(
        || Box::pin(api_client.get_user(user_id)),
        3,  // max_retries
        Duration::from_secs(1),  // initial_delay
    ).await;

    match primary_result {
        Ok(data) => Ok(data),
        Err(e) => {
            warn!("Primary API failed: {}", e);

            // Check error type using pattern matching
            match e {
                // Network/transport errors - try cache
                Error::Transport(_) | Error::Timeout(_) => {
                    warn!("Network error, checking cache...");
                    cache.get_user(user_id).ok_or_else(|| {
                        Error::internal("Primary API down and no cached data")
                    })
                }

                // Not found - use proper error type
                Error::Protocol { code, .. } if code == ErrorCode::METHOD_NOT_FOUND => {
                    Err(Error::not_found(format!("User {} not found", user_id)))
                }

                // Rate limited - propagate with suggestion
                Error::RateLimited => {
                    Err(Error::protocol(
                        ErrorCode::RATE_LIMITED,
                        "API rate limit exceeded. Please retry later.".to_string()
                    ))
                }

                // Other errors - propagate
                _ => Err(e),
            }
        }
    }
}
```

## Running the Example

The `12_error_handling.rs` example demonstrates all these patterns:

```bash
cargo run --example 12_error_handling
```

This example shows:

1. **Different Error Types** - Parse, validation, internal, rate limiting
2. **Input Validation** - Length checks, character validation
3. **Retry Logic** - Exponential backoff for transient failures
4. **Timeout Handling** - Preventing hung operations
5. **Recovery Strategies** - Fallback and circuit breaker patterns
6. **Batch Operations** - Error aggregation and success rate tracking

## Error Handling Checklist

When implementing error handling in your PMCP application:

- [ ] **Use appropriate error types** - Choose the right `ErrorCode` for the situation
- [ ] **Provide clear messages** - Include context, got vs. expected, suggestions
- [ ] **Validate inputs early** - Fail fast with meaningful feedback
- [ ] **Handle transient failures** - Implement retries with backoff
- [ ] **Set timeouts** - Prevent operations from hanging
- [ ] **Log errors properly** - Include context for debugging
- [ ] **Test error paths** - Don't just test the happy path
- [ ] **Document error behavior** - Tell users what errors they might see

## Library vs Application Error Handling

**Libraries** (creating reusable MCP tools/servers):
- Use PMCP's typed `Error` enum for all public APIs
- Avoid `Error::Other(anyhow::Error)` in library interfaces
- Provide specific error types that callers can match on
- Use `thiserror` for custom error types if needed

**Applications** (building MCP clients/servers):
- Can use `anyhow::Result<T>` for internal error context
- Convert to PMCP errors at API boundaries
- `Error::Other(anyhow::Error)` is acceptable for application-level errors

```rust
// Library - typed errors
pub async fn fetch_resource(uri: &str) -> Result<Resource, Error> {
    // Use specific PMCP error types
    if !is_valid_uri(uri) {
        return Err(Error::invalid_params("Invalid URI format"));
    }
    // ...
}

// Application - anyhow for context
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let resource = fetch_resource("file:///data.json")
        .await
        .context("Failed to fetch configuration")?;
    Ok(())
}
```

## Error Boundary Mapping

When building MCP applications, errors flow through different layers. Use the right error type at each layer:

**Internal Layer (typed variants):**
```rust
// Use typed error variants internally for pattern matching
if is_rate_limited() {
    return Err(Error::RateLimited);
}

if circuit_open {
    return Err(Error::CircuitBreakerOpen);
}

if elapsed > timeout {
    return Err(Error::timeout(timeout_ms));
}
```

**API Boundary (protocol errors):**
```rust
// Convert to protocol errors at JSON-RPC boundaries
match internal_operation().await {
    Ok(result) => result,
    Err(Error::RateLimited) => {
        // Add metadata when constructing JSON-RPC responses
        return Err(Error::protocol(
            ErrorCode::RATE_LIMITED,
            json!({
                "message": "Rate limit exceeded",
                "retry_after": 60,
                "limit": 100
            }).to_string()
        ));
    }
    Err(e) => return Err(e),
}
```

Use `Error::error_code()` to extract the error code when needed:
```rust
if let Some(code) = error.error_code() {
    match code {
        ErrorCode::RATE_LIMITED => { /* handle rate limit */ }
        ErrorCode::TIMEOUT => { /* handle timeout */ }
        _ => { /* handle others */ }
    }
}
```

## Security Considerations

**⚠️ Never leak sensitive information in error messages:**

```rust
// ❌ Bad: Exposes sensitive data
Err(Error::validation(format!("Invalid API key: {}", api_key)))

// ✅ Good: Generic message
Err(Error::protocol(
    ErrorCode::AUTHENTICATION_REQUIRED,
    "Invalid authentication credentials".to_string()
))

// ❌ Bad: Exposes internal paths
Err(Error::internal(format!("Failed to read /etc/secrets/db.conf: {}", e)))

// ✅ Good: Sanitized message
Err(Error::internal("Failed to read configuration file".to_string()))

// ❌ Bad: Reveals user existence (timing attack)
if !user_exists(username) {
    return Err(Error::not_found("User not found"));
}
if !password_valid(username, password) {
    return Err(Error::validation("Invalid password"));
}

// ✅ Good: Constant-time response
if !authenticate(username, password) {
    // Same error for both cases
    return Err(Error::protocol(
        ErrorCode::AUTHENTICATION_REQUIRED,
        "Invalid username or password".to_string()
    ));
}
```

**Security checklist:**
- [ ] No secrets, tokens, or API keys in error messages
- [ ] No internal file paths or system information
- [ ] No database query details or schema information
- [ ] No user enumeration (same error for "not found" vs "wrong password")
- [ ] No stack traces in production error responses
- [ ] Sanitize all user input before including in errors

## Best Practices Summary

✅ **Use pattern matching** - Match error variants instead of parsing strings
✅ **Use `tracing`** - Prefer `warn!`/`error!` over `println!`/`eprintln!`
✅ **Use specific errors** - `Error::not_found` for missing resources, not `Error::validation`
✅ **Add imports** - Make code snippets self-contained and copy-pasteable
✅ **Avoid double-logging** - Log at boundaries, not at every layer
✅ **Use constants** - `ErrorCode::METHOD_NOT_FOUND` not magic numbers
✅ **Map at boundaries** - Use typed errors internally, protocol errors at API boundaries
✅ **Protect secrets** - Never expose sensitive data in error messages
✅ **Retry wisely** - Only retry idempotent operations

## Key Takeaways

1. **Rust makes errors visible** - `Result<T, E>` shows what can fail
2. **Pattern matching is powerful** - Handle all cases exhaustively
3. **The `?` operator is your friend** - Concise error propagation
4. **PMCP provides rich error types** - Aligned with MCP and JSON-RPC standards
5. **Different errors need different strategies** - Retry, fallback, fail fast
6. **Clear error messages help users** - Be specific and actionable
7. **Test your error handling** - Errors are part of your API
8. **Match on variants, not strings** - Type-safe error classification

With Rust's error handling and PMCP's comprehensive error types, you can build MCP applications that are robust, predictable, and provide excellent user experience even when things go wrong.

## Further Reading

- [Rust Book: Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [PMCP Error API Documentation](https://docs.rs/pmcp/latest/pmcp/error/)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- Example: `examples/12_error_handling.rs`
