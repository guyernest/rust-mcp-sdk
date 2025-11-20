---
inclusion: manual
---

# MCP Observability - Logging, Metrics, and Monitoring

This guide covers observability for MCP servers built with pmcp SDK. Proper observability enables debugging, performance monitoring, and production operations.

## Observability Philosophy (Toyota Way)

### Visual Management (見える化 - Mieruka)

In Toyota Production System, visual management makes problems immediately visible. For MCP servers:

- **Structured logging**: See what's happening in real-time
- **Metrics**: Measure performance and identify bottlenecks
- **Tracing**: Understand request flow through the system
- **Alerts**: Be notified when something goes wrong

### Jidoka (自働化 - Automation with Human Touch)

Observability should automatically detect problems, but humans should understand and fix them:

- **Auto-detect**: Metrics and logs catch anomalies
- **Human-readable**: Logs are clear and actionable
- **Stop and fix**: When problems appear, investigate immediately
- **Root cause**: Logs provide context for debugging

## Logging with `tracing`

### Why `tracing` Over `log`?

The pmcp SDK uses the `tracing` crate (not `log`) because it provides:

- **Structured logging**: Fields are typed, searchable
- **Async-aware**: Tracks context across `.await` points
- **Spans**: Group related events together
- **Performance**: Minimal overhead when disabled

### Basic Setup

**Add dependencies** to `Cargo.toml`:

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

**Initialize in `main.rs`**:

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,mcp_myserver=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting MCP server");

    // Server setup...
    Ok(())
}
```

### Logging Levels

```rust
use tracing::{error, warn, info, debug, trace};

// ERROR: Something failed, requires immediate attention
error!("Failed to connect to database: {}", err);

// WARN: Something unexpected, but not fatal
warn!("API rate limit approaching: {}/1000 requests", count);

// INFO: Important state changes, user actions
info!("Weather forecast requested for city: {}", city);

// DEBUG: Detailed diagnostic information
debug!("Cache hit for key: {}", cache_key);

// TRACE: Very detailed, only for deep debugging
trace!("Parsing JSON: {}", raw_json);
```

### Structured Logging

**Always use structured fields** instead of string interpolation:

```rust
// ❌ Bad - string formatting
info!("User {} requested weather for {}", user_id, city);

// ✅ Good - structured fields
info!(
    user_id = %user_id,
    city = %city,
    "Weather requested"
);
```

**Benefits**:
- Searchable: `grep city=London logs.json`
- Aggregatable: Count requests per city
- Type-safe: Numbers stay numbers, not strings

### Logging in Tool Handlers

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra, Error};
use tracing::{info, debug, warn, instrument};

#[instrument(skip(extra), fields(city = %input.city))]
async fn handler(
    input: WeatherInput,
    extra: RequestHandlerExtra
) -> Result<WeatherOutput> {
    // Automatically logs function entry/exit with args

    // Validate
    if input.city.is_empty() {
        warn!("Empty city name provided");
        return Err(Error::validation("City cannot be empty"));
    }

    let days = input.days.unwrap_or(1);
    debug!(days = days, "Using default days value");

    // Call external API
    info!(
        city = %input.city,
        days = days,
        "Fetching weather from API"
    );

    let client = reqwest::Client::new();
    let response = match client
        .get(&format!("https://api.weather.com/{}", input.city))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            error!(
                error = %e,
                city = %input.city,
                "API request failed"
            );
            return Err(Error::internal("Failed to fetch weather"));
        }
    };

    if !response.status().is_success() {
        warn!(
            status = %response.status(),
            city = %input.city,
            "API returned error status"
        );
        return Err(Error::validation(
            format!("City '{}' not found", input.city)
        ));
    }

    let data: WeatherOutput = response.json().await?;

    info!(
        city = %input.city,
        temperature = data.temperature,
        "Successfully fetched weather"
    );

    Ok(data)
}
```

### Spans for Request Tracing

Use spans to group related operations:

```rust
use tracing::instrument;

#[instrument(name = "weather_forecast", skip(extra))]
async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    // Everything in this function is part of the "weather_forecast" span

    let data = fetch_from_api(&input.city).await?;
    let processed = process_weather_data(data).await?;

    Ok(processed)
}

#[instrument]
async fn fetch_from_api(city: &str) -> Result<RawWeatherData> {
    // This creates a child span: weather_forecast -> fetch_from_api
    tracing::debug!("Fetching from API");
    // ...
}

#[instrument]
async fn process_weather_data(data: RawWeatherData) -> Result<WeatherOutput> {
    // Another child span: weather_forecast -> process_weather_data
    tracing::debug!("Processing data");
    // ...
}
```

