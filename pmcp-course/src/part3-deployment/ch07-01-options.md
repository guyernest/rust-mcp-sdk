# Serverless vs Containers vs Edge

When deploying MCP servers to the cloud, you have three fundamental architectural choices: **serverless functions**, **containers**, and **edge computing**. Each approach has distinct characteristics that affect performance, cost, and operational complexity.

This lesson provides a deep technical comparison to help you make informed deployment decisions.

## The Three Paradigms

### Serverless Functions (AWS Lambda)

Serverless functions execute your code in response to events, with the cloud provider managing all infrastructure.

```
┌─────────────────────────────────────────────────────────────────┐
│                    SERVERLESS ARCHITECTURE                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Request ──▶ API Gateway ──▶ Lambda Function ──▶ Response     │
│                                    │                            │
│                                    ▼                            │
│                              ┌──────────┐                       │
│                              │ Your Code│                       │
│                              │ (frozen) │                       │
│                              └──────────┘                       │
│                                                                 │
│   Between requests: Function is frozen or terminated            │
│   Scaling: Cloud spawns new instances automatically             │
│   Billing: Pay only for execution time (GB-seconds)             │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**How it works:**
1. Your code is packaged as a deployment artifact (ZIP or container)
2. When a request arrives, AWS loads your code into a "microVM"
3. Your handler function executes and returns a response
4. The runtime may be reused for subsequent requests (warm start) or terminated (cold start)

**Rust-specific behavior:**

```rust
// Lambda handler - runs for each request
async fn handler(event: LambdaEvent<Request>) -> Result<Response, Error> {
    // This code runs per-request
    let response = process_mcp_request(event.payload).await?;
    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // This runs ONCE during cold start
    // Initialize expensive resources here
    tracing_subscriber::fmt::init();

    run(service_fn(handler)).await
}
```

### Containers (Google Cloud Run)

Containers package your application with its dependencies into a portable image that runs on managed infrastructure.

```
┌─────────────────────────────────────────────────────────────────┐
│                    CONTAINER ARCHITECTURE                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Request ──▶ Load Balancer ──▶ Container Instance ──▶ Response│
│                                       │                         │
│                                       ▼                         │
│                              ┌────────────────┐                 │
│                              │  Your Server   │                 │
│                              │  (always on)   │                 │
│                              │                │                 │
│                              │  HTTP :8080    │                 │
│                              └────────────────┘                 │
│                                                                 │
│   Between requests: Server stays running, handles concurrency   │
│   Scaling: Platform adjusts container count based on load       │
│   Billing: Pay for container uptime (vCPU-seconds + memory)     │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**How it works:**
1. Your application is packaged as a Docker image
2. The platform runs your container and routes HTTP traffic to it
3. Your server handles multiple concurrent requests
4. The platform scales containers up/down based on traffic

**Rust container example:**

```dockerfile
# Multi-stage build for minimal image
FROM rust:1.75 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/mcp-server /
EXPOSE 8080
CMD ["/mcp-server"]
```

```rust
// Container server - runs continuously
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize once at startup
    let server = build_mcp_server().await?;

    // Run HTTP server - handles many requests
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    StreamableHttpServer::new(addr, server)
        .run()
        .await
}
```

### Edge Computing (Cloudflare Workers)

Edge functions run your code at network edge locations, close to users worldwide.

