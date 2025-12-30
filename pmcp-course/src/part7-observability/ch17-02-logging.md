# Logging Best Practices

Effective logging transforms debugging from guesswork into investigation. This section covers structured logging with the `tracing` ecosystem, MCP protocol logging, sensitive data handling, and log output strategies.

## Why Logging Matters

If you're new to production logging, you might wonder why we need anything beyond `println!` or simple file writes. The answer lies in what happens when things go wrong in production—and they will.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    The Production Debugging Challenge                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  The Scenario:                                                          │
│  ═════════════                                                          │
│  It's 3 AM. Your MCP server is failing intermittently. Users report     │
│  "sometimes it works, sometimes it doesn't." You need to find out:      │
│                                                                         │
│  • Which requests are failing?                                          │
│  • What was the server doing when it failed?                            │
│  • What external services was it calling?                               │
│  • What user data was involved (without exposing PII)?                  │
│  • How long did each step take?                                         │
│  • What happened BEFORE the failure?                                    │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  With println! debugging:         With production logging:              │
│  ═══════════════════════          ═══════════════════════               │
│                                                                         │
│  "Request received"               {"timestamp": "2024-12-30T03:14:22",  │
│  "Processing..."                   "level": "ERROR",                    │
│  "Error: something failed"         "request_id": "abc-123",             │
│                                    "user_tier": "enterprise",           │
│  Problems:                         "tool": "database-query",            │
│  • No timestamp                    "duration_ms": 30042,                │
│  • No context                      "error": "Connection timeout",       │
│  • Can't search/filter             "span": {                            │
│  • Can't correlate requests          "db_host": "prod-db-02",           │
│  • No way to analyze patterns        "query_type": "select"             │
│                                    }}                                   │
│                                                                         │
│                                   Benefits:                             │
│                                   ✓ Exact time of failure               │
│                                   ✓ Which request failed                │
│                                   ✓ Full context chain                  │
│                                   ✓ Searchable & filterable             │
│                                   ✓ Correlate across services           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### The Three Purposes of Logging

| Purpose | Example | What Good Logging Provides |
|---------|---------|---------------------------|
| **Debugging** | "Why did this request fail?" | Full context: request ID, user, inputs, error chain |
| **Auditing** | "Who accessed this data?" | Immutable record: who, what, when (without sensitive data) |
| **Monitoring** | "Is the system healthy?" | Patterns: error rates, latency trends, usage spikes |

### Logging vs. Metrics vs. Tracing

These three observability tools serve different purposes:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    The Three Pillars of Observability                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  LOGS                    METRICS                  TRACES                │
│  ════                    ═══════                  ══════                │
│                                                                         │
│  What happened?          How much/how many?       Where did time go?    │
│                                                                         │
│  • Detailed events       • Numeric measurements   • Request flow        │
│  • Error messages        • Aggregated over time   • Cross-service       │
│  • Context-rich          • Alerts & dashboards    • Latency breakdown   │
│                                                                         │
│  Example:                Example:                 Example:              │
│  "User X called tool Y   "95th percentile         "Request took 500ms:  │
│   at time Z, got error   latency is 250ms"        - 50ms auth           │
│   E because of F"                                 - 400ms database      │
│                                                   - 50ms serialization" │
│                                                                         │
│  Best for:               Best for:                Best for:             │
│  • Debugging             • Alerting               • Performance         │
│  • Auditing              • Capacity planning      • Bottleneck finding  │
│  • Investigation         • SLA monitoring         • Distributed systems │
│                                                                         │
│  In this chapter, we focus on LOGS and touch on TRACES (spans).         │
│  Metrics are covered in the next chapter.                               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## The Tracing Ecosystem

