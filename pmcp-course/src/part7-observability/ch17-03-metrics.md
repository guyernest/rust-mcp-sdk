# Metrics Collection

Metrics transform operations from reactive firefighting to proactive monitoring. This section covers Rust's metrics ecosystem, PMCP's built-in metrics middleware, and integration with popular observability platforms.

## What are Metrics?

If you're new to production metrics, think of them as the vital signs of your application. Just as a doctor monitors heart rate, blood pressure, and temperature to assess health, metrics give you numbers that indicate whether your system is healthy.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Metrics vs Logs: When to Use Each                    │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  LOGS answer: "What happened?"                                          │
│  METRICS answer: "How much/how fast/how many?"                          │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Scenario: Your MCP server is "slow"                                    │
│                                                                         │
│  Logs tell you:                    Metrics tell you:                    │
│  ═══════════════                   ═════════════════                    │
│                                                                         │
│  "Request abc-123 took 5000ms"     Requests/second: 150                 │
│  "Request def-456 took 3200ms"     P50 latency: 45ms                    │
│  "Request ghi-789 took 4800ms"     P95 latency: 250ms                   │
│  "Request jkl-012 took 50ms"       P99 latency: 4,800ms  ← Problem!     │
│  ... (thousands more)              Error rate: 0.5%                     │
│                                                                         │
│  To find the problem in logs:      To find the problem in metrics:      │
│  • Search through thousands        • Glance at dashboard                │
│  • Calculate averages manually     • See P99 spike immediately          │
│  • Hard to spot patterns           • Correlate with time                │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Use LOGS when you need:           Use METRICS when you need:           │
│  • Full context of an event        • Trends over time                   │
│  • Debugging specific issues       • Alerting on thresholds             │
│  • Audit trails                    • Capacity planning                  │
│  • Error messages                  • SLA monitoring                     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Why Metrics Matter

| Without Metrics | With Metrics |
|-----------------|--------------|
| "Users say it's slow" | "P95 latency increased from 100ms to 500ms at 2:30 PM" |
| "Something is wrong" | "Error rate jumped from 0.1% to 5% after the last deployment" |
| "We need more capacity" | "At current growth rate, we'll hit capacity limits in 3 weeks" |
| "Is the fix working?" | "Error rate dropped from 5% to 0.2% after the hotfix" |

### The Three Types of Metrics

Before diving into code, let's understand the three fundamental metric types. Each serves a different purpose:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    The Three Metric Types                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  COUNTER                                                                │
│  ═══════                                                                │
│  "How many times did X happen?"                                         │
│                                                                         │
│  • Only goes UP (or resets to 0)                                        │
│  • Like an odometer in a car                                            │
│                                                                         │
│  Examples:                          ┌─────────────────────────┐         │
│  • Total requests served            │ requests_total          │         │
│  • Total errors                     │ ████████████████ 1,523  │         │
│  • Total bytes transferred          │                         │         │
│                                     │ errors_total            │         │
│  Use when: You want to count        │ ██ 47                   │         │
│  events that accumulate             └─────────────────────────┘         │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  GAUGE                                                                  │
│  ═════                                                                  │
│  "What is the current value of X?"                                      │
│                                                                         │
│  • Can go UP and DOWN                                                   │
│  • Like a thermometer or fuel gauge                                     │
│                                                                         │
│  Examples:                          ┌─────────────────────────┐         │
│  • Active connections               │ connections_active      │         │
│  • Queue depth                      │ ████████░░░░ 42         │         │
│  • Memory usage                     │                         │         │
│  • Temperature                      │ (can increase/decrease) │         │
│                                     └─────────────────────────┘         │
│  Use when: You want to track                                            │
│  current state that fluctuates                                          │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  HISTOGRAM                                                              │
│  ═════════                                                              │
│  "What is the distribution of X?"                                       │
│                                                                         │
│  • Records many values, calculates percentiles                          │
│  • Like tracking all marathon finish times, not just the average        │
│                                                                         │
│  Examples:                          ┌─────────────────────────┐         │
│  • Request latency                  │ request_duration_ms     │         │
│  • Response size                    │                         │         │
│  • Query execution time             │  ▂▅█▇▄▂▁                │         │
│                                     │  10 50 100 200 500 ms   │         │
│  Use when: You need percentiles     │                         │         │
│  (P50, P95, P99) not just averages  │  P50: 45ms  P99: 450ms  │         │
│                                     └─────────────────────────┘         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Understanding Percentiles