```
┌─────────────────────────────────────────────────────────────────┐
│                      EDGE ARCHITECTURE                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│        ┌──────────┐  ┌──────────┐  ┌──────────┐                │
│        │ Tokyo    │  │ London   │  │ NYC      │                │
│        │ Edge     │  │ Edge     │  │ Edge     │                │
│        └────┬─────┘  └────┬─────┘  └────┬─────┘                │
│             │             │             │                       │
│      User ──┘      User ──┘      User ──┘                       │
│      (5ms)         (5ms)         (5ms)                          │
│                                                                 │
│   Your code: Compiled to WebAssembly, distributed globally      │
│   Execution: Runs in V8 isolates (not containers)               │
│   Scaling: Automatic across 300+ locations                      │
│   Billing: Pay per request + CPU time                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**How it works:**
1. Your Rust code is compiled to WebAssembly (WASM)
2. The WASM module is deployed to edge locations worldwide
3. Each request runs in an isolated V8 environment
4. No cold start in the traditional sense - isolates spin up in microseconds

**Rust WASM example:**

```rust
use worker::*;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Each request runs in its own isolate
    let router = Router::new();

    router
        .post_async("/mcp", |req, _| async move {
            let body = req.text().await?;
            let response = handle_mcp_request(&body).await?;
            Response::ok(response)
        })
        .run(req, env)
        .await
}
```

## Execution Model Comparison

### Cold Start Behavior

Cold starts occur when the platform must initialize a new execution environment:

| Platform | Cold Start Cause | Typical Duration (Rust) |
|----------|------------------|-------------------------|
| Lambda | No warm instance available | 50-150ms |
| Cloud Run | Container scaling up | 100-500ms |
| Workers | First request to edge location | 0-5ms |

**Lambda cold start breakdown:**

```
┌─────────────────────────────────────────────────────────────────┐
│                   LAMBDA COLD START TIMELINE                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  0ms          50ms         100ms        150ms       200ms       │
│   │            │            │            │           │          │
│   ├────────────┼────────────┼────────────┼───────────┤          │
│   │  MicroVM   │  Runtime   │   Your     │  Handler  │          │
│   │  Init      │  Init      │   main()   │  Exec     │          │
│   │  (~30ms)   │  (~10ms)   │  (~10ms)   │  (~50ms)  │          │
│   │            │            │            │           │          │
│   └────────────────────────────────────────────────────────────│
│                                                                 │
│   Rust advantage: main() initialization is minimal              │
│   Python/Node: Interpreter startup adds 200-500ms               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Strategies to minimize cold starts:**

```rust
// Lambda: Initialize expensive resources once
static DB_POOL: OnceCell<Pool<Postgres>> = OnceCell::new();

async fn get_pool() -> &'static Pool<Postgres> {
    DB_POOL.get_or_init(|| async {
        PgPoolOptions::new()
            .max_connections(5)
            .connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap()
    }).await
}

async fn handler(event: Request) -> Result<Response> {
    // Pool is reused across warm invocations
    let pool = get_pool().await;
    // ...
}
```

### Concurrency Model

Each platform handles concurrent requests differently:

```
┌─────────────────────────────────────────────────────────────────┐
│                    CONCURRENCY MODELS                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  LAMBDA (1 request per instance):                               │
│                                                                 │
│    Request 1 ──▶ [Instance A] ──▶ Response 1                   │
│    Request 2 ──▶ [Instance B] ──▶ Response 2                   │
│    Request 3 ──▶ [Instance C] ──▶ Response 3                   │
│                                                                 │
│    Scaling: New instance for each concurrent request            │
│    Memory: Separate per instance (128MB-10GB configurable)      │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  CLOUD RUN (many requests per container):                       │
│                                                                 │
│    Request 1 ──┐                                                │
│    Request 2 ──┼──▶ [Container A] ──┬──▶ Response 1            │
│    Request 3 ──┘        │           ├──▶ Response 2            │
│                         │           └──▶ Response 3            │
│                   (async runtime)                               │
│                                                                 │
│    Scaling: Container handles up to 80 concurrent requests      │
│    Memory: Shared within container (configurable 128MB-32GB)    │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  WORKERS (isolated per request):                                │
│                                                                 │
│    Request 1 ──▶ [Isolate A] ──▶ Response 1                    │
│    Request 2 ──▶ [Isolate B] ──▶ Response 2                    │
│    Request 3 ──▶ [Isolate C] ──▶ Response 3                    │
│                                                                 │
│    Scaling: Isolates are lightweight (microseconds to create)   │
│    Memory: 128MB limit per isolate                              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Resource Limits

| Resource | Lambda | Cloud Run | Workers |
|----------|--------|-----------|---------|
| Memory | 128MB - 10GB | 128MB - 32GB | 128MB |
| CPU | Proportional to memory | 1-8 vCPU | 10-50ms CPU time |
| Timeout | 15 minutes | 60 minutes | 30 seconds |
| Payload size | 6MB (sync) / 20MB (async) | 32MB | 100MB |
| Tmp storage | 512MB - 10GB | Ephemeral disk | None |

## When to Choose Each Option

### Choose Lambda When:

- ✅ **Sporadic traffic** - Pay nothing during idle periods
- ✅ **AWS-native environment** - VPC, RDS, DynamoDB integration
- ✅ **Unpredictable scaling** - 0 to thousands of concurrent users
- ✅ **Simple deployment** - No container management
- ✅ **OAuth with Cognito** - Built-in user management

```bash
# Ideal Lambda use case: Internal business tool
cargo pmcp deploy init --target aws-lambda
cargo pmcp deploy

