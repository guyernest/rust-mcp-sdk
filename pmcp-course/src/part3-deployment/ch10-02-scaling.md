# Auto-Scaling Configuration

Cloud Run automatically scales your MCP servers based on incoming traffic, but fine-tuning the scaling parameters is crucial for balancing cost, performance, and user experience. This lesson covers the scaling model, configuration options, and optimization strategies.

## Learning Objectives

By the end of this lesson, you will:
- Understand Cloud Run's scaling model and triggers
- Configure min/max instances for your workload
- Optimize concurrency settings for MCP servers
- Implement cold start mitigation strategies
- Design for cost-efficient scaling

## Understanding Cloud Run Scaling

### The Scaling Model

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Cloud Run Scaling Model                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Requests/sec    Active Instances    Scaling Behavior              │
│  ────────────   ────────────────    ────────────────               │
│       0         minInstances        Idle (scale to min)            │
│       1-10      1-2                 Gradual scale up               │
│       50        3-5                 Moderate load                  │
│       200       10-15               Heavy load                     │
│       1000+     50+ (up to max)     Burst scaling                  │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │                                                             │   │
│  │  Instances                                                  │   │
│  │      │                                            ┌────┐   │   │
│  │   50 ┤                                         ┌──┘    │   │   │
│  │      │                                      ┌──┘       │   │   │
│  │   25 ┤                              ┌───────┘          │   │   │
│  │      │                    ┌─────────┘                  │   │   │
│  │    5 ┤          ┌─────────┘                            │   │   │
│  │      │ ─────────┘                                      │   │   │
│  │    1 ┼──────────────────────────────────────────────────   │   │
│  │      └────────────────────────────────────────────────▶    │   │
│  │           Traffic over time                                │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Scaling Triggers

Cloud Run scales based on these factors:

| Trigger | Description | Default |
|---------|-------------|---------|
| **Request concurrency** | Requests per instance | 80 |
| **CPU utilization** | Target CPU percentage | 60% |
| **Startup time** | Time to accept requests | - |
| **Queue depth** | Pending requests | - |

### Request Lifecycle

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Request Lifecycle                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Request Arrives                                                    │
│       │                                                             │
│       ▼                                                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Is there an instance with capacity?                         │   │
│  └──────────────────────┬──────────────────────────────────────┘   │
│            Yes ─────────┴─────────── No                            │
│             │                         │                             │
│             ▼                         ▼                             │
│      Route to instance         Is max instances reached?           │
│             │                   Yes ──┴── No                       │
│             │                    │        │                         │
│             │                    ▼        ▼                         │
│             │               Queue or   Start new instance          │
│             │               429 error   (cold start)               │
│             │                              │                        │
│             └──────────────┬───────────────┘                       │
│                            ▼                                        │
│                    Process request                                  │
│                            │                                        │
│                            ▼                                        │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ Instance idle for scale-down period?                        │   │
│  └──────────────────────┬──────────────────────────────────────┘   │
│            No ──────────┴────────── Yes                            │
│             │                         │                             │
│             ▼                         ▼                             │
│        Keep warm              Scale down (if > min)                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Configuring Scaling Parameters

### Min and Max Instances

```bash
# Basic scaling configuration
gcloud run deploy my-mcp-server \
  --min-instances 1 \       # Always keep 1 instance warm
  --max-instances 100       # Maximum scale limit

# Zero to N scaling (scale to zero when idle)
gcloud run deploy my-mcp-server \
  --min-instances 0 \       # Scale to zero
  --max-instances 50
```

### Choosing Min Instances

| Scenario | Recommended Min | Reason |
|----------|-----------------|--------|
| Development | 0 | Cost savings |
| Low-traffic production | 1 | Avoid cold starts |
| Business-critical | 2+ | High availability |
| Predictable traffic | Based on baseline | Match minimum load |

```yaml
# service.yaml
spec:
  template:
    metadata:
      annotations:
        # Min instances annotation
        autoscaling.knative.dev/minScale: "2"
        # Max instances annotation
        autoscaling.knative.dev/maxScale: "100"
```

### Concurrency Settings

Concurrency determines how many requests a single instance handles simultaneously:

```bash
# Set concurrency
gcloud run deploy my-mcp-server \
  --concurrency 80  # Default

# Single-threaded workloads
gcloud run deploy my-mcp-server \
  --concurrency 1

# High-concurrency async workloads
gcloud run deploy my-mcp-server \
  --concurrency 250
```

### Choosing Concurrency for MCP Servers

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Concurrency Selection Guide                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  MCP Server Type              Recommended Concurrency              │
│  ─────────────────           ────────────────────────              │
│  CPU-intensive tools          10-20                                │
│  Database query tools         50-80                                │
│  Simple HTTP proxy            100-250                              │
│  Stateless transforms         100-200                              │
│                                                                     │
│  Formula: concurrency = (CPU cores × target_utilization) /         │
│           average_request_duration_seconds                         │
│                                                                     │
│  Example: 2 cores × 0.7 / 0.1s = 14 concurrent requests           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

```rust
// Measuring actual concurrency capacity
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

static ACTIVE_REQUESTS: AtomicUsize = AtomicUsize::new(0);

async fn handle_mcp_request(request: McpRequest) -> McpResponse {
    let current = ACTIVE_REQUESTS.fetch_add(1, Ordering::SeqCst);
    tracing::info!(active_requests = current + 1, "Request started");

    let result = process_request(request).await;

    let current = ACTIVE_REQUESTS.fetch_sub(1, Ordering::SeqCst);
    tracing::info!(active_requests = current - 1, "Request completed");

    result
}
```

## CPU Allocation Modes

### Always-On CPU

By default, Cloud Run throttles CPU between requests. Disable this for consistent performance:

```bash
# Always allocate CPU (no throttling)
gcloud run deploy my-mcp-server \
  --no-cpu-throttling

# Default behavior (CPU throttled between requests)
gcloud run deploy my-mcp-server \
  --cpu-throttling
```

```yaml
# service.yaml
spec:
  template:
    metadata:
      annotations:
        run.googleapis.com/cpu-throttling: "false"
```

### When to Use Always-On CPU

| Use Case | CPU Throttling | Reason |
|----------|----------------|--------|
| Standard HTTP APIs | Yes (default) | Cost savings |
| WebSocket connections | No | Maintains connections |
| Background processing | No | Consistent performance |
| MCP with long operations | No | Predictable latency |

## Cold Start Optimization

### Understanding Cold Starts

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Cold Start Timeline                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Python/Node.js MCP Server:                                        │
│  ├── Container start ────────── 2-5s                               │
│  ├── Runtime initialization ─── 1-3s                               │
│  ├── Dependency loading ─────── 2-10s                              │
│  ├── Application startup ────── 1-5s                               │
│  └── Total ──────────────────── 6-23s                              │
│                                                                     │
│  Rust MCP Server:                                                  │
│  ├── Container start ────────── 0.5-2s                             │
│  ├── Binary loading ─────────── 0.1-0.5s                           │
│  ├── Application startup ────── 0.1-1s                             │
│  └── Total ──────────────────── 0.7-3.5s                           │
│                                                                     │
│  Rust advantage: 3-10x faster cold starts                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Optimizing Startup Time

```rust
// Lazy initialization for faster startup
use once_cell::sync::Lazy;
use tokio::sync::OnceCell;

// AVOID: Blocking initialization at startup
fn main() {
    let pool = PgPool::connect_blocking(&database_url); // Blocks startup
    run_server(pool);
}

// BETTER: Lazy initialization
static DB_POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn get_pool() -> &'static PgPool {
    DB_POOL.get_or_init(|| async {
        PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .expect("Failed to connect to database")
    }).await
}

#[tokio::main]
async fn main() {
    // Start accepting requests immediately
    let app = Router::new()
        .route("/health", get(|| async { "OK" }))
        .route("/mcp", post(handle_mcp));

    // Server starts fast, DB connection happens on first request
    serve(app).await;
}
```

### CPU Boost for Cold Starts

Cloud Run can temporarily allocate extra CPU during startup:

```bash
gcloud run deploy my-mcp-server \
  --cpu-boost  # Temporarily allocate more CPU during startup
```

