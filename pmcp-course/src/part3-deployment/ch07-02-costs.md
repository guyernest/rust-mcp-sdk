# Cost Analysis Framework

Understanding cloud costs is essential for production MCP deployments. This lesson provides a practical framework for estimating, comparing, and optimizing costs across deployment targets.

## The Cost Equation

Cloud costs for MCP servers typically consist of three components:

```
┌─────────────────────────────────────────────────────────────────┐
│                      TOTAL COST                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   Total Cost = Compute + Data Transfer + Storage + Extras       │
│                                                                 │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐            │
│   │  Compute    │  │  Network    │  │  Storage    │            │
│   │             │  │             │  │             │            │
│   │  - CPU time │  │  - Egress   │  │  - Database │            │
│   │  - Memory   │  │  - API GW   │  │  - Logs     │            │
│   │  - Requests │  │  - CDN      │  │  - Secrets  │            │
│   └─────────────┘  └─────────────┘  └─────────────┘            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Platform Pricing Models

### AWS Lambda

Lambda charges based on **requests** and **duration** (measured in GB-seconds):

```
┌─────────────────────────────────────────────────────────────────┐
│                    AWS LAMBDA PRICING                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Requests:     $0.20 per 1 million requests                     │
│  Duration:     $0.0000166667 per GB-second (x86)                │
│                $0.0000133334 per GB-second (ARM64) ← 20% cheaper│
│                                                                 │
│  Free tier:    1M requests + 400,000 GB-seconds per month       │
│                                                                 │
│  API Gateway:  $1.00 per million requests (HTTP API)            │
│                $3.50 per million requests (REST API)            │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Example calculation for Rust MCP server:**

```
Scenario: 100,000 requests/month, 100ms average, 128MB memory

Compute:
  Requests:     100,000 × $0.20/1M = $0.02
  GB-seconds:   100,000 × 0.1s × 0.128GB = 1,280 GB-seconds
  Duration:     1,280 × $0.0000133334 = $0.017 (ARM64)

API Gateway:
  HTTP API:     100,000 × $1.00/1M = $0.10

Total:          $0.02 + $0.017 + $0.10 = $0.137/month

Compare to Python (500ms avg, 256MB):
  GB-seconds:   100,000 × 0.5s × 0.256GB = 12,800 GB-seconds
  Duration:     12,800 × $0.0000133334 = $0.17

  Total:        $0.02 + $0.17 + $0.10 = $0.29/month (2.1× more)
```

**Rust advantage**: Faster execution and lower memory = lower costs.

### Google Cloud Run

Cloud Run charges for **vCPU-seconds**, **memory**, and **requests**:

```
┌─────────────────────────────────────────────────────────────────┐
│                  GOOGLE CLOUD RUN PRICING                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  CPU:          $0.00002400 per vCPU-second                      │
│  Memory:       $0.00000250 per GiB-second                       │
│  Requests:     $0.40 per million requests                       │
│                                                                 │
│  Free tier:    2M requests, 360,000 vCPU-seconds,               │
│                180,000 GiB-seconds per month                    │
│                                                                 │
│  Min instances: Billed even when idle (if configured)           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Example calculation:**

```
Scenario: 100,000 requests/month, 1 vCPU, 512MB, 100ms average

CPU:         100,000 × 0.1s × 1 vCPU = 10,000 vCPU-seconds
             10,000 × $0.000024 = $0.24

Memory:      100,000 × 0.1s × 0.5 GiB = 5,000 GiB-seconds
             5,000 × $0.0000025 = $0.0125

Requests:    100,000 × $0.40/1M = $0.04

Total:       $0.24 + $0.0125 + $0.04 = $0.29/month
```

**With minimum instances (avoid cold starts):**

```
1 min instance, always on:
  Hours/month:  730 hours × 3600s = 2,628,000 seconds
  CPU:          2,628,000 × 1 vCPU × $0.000024 = $63.07
  Memory:       2,628,000 × 0.5 GiB × $0.0000025 = $3.29

Total with min instance: $63.07 + $3.29 = $66.36/month (idle cost)
```

### Cloudflare Workers

Workers has a simpler pricing model based on **requests**:

```
┌─────────────────────────────────────────────────────────────────┐
│                 CLOUDFLARE WORKERS PRICING                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Free plan:    100,000 requests/day (no cost)                   │
│                                                                 │
│  Paid plan:    $5/month base                                    │
│                First 10M requests included                      │
│                $0.50 per additional million requests            │
│                                                                 │
│  CPU time:     10ms free, then $0.02 per additional million ms  │
│                                                                 │
│  KV storage:   Free reads, $0.50/million writes                 │
│  D1 database:  $0.75/million rows read, $1.00/million written   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Example calculation:**

```
Scenario: 100,000 requests/month

Free plan (if ≤100K/day):
  Cost: $0/month

Paid plan (for higher volume):
  Base:      $5/month
  Requests:  Included (under 10M)

Total:       $5/month (flat)
```