Percentiles are crucial for understanding real user experience. Here's why averages can be misleading:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Why Percentiles Matter                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Scenario: 100 requests with these latencies:                           │
│                                                                         │
│  • 90 requests: 50ms each                                               │
│  • 9 requests: 100ms each                                               │
│  • 1 request: 5,000ms (timeout!)                                        │
│                                                                         │
│  Average = (90×50 + 9×100 + 1×5000) / 100 = 104ms  ← "Looks fine!"      │
│                                                                         │
│  But look at percentiles:                                               │
│  • P50 (median) = 50ms    ← Half of users see 50ms or less              │
│  • P90 = 50ms             ← 90% of users see 50ms or less               │
│  • P95 = 100ms            ← 95% of users see 100ms or less              │
│  • P99 = 5,000ms          ← 1% of users wait 5 SECONDS! 🚨              │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Which percentile to monitor?                                           │
│                                                                         │
│  • P50 (median): Typical user experience                                │
│  • P95: Most users' worst-case experience                               │
│  • P99: Your "long tail" - affects 1 in 100 users                       │
│  • P99.9: For high-traffic sites (1 in 1000 users)                      │
│                                                                         │
│  If you have 1 million requests/day:                                    │
│  • P99 = 10,000 users having a bad experience daily                     │
│  • P99.9 = 1,000 users having a bad experience daily                    │
│                                                                         │
│  Rule of thumb: Alert on P95 or P99, not averages                       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## The Metrics Ecosystem

Rust's `metrics` crate provides a facade pattern similar to `log` for logging—you write metrics once and choose the backend at runtime:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Metrics Architecture                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Application Code                                                       │
│  ════════════════                                                       │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │  counter!("requests_total").increment(1);                       │    │
│  │  histogram!("request_duration_ms").record(45.5);                │    │
│  │  gauge!("active_connections").set(12);                          │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                              │                                          │
│                              ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                    metrics (facade crate)                       │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                              │                                          │
│            ┌─────────────────┼─────────────────┐                        │
│            ▼                 ▼                 ▼                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                   │
│  │ Prometheus   │  │   Datadog    │  │  CloudWatch  │                   │
│  │  Exporter    │  │    Agent     │  │    Agent     │                   │
│  └──────────────┘  └──────────────┘  └──────────────┘                   │
│         │                  │                 │                          │
│         ▼                  ▼                 ▼                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                   │
│  │  Prometheus  │  │   Datadog    │  │     AWS      │                   │
│  │    Server    │  │    Cloud     │  │  CloudWatch  │                   │
│  └──────────────┘  └──────────────┘  └──────────────┘                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Metric Types

| Type | Purpose | Example |
|------|---------|---------|
| **Counter** | Monotonically increasing count | Total requests, errors |
| **Gauge** | Value that can go up or down | Active connections, queue depth |
| **Histogram** | Distribution of values | Request duration, response size |

```rust
use metrics::{counter, gauge, histogram};

async fn handler(input: Input) -> Result<Output> {
    let start = Instant::now();

    // Count the request
    counter!("mcp.requests_total", "tool" => "get-weather").increment(1);

    // Track active requests
    gauge!("mcp.requests_active").increment(1.0);

    let result = process(input).await;

    // Record duration
    histogram!("mcp.request_duration_ms", "tool" => "get-weather")
        .record(start.elapsed().as_millis() as f64);

    // Track active requests
    gauge!("mcp.requests_active").decrement(1.0);

    // Count success/failure
    match &result {
        Ok(_) => counter!("mcp.requests_success").increment(1),
        Err(_) => counter!("mcp.requests_error").increment(1),
    }

    result
}
```