Rust's `tracing` crate provides structured, contextual logging designed for async applications:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Tracing vs Traditional Logging                       │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Traditional Logging:                                                   │
│  ═══════════════════                                                    │
│                                                                         │
│  println!("User {} called tool {}", user_id, tool_name);                │
│                                                                         │
│  Output: "User user-123 called tool get-weather"                        │
│                                                                         │
│  Problems:                                                              │
│  • No structure - hard to parse                                         │
│  • No context across async calls                                        │
│  • No levels, filtering, or sampling                                    │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Structured Tracing:                                                    │
│  ═══════════════════                                                    │
│                                                                         │
│  tracing::info!(                                                        │
│      user_id = %user_id,                                                │
│      tool = %tool_name,                                                 │
│      "Tool invocation"                                                  │
│  );                                                                     │
│                                                                         │
│  Output: {                                                              │
│    "timestamp": "2024-12-30T10:15:30Z",                                 │
│    "level": "INFO",                                                     │
│    "target": "weather_server::tools",                                   │
│    "fields": {                                                          │
│      "user_id": "user-123",                                             │
│      "tool": "get-weather",                                             │
│      "message": "Tool invocation"                                       │
│    },                                                                   │
│    "span": {                                                            │
│      "request_id": "abc-123",                                           │
│      "session_id": "session-456"                                        │
│    }                                                                    │
│  }                                                                      │
│                                                                         │
│  Benefits:                                                              │
│  ✓ Machine-parseable JSON                                               │
│  ✓ Context from parent spans                                            │
│  ✓ Levels, filtering, sampling                                          │
│  ✓ Works naturally with async                                           │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Setting Up Tracing

```rust
// Cargo.toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }

// main.rs
fn main() {
    // Initialize with JSON output for production
    tracing_subscriber::fmt()
        .json()
        .with_env_filter("info,pmcp=debug,my_server=trace")
        .with_current_span(true)
        .with_span_list(true)
        .init();

    // Now use tracing macros
    tracing::info!("Server starting");
}
```

### Log Levels

Choosing the right log level is crucial—too verbose and you'll drown in noise; too quiet and you'll miss important events. Think of log levels as a filter that determines what appears in production logs.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Log Level Pyramid                                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│                           ┌─────────┐                                   │
│                           │  ERROR  │  ← Something broke, needs fixing  │
│                           └────┬────┘    (always logged)                │
│                        ┌───────┴───────┐                                │
│                        │     WARN      │  ← Might become a problem      │
│                        └───────┬───────┘    (always logged)             │
│                   ┌────────────┴────────────┐                           │
│                   │          INFO           │  ← Normal milestones      │
│                   └────────────┬────────────┘    (production default)   │
│              ┌─────────────────┴─────────────────┐                      │
│              │             DEBUG                 │ ← Diagnostic details │
│              └─────────────────┬─────────────────┘    (development)     │
│         ┌──────────────────────┴──────────────────────┐                 │
│         │                   TRACE                     │  ← Everything   │
│         └─────────────────────────────────────────────┘    (debugging)  │
│                                                                         │
│  Production typically runs at INFO level:                               │
│  • ERROR ✓  WARN ✓  INFO ✓  DEBUG ✗  TRACE ✗                            │
│                                                                         │
│  Development runs at DEBUG or TRACE:                                    │
│  • ERROR ✓  WARN ✓  INFO ✓  DEBUG ✓  TRACE ✓                            │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

| Level | When to Use | Examples | Common Mistakes |
|-------|-------------|----------|-----------------|
| `ERROR` | Operation failed, needs attention | Database down, API key invalid, unrecoverable error | Using for expected failures (user not found) |
| `WARN` | Degraded but working, or suspicious activity | Rate limit at 80%, deprecated API used, retry succeeded | Using for normal operation |
| `INFO` | Normal milestones worth knowing | Server started, tool executed, request completed | Too verbose (every cache hit) |
| `DEBUG` | Detailed info for developers | Cache hit/miss, full request params, decision paths | Logging in hot paths (performance) |
| `TRACE` | Very fine-grained tracing | Function entry/exit, loop iterations, wire format | Using in production (extreme noise) |

**The Golden Rule**: Ask yourself "Would I want to be woken up at 3 AM for this?"
- **Yes** → ERROR
- **Maybe tomorrow** → WARN
- **Good to know** → INFO
- **Only when debugging** → DEBUG/TRACE

```rust
use tracing::{error, warn, info, debug, trace};

async fn handler(input: WeatherInput) -> Result<Weather> {
    trace!(city = %input.city, "Handler entry");

    debug!("Checking cache for {}", input.city);

    let weather = match cache.get(&input.city) {
        Some(cached) => {
            info!(city = %input.city, "Cache hit");
            cached
        }
        None => {
            debug!(city = %input.city, "Cache miss, fetching from API");
            let result = api.fetch(&input.city).await?;
            cache.insert(input.city.clone(), result.clone());
            result
        }
    };

    if weather.temperature > 40.0 {
        warn!(
            city = %input.city,
            temp = %weather.temperature,
            "Extreme heat detected"
        );
    }

    trace!(city = %input.city, "Handler exit");
    Ok(weather)
}
```