## Cost Comparison by Usage Pattern

### Low Volume (10K requests/month)

| Platform | Monthly Cost | Notes |
|----------|--------------|-------|
| Lambda (ARM64) | ~$0.01 | Free tier covers most |
| Cloud Run | ~$0.03 | Free tier covers most |
| Workers | $0.00 | Free plan |
| pmcp.run | TBD | Coming soon |

### Medium Volume (1M requests/month)

| Platform | Monthly Cost | Notes |
|----------|--------------|-------|
| Lambda (ARM64) | ~$3.50 | $0.20 requests + $0.17 compute + $1 API GW |
| Cloud Run | ~$5.00 | Higher per-request compute |
| Workers | $5.00 | Flat rate (paid plan) |
| pmcp.run | TBD | Coming soon |

### High Volume (100M requests/month)

| Platform | Monthly Cost | Notes |
|----------|--------------|-------|
| Lambda (ARM64) | ~$140 | Scales linearly |
| Cloud Run | ~$250 | Higher compute costs |
| Workers | ~$50 | Extremely cost-effective at scale |
| pmcp.run | TBD | Coming soon |

## Hidden Costs to Consider

### 1. Data Transfer (Egress)

Sending data out of cloud providers costs money:

```
┌─────────────────────────────────────────────────────────────────┐
│                    DATA TRANSFER COSTS                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  AWS (Lambda):                                                  │
│    First 10TB:   $0.09/GB                                       │
│    Next 40TB:    $0.085/GB                                      │
│    Over 150TB:   $0.07/GB                                       │
│                                                                 │
│  GCP (Cloud Run):                                               │
│    First 1TB:    Free                                           │
│    1-10TB:       $0.12/GB                                       │
│    Over 10TB:    $0.11/GB                                       │
│                                                                 │
│  Cloudflare:                                                    │
│    All egress:   Free (included in plan)                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Example impact:**

```
Scenario: 1M requests, 10KB average response

Data transfer: 1M × 10KB = 10GB

AWS cost:    10GB × $0.09 = $0.90/month
GCP cost:    Free (under 1TB)
Cloudflare:  Free
```

For MCP servers returning large datasets, egress can exceed compute costs.

### 2. Logging and Monitoring

CloudWatch, Cloud Logging, and observability tools add costs:

```
CloudWatch Logs (AWS):
  Ingestion:  $0.50/GB
  Storage:    $0.03/GB/month
  Queries:    $0.005/GB scanned

Cloud Logging (GCP):
  First 50GB: Free
  Over 50GB:  $0.50/GB

Cloudflare:
  Workers logs: Included
  Analytics:    Included in paid plan
```

**Cost optimization:**

```rust
// Bad: Verbose logging in production
tracing::info!("Processing request: {:?}", full_request_body);

// Good: Log only essential data
tracing::info!(
    request_id = %request.id,
    tool = %request.method,
    "MCP request"
);
```

### 3. Database Connections

Database costs often dominate for data-heavy MCP servers:

```
┌─────────────────────────────────────────────────────────────────┐
│                    DATABASE COSTS                               │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  RDS PostgreSQL (db.t3.micro):                                  │
│    Instance:   ~$15/month                                       │
│    Storage:    $0.115/GB/month                                  │
│                                                                 │
│  DynamoDB (on-demand):                                          │
│    Reads:      $0.25 per million                                │
│    Writes:     $1.25 per million                                │
│                                                                 │
│  Cloud SQL (db-f1-micro):                                       │
│    Instance:   ~$9/month                                        │
│    Storage:    $0.17/GB/month                                   │
│                                                                 │
│  Cloudflare D1:                                                 │
│    Reads:      $0.75 per million rows                           │
│    Writes:     $1.00 per million rows                           │
│    Storage:    First 5GB free                                   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4. Cold Start Costs (Provisioned Concurrency)

To eliminate cold starts, you pay for always-on capacity:

```
Lambda Provisioned Concurrency:
  $0.000004167 per GB-second (on top of regular pricing)

Example: 10 provisioned instances, 128MB
  Monthly: 10 × 0.128GB × 2,628,000s × $0.000004167 = $14.02

Cloud Run Min Instances:
  Same as regular instance pricing when idle
  1 min instance (1 vCPU, 512MB): ~$66/month
```

## Cost Optimization Strategies

### 1. Right-Size Memory

Lambda performance scales with memory. Find the sweet spot:

```
┌─────────────────────────────────────────────────────────────────┐
│              MEMORY VS COST OPTIMIZATION                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  128MB:  Slowest, cheapest per GB-second, often most expensive  │
│  256MB:  2× CPU, often 2× faster, same total cost               │
│  512MB:  4× CPU, diminishing returns for IO-bound work          │
│  1GB+:   For CPU-heavy processing only                          │
│                                                                 │
│  Optimal for Rust MCP servers: 256-512MB                        │
│  (Fast enough for instant response, not paying for unused CPU)  │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Benchmarking approach:**

```bash
# Test different memory configurations
for mem in 128 256 512 1024; do
  echo "Testing ${mem}MB..."
  # Update Lambda config and run load test
  aws lambda update-function-configuration \
    --function-name my-mcp-server \
    --memory-size $mem

  # Run benchmark
  hey -n 1000 -c 10 https://api.example.com/mcp

  # Calculate cost per request
