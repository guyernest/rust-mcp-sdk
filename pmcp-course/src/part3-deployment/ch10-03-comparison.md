# Comparison: Cloud Run vs Lambda vs Workers

Choosing the right deployment platform for your MCP server is one of the most impactful architectural decisions you'll make. This lesson provides a comprehensive comparison of AWS Lambda, Cloudflare Workers, and Google Cloud Run to help you make an informed choice.

## Learning Objectives

By the end of this lesson, you will:
- Understand the architectural differences between platforms
- Compare costs across different usage patterns
- Match platform capabilities to MCP server requirements
- Choose the right platform for your specific use case

## Platform Architecture Comparison

### Fundamental Differences

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Platform Architecture Comparison                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  AWS Lambda                                                         │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  ZIP Package → Lambda Runtime → Firecracker microVM         │   │
│  │  Event-driven, 15min timeout, 10GB memory                   │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Cloudflare Workers                                                 │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  WASM Binary → V8 Isolate → Edge Network (300+ locations)   │   │
│  │  Request-driven, 30s CPU time, 128MB memory                 │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Google Cloud Run                                                   │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Docker Image → gVisor Sandbox → Managed Kubernetes         │   │
│  │  Request-driven, 60min timeout, 32GB memory                 │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Capability Matrix

| Capability | Lambda | Workers | Cloud Run |
|------------|--------|---------|-----------|
| **Max timeout** | 15 min | 30s (CPU) | 60 min |
| **Max memory** | 10 GB | 128 MB | 32 GB |
| **Max request size** | 6 MB | 100 MB | 32 MB |
| **Max response size** | 6 MB | 100 MB | 32 MB |
| **Filesystem** | /tmp (10 GB) | None | In-memory |
| **Concurrency** | 1 per instance | 1 per isolate | Configurable |
| **Cold start** | 100-500ms (Rust) | <5ms | 500ms-3s |
| **GPU support** | No | No | Yes |
| **WebSockets** | Via API Gateway | Yes (beta) | Yes |
| **Deployment** | ZIP, Container | WASM | Container |

## Cold Start Comparison

### Measured Cold Start Times

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Cold Start Times (Rust MCP Server)               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Platform           p50        p95        p99                      │
│  ─────────────────  ─────────  ─────────  ─────────                │
│  Workers            2ms        5ms        10ms                     │
│  Lambda (SnapStart) 50ms       150ms      300ms                    │
│  Lambda (standard)  100ms      300ms      500ms                    │
│  Cloud Run          400ms      1.2s       2.5s                     │
│                                                                     │
│  Cold Start Breakdown:                                             │
│                                                                     │
│  Workers:                                                          │
│  ├── WASM instantiation ─── 1-3ms                                  │
│  └── Total ─────────────── ~5ms                                    │
│                                                                     │
│  Lambda (Rust):                                                    │
│  ├── Environment setup ──── 50-100ms                               │
│  ├── Binary loading ──────── 10-30ms                               │
│  ├── Runtime init ────────── 10-50ms                               │
│  └── Total ─────────────── 70-180ms                                │
│                                                                     │
│  Cloud Run:                                                        │
│  ├── Container pull ──────── 200-500ms (cached)                    │
│  ├── Container start ─────── 100-300ms                             │
│  ├── Application init ────── 50-200ms                              │
│  └── Total ─────────────── 350-1000ms                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Cold Start Mitigation

| Platform | Mitigation Strategy | Cost Impact |
|----------|---------------------|-------------|
| Lambda | Provisioned concurrency | $$$ |
| Lambda | SnapStart (Java) | Free |
| Workers | Always fast (by design) | Free |
| Cloud Run | Min instances | $$ |
| Cloud Run | CPU boost | $ |

## Cost Comparison

### Pricing Models

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Pricing Model Comparison                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  AWS Lambda                                                         │
│  ├── Requests: $0.20 per 1M requests                               │
│  ├── Duration: $0.0000166667 per GB-second                         │
│  └── Free tier: 1M requests, 400,000 GB-seconds/month              │
│                                                                     │
│  Cloudflare Workers                                                 │
│  ├── Requests: $0.30 per 1M requests (after 10M free)              │
│  ├── Duration: $12.50 per 1M GB-seconds                            │
│  └── Free tier: 100,000 requests/day, 10ms CPU/request             │
│                                                                     │
│  Google Cloud Run                                                   │
│  ├── CPU: $0.00002400 per vCPU-second                              │
│  ├── Memory: $0.00000250 per GiB-second                            │
│  ├── Requests: $0.40 per 1M requests                               │
│  └── Free tier: 2M requests, 180,000 vCPU-seconds/month            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Cost Scenarios

#### Scenario 1: Low Volume (10,000 requests/month)