**Output**:
```
2025-11-20T10:30:00Z INFO weather_forecast{city="London"}: Entered
2025-11-20T10:30:00Z DEBUG fetch_from_api{city="London"}: Fetching from API
2025-11-20T10:30:01Z DEBUG process_weather_data: Processing data
2025-11-20T10:30:01Z INFO weather_forecast{city="London"}: Exited
```

### Environment-Based Configuration

Control logging via environment variables:

```bash
# Show all INFO and above
RUST_LOG=info ./myserver-server

# Show DEBUG for your server, INFO for others
RUST_LOG=info,mcp_myserver=debug ./myserver-server

# Show TRACE for specific module
RUST_LOG=mcp_myserver::tools::weather=trace ./myserver-server

# JSON output for production
RUST_LOG=info RUST_LOG_FORMAT=json ./myserver-server
```

**In code**:

```rust
// Supports RUST_LOG env var
tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();
```

## Metrics Collection

### Using `metrics` Crate

**Add dependencies**:

```toml
[dependencies]
metrics = "0.21"
metrics-exporter-prometheus = "0.12"
```

**Setup in `main.rs`**:

```rust
use metrics_exporter_prometheus::PrometheusBuilder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Prometheus metrics exporter
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(([0, 0, 0, 0], 9090))
        .install()
        .expect("Failed to install Prometheus exporter");

    tracing::info!("Metrics available at http://0.0.0.0:9090/metrics");

    // Server setup...
    Ok(())
}
```

### Metric Types

#### 1. Counter (Monotonic Increase)

```rust
use metrics::counter;

// Count total requests
counter!("mcp.requests.total", 1);

// Count with labels
counter!("mcp.requests.total", 1, "tool" => "get-weather", "status" => "success");
counter!("mcp.requests.total", 1, "tool" => "get-weather", "status" => "error");
```

#### 2. Gauge (Current Value)

```rust
use metrics::gauge;

// Track active connections
gauge!("mcp.connections.active", active_count as f64);

// Track cache size
gauge!("mcp.cache.size", cache.len() as f64, "cache_name" => "weather");
```

#### 3. Histogram (Distribution)

```rust
use metrics::histogram;
use std::time::Instant;

let start = Instant::now();

// ... do work ...

let duration = start.elapsed();
histogram!("mcp.request.duration", duration.as_secs_f64(),
    "tool" => "get-weather");
```

### Metrics in Tool Handlers

```rust
use metrics::{counter, histogram};
use std::time::Instant;

async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    let start = Instant::now();

    // Count request
    counter!("weather.requests.total", 1, "city" => input.city.clone());

    let result = match fetch_weather(&input.city).await {
        Ok(data) => {
            counter!("weather.requests.total", 1, "status" => "success");
            Ok(data)
        }
        Err(e) => {
            counter!("weather.requests.total", 1, "status" => "error");
            counter!("weather.errors.total", 1, "error_type" => "api_failure");
            Err(e)
        }
    };

    // Record duration
    let duration = start.elapsed();
    histogram!("weather.request.duration", duration.as_secs_f64());

    result
}
```

### Common Metrics to Track

**Request metrics**:
```rust
counter!("mcp.requests.total", 1, "tool" => tool_name, "status" => status);
histogram!("mcp.request.duration", duration);
counter!("mcp.errors.total", 1, "tool" => tool_name, "error_type" => error_type);
```

**Cache metrics**:
```rust
counter!("mcp.cache.hits", 1);
counter!("mcp.cache.misses", 1);
gauge!("mcp.cache.size", size as f64);
gauge!("mcp.cache.hit_rate", hit_rate);
```

**External API metrics**:
```rust
counter!("mcp.external.requests", 1, "api" => "weather_api");
histogram!("mcp.external.latency", latency);
counter!("mcp.external.errors", 1, "api" => "weather_api", "status_code" => code);
```

**Resource metrics**:
```rust
gauge!("mcp.memory.usage", memory_mb);
gauge!("mcp.cpu.usage", cpu_percent);
gauge!("mcp.goroutines", thread_count as f64); // Rust doesn't have goroutines but tracks tasks
```

## Production Logging Formats

### JSON Logging for Aggregation

**Setup**:

```rust
use tracing_subscriber::fmt::format::FmtSpan;

tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info".into()),
    )
    .with(
        tracing_subscriber::fmt::layer()
            .json() // JSON format
            .with_span_events(FmtSpan::CLOSE) // Include span timing
            .with_current_span(true) // Include span context
            .with_target(true) // Include module path
    )
    .init();
```