## Spans for Context

### What is a Span?

If you're new to distributed tracing, a **span** represents a unit of work—like a function call, database query, or API request. Spans are essential in async and distributed systems because traditional stack traces don't work when execution jumps between tasks and services.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Why Spans Matter in Async Systems                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  The Problem with Async:                                                │
│  ═══════════════════════                                                │
│                                                                         │
│  In synchronous code, you can look at the call stack:                   │
│                                                                         │
│    main() → handle_request() → fetch_weather() → ERROR                  │
│                                                                         │
│  In async code, execution bounces between tasks:                        │
│                                                                         │
│    Task A: handle_request() starts, awaits...                           │
│    Task B: different_request() runs                                     │
│    Task C: yet_another_request() runs                                   │
│    Task A: ...fetch_weather() resumes, ERROR!                           │
│                                                                         │
│  When the error happens, you can't see the original context!            │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  The Solution - Spans:                                                  │
│  ════════════════════                                                   │
│                                                                         │
│  Spans carry context through async boundaries:                          │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │ Span: "handle_request" (request_id=abc-123, user=enterprise)    │    │
│  │   │                                                             │    │
│  │   ├─▶ Span: "validate_input"                                    │    │
│  │   │     └─▶ log: "Input validated"                              │    │
│  │   │                                                             │    │
│  │   ├─▶ Span: "fetch_weather" (city=London)                       │    │
│  │   │     ├─▶ Span: "cache_lookup"                                │    │
│  │   │     │     └─▶ log: "Cache miss"                             │    │
│  │   │     │                                                       │    │
│  │   │     └─▶ Span: "api_call" (endpoint=weather-api)             │    │
│  │   │           └─▶ log: "ERROR: Connection timeout"  ← HERE!     │    │
│  │   │                                                             │    │
│  │   └─▶ Total duration: 30,042ms                                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
│  Now when you see the error, you know:                                  │
│  • request_id: abc-123 (find all logs for this request)                 │
│  • user: enterprise (who was affected)                                  │
│  • city: London (what they were looking for)                            │
│  • It happened in api_call inside fetch_weather                         │
│  • The whole request took 30 seconds                                    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key Span Concepts

| Concept | Description | Example |
|---------|-------------|---------|
| **Parent span** | The outer operation containing this work | `handle_request` is parent of `fetch_weather` |
| **Child span** | A sub-operation within a parent | `api_call` is child of `fetch_weather` |
| **Span context** | Data attached to a span (and inherited by children) | `request_id`, `user_id` |
| **Span duration** | Time from span start to end | Helps find slow operations |

Spans create hierarchical context that flows through async calls:

```rust
use tracing::{instrument, span, Level, Instrument};

// Automatic span creation with #[instrument]
#[instrument(
    name = "get_weather",
    skip(input),
    fields(
        tool = "get-current-weather",
        city = %input.city,
        request_id = %Uuid::new_v4()
    )
)]
async fn handler(input: WeatherInput) -> Result<Weather> {
    // All logs inside here include the span context
    info!("Starting weather lookup");

    // Nested span for sub-operation
    let api_result = fetch_from_api(&input.city)
        .instrument(tracing::info_span!("api_call", endpoint = "weather"))
        .await?;

    info!(temp = %api_result.temperature, "Weather retrieved");
    Ok(api_result)
}

// Manual span creation
async fn process_batch(items: Vec<Item>) {
    let span = span!(Level::INFO, "batch_process", count = items.len());
    let _guard = span.enter();

    for (i, item) in items.iter().enumerate() {
        let item_span = span!(Level::DEBUG, "item", index = i, id = %item.id);
        let _item_guard = item_span.enter();

        process_item(item).await;
    }
}
```

### Span Output

```json
{
  "timestamp": "2024-12-30T10:15:30.123Z",
  "level": "INFO",
  "message": "Weather retrieved",
  "target": "weather_server::tools::weather",
  "span": {
    "name": "get_weather",
    "tool": "get-current-weather",
    "city": "London",
    "request_id": "abc-123-def-456"
  },
  "spans": [
    { "name": "handle_request", "session_id": "session-789" },
    { "name": "get_weather", "city": "London" },
    { "name": "api_call", "endpoint": "weather" }
  ],
  "fields": {
    "temp": "22.5"
  }
}
```