```
┌─────────────────────────────────────────────────────────────────────┐
│  Assumptions: 10,000 requests/month, 200ms avg duration            │
│               512MB memory (Lambda/Cloud Run)                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Lambda:                                                           │
│  ├── Requests: 10K × $0.0000002 = $0.002                          │
│  ├── Duration: 10K × 0.2s × 0.5GB × $0.0000166667 = $0.017        │
│  └── Total: $0.02 (within free tier)                              │
│                                                                     │
│  Workers:                                                          │
│  ├── Requests: Within free tier                                   │
│  └── Total: $0.00                                                 │
│                                                                     │
│  Cloud Run (min=0):                                                │
│  ├── Requests: 10K × $0.0000004 = $0.004                          │
│  ├── CPU: 10K × 0.2s × 1vCPU × $0.000024 = $0.048                 │
│  ├── Memory: 10K × 0.2s × 0.5GB × $0.0000025 = $0.0025            │
│  └── Total: $0.05 (within free tier)                              │
│                                                                     │
│  Winner: Workers (always free at this volume)                      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### Scenario 2: Medium Volume (1M requests/month)

```
┌─────────────────────────────────────────────────────────────────────┐
│  Assumptions: 1M requests/month, 200ms avg duration                │
│               512MB memory, consistent traffic                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Lambda:                                                           │
│  ├── Requests: 1M × $0.0000002 = $0.20                            │
│  ├── Duration: 1M × 0.2s × 0.5GB × $0.0000166667 = $1.67          │
│  └── Total: ~$1.87/month                                          │
│                                                                     │
│  Workers:                                                          │
│  ├── Requests: (1M - 300K free) × $0.0000003 = $0.21              │
│  └── Total: ~$0.21/month                                          │
│                                                                     │
│  Cloud Run (min=0):                                                │
│  ├── Requests: (1M - 2M free) = $0 (within free tier)             │
│  ├── CPU: 1M × 0.2s × 1vCPU × $0.000024 = $4.80                   │
│  ├── Memory: 1M × 0.2s × 0.5GB × $0.0000025 = $0.25               │
│  └── Total: ~$5.05/month                                          │
│                                                                     │
│  Cloud Run (min=1):                                                │
│  ├── Base: 720h × 1vCPU × $0.0864/h = $62.21                      │
│  └── Total: ~$62/month (always-on instance)                       │
│                                                                     │
│  Winner: Workers ($0.21) < Lambda ($1.87) < Cloud Run ($5-62)      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### Scenario 3: High Volume (100M requests/month)

```
┌─────────────────────────────────────────────────────────────────────┐
│  Assumptions: 100M requests/month, 200ms avg duration              │
│               1GB memory, peak traffic patterns                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Lambda:                                                           │
│  ├── Requests: 100M × $0.0000002 = $20                            │
│  ├── Duration: 100M × 0.2s × 1GB × $0.0000166667 = $333           │
│  └── Total: ~$353/month                                           │
│                                                                     │
│  Workers:                                                          │
│  ├── Requests: (100M - 10M) × $0.0000003 = $27                    │
│  ├── Duration: 100M × 0.01s × $0.0000125 = $12.50                 │
│  └── Total: ~$40/month                                            │
│                                                                     │
│  Cloud Run (min=5, max=50):                                        │
│  ├── Base min instances: 720h × 5 × $0.12/h = $432                │
│  ├── Burst capacity: variable                                     │
│  └── Total: ~$500-800/month                                       │
│                                                                     │
│  Winner: Workers ($40) < Lambda ($353) < Cloud Run ($500+)         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Cost Summary

| Volume | Best Choice | Monthly Cost |
|--------|-------------|--------------|
| <100K | Workers (free) | $0 |
| 100K-1M | Workers | $0-1 |
| 1M-10M | Workers | $1-30 |
| 10M-100M | Workers or Lambda | $30-400 |
| 100M+ | Workers | $40+ |

**Note**: Cloud Run becomes competitive when you need features it uniquely provides (long timeouts, large memory, GPUs).

## Use Case Decision Matrix

### Decision Flowchart

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Platform Selection Flowchart                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Start                                                              │
│    │                                                                │
│    ▼                                                                │
│  Need GPU acceleration?                                             │
│    │                                                                │
│   Yes ──────────────────────────────────────▶ Cloud Run             │
│    │                                                                │
│   No                                                                │
│    │                                                                │
│    ▼                                                                │
│  Need >15 minute timeout?                                           │
│    │                                                                │
│   Yes ──────────────────────────────────────▶ Cloud Run             │
│    │                                                                │
│   No                                                                │
│    │                                                                │
│    ▼                                                                │
│  Need >128MB memory?                                                │
│    │                                                                │
│   Yes                                                               │
│    │                                                                │
│    ▼                                                                │
│  Need >10GB memory?                                                 │
│    │                                                                │
│   Yes ──────────────────────────────────────▶ Cloud Run             │
│    │                                                                │
│   No ───────────────────────────────────────▶ Lambda                │
│    │                                                                │
│   No (≤128MB)                                                       │
│    │                                                                │
│    ▼                                                                │
│  Need global edge deployment?                                       │
│    │                                                                │
│   Yes                                                               │
│    │                                                                │
│    ▼                                                                │
│  Operations take <30s CPU time?                                     │
│    │                                                                │
│   Yes ──────────────────────────────────────▶ Workers               │
│    │                                                                │
│   No ───────────────────────────────────────▶ Lambda + CloudFront   │
│    │                                                                │
│   No (regional is fine)                                             │
│    │                                                                │
│    ▼                                                                │
│  In AWS ecosystem?                                                  │
│    │                                                                │
│   Yes ──────────────────────────────────────▶ Lambda                │
│    │                                                                │
│   No ───────────────────────────────────────▶ Workers (default)     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Platform-Specific Strengths

#### Choose Lambda When:

- **AWS ecosystem integration**: RDS, DynamoDB, S3, Cognito
- **Event-driven patterns**: SQS, SNS, EventBridge triggers
- **Moderate memory needs**: 128MB to 10GB
- **Existing AWS infrastructure**: VPC, IAM, CloudWatch
- **Step Functions orchestration**: Complex workflows

```rust
// Lambda excels at AWS integrations
use aws_sdk_dynamodb::Client;
use lambda_runtime::{service_fn, LambdaEvent};

