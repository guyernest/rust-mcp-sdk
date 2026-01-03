# Middleware and Instrumentation

Enterprise MCP servers require comprehensive observability—you can't fix what you can't see. This chapter explores PMCP's middleware system for request/response instrumentation, structured logging, and metrics collection that integrates with modern observability platforms.

## Why Observability Matters for MCP

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    The Observability Challenge                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Without Observability:             With Observability:                 │
│  ═════════════════════              ═════════════════                   │
│                                                                         │
│  AI Client                            AI Client                         │
│      │                                    │                             │
│      ▼                                    ▼                             │
│  ┌───────────┐                      ┌──────────────┐                    │
│  │ MCP Server│ ← "It's broken"      │ MCP Server   │                    │
│  │           │                      │  ┌────────┐  │                    │
│  │  [????]   │                      │  │Logs    │  │ ← Request traced   │
│  │           │                      │  │────────│  │                    │
│  │  [????]   │                      │  │Metrics │  │ ← Duration: 250ms  │
│  │           │                      │  │────────│  │                    │
│  │  [????]   │                      │  │Traces  │  │ ← Error: DB timeout│
│  │           │                      │  └────────┘  │                    │
│  └───────────┘                      └──────────────┘                    │
│      │                                      │                           │
│      ▼                                      ▼                           │
│  "No idea what                      "DB connection pool                 │
│   happened"                          exhausted at 14:23"                │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Enterprise Requirements

Production MCP servers must answer:

| Question | Required Capability |
|----------|-------------------|
| What requests are failing? | Structured logging with context |
| How long do tools take? | Request duration metrics |
| What's the error rate? | Error tracking and categorization |
| Why did this request fail? | Distributed tracing |
| Are my dependencies healthy? | Health checks and circuit breakers |
| Who's using the server? | Authentication and audit logs |

### Rust's Observability Ecosystem

Rust provides excellent foundations for observability:

```rust
// The tracing ecosystem - structured, contextual logging
use tracing::{info, error, instrument, span, Level};

// Metrics with compile-time validation
use metrics::{counter, gauge, histogram};

// Async-first design works perfectly with MCP's async handlers
#[instrument(skip(input), fields(tool = "get-weather", city = %input.city))]
async fn handler(input: WeatherInput) -> Result<Weather> {
    let start = Instant::now();

    let result = fetch_weather(&input.city).await;

    histogram!("tool.duration_ms").record(start.elapsed().as_millis() as f64);
    counter!("tool.calls_total", "tool" => "get-weather").increment(1);

    result
}
```

## Built-in Observability Module (Recommended)

PMCP v1.9.2+ includes a **built-in observability module** that handles logging, metrics, and distributed tracing out of the box. For most use cases, this is the recommended approach—you get production-ready observability with a single method call.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Built-in vs Custom Observability                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Built-in Observability:              Custom Middleware:                │
│  ═══════════════════════              ═════════════════                 │
│                                                                         │
│  ServerCoreBuilder::new()             ServerCoreBuilder::new()          │
│      .name("my-server")                   .name("my-server")            │
│      .tool("weather", WeatherTool)        .tool("weather", WeatherTool) │
│      .with_observability(config)  ←       .with_tool_middleware(...)    │
│      .build()                             .with_tool_middleware(...)    │
│                                           .with_tool_middleware(...)    │
│  One line, full observability!            .build()                      │
│                                                                         │
│  Use built-in when:                   Use custom when:                  │
│  • Starting a new project             • Need custom metrics             │
│  • Standard observability needs       • Complex business logic          │
│  • Quick setup required               • Custom backends                 │
│  • CloudWatch or console output       • Non-standard integrations       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Quick Start with Built-in Observability

```rust
use pmcp::{
    server::builder::ServerCoreBuilder,
    server::observability::ObservabilityConfig,
    ServerCapabilities,
};

fn main() -> pmcp::Result<()> {
    // Development: Pretty console output
    let config = ObservabilityConfig::development();

    let server = ServerCoreBuilder::new()
        .name("my-weather-server")
        .version("1.0.0")
        .tool("get_weather", GetWeatherTool)
        .capabilities(ServerCapabilities::tools_only())
        .with_observability(config)  // One line adds full observability!
        .build()?;

    Ok(())
}
```

### Configuration Presets

PMCP provides ready-to-use configuration presets:

| Preset | Backend | Use Case |
|--------|---------|----------|
| `ObservabilityConfig::development()` | Console (pretty) | Local development |
| `ObservabilityConfig::production()` | CloudWatch EMF | AWS production |
| `ObservabilityConfig::disabled()` | None | Testing, minimal overhead |
| `ObservabilityConfig::default()` | Console | General purpose |

### TOML Configuration

Configure observability via `.pmcp-config.toml`:

```toml
[observability]
enabled = true
backend = "console"  # or "cloudwatch"
sample_rate = 1.0    # 1.0 = 100% of requests
max_depth = 10       # Loop prevention for composed servers

[observability.console]
pretty = true
verbose = false

[observability.cloudwatch]
namespace = "MyApp/MCP"
emf_enabled = true   # CloudWatch Embedded Metric Format
```

### Environment Variable Overrides

Override any configuration via environment variables:

```bash
# Master controls
export PMCP_OBSERVABILITY_ENABLED=true
export PMCP_OBSERVABILITY_BACKEND=cloudwatch
export PMCP_OBSERVABILITY_SAMPLE_RATE=0.1  # Sample 10% in high-traffic

# CloudWatch settings
export PMCP_CLOUDWATCH_NAMESPACE="Production/MCPServers"
export PMCP_CLOUDWATCH_EMF_ENABLED=true

# Console settings
export PMCP_CONSOLE_PRETTY=false  # JSON output for log aggregation
```

### TraceContext for Distributed Tracing

The built-in module includes `TraceContext` for request correlation:

```rust
use pmcp::server::observability::TraceContext;

// Create a root trace for a new request
let root_trace = TraceContext::new_root();
println!("trace_id: {}", root_trace.short_trace_id());
println!("span_id: {}", &root_trace.span_id[..8]);

// Create child spans for sub-operations
let child_trace = root_trace.child();
println!("parent_span_id: {}", child_trace.parent_span_id.as_ref().unwrap());
println!("depth: {}", child_trace.depth);  // Increments for nested calls
```

### What Gets Captured

The built-in observability middleware automatically captures:

| Event Type | Data Captured |
|------------|---------------|
| **Request Events** | trace_id, span_id, server_name, method, tool_name, user_id, tenant_id |
| **Response Events** | duration_ms, success/failure, error_code, response_size |
| **Metrics** | request_count, request_duration, error_count, composition_depth |

### CloudWatch EMF Integration

For AWS deployments, CloudWatch Embedded Metric Format (EMF) enables automatic metric extraction from structured logs:

```rust
let config = ObservabilityConfig::production();
// EMF logs automatically become CloudWatch metrics:
// - MCP/RequestDuration
// - MCP/RequestCount
// - MCP/ErrorCount
```

### Full Example

See the complete example at `examples/61_observability_middleware.rs`:

```bash
cargo run --example 61_observability_middleware
```

This demonstrates:
- Development configuration (console output)
- Production configuration (CloudWatch EMF)
- Custom configuration (sampling, field capture)
- Disabled observability (for testing)
- Loading from file/environment
- Trace context propagation

### When to Use Custom Middleware Instead

The built-in observability is sufficient for most use cases. Consider custom middleware when you need:

- **Custom metric backends** (Prometheus, Datadog, Grafana Cloud)
- **Business-specific metrics** (cache hit rates, API quotas)
- **Custom log formats** (specific compliance requirements)
- **Integration with existing observability infrastructure**

The following sections cover building custom middleware for these advanced scenarios.

## PMCP Middleware Architecture

PMCP provides a layered middleware system for both protocol-level and HTTP-level instrumentation:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    PMCP Middleware Layers                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                    HTTP Layer (Transport)                       │    │
│  │                                                                 │    │
│  │  ServerHttpMiddleware / HttpMiddleware                          │    │
│  │  • CORS headers                                                 │    │
│  │  • Rate limiting                                                │    │
│  │  • OAuth token injection                                        │    │
│  │  • Request/response logging (with redaction)                    │    │
│  │  • Compression                                                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                              │                                          │
│                              ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                Protocol Layer (JSON-RPC)                        │    │
│  │                                                                 │    │
│  │  AdvancedMiddleware / Middleware                                │    │
│  │  • Request validation                                           │    │
│  │  • Metrics collection                                           │    │
│  │  • Circuit breaker                                              │    │
│  │  • Request timing                                               │    │
│  │  • Context propagation                                          │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                              │                                          │
│                              ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                   Tool Handlers                                 │    │
│  │                                                                 │    │
│  │  TypedToolWithOutput implementations                            │    │
│  │  • Business logic                                               │    │
│  │  • Tool-specific metrics                                        │    │
│  │  • Domain logging                                               │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Middleware Priority System