## PMCP's MetricsMiddleware

PMCP includes a `MetricsMiddleware` that automatically tracks request metrics:

```rust
use pmcp::shared::MetricsMiddleware;
use pmcp::shared::EnhancedMiddlewareChain;
use std::sync::Arc;

fn build_instrumented_chain() -> EnhancedMiddlewareChain {
    let mut chain = EnhancedMiddlewareChain::new();

    // Add metrics collection
    chain.add(Arc::new(MetricsMiddleware::new("my-server".to_string())));

    chain
}
```

### Recorded Metrics

The `MetricsMiddleware` automatically records:

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `mcp.requests.total` | Counter | service, method | Total requests processed |
| `mcp.requests.duration_ms` | Histogram | service, method | Request latency |
| `mcp.requests.errors` | Counter | service, error_type | Error count by type |
| `mcp.requests.active` | Gauge | service | In-flight requests |

### Custom Metrics in Handlers

Add tool-specific metrics directly in handlers:

```rust
use metrics::{counter, histogram};
use std::time::Instant;

async fn handler(input: WeatherInput) -> Result<Weather> {
    let start = Instant::now();

    // Business metrics
    counter!(
        "weather.lookups_total",
        "city" => input.city.clone(),
        "units" => input.units.as_str()
    ).increment(1);

    let weather = match cache.get(&input.city) {
        Some(cached) => {
            counter!("weather.cache_hits").increment(1);
            cached
        }
        None => {
            counter!("weather.cache_misses").increment(1);
            let result = fetch_weather(&input.city).await?;

            histogram!("weather.api_latency_ms")
                .record(start.elapsed().as_millis() as f64);

            result
        }
    };

    // Track temperature extremes
    if weather.temperature > 40.0 {
        counter!("weather.extreme_heat_events").increment(1);
    }

    Ok(weather)
}
```

## Platform Integration

### Prometheus

Prometheus is the industry standard for cloud-native metrics:

```rust
// Cargo.toml
[dependencies]
metrics = "0.23"
metrics-exporter-prometheus = "0.15"

// main.rs
use metrics_exporter_prometheus::PrometheusBuilder;

fn init_metrics() {
    // Start Prometheus exporter on port 9090
    PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], 9090))
        .install()
        .expect("Failed to install Prometheus exporter");
}

#[tokio::main]
async fn main() {
    init_metrics();

    // Metrics now available at http://localhost:9090/metrics
    run_server().await;
}
```

**Prometheus output format:**
```
# HELP mcp_requests_total Total MCP requests
# TYPE mcp_requests_total counter
mcp_requests_total{service="weather-server",method="get-weather"} 1523

# HELP mcp_request_duration_ms Request latency in milliseconds
# TYPE mcp_request_duration_ms histogram
mcp_request_duration_ms_bucket{service="weather-server",le="10"} 450
mcp_request_duration_ms_bucket{service="weather-server",le="50"} 1200
mcp_request_duration_ms_bucket{service="weather-server",le="100"} 1500
mcp_request_duration_ms_bucket{service="weather-server",le="+Inf"} 1523
mcp_request_duration_ms_sum{service="weather-server"} 45678.5
mcp_request_duration_ms_count{service="weather-server"} 1523
```

### Datadog

Datadog integration via StatsD or direct API:

```rust
// Cargo.toml
[dependencies]
metrics = "0.23"
metrics-exporter-statsd = "0.7"

// Using StatsD (Datadog agent listens on port 8125)
use metrics_exporter_statsd::StatsdBuilder;

fn init_metrics() {
    StatsdBuilder::from("127.0.0.1", 8125)
        .with_queue_size(5000)
        .with_buffer_size(1024)
        .install()
        .expect("Failed to install StatsD exporter");
}
```