**Output**:
```json
{"timestamp":"2025-11-20T10:30:00.123Z","level":"INFO","fields":{"message":"Weather requested","city":"London","user_id":"user123"},"target":"mcp_weather::tools::weather","span":{"name":"weather_forecast"}}
```

**Benefits**:
- Parseable by log aggregation tools (CloudWatch, Datadog, etc.)
- Searchable by field
- Structured for analysis

### Compact Logging for Development

**Setup**:

```rust
tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "debug".into()),
    )
    .with(
        tracing_subscriber::fmt::layer()
            .compact() // Compact format
            .with_target(false) // Hide module path
    )
    .init();
```

**Output**:
```
2025-11-20 10:30:00 INFO weather_forecast{city="London"}: Weather requested
```

## Deployment-Specific Observability

### AWS CloudWatch

**Setup CloudWatch integration**:

```toml
[dependencies]
tracing-subscriber = { version = "0.3", features = ["json"] }
```

```rust
// JSON logs are automatically ingested by CloudWatch
tracing_subscriber::fmt()
    .json()
    .with_current_span(true)
    .init();
```

**CloudWatch Insights queries**:

```
# Find errors
fields @timestamp, @message, city, error
| filter level = "ERROR"
| sort @timestamp desc

# Count requests by city
fields city
| stats count() by city
| sort count desc

# P95 latency
fields duration
| stats percentile(duration, 95) as p95_latency
```

**Metrics via CloudWatch**:

```rust
// Emit custom metrics to CloudWatch
use aws_sdk_cloudwatch::{Client, types::MetricDatum};

async fn emit_metric(name: &str, value: f64) {
    let client = Client::new(&config);
    client
        .put_metric_data()
        .namespace("MCP/MyServer")
        .metric_data(
            MetricDatum::builder()
                .metric_name(name)
                .value(value)
                .build()
        )
        .send()
        .await
        .ok();
}
```

### Cloudflare Workers

**Cloudflare Workers logging**:

```rust
// Workers have console API
use worker::console_log;

async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    console_log!("Weather request: city={}", input.city);

    // ... implementation ...

    Ok(output)
}
```

**Cloudflare Analytics**:

Cloudflare Workers automatically track:
- Request count
- Request duration
- Status codes
- Errors

Access via Cloudflare dashboard or Analytics API.

**Custom metrics**:

```rust
// Use Cloudflare Analytics Engine (when available)
// For now, log structured data that Cloudflare ingests

tracing::info!(
    event = "weather_request",
    city = %input.city,
    duration_ms = duration.as_millis(),
    status = "success"
);
```

### Docker/Kubernetes

**Docker logging**:

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /app/target/release/myserver-server /usr/local/bin/
# JSON logs go to stdout/stderr (Docker captures)
ENV RUST_LOG=info
ENV RUST_LOG_FORMAT=json
CMD ["myserver-server"]
```

**Kubernetes logging**:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: mcp-weather
spec:
  containers:
    - name: server
      image: mcp-weather:latest
      env:
        - name: RUST_LOG
          value: "info,mcp_weather=debug"
        - name: RUST_LOG_FORMAT
          value: "json"
      ports:
        - containerPort: 3000 # MCP server
        - containerPort: 9090 # Metrics
```

**Prometheus in Kubernetes**:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: mcp-weather
  labels:
    app: mcp-weather
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/port: "9090"
    prometheus.io/path: "/metrics"
spec:
  selector:
    app: mcp-weather
  ports:
    - name: mcp
      port: 3000
    - name: metrics
      port: 9090
```

## Error Tracking

### Structured Error Logging

```rust
use pmcp::Error;
use tracing::error;

async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    let response = client.get(&url).send().await.map_err(|e| {
        error!(
            error = %e,
            url = %url,
            error_type = "network",
            "Failed to fetch from API"
        );
        Error::internal("API unavailable")
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        error!(
            status_code = %status,
            response_body = %body,
            url = %url,
            error_type = "api_error",
            "API returned error"
        );

        return match status.as_u16() {
            404 => Err(Error::validation(format!("City '{}' not found", input.city))),
            401 | 403 => Err(Error::internal("API authentication failed")),
            429 => Err(Error::internal("API rate limit exceeded")),
            _ => Err(Error::internal("API error")),
        };
    }

    Ok(response.json().await?)
}
```

### Error Aggregation

Group errors for analysis:

```rust
counter!("mcp.errors.total", 1,
    "error_type" => error_type,    // "network", "validation", "timeout"
    "tool" => tool_name,            // "get-weather"
    "severity" => severity          // "warning", "error", "critical"
);
```

**Query in production**:
```
# Prometheus
sum by (error_type, tool) (rate(mcp_errors_total[5m]))

