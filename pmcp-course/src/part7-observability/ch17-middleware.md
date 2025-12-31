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