**Datadog tags:**
```rust
counter!(
    "mcp.requests",
    "service" => "weather-server",
    "tool" => "get-weather",
    "env" => "production"
).increment(1);

// Becomes: mcp.requests:1|c|#service:weather-server,tool:get-weather,env:production
```

### AWS CloudWatch

CloudWatch integration for AWS-hosted servers:

```rust
// Cargo.toml
[dependencies]
metrics = "0.23"
aws-sdk-cloudwatch = "1.0"
tokio = { version = "1", features = ["full"] }

// Custom CloudWatch recorder
use aws_sdk_cloudwatch::{Client, types::MetricDatum, types::StandardUnit};
use metrics::{Counter, Gauge, Histogram, Key, KeyName, Recorder, Unit};
use std::sync::Arc;

struct CloudWatchRecorder {
    client: Client,
    namespace: String,
}

impl CloudWatchRecorder {
    async fn new(namespace: &str) -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        Self {
            client: Client::new(&config),
            namespace: namespace.to_string(),
        }
    }

    async fn publish_metrics(&self, metrics: Vec<MetricDatum>) {
        self.client
            .put_metric_data()
            .namespace(&self.namespace)
            .set_metric_data(Some(metrics))
            .send()
            .await
            .expect("Failed to publish metrics");
    }
}
```

### Grafana Cloud / OpenTelemetry

For Grafana Cloud or any OpenTelemetry-compatible backend:

```rust
// Cargo.toml
[dependencies]
opentelemetry = "0.24"
opentelemetry_sdk = "0.24"
opentelemetry-otlp = "0.17"
tracing-opentelemetry = "0.25"

use opentelemetry::global;
use opentelemetry_sdk::metrics::MeterProvider;
use opentelemetry_otlp::WithExportConfig;

fn init_otel_metrics() -> Result<(), Box<dyn std::error::Error>> {
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint("https://otlp.grafana.net:4317");

    let provider = MeterProvider::builder()
        .with_reader(
            opentelemetry_sdk::metrics::PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio)
                .with_interval(std::time::Duration::from_secs(30))
                .build()
        )
        .build();

    global::set_meter_provider(provider);
    Ok(())
}
```

## Multi-Platform Strategy

Design metrics to work across platforms:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Multi-Platform Metrics Design                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                    Application Layer                            │    │
│  │                                                                 │    │
│  │  Use metrics crate with consistent naming:                      │    │
│  │  • mcp.requests.total                                           │    │
│  │  • mcp.requests.duration_ms                                     │    │
│  │  • mcp.requests.errors                                          │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                              │                                          │
│                              ▼                                          │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                   Platform Adapter                              │    │
│  │                                                                 │    │
│  │  Choose at deployment time via environment/config:              │    │
│  │                                                                 │    │
│  │  METRICS_BACKEND=prometheus  →  PrometheusBuilder               │    │
│  │  METRICS_BACKEND=datadog     →  StatsdBuilder                   │    │
│  │  METRICS_BACKEND=cloudwatch  →  CloudWatchRecorder              │    │
│  │  METRICS_BACKEND=otlp        →  OpenTelemetry                   │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Platform Selection at Runtime

```rust
use std::env;

fn init_metrics_backend() {
    let backend = env::var("METRICS_BACKEND")
        .unwrap_or_else(|_| "prometheus".to_string());

    match backend.as_str() {
        "prometheus" => {
            metrics_exporter_prometheus::PrometheusBuilder::new()
                .with_http_listener(([0, 0, 0, 0], 9090))
                .install()
                .expect("Prometheus exporter failed");
        }
        "statsd" | "datadog" => {
            let host = env::var("STATSD_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = env::var("STATSD_PORT")
                .unwrap_or_else(|_| "8125".to_string())
                .parse()
                .expect("Invalid STATSD_PORT");

            metrics_exporter_statsd::StatsdBuilder::from(&host, port)
                .install()
                .expect("StatsD exporter failed");
        }
        "none" | "disabled" => {
            // No-op for local development
            tracing::info!("Metrics collection disabled");
        }
        other => {
            panic!("Unknown metrics backend: {}", other);
        }
    }
}
```