## MCP Protocol Logging

### Logging in Tools

Use PMCP's protocol logging for client-visible messages:

```rust
use pmcp::types::protocol::LogLevel;

async fn handler(input: DatabaseInput) -> Result<QueryResult> {
    // Log to MCP client (visible in AI interface)
    pmcp::log(
        LogLevel::Info,
        "Starting database query",
        Some(serde_json::json!({
            "query_type": "select",
            "table": input.table
        }))
    ).await;

    // Simulate work
    for step in 1..=3 {
        pmcp::log(
            LogLevel::Info,
            &format!("Processing step {}/3", step),
            Some(serde_json::json!({
                "step": step,
                "progress": format!("{}%", step * 33)
            }))
        ).await;
    }

    // Warn about high resource usage
    pmcp::log(
        LogLevel::Warning,
        "Query returned large result set",
        Some(serde_json::json!({
            "row_count": 15000,
            "recommendation": "Consider pagination"
        }))
    ).await;

    Ok(result)
}
```

### Server Lifecycle Logging

```rust
async fn run_server() -> Result<()> {
    // Log startup with structured metadata
    pmcp::log(
        LogLevel::Info,
        "Server initialized and ready",
        Some(serde_json::json!({
            "name": "weather-server",
            "version": "1.0.0",
            "pid": std::process::id(),
            "transport": "http",
            "port": 8080
        }))
    ).await;

    let server = Server::builder()
        .name("weather-server")
        .version("1.0.0")
        .build()?;

    // Log shutdown
    pmcp::log(LogLevel::Info, "Server shutting down", None).await;

    Ok(())
}
```

## Sensitive Data Handling

Never log sensitive data in production:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Sensitive Data Categories                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ❌ NEVER LOG:                                                           │
│  ═════════════                                                          │
│                                                                         │
│  • API keys, tokens, secrets                                            │
│  • Passwords, password hashes                                           │
│  • Personal identifiable information (PII)                              │
│  • Credit card numbers                                                  │
│  • Session tokens, JWTs                                                 │
│  • OAuth access/refresh tokens                                          │
│  • Database credentials                                                 │
│                                                                         │
│  ✅ SAFE TO LOG:                                                        │
│  ═══════════════                                                        │
│                                                                         │
│  • Request IDs, correlation IDs                                         │
│  • User IDs (if not considered PII)                                     │
│  • Timestamps, durations                                                │
│  • Error codes (not messages with user data)                            │
│  • Operation types, method names                                        │
│  • Aggregate counts, statistics                                         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Redaction Patterns

```rust
use std::fmt;

/// Wrapper that redacts value in Display/Debug
pub struct Redacted<T>(pub T);

impl<T> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

// Usage
async fn authenticate(token: &str) -> Result<User> {
    tracing::info!(
        token = %Redacted(token),  // Logs as "[REDACTED]"
        "Authentication attempt"
    );

    // Actual auth logic
    Ok(user)
}
```

### Automatic Redaction Middleware

```rust
use pmcp::server::http_middleware::ServerHttpLoggingMiddleware;

// HTTP middleware with automatic redaction
let logging = ServerHttpLoggingMiddleware::new()
    .with_level(tracing::Level::INFO)
    .with_redact_query(true);       // Strips ?token=xxx from URLs

// Automatically redacted headers:
// - Authorization
// - Cookie
// - X-Api-Key
```

### Field-Level Redaction

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
struct UserCredentials {
    username: String,
    #[serde(skip_serializing)]  // Never serialize password
    password: String,
}

// Custom Debug that redacts
#[derive(Serialize)]
struct ApiConfig {
    base_url: String,
    api_key: String,
}

impl fmt::Debug for ApiConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}
```

## Log Output Strategies

### Development: Human-Readable

```rust
// Pretty, colored output for local development
tracing_subscriber::fmt()
    .pretty()
    .with_target(true)
    .with_level(true)
    .with_env_filter("debug")
    .init();

// Output:
// 2024-12-30T10:15:30.123Z DEBUG weather_server::tools
//   in get_weather{city="London"}
//   Weather retrieved
//     temp: 22.5
```

### Production: JSON

```rust
// Structured JSON for log aggregation
tracing_subscriber::fmt()
    .json()
    .with_current_span(true)
    .with_env_filter("info")
    .init();