# CloudWatch Insights
fields error_type, tool
| stats count() by error_type, tool
```

## Performance Monitoring

### Request Latency Tracking

```rust
use std::time::Instant;
use tracing::info;
use metrics::histogram;

async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    let start = Instant::now();

    // Track phases
    let validate_start = Instant::now();
    validate_input(&input)?;
    histogram!("weather.validation.duration", validate_start.elapsed().as_secs_f64());

    let api_start = Instant::now();
    let data = fetch_from_api(&input.city).await?;
    histogram!("weather.api.duration", api_start.elapsed().as_secs_f64());

    let process_start = Instant::now();
    let output = process_data(data)?;
    histogram!("weather.processing.duration", process_start.elapsed().as_secs_f64());

    let total_duration = start.elapsed();
    histogram!("weather.total.duration", total_duration.as_secs_f64());

    info!(
        duration_ms = total_duration.as_millis(),
        city = %input.city,
        "Request completed"
    );

    Ok(output)
}
```

### Identifying Slow Operations

```rust
use tracing::{warn, info};

const SLOW_THRESHOLD: Duration = Duration::from_millis(500);

async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    let start = Instant::now();

    let result = perform_operation().await;

    let duration = start.elapsed();
    if duration > SLOW_THRESHOLD {
        warn!(
            duration_ms = duration.as_millis(),
            threshold_ms = SLOW_THRESHOLD.as_millis(),
            city = %input.city,
            "Slow request detected"
        );
    } else {
        info!(duration_ms = duration.as_millis(), "Request completed");
    }

    result
}
```

## Health Checks and Readiness

### Health Check Endpoint

```rust
// In your server setup
use axum::{Router, routing::get};

async fn health_check() -> &'static str {
    "OK"
}

async fn readiness_check() -> Result<&'static str, StatusCode> {
    // Check dependencies
    if database_healthy().await && api_reachable().await {
        Ok("READY")
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

let app = Router::new()
    .route("/health", get(health_check))
    .route("/ready", get(readiness_check))
    .route("/metrics", get(metrics_handler));
```

## Best Practices

### 1. Log Levels in Production

```
ERROR   - Only for actual errors requiring attention
WARN    - Unexpected but handled conditions
INFO    - Important state changes (default in production)
DEBUG   - Detailed diagnostic info (disabled in production)
TRACE   - Very verbose (disabled in production)
```

### 2. Sensitive Data

```rust
// ❌ Bad - logs API key
info!("Using API key: {}", api_key);

// ✅ Good - redacts sensitive data
info!("Using API key: {}...", &api_key[0..8]);

// ✅ Better - doesn't log at all
debug!("API authentication configured");
```

### 3. Structured Over Unstructured

```rust
// ❌ Bad
info!("User 123 requested weather for London at 2025-11-20");

// ✅ Good
info!(
    user_id = 123,
    city = "London",
    timestamp = %Utc::now(),
    "Weather requested"
);
```

### 4. Correlation IDs

```rust
use uuid::Uuid;

async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    let request_id = Uuid::new_v4();

    tracing::info!(
        request_id = %request_id,
        city = %input.city,
        "Processing request"
    );

    // Pass request_id to all sub-operations for correlation
    let data = fetch_weather(&input.city, request_id).await?;

    Ok(data)
}
```

## Future: cargo-pmcp Deployment Integration

When `cargo-pmcp` adds deployment features, observability will integrate automatically:

```bash
# Future cargo-pmcp deployment commands
cargo pmcp deploy --platform cloudflare --observability
# Automatically configures:
# - CloudWatch integration (AWS)
# - Cloudflare Analytics (Cloudflare)
# - Prometheus metrics export
# - Structured JSON logging

cargo pmcp observe --server myserver --platform cloudflare
# Opens dashboard with:
# - Request rates
# - Error rates
# - Latency percentiles
# - Active connections
```

## Resources

- **tracing docs**: https://docs.rs/tracing
- **metrics docs**: https://docs.rs/metrics
- **Prometheus**: https://prometheus.io/
- **CloudWatch Logs**: https://docs.aws.amazon.com/AmazonCloudWatch/latest/logs/
- **Cloudflare Analytics**: https://developers.cloudflare.com/analytics/

---

**Remember**: Observability is not optional in production. Logs, metrics, and tracing are how you understand what your MCP server is doing and quickly fix problems when they occur.