## Metrics Best Practices

### Naming Conventions

```rust
// GOOD: Hierarchical, consistent naming
counter!("mcp.tool.requests_total", "tool" => "weather").increment(1);
histogram!("mcp.tool.duration_ms", "tool" => "weather").record(45.0);
counter!("mcp.tool.errors_total", "tool" => "weather", "error" => "timeout").increment(1);

// BAD: Inconsistent, flat naming
counter!("weather_requests").increment(1);
counter!("weatherToolDurationMs").increment(1);
counter!("errors").increment(1);
```

### Cardinality Control

**Cardinality** refers to the number of unique combinations of label values for a metric. This is one of the most common pitfalls for newcomers to metrics—and it can crash your monitoring system.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    The Cardinality Problem                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  What happens with high cardinality labels?                             │
│  ══════════════════════════════════════════                             │
│                                                                         │
│  Each unique label combination creates a NEW time series in memory:     │
│                                                                         │
│  counter!("requests", "user_id" => user_id)                             │
│                                                                         │
│  With 1 million users, this creates 1 MILLION time series:              │
│                                                                         │
│  requests{user_id="user-000001"} = 5                                    │
│  requests{user_id="user-000002"} = 12                                   │
│  requests{user_id="user-000003"} = 3                                    │
│  ... (999,997 more) ...                                                 │
│  requests{user_id="user-999999"} = 7                                    │
│  requests{user_id="user-1000000"} = 1                                   │
│                                                                         │
│  Each time series consumes memory in:                                   │
│  • Your application                                                     │
│  • Prometheus/metrics backend                                           │
│  • Grafana/dashboard queries                                            │
│                                                                         │
│  Result: Memory exhaustion, slow queries, crashed monitoring            │
│                                                                         │
│  ─────────────────────────────────────────────────────────────────────  │
│                                                                         │
│  Good labels (bounded):              Bad labels (unbounded):            │
│  ══════════════════════              ══════════════════════             │
│                                                                         │
│  • tool: 10-50 tools max             • user_id: millions of users       │
│  • status: success/error             • request_id: infinite             │
│  • tier: free/pro/enterprise         • city: thousands of cities        │
│  • environment: dev/staging/prod     • email: unbounded                 │
│  • http_method: GET/POST/PUT/DELETE  • timestamp: infinite              │
│                                                                         │
│  Rule of thumb: Labels should have fewer than 100 possible values       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

**If you need per-user or per-request data, use logs instead of metrics.** Logs are designed for high-cardinality data; metrics are not.

```rust
// BAD: Unbounded cardinality (user_id could be millions)
counter!("requests", "user_id" => user_id).increment(1);

// BAD: High cardinality (city names - thousands of values)
counter!("weather_requests", "city" => &input.city).increment(1);

// GOOD: Bounded cardinality (only 3 possible values)
counter!(
    "requests",
    "user_tier" => user.tier.as_str()  // "free", "pro", "enterprise"
).increment(1);

// GOOD: Use histogram for distribution instead of labels
histogram!("request_duration_ms").record(duration);

// GOOD: Log high-cardinality data instead of metrics
tracing::info!(user_id = %user_id, city = %city, "Request processed");
```

### Standard Labels

Apply consistent labels across all metrics:

```rust
use std::sync::OnceLock;

struct MetricsContext {
    service: String,
    version: String,
    environment: String,
}

static CONTEXT: OnceLock<MetricsContext> = OnceLock::new();

fn init_context() {
    CONTEXT.get_or_init(|| MetricsContext {
        service: env::var("SERVICE_NAME").unwrap_or_else(|_| "mcp-server".to_string()),
        version: env!("CARGO_PKG_VERSION").to_string(),
        environment: env::var("ENV").unwrap_or_else(|_| "development".to_string()),
    });
}

// Helper for consistent labeling
macro_rules! labeled_counter {
    ($name:expr, $($key:expr => $value:expr),*) => {{
        let ctx = CONTEXT.get().expect("Metrics context not initialized");
        counter!(
            $name,
            "service" => ctx.service.clone(),
            "version" => ctx.version.clone(),
            "env" => ctx.environment.clone(),
            $($key => $value),*
        )
    }};
}

// Usage
labeled_counter!("mcp.requests", "tool" => "weather").increment(1);
```

## Dashboard Examples

### Key Performance Indicators

```yaml
# Grafana dashboard panels (pseudo-config)
panels:
  - title: "Request Rate"
    query: rate(mcp_requests_total[5m])
    type: graph

  - title: "P95 Latency"
    query: histogram_quantile(0.95, rate(mcp_request_duration_ms_bucket[5m]))
    type: graph

  - title: "Error Rate"
    query: rate(mcp_requests_errors_total[5m]) / rate(mcp_requests_total[5m])
    type: gauge
    thresholds:
      - value: 0.01
        color: yellow
      - value: 0.05
        color: red

  - title: "Active Connections"
    query: mcp_connections_active
    type: stat
```

### Alert Rules

```yaml
# Prometheus alerting rules
groups:
  - name: mcp-server
    rules:
      - alert: HighErrorRate
        expr: rate(mcp_requests_errors_total[5m]) / rate(mcp_requests_total[5m]) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "MCP server error rate above 5%"

      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(mcp_request_duration_ms_bucket[5m])) > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "MCP server P95 latency above 1 second"

      - alert: ServiceDown
        expr: up{job="mcp-server"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "MCP server is down"
```

## Testing with Metrics

Use test scenarios as health checks that verify metrics:

```yaml
# scenarios/smoke.yaml
name: "Smoke Test with Metrics Verification"
steps:
  - name: "Call weather tool"
    operation:
      type: tool_call
      tool: "get-weather"
      arguments:
        city: "London"
    assertions:
      - type: success
      - type: duration_ms
        max: 1000

  # Verify metrics endpoint
  - name: "Check metrics"
    operation:
      type: http_get
      url: "http://localhost:9090/metrics"
    assertions:
      - type: contains
        value: "mcp_requests_total"
      - type: contains
        value: 'tool="get-weather"'
```

### Metrics in CI/CD

```yaml
# .github/workflows/test.yml
jobs:
  test:
    steps:
      - name: Start server
        run: cargo run --release &
        env:
          METRICS_BACKEND: prometheus

      - name: Wait for startup
        run: sleep 5

      - name: Run tests
        run: cargo pmcp test --server weather

      - name: Verify metrics
        run: |
          curl -s http://localhost:9090/metrics | grep mcp_requests_total
          curl -s http://localhost:9090/metrics | grep mcp_request_duration_ms
```

## Summary

| Aspect | Recommendation |
|--------|---------------|
| **Crate** | Use `metrics` facade for portability |
| **Types** | Counter (totals), Histogram (durations), Gauge (current state) |
| **Naming** | Hierarchical: `mcp.component.metric_name` |
| **Labels** | Service, tool, environment; avoid high cardinality |
| **Platform** | Configure at runtime via environment variables |
| **Prometheus** | Default for cloud-native, excellent Grafana support |
| **Datadog** | StatsD exporter, good for existing Datadog users |
| **CloudWatch** | Custom recorder for AWS-native deployments |
| **Alerting** | Error rate > 5%, P95 latency > 1s, service down |

Metrics provide the quantitative foundation for understanding system behavior. Combined with logging and tracing, they complete the observability picture for enterprise MCP servers.

---

*Return to [Middleware and Instrumentation](./ch17-middleware.md) | Continue to [Operations and Monitoring →](./ch18-operations.md)*