// Output (single line):
// {"timestamp":"2024-12-30T10:15:30.123Z","level":"INFO",...}
```

### Multi-Output Configuration

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

fn init_logging() {
    // JSON logs to stdout for production systems
    let json_layer = tracing_subscriber::fmt::layer()
        .json()
        .with_filter(tracing_subscriber::EnvFilter::new("info"));

    // Pretty logs to stderr for local debugging
    let pretty_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_writer(std::io::stderr)
        .with_filter(tracing_subscriber::EnvFilter::new("debug"));

    tracing_subscriber::registry()
        .with(json_layer)
        .with(pretty_layer)
        .init();
}
```

### Cloud Platform Integration

```rust
// AWS CloudWatch format (JSON with specific fields)
use tracing_subscriber::fmt::format::JsonFields;

tracing_subscriber::fmt()
    .json()
    .flatten_event(true)
    .with_current_span(true)
    .init();

// Output compatible with CloudWatch Insights:
// {"level":"INFO","target":"weather_server","city":"London","message":"Weather retrieved"}
```

## Error Logging Patterns

### Contextual Error Logging

```rust
use anyhow::{Context, Result};
use tracing::error;

async fn fetch_weather(city: &str) -> Result<Weather> {
    let response = client
        .get(&format!("{}/weather/{}", base_url, city))
        .send()
        .await
        .context("Failed to send request to weather API")?;

    if !response.status().is_success() {
        error!(
            city = %city,
            status = %response.status(),
            "Weather API returned error"
        );
        return Err(anyhow::anyhow!("Weather API error: {}", response.status()));
    }

    response
        .json::<Weather>()
        .await
        .context("Failed to parse weather response")
}
```

### Error Chain Logging

```rust
fn log_error_chain(error: &anyhow::Error) {
    error!(error = %error, "Operation failed");

    // Log each cause in the chain
    for (i, cause) in error.chain().enumerate().skip(1) {
        error!(cause = %cause, depth = i, "Caused by");
    }
}

// Usage
if let Err(e) = process_request().await {
    log_error_chain(&e);
}

// Output:
// ERROR Operation failed: Failed to fetch weather
// ERROR Caused by: HTTP request failed | depth=1
// ERROR Caused by: connection refused | depth=2
```

## Log Filtering and Sampling

### Environment-Based Filtering

```rust
// Set via environment variable:
// RUST_LOG=warn,pmcp=info,my_server=debug

tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .init();
```

### Per-Module Filtering

```rust
use tracing_subscriber::EnvFilter;

let filter = EnvFilter::new("")
    .add_directive("warn".parse().unwrap())           // Default: warn
    .add_directive("pmcp=info".parse().unwrap())      // PMCP: info
    .add_directive("my_server=debug".parse().unwrap()) // Our code: debug
    .add_directive("hyper=warn".parse().unwrap())     // HTTP: warn only
    .add_directive("sqlx=info".parse().unwrap());     // Database: info

tracing_subscriber::fmt()
    .with_env_filter(filter)
    .init();
```

### Request Sampling

For high-traffic servers, sample logs:

```rust
use rand::Rng;

struct SamplingMiddleware {
    sample_rate: f64,  // 0.01 = 1% of requests
}

#[async_trait]
impl AdvancedMiddleware for SamplingMiddleware {
    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let should_sample = rand::thread_rng().gen::<f64>() < self.sample_rate;
        context.set_metadata(
            "sample".to_string(),
            should_sample.to_string()
        );

        if should_sample {
            tracing::debug!(
                method = %request.method,
                "Request sampled for detailed logging"
            );
        }

        Ok(())
    }
}
```

## Summary

| Practice | Implementation |
|----------|---------------|
| **Structured logging** | Use `tracing` with JSON output |
| **Contextual spans** | Use `#[instrument]` on handlers |
| **Log levels** | ERROR for failures, INFO for operations, DEBUG for diagnostics |
| **Sensitive data** | Use `Redacted<T>` wrapper, `#[serde(skip)]` |
| **Error context** | Use `anyhow::Context`, log error chains |
| **Cloud integration** | JSON format with CloudWatch/Datadog fields |
| **High traffic** | Sample logs, filter by module |

The combination of `tracing` for Rust-side logging and PMCP's protocol logging provides comprehensive visibility into both server internals and client-facing operations.

---

*Continue to [Metrics Collection](./ch17-03-metrics.md) →*
