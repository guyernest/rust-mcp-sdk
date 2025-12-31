::: exercise
id: ch17-02-metrics-collection
difficulty: intermediate
time: 45 minutes
:::

Implement comprehensive metrics collection for your MCP server. While logs
tell you "what happened", metrics tell you "how we're doing" at a glance.

::: objectives
thinking:
  - The difference between counters, gauges, and histograms
  - Why label cardinality matters (unbounded labels = OOM)
  - How metrics enable alerting and capacity planning
doing:
  - Add tool invocation counters with labels
  - Record request duration histograms
  - Track concurrent connections with gauges
  - Export metrics in Prometheus format
:::

::: discussion
- If you could only have three metrics about your server, what would they be?
- How would you know if your server is overloaded?
- What latency would you consider "too slow" for your tools?
:::

## Step 1: Add Dependencies

In `Cargo.toml`:

```toml
[dependencies]
metrics = "0.22"
metrics-exporter-prometheus = "0.13"
```

## Step 2: Initialize Metrics

```rust
use metrics_exporter_prometheus::PrometheusBuilder;

fn init_metrics() -> impl Fn() -> String {
    let recorder = PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();

    metrics::set_global_recorder(recorder)
        .expect("Failed to set metrics recorder");

    // Describe metrics for documentation
    metrics::describe_counter!(
        "mcp_requests_total",
        "Total number of MCP tool invocations"
    );
    metrics::describe_histogram!(
        "mcp_request_duration_seconds",
        "Request duration in seconds"
    );
    metrics::describe_gauge!(
        "mcp_active_requests",
        "Number of currently active requests"
    );

    move || handle.render()
}
```

## Step 3: Build Metrics Middleware

```rust
use metrics::{counter, gauge, histogram};
use std::time::Instant;

pub struct MetricsMiddleware;

#[async_trait]
impl AdvancedMiddleware for MetricsMiddleware {
    async fn on_request(
        &self,
        request: &Request,
        context: &mut Context,
    ) -> Result<()> {
        let start = Instant::now();
        context.set("metrics_start", start);

        // Track concurrent requests
        gauge!("mcp_active_requests").increment(1.0);

        Ok(())
    }

    async fn on_response(
        &self,
        response: &Response,
        context: &Context,
    ) -> Result<()> {
        let start: Instant = context.get("metrics_start")?;
        let tool_name = context.get::<String>("tool_name")
            .unwrap_or_else(|_| "unknown".to_string());

        // Record duration
        histogram!(
            "mcp_request_duration_seconds",
            "tool" => tool_name.clone()
        ).record(start.elapsed().as_secs_f64());

        // Count by status
        counter!(
            "mcp_requests_total",
            "tool" => tool_name,
            "status" => "success"
        ).increment(1);

        // Decrement concurrent gauge
        gauge!("mcp_active_requests").decrement(1.0);

        Ok(())
    }

    async fn on_error(
        &self,
        error: &Error,
        context: &Context,
    ) -> Result<()> {
        let tool_name = context.get::<String>("tool_name")
            .unwrap_or_else(|_| "unknown".to_string());

        counter!(
            "mcp_requests_total",
            "tool" => tool_name,
            "status" => "error"
        ).increment(1);

        gauge!("mcp_active_requests").decrement(1.0);

        Ok(())
    }
}
```

## Step 4: Add Prometheus Endpoint

```rust
use axum::{Router, routing::get};

fn create_app(render_metrics: impl Fn() -> String + Clone + Send + 'static) -> Router {
    Router::new()
        .route("/metrics", get(move || {
            let render = render_metrics.clone();
            async move { render() }
        }))
        // ... other routes
}
```

## Step 5: Add to Server

```rust
fn main() {
    let render_metrics = init_metrics();

    let server = ServerBuilder::new("metrics-server", "1.0.0")
        .middleware(MetricsMiddleware)
        .with_tool(tools::MyTool)
        .build()?;

    // Run server with metrics endpoint
    // ...
}
```

## Step 6: Verify Metrics

```bash
# Start server
cargo run --release

# Check metrics endpoint
curl http://localhost:3000/metrics

# Expected output:
# # HELP mcp_requests_total Total number of MCP tool invocations
# # TYPE mcp_requests_total counter
# mcp_requests_total{tool="list_tables",status="success"} 5
# mcp_requests_total{tool="execute_query",status="success"} 12
# ...
```

::: hints
level_1: "Never use user input as label values - this creates unbounded cardinality."
level_2: "Set histogram buckets based on expected latency: [0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]."
level_3: "Always decrement gauges in both on_response AND on_error to avoid drift."
:::

## Essential MCP Metrics

```
# Rate metrics
- mcp_requests_total{tool, status} - Request count by tool and outcome
- mcp_errors_total{error_type} - Error breakdown for debugging

# Latency metrics
- mcp_request_duration_seconds{tool} - How long tools take
- mcp_db_query_duration_seconds - Database latency (if applicable)

# Saturation metrics
- mcp_active_requests - Current concurrent requests
- mcp_connection_pool_size - Database connection usage
```

## Success Criteria

- [ ] Tool invocation counter with tool/status labels
- [ ] Request duration histogram with appropriate buckets
- [ ] Active connections gauge properly incremented/decremented
- [ ] /metrics endpoint returns Prometheus format
- [ ] Metrics have descriptive names and documentation
- [ ] Can query specific tool's error rate

---

*This connects to dashboarding and alerting in [Operations](../../part7-observability/ch18-operations.md).*