# Result: HTTPS endpoint with automatic scaling
# Cost: ~$0.20 per million requests (128MB, 100ms avg)
```

### Choose Cloud Run When:

- ✅ **Long-running requests** - Up to 60 minutes per request
- ✅ **High concurrency per instance** - Efficient resource usage
- ✅ **Custom dependencies** - Docker flexibility
- ✅ **GCP-native environment** - Cloud SQL, Firestore
- ✅ **Minimum instances needed** - Avoid cold starts entirely

```bash
# Ideal Cloud Run use case: Data processing with large queries
cargo pmcp deploy init --target google-cloud-run
cargo pmcp deploy --target google-cloud-run

# Result: Container-based deployment with persistent connections
# Cost: ~$0.00002400/vCPU-second + memory
```

### Choose Workers When:

- ✅ **Global user base** - Minimize latency worldwide
- ✅ **Stateless operations** - No database, or using KV/D1
- ✅ **High request volume** - Millions of requests/day
- ✅ **CPU-bound tasks** - Parsing, transformation, validation

```bash
# Ideal Workers use case: Global API with caching
cargo pmcp deploy init --target cloudflare-workers
cargo pmcp deploy --target cloudflare-workers

# Result: Edge deployment to 300+ locations
# Cost: $0.50 per million requests (free tier: 100K/day)
```

## Hybrid Architectures

For complex applications, you may combine deployment targets:

```
┌─────────────────────────────────────────────────────────────────┐
│                    HYBRID DEPLOYMENT                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Global Users                                                   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────┐                   │
│  │        Cloudflare Workers (Edge)        │                   │
│  │  - Request routing                      │                   │
│  │  - Caching                              │                   │
│  │  - Rate limiting                        │                   │
│  │  - Authentication                       │                   │
│  └─────────────────────────────────────────┘                   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────┐                   │
│  │         AWS Lambda (Serverless)         │                   │
│  │  - Business logic                       │                   │
│  │  - Database queries                     │                   │
│  │  - Complex processing                   │                   │
│  └─────────────────────────────────────────┘                   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────┐                   │
│  │    RDS / DynamoDB (Data Layer)          │                   │
│  └─────────────────────────────────────────┘                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

This architecture uses Workers for edge caching and routing, Lambda for serverless compute, and managed databases for persistence.

## Migration Considerations

### Lambda → Cloud Run

When to migrate:
- Hitting 15-minute timeout limit
- Need more than 10GB memory
- Want to reduce cold start impact with min instances

```bash
# Migration path
cargo pmcp deploy init --target google-cloud-run
# Update environment variables
cargo pmcp deploy --target google-cloud-run
# Verify, then destroy Lambda
cargo pmcp deploy destroy --target aws-lambda --clean
```

### Lambda → Workers

When to migrate:
- Need global low-latency
- Workload is stateless
- Can use KV/D1 instead of RDS

**Considerations:**
- WASM has different capabilities than native code
- Database access patterns may need redesign
- Some crates don't compile to WASM

## Summary

| Aspect | Lambda | Cloud Run | Workers |
|--------|--------|-----------|---------|
| Execution model | Function per request | Container server | WASM isolate |
| Cold start (Rust) | 50-150ms | 100-500ms | 0-5ms |
| Concurrency | 1 per instance | Many per container | 1 per isolate |
| Max timeout | 15 min | 60 min | 30s |
| Best for | General serverless | Long-running, GCP | Global edge |
| Rust advantage | Fast cold start | Tiny images | Native WASM |

Choose based on your specific requirements:
- **Traffic pattern** (sporadic vs steady)
- **Latency requirements** (regional vs global)
- **Execution duration** (seconds vs minutes)
- **Cloud ecosystem** (AWS vs GCP vs Cloudflare)