```yaml
# service.yaml
spec:
  template:
    metadata:
      annotations:
        run.googleapis.com/startup-cpu-boost: "true"
```

### Startup Probes

Configure startup probes to give your application time to initialize:

```yaml
# service.yaml
spec:
  template:
    spec:
      containers:
        - image: my-image
          startupProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 0
            periodSeconds: 2
            timeoutSeconds: 3
            failureThreshold: 30  # Allow 60 seconds for startup
```

```rust
// Health check that reflects actual readiness
use std::sync::atomic::{AtomicBool, Ordering};

static READY: AtomicBool = AtomicBool::new(false);

async fn health_check() -> impl IntoResponse {
    if READY.load(Ordering::SeqCst) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn initialize_app() {
    // Perform initialization
    let _ = get_pool().await;  // Initialize DB connection
    // Mark as ready
    READY.store(true, Ordering::SeqCst);
}
```

## Scaling Strategies for MCP Servers

### Low-Latency Strategy

For MCP servers where response time is critical:

```yaml
# service.yaml - Low latency configuration
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/minScale: "3"    # Always warm
        autoscaling.knative.dev/maxScale: "100"
        run.googleapis.com/cpu-throttling: "false"
        run.googleapis.com/startup-cpu-boost: "true"
    spec:
      containerConcurrency: 50  # Conservative concurrency
      timeoutSeconds: 30
      containers:
        - resources:
            limits:
              cpu: "2"
              memory: 2Gi
```

### Cost-Optimized Strategy

For development or low-priority workloads:

```yaml
# service.yaml - Cost optimized configuration
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/minScale: "0"    # Scale to zero
        autoscaling.knative.dev/maxScale: "10"
        run.googleapis.com/cpu-throttling: "true"  # Throttle CPU
    spec:
      containerConcurrency: 100  # High concurrency
      timeoutSeconds: 300
      containers:
        - resources:
            limits:
              cpu: "1"
              memory: 512Mi
```

### Burst Traffic Strategy

For workloads with occasional traffic spikes:

```yaml
# service.yaml - Burst traffic configuration
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/minScale: "1"    # Minimum warm
        autoscaling.knative.dev/maxScale: "500"   # High burst capacity
        run.googleapis.com/startup-cpu-boost: "true"
    spec:
      containerConcurrency: 80
      timeoutSeconds: 60
      containers:
        - resources:
            limits:
              cpu: "2"
              memory: 1Gi
```

## Request Queuing and Overflow

### Understanding Request Queuing

When all instances are at maximum concurrency, Cloud Run queues requests:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Request Queuing Behavior                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Scenario: max_instances=3, concurrency=2, 10 concurrent requests  │
│                                                                     │
│  Instance 1: [req1] [req2]  ← at capacity                          │
│  Instance 2: [req3] [req4]  ← at capacity                          │
│  Instance 3: [req5] [req6]  ← at capacity                          │
│                                                                     │
│  Queue: [req7, req8, req9, req10]  ← waiting for capacity          │
│                                                                     │
│  If queue wait exceeds timeout → 429 Too Many Requests             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Handling 429 Errors

Implement retry logic in your MCP client:

```rust
// Client-side retry with backoff
use backoff::{ExponentialBackoff, future::retry};

async fn call_mcp_with_retry(request: McpRequest) -> Result<McpResponse> {
    let backoff = ExponentialBackoff {
        max_elapsed_time: Some(Duration::from_secs(30)),
        ..Default::default()
    };

    retry(backoff, || async {
        match call_mcp(&request).await {
            Ok(response) => Ok(response),
            Err(e) if e.is_rate_limited() => {
                tracing::warn!("Rate limited, retrying...");
                Err(backoff::Error::transient(e))
            }
            Err(e) => Err(backoff::Error::permanent(e)),
        }
    }).await
}
```

## Monitoring and Tuning

### Key Metrics to Monitor

```bash
# View scaling metrics
gcloud monitoring dashboards create --config-from-file=scaling-dashboard.yaml
```