async fn handler(event: LambdaEvent<McpRequest>) -> Result<McpResponse, Error> {
    let config = aws_config::load_from_env().await;
    let client = Client::new(&config);

    // Native DynamoDB integration
    let result = client
        .get_item()
        .table_name("mcp-data")
        .key("id", AttributeValue::S(event.payload.id))
        .send()
        .await?;

    Ok(process_result(result))
}
```

#### Choose Workers When:

- **Global edge deployment**: Sub-50ms latency worldwide
- **Low memory requirements**: ≤128MB is sufficient
- **Simple compute**: Transformations, routing, caching
- **Cost sensitivity**: Best pricing at most volumes
- **Fast cold starts**: User-facing APIs

```rust
// Workers excels at edge compute
use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Request processed at edge location closest to user
    let cache = env.kv("CACHE")?;

    // Check edge cache first
    if let Some(cached) = cache.get("result").text().await? {
        return Response::ok(cached);
    }

    // Process and cache at edge
    let result = process_request(&req).await?;
    cache.put("result", &result)?.execute().await?;

    Response::ok(result)
}
```

#### Choose Cloud Run When:

- **Long operations**: Processing takes >15 minutes
- **Large memory**: Need 10GB+ for ML models, large datasets
- **GPU workloads**: ML inference, image processing
- **Complex containers**: Multiple processes, specific OS needs
- **Portability**: Same container runs anywhere

```rust
// Cloud Run excels at long/heavy operations
use axum::{routing::post, Router};
use tokio::time::Duration;