PMCP middleware executes in priority order:

```rust
pub enum MiddlewarePriority {
    Critical = 0,  // Security, validation - runs first
    High = 1,      // Authentication, rate limiting
    Normal = 2,    // Business logic middleware
    Low = 3,       // Logging, metrics
    Lowest = 4,    // Cleanup, finalization
}
```

Requests flow **down** through priorities (Critical → Lowest).
Responses flow **up** through priorities (Lowest → Critical).

### Built-in Middleware

PMCP includes production-ready middleware:

| Middleware | Purpose | Priority |
|------------|---------|----------|
| `MetricsMiddleware` | Performance metrics collection | Low |
| `LoggingMiddleware` | Request/response logging | Low |
| `RateLimitMiddleware` | Request throttling | High |
| `CircuitBreakerMiddleware` | Failure isolation | High |
| `CompressionMiddleware` | Response compression | Normal |
| `ServerHttpLoggingMiddleware` | HTTP-level logging with redaction | Normal |
| `OAuthClientMiddleware` | Token injection | High |

## Testing as Observability

Your test scenarios from earlier chapters become observability tools:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Testing as Observability                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────────┐  │
│  │   CI/CD     │    │  Scheduled  │    │   Continuous Monitoring     │  │
│  │  Pipeline   │    │   Jobs      │    │                             │  │
│  └──────┬──────┘    └──────┬──────┘    └──────────────┬──────────────┘  │
│         │                  │                          │                 │
│         ▼                  ▼                          ▼                 │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                cargo pmcp test --server <name>                  │    │
│  │                                                                 │    │
│  │  scenarios/server-name/                                         │    │
│  │  ├── smoke.yaml        # Basic connectivity                     │    │
│  │  ├── tools.yaml        # Tool functionality                     │    │
│  │  ├── edge_cases.yaml   # Error handling                         │    │
│  │  └── perf.yaml         # Performance baselines                  │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                              │                                          │
│                              ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                     Observability Signals                       │    │
│  │                                                                 │    │
│  │  ✓ Database still accessible    (data system availability)      │    │
│  │  ✓ API keys valid               (secret rotation check)         │    │
│  │  ✓ Response times normal        (performance regression)        │    │
│  │  ✓ Error rates acceptable       (quality baseline)              │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Automated Health Checks

Run tests periodically to catch issues:

```yaml
# .github/workflows/health-check.yml
name: MCP Server Health Check

on:
  schedule:
    - cron: '*/15 * * * *'  # Every 15 minutes
  workflow_dispatch:

jobs:
  health-check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run smoke tests
        run: |
          cargo pmcp test --server weather --scenario smoke
        env:
          WEATHER_API_KEY: ${{ secrets.WEATHER_API_KEY }}

      - name: Alert on failure
        if: failure()
        uses: slackapi/slack-github-action@v1
        with:
          payload: |
            {
              "text": "MCP Server health check failed!"
            }
```

### Detecting Issues

| Issue Type | Detection Method |
|------------|-----------------|
| Database unavailable | Scenario step times out |
| Secret rotation needed | Authentication error in test |
| Performance regression | Duration assertion fails |
| API breaking change | Schema validation fails |
| Rate limit exhausted | Error response matches pattern |

## Chapter Contents

This chapter covers:

1. **[Middleware Architecture](./ch17-01-architecture.md)** - Building custom middleware, priority ordering, context propagation
2. **[Logging Best Practices](./ch17-02-logging.md)** - Structured logging with tracing, sensitive data handling
3. **[Metrics Collection](./ch17-03-metrics.md)** - Performance metrics, multi-platform integration, dashboards

## Key Takeaways

- **Observability is not optional** for enterprise MCP servers
- **Middleware provides the instrumentation hooks** at both HTTP and protocol layers
- **Rust's tracing ecosystem** offers structured, zero-cost logging
- **Metrics enable alerting** before users notice problems
- **Test scenarios become health checks** when run periodically
- **Platform-agnostic design** lets you integrate with any observability stack

## Knowledge Check

Test your understanding of MCP middleware and observability:

{{#quiz ../quizzes/ch17-observability.toml}}

---

*Continue to [Middleware Architecture](./ch17-01-architecture.md) →*