```yaml
# scaling-dashboard.yaml
displayName: "MCP Server Scaling"
mosaicLayout:
  tiles:
    - widget:
        title: "Active Instances"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: >
                    resource.type="cloud_run_revision"
                    AND metric.type="run.googleapis.com/container/instance_count"
    - widget:
        title: "Request Latency (p99)"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: >
                    resource.type="cloud_run_revision"
                    AND metric.type="run.googleapis.com/request_latencies"
    - widget:
        title: "Container CPU Utilization"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: >
                    resource.type="cloud_run_revision"
                    AND metric.type="run.googleapis.com/container/cpu/utilizations"
    - widget:
        title: "Concurrent Requests"
        xyChart:
          dataSets:
            - timeSeriesQuery:
                timeSeriesFilter:
                  filter: >
                    resource.type="cloud_run_revision"
                    AND metric.type="run.googleapis.com/container/max_request_concurrencies"
```

### Tuning Based on Metrics

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Scaling Tuning Guide                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Symptom                        Action                             │
│  ────────────────────────────   ──────────────────────────────     │
│  High latency spikes            Increase min instances             │
│  CPU utilization > 80%          Decrease concurrency               │
│  Memory pressure                Increase memory limit              │
│  Frequent cold starts           Increase min instances             │
│  429 errors during peaks        Increase max instances             │
│  High costs during idle         Decrease min instances             │
│  Inconsistent response times    Disable CPU throttling             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Load Testing

```bash
# Install hey for load testing
brew install hey

# Test with increasing concurrency
hey -n 1000 -c 10 https://my-mcp-server.run.app/mcp
hey -n 1000 -c 50 https://my-mcp-server.run.app/mcp
hey -n 1000 -c 100 https://my-mcp-server.run.app/mcp

# Test with sustained load
hey -z 5m -c 50 https://my-mcp-server.run.app/mcp
```

## Multi-Region Scaling

### Global Load Balancing

For global MCP deployments:

```bash
# Deploy to multiple regions
gcloud run deploy my-mcp-server --region us-central1
gcloud run deploy my-mcp-server --region europe-west1
gcloud run deploy my-mcp-server --region asia-northeast1

# Create global load balancer
gcloud compute backend-services create my-mcp-backend \
  --global \
  --load-balancing-scheme=EXTERNAL_MANAGED

# Add region NEGs
gcloud compute network-endpoint-groups create my-mcp-neg-us \
  --region=us-central1 \
  --network-endpoint-type=SERVERLESS \
  --cloud-run-service=my-mcp-server
```

### Region-Specific Scaling

```yaml
# Different scaling per region
# us-central1 (high traffic)
autoscaling.knative.dev/minScale: "5"
autoscaling.knative.dev/maxScale: "200"

# europe-west1 (medium traffic)
autoscaling.knative.dev/minScale: "2"
autoscaling.knative.dev/maxScale: "50"

# asia-northeast1 (low traffic)
autoscaling.knative.dev/minScale: "1"
autoscaling.knative.dev/maxScale: "20"
```

## Summary

Effective auto-scaling for MCP servers requires:

1. **Understanding your workload** - CPU-bound vs I/O-bound, latency requirements
2. **Right-sizing min/max instances** - Balance cost vs cold start impact
3. **Tuning concurrency** - Match your application's capacity
4. **CPU allocation strategy** - Throttling vs always-on based on use case
5. **Cold start optimization** - Fast startup code, CPU boost, startup probes
6. **Continuous monitoring** - Track metrics and adjust settings

Key configuration summary:

| Setting | Low Latency | Cost Optimized | Balanced |
|---------|-------------|----------------|----------|
| Min instances | 3+ | 0 | 1 |
| Max instances | 100+ | 10 | 50 |
| Concurrency | 50 | 100 | 80 |
| CPU throttling | No | Yes | No |
| CPU boost | Yes | No | Yes |

## Exercises

### Exercise 1: Load Test Analysis
Run load tests against your MCP server and identify the optimal concurrency setting.

### Exercise 2: Cold Start Measurement
Measure cold start times with different configurations (CPU boost, min instances) and document the results.

### Exercise 3: Cost Optimization
Calculate the monthly cost difference between min=0 and min=1 configurations for your workload.