async fn ml_inference(input: Json<InferenceRequest>) -> Json<InferenceResponse> {
    // Load large model into memory (needs >10GB)
    let model = load_model("s3://models/large-llm.bin").await;

    // Long-running inference (can take 5+ minutes)
    let result = model.infer(&input.prompt).await;

    Json(InferenceResponse { result })
}
```

## Migration Considerations

### Lambda to Cloud Run

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Lambda → Cloud Run Migration                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  What Changes:                                                      │
│  ├── ZIP → Docker image                                            │
│  ├── Handler function → HTTP server                                │
│  ├── AWS SDK → GCP SDK (or keep AWS with credentials)              │
│  ├── CloudWatch → Cloud Logging/Monitoring                         │
│  └── IAM roles → Service accounts                                  │
│                                                                     │
│  What Stays:                                                        │
│  ├── Rust code (mostly)                                            │
│  ├── Business logic                                                │
│  ├── MCP protocol handling                                         │
│  └── External API integrations                                     │
│                                                                     │
│  Effort: Medium (1-2 weeks for typical MCP server)                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Lambda to Workers

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Lambda → Workers Migration                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  What Changes:                                                      │
│  ├── ZIP → WASM binary                                             │
│  ├── tokio → wasm-bindgen-futures                                  │
│  ├── AWS SDK → Workers bindings (KV, D1, R2)                       │
│  ├── std::fs → Workers storage APIs                                │
│  └── Some crates may not compile to WASM                           │
│                                                                     │
│  What Stays:                                                        │
│  ├── Pure Rust logic                                               │
│  ├── serde serialization                                           │
│  ├── MCP protocol handling                                         │
│  └── HTTP request/response patterns                                │
│                                                                     │
│  Effort: High (2-4 weeks, WASM compatibility work)                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Workers to Lambda

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Workers → Lambda Migration                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  What Changes:                                                      │
│  ├── WASM → Native binary (easier)                                 │
│  ├── Workers bindings → AWS SDK                                    │
│  ├── KV/D1 → DynamoDB/RDS                                         │
│  ├── R2 → S3                                                       │
│  └── Edge deployment → Regional deployment                         │
│                                                                     │
│  What Stays:                                                        │
│  ├── All Rust code (WASM subset compiles to native)                │
│  ├── Business logic                                                │
│  ├── MCP protocol handling                                         │
│  └── HTTP patterns                                                 │
│                                                                     │
│  Effort: Low-Medium (1-2 weeks, mostly SDK swaps)                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Multi-Platform Architecture

### Hybrid Deployment Pattern

For complex MCP servers, consider a hybrid approach:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Hybrid MCP Architecture                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                         ┌─────────────────┐                        │
│    Client Request ────▶│ Workers (Edge)  │                        │
│                         │ - Auth check    │                        │
│                         │ - Rate limiting │                        │
│                         │ - Caching       │                        │
│                         └────────┬────────┘                        │
│                                  │                                  │
│          ┌───────────────────────┼───────────────────────┐         │
│          │                       │                       │         │
│          ▼                       ▼                       ▼         │
│  ┌───────────────┐    ┌───────────────┐    ┌───────────────┐      │
│  │    Lambda     │    │    Lambda     │    │  Cloud Run    │      │
│  │ - Quick tools │    │ - DB queries  │    │ - ML inference│      │
│  │ - <100ms      │    │ - AWS integr. │    │ - Long ops    │      │
│  └───────────────┘    └───────────────┘    └───────────────┘      │
│                                                                     │
│  Benefits:                                                         │
│  ├── Edge caching reduces backend calls                            │
│  ├── Route to best platform per operation type                     │
│  ├── Scale each tier independently                                 │
│  └── Graceful fallback between platforms                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Implementation

```rust
// Workers edge router
#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let mcp_request: McpRequest = req.json().await?;

    // Route based on tool type
    let backend_url = match mcp_request.tool_name.as_str() {
        // Quick operations → Lambda
        "search" | "lookup" | "validate" => {
            env.var("LAMBDA_URL")?.to_string()
        }
        // Database operations → Lambda (AWS integration)
        "query" | "insert" | "update" => {
            env.var("LAMBDA_DB_URL")?.to_string()
        }
        // Heavy operations → Cloud Run
        "analyze" | "generate" | "process" => {
            env.var("CLOUD_RUN_URL")?.to_string()
        }
        // Default to Lambda
        _ => env.var("LAMBDA_URL")?.to_string()
    };

    // Forward to appropriate backend
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;

    Fetch::Request(Request::new_with_init(
        &backend_url,
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(serde_json::to_string(&mcp_request)?.into())),
    )?)
    .send()
    .await
}
```

## Summary

### Quick Reference

| Factor | Lambda | Workers | Cloud Run |
|--------|--------|---------|-----------|
| **Best for** | AWS integration | Global edge | Heavy workloads |
| **Cold start** | 100-500ms | <5ms | 500ms-3s |
| **Max memory** | 10 GB | 128 MB | 32 GB |
| **Max timeout** | 15 min | 30s CPU | 60 min |
| **Pricing model** | Per request + duration | Per request | Per resource |
| **Cost at scale** | Medium | Lowest | Highest |
| **Deployment** | ZIP or Container | WASM | Container |
| **Ecosystem** | AWS | Cloudflare | GCP |

### Recommendations by Use Case

| MCP Server Type | Recommended Platform |
|-----------------|---------------------|
| Database explorer | Lambda (AWS) or Cloud Run (GCP) |
| File system tools | Cloud Run |
| API integration | Workers or Lambda |
| ML inference | Cloud Run |
| Real-time data | Workers |
| Multi-step workflows | Lambda + Step Functions |
| Global availability | Workers |
| Cost-sensitive | Workers |

### Final Advice

1. **Start with Workers** if your requirements fit within its constraints (128MB memory, 30s CPU time)
2. **Use Lambda** for AWS ecosystem integration or when you need more memory/time
3. **Choose Cloud Run** when you need maximum flexibility, GPUs, or very long operations
4. **Consider hybrid** for complex MCP servers with varied operation types

The best platform is the one that matches your specific requirements while minimizing complexity and cost.

## Exercises

### Exercise 1: Platform Comparison
Deploy the same MCP server to all three platforms and measure cold start times, response latency, and costs.

### Exercise 2: Cost Analysis
Calculate the monthly cost for your expected traffic pattern on each platform and identify the break-even points.

### Exercise 3: Migration Plan
Create a migration plan for moving an existing MCP server from one platform to another, identifying all required changes.