done
```

### 2. Use ARM64 (Graviton2)

AWS Lambda on ARM64 is 20% cheaper and often faster for Rust:

```toml
# .pmcp/deploy.toml
[lambda]
architecture = "arm64"  # Default in PMCP

# Building for ARM64
# cargo pmcp deploy automatically uses cargo-lambda with ARM64 target
```

### 3. Batch Requests When Possible

Instead of many small requests, batch operations:

```rust
// Expensive: 10 separate tool calls
for item in items {
    client.call_tool("process_item", json!({ "item": item })).await?;
}

// Cheaper: 1 batched call
client.call_tool("process_items", json!({ "items": items })).await?;
```

### 4. Cache Aggressively

Reduce database queries with caching:

```rust
use moka::future::Cache;

// In-memory cache for Lambda warm instances
static CACHE: Lazy<Cache<String, Vec<User>>> = Lazy::new(|| {
    Cache::builder()
        .max_capacity(1000)
        .time_to_live(Duration::from_secs(300))
        .build()
});

async fn get_users(department: &str) -> Result<Vec<User>> {
    if let Some(users) = CACHE.get(department).await {
        return Ok(users);
    }

    let users = db.query_users(department).await?;
    CACHE.insert(department.to_string(), users.clone()).await;
    Ok(users)
}
```

### 5. Set Appropriate Timeouts

Don't pay for hung requests:

```toml
# .pmcp/deploy.toml
[lambda]
timeout_seconds = 30  # Default: 30s, max: 900s

[cloud_run]
timeout_seconds = 60  # Default: 60s, max: 3600s
```

## Cost Monitoring

### AWS Cost Explorer

Track Lambda costs by function:

```bash
# View Lambda costs for last 30 days
aws ce get-cost-and-usage \
  --time-period Start=2024-01-01,End=2024-01-31 \
  --granularity MONTHLY \
  --metrics BlendedCost \
  --filter '{"Dimensions":{"Key":"SERVICE","Values":["AWS Lambda"]}}'
```

### GCP Billing Reports

Filter by Cloud Run service:

```bash
gcloud billing budgets create \
  --billing-account=ACCOUNT_ID \
  --display-name="MCP Server Budget" \
  --budget-amount=100USD \
  --threshold-rules=percent=80,percent=100
```

### Setting Up Alerts

```yaml
# AWS CloudWatch alarm for unexpected costs
Resources:
  CostAlarm:
    Type: AWS::CloudWatch::Alarm
    Properties:
      AlarmName: MCPServerCostAlert
      MetricName: EstimatedCharges
      Namespace: AWS/Billing
      Statistic: Maximum
      Period: 86400
      EvaluationPeriods: 1
      Threshold: 50
      ComparisonOperator: GreaterThanThreshold
```

## Total Cost of Ownership (TCO)

Beyond cloud bills, consider:

| Factor | Lambda | Cloud Run | Workers |
|--------|--------|-----------|---------|
| Development time | Low | Medium | Medium (WASM) |
| Operational overhead | Very low | Low | Very low |
| Debugging complexity | Medium | Low | Medium |
| Vendor lock-in | Medium | Low | High |
| Team expertise needed | AWS | Docker/GCP | WASM |

## Summary

```
┌─────────────────────────────────────────────────────────────────┐
│                    COST DECISION MATRIX                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  Low volume (<100K/month):                                      │
│    → Workers free tier or Lambda free tier                      │
│    → Cost: $0-5/month                                           │
│                                                                 │
│  Medium volume (100K-10M/month):                                │
│    → Lambda (ARM64) or Workers paid                             │
│    → Cost: $5-50/month                                          │
│                                                                 │
│  High volume (>10M/month):                                      │
│    → Workers (best per-request cost)                            │
│    → Or Lambda with reserved concurrency                        │
│    → Cost: $50+/month, optimize aggressively                    │
│                                                                 │
│  Need zero cold starts:                                         │
│    → Cloud Run with min instances                               │
│    → Or Lambda with provisioned concurrency                     │
│    → Cost: $50-100+/month baseline                              │
│                                                                 │
│  Rust advantage across all platforms:                           │
│    → 50-80% lower compute costs vs Python/Node                  │
│    → Faster execution = better user experience                  │
│    → Lower memory = cheaper instances                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Key takeaways:**
1. **Start with free tiers** - All platforms offer generous free usage
2. **Rust reduces costs** - Faster execution and lower memory usage
3. **Watch hidden costs** - Egress, logging, and databases can dominate
4. **Set budgets and alerts** - Prevent surprise bills
5. **Benchmark before optimizing** - Measure actual costs before over-engineering
