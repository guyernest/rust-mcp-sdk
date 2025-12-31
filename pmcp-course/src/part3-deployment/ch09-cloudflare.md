# Cloudflare Workers Deployment

Cloudflare Workers runs your MCP server as WebAssembly (WASM) on Cloudflare's global edge network. With 300+ locations worldwide and sub-millisecond cold starts, Workers delivers the lowest latency for globally distributed users.

This chapter provides a comprehensive guide to deploying Rust MCP servers on Cloudflare Workers.

## Why Cloudflare Workers?

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    CLOUDFLARE EDGE NETWORK                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│                         Your MCP Server                                 │
│                    (compiled to WebAssembly)                            │
│                              │                                          │
│              ┌───────────────┼───────────────┐                          │
│              │               │               │                          │
│              ▼               ▼               ▼                          │
│     ┌─────────────┐  ┌─────────────┐  ┌─────────────┐                  │
│     │   Tokyo     │  │   London    │  │  New York   │                  │
│     │   (5ms)     │  │   (5ms)     │  │   (5ms)     │                  │
│     └──────┬──────┘  └──────┬──────┘  └──────┬──────┘                  │
│            │               │               │                            │
│     ┌──────┴──────┐  ┌──────┴──────┐  ┌──────┴──────┐                  │
│     │ Users in    │  │ Users in    │  │ Users in    │                  │
│     │ Asia        │  │ Europe      │  │ Americas    │                  │
│     └─────────────┘  └─────────────┘  └─────────────┘                  │
│                                                                         │
│     Benefits:                                                           │
│     • 300+ edge locations worldwide                                     │
│     • Sub-millisecond cold starts (V8 isolates)                         │
│     • Unlimited free egress bandwidth                                   │
│     • Built-in DDoS protection                                          │
│     • Integrated storage (KV, D1, R2)                                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### When to Choose Workers

| Use Case | Workers | Lambda |
|----------|---------|--------|
| Global low-latency | ✅ Best choice | ❌ Regional only |
| Stateless API | ✅ Ideal | ✅ Good |
| Database access | ⚠️ D1/Hyperdrive | ✅ RDS/DynamoDB |
| Long computations | ❌ 30s limit | ✅ 15min limit |
| File system access | ❌ No filesystem | ✅ /tmp available |
| Complex dependencies | ⚠️ WASM compat | ✅ Full native |

## Prerequisites

```bash
# Node.js (for wrangler)
node --version  # 18+ recommended

# Wrangler CLI
npm install -g wrangler

# Login to Cloudflare
wrangler login

# Rust with WASM target
rustup target add wasm32-unknown-unknown

# wasm-pack for building
cargo install wasm-pack
```

## Architecture Overview

Workers uses V8 isolates instead of containers:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    WORKERS EXECUTION MODEL                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  TRADITIONAL CONTAINER                    V8 ISOLATE                    │
│                                                                         │
│  ┌─────────────────────┐                 ┌─────────────────────┐        │
│  │     Container       │                 │     V8 Engine       │        │
│  │  ┌───────────────┐  │                 │  ┌───────────────┐  │        │
│  │  │   OS Layer    │  │                 │  │  Isolate A    │  │        │
│  │  ├───────────────┤  │                 │  │  (your WASM)  │  │        │
│  │  │   Runtime     │  │                 │  ├───────────────┤  │        │
│  │  ├───────────────┤  │                 │  │  Isolate B    │  │        │
│  │  │   Your Code   │  │                 │  │  (other user) │  │        │
│  │  └───────────────┘  │                 │  ├───────────────┤  │        │
│  └─────────────────────┘                 │  │  Isolate C    │  │        │
│                                          │  │  (other user) │  │        │
│  Startup: 50-500ms                       │  └───────────────┘  │        │
│  Memory: Dedicated                       └─────────────────────┘        │
│                                                                         │
│                                          Startup: <1ms                  │
│                                          Memory: Shared engine          │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

V8 isolates are lightweight sandboxes that:
- Share the V8 JavaScript/WASM engine
- Start in microseconds (not milliseconds)
- Provide strong security isolation
- Have 128MB memory limit per request

## Step-by-Step Deployment

### Step 1: Initialize Deployment

```bash
# From your MCP server project
cargo pmcp deploy init --target cloudflare-workers
```

This creates the deployment configuration:

```
.pmcp/
├── deploy.toml              # Deployment configuration
└── workers/
    ├── wrangler.toml        # Wrangler configuration
    ├── src/
    │   └── lib.rs           # Worker entry point (generated)
    └── Cargo.toml           # WASM-specific dependencies
```

### Step 2: Configure Deployment

Edit `.pmcp/deploy.toml`:

```toml
[target]
target_type = "cloudflare-workers"

[server]
name = "my-mcp-server"

[cloudflare]
account_id = "your-account-id"  # From Cloudflare dashboard
zone_id = "your-zone-id"        # Optional: for custom domains

[workers]
name = "my-mcp-server"
compatibility_date = "2024-01-01"
main = "build/worker/shim.mjs"

# Environment variables (non-secret)
[workers.vars]
RUST_LOG = "info"
ENVIRONMENT = "production"

# Bindings to Cloudflare services
[workers.kv_namespaces]
# KV_CACHE = "your-kv-namespace-id"

[workers.d1_databases]
# DB = "your-d1-database-id"

[workers.r2_buckets]
# STORAGE = "your-r2-bucket-name"
```

Edit the generated `wrangler.toml`:

```toml
name = "my-mcp-server"
main = "build/worker/shim.mjs"
compatibility_date = "2024-01-01"

[build]
command = "cargo pmcp deploy build --target cloudflare-workers"

# Route configuration
[[routes]]
pattern = "mcp.example.com/*"
zone_id = "your-zone-id"

# Or use workers.dev subdomain (default)
# workers_dev = true
```

### Step 3: Build and Deploy

```bash
# Build WASM and deploy
cargo pmcp deploy --target cloudflare-workers

# Or step by step:
cargo pmcp deploy build --target cloudflare-workers
wrangler deploy
```

**First deployment** creates:
- Worker script on Cloudflare's network
- workers.dev subdomain (e.g., `my-mcp-server.username.workers.dev`)
- KV/D1/R2 bindings if configured

### Step 4: Verify Deployment

```bash
# Get deployment URL
cargo pmcp deploy outputs --target cloudflare-workers

# Output:
# WorkerUrl: https://my-mcp-server.username.workers.dev
# McpEndpoint: https://my-mcp-server.username.workers.dev/mcp

# Test the endpoint
curl -X POST https://my-mcp-server.username.workers.dev/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}'
```

## Worker Entry Point

The generated worker entry point bridges HTTP to your MCP server:

```rust
// .pmcp/workers/src/lib.rs
use worker::*;
use pmcp::server::Server;
use pmcp::transport::WorkersTransport;

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Initialize router
    let router = Router::new();

    router
        // Health check
        .get("/health", |_, _| Response::ok("OK"))

        // MCP endpoint
        .post_async("/mcp", |mut req, ctx| async move {
            let body = req.text().await?;

            // Build MCP server (stateless per request)
            let server = build_mcp_server(&ctx.env)?;

            // Process MCP request
            let response = server.handle_request(&body).await?;

            Response::from_json(&response)
        })

        // Run router
        .run(req, env)
        .await
}

fn build_mcp_server(env: &Env) -> Result<Server> {
    Server::builder()
        .name("my-mcp-server")
        .version("1.0.0")
        .tool("query", TypedTool::new("query", |input: QueryInput| async move {
            // Tool implementation
            Ok(json!({"result": "data"}))
        }))
        .build()
        .map_err(|e| Error::from(e.to_string()))
}
```

## Workers Bindings

Cloudflare provides integrated storage services accessible via bindings.

### KV (Key-Value Store)

Low-latency, globally distributed key-value storage:

```rust
use worker::*;

async fn cache_handler(env: &Env, key: &str) -> Result<Option<String>> {
    // Get KV namespace from binding
    let kv = env.kv("CACHE")?;

    // Read value
    let value = kv.get(key).text().await?;

    Ok(value)
}

async fn cache_set(env: &Env, key: &str, value: &str, ttl_seconds: u64) -> Result<()> {
    let kv = env.kv("CACHE")?;

    // Write with expiration
    kv.put(key, value)?
        .expiration_ttl(ttl_seconds)
        .execute()
        .await?;

    Ok(())
}
```

Configure in `wrangler.toml`:

```toml
[[kv_namespaces]]
binding = "CACHE"
id = "your-namespace-id"
# preview_id = "preview-namespace-id"  # For local dev
```

### D1 (SQLite Database)

Serverless SQL database at the edge:

```rust
use worker::*;

async fn query_users(env: &Env, department: &str) -> Result<Vec<User>> {
    let db = env.d1("DB")?;

    let statement = db.prepare("SELECT * FROM users WHERE department = ?1");
    let results = statement
        .bind(&[department.into()])?
        .all()
        .await?;

    let users: Vec<User> = results.results()?;
    Ok(users)
}

async fn insert_user(env: &Env, user: &User) -> Result<()> {
    let db = env.d1("DB")?;

    db.prepare("INSERT INTO users (name, email, department) VALUES (?1, ?2, ?3)")
        .bind(&[user.name.into(), user.email.into(), user.department.into()])?
        .run()
        .await?;

    Ok(())
}
```

Configure in `wrangler.toml`:

```toml
[[d1_databases]]
binding = "DB"
database_name = "my-database"
database_id = "your-database-id"
```

Create and migrate database:

```bash
# Create database
wrangler d1 create my-database

# Run migrations
wrangler d1 migrations apply my-database

# Query interactively
wrangler d1 execute my-database --command "SELECT * FROM users"
```

### R2 (Object Storage)

S3-compatible object storage with zero egress fees:

```rust
use worker::*;

async fn get_file(env: &Env, key: &str) -> Result<Option<Vec<u8>>> {
    let bucket = env.bucket("STORAGE")?;

    match bucket.get(key).execute().await? {
        Some(object) => {
            let bytes = object.body().unwrap().bytes().await?;
            Ok(Some(bytes))
        }
        None => Ok(None),
    }
}

async fn put_file(env: &Env, key: &str, data: Vec<u8>) -> Result<()> {
    let bucket = env.bucket("STORAGE")?;

    bucket.put(key, data).execute().await?;

    Ok(())
}
```

Configure in `wrangler.toml`:

```toml
[[r2_buckets]]
binding = "STORAGE"
bucket_name = "my-bucket"
```

### Hyperdrive (External Database Connection)

Connect to external PostgreSQL/MySQL with connection pooling:

```rust
use worker::*;

async fn query_external_db(env: &Env) -> Result<Vec<Record>> {
    // Hyperdrive provides a connection string
    let hyperdrive = env.hyperdrive("EXTERNAL_DB")?;
    let connection_string = hyperdrive.connection_string();

    // Use with your preferred database client
    // Note: Must be WASM-compatible (e.g., using HTTP-based drivers)

    Ok(records)
}
```

Configure in `wrangler.toml`:

```toml
[[hyperdrive]]
binding = "EXTERNAL_DB"
id = "your-hyperdrive-id"
```

## Secrets Management

Store sensitive values securely:

```bash
# Set a secret (entered interactively, not in shell history)
wrangler secret put DATABASE_PASSWORD

# List secrets
wrangler secret list

# Delete a secret
wrangler secret delete DATABASE_PASSWORD
```

Access in your worker:

```rust
async fn handler(env: &Env) -> Result<Response> {
    let api_key = env.secret("API_KEY")?.to_string();
    // Use api_key...
    Ok(Response::ok("OK"))
}
```

## Custom Domains

Route traffic from your domain to the worker:

```toml
# wrangler.toml

# Option 1: Route pattern (requires zone in Cloudflare)
[[routes]]
pattern = "mcp.example.com/*"
zone_id = "your-zone-id"

# Option 2: Custom domain (simpler)
[[routes]]
pattern = "mcp.example.com"
custom_domain = true
```

Then add a DNS record in Cloudflare dashboard pointing to your worker.

## Environment-Specific Deployments

Use environments for staging/production:

```toml
# wrangler.toml
name = "my-mcp-server"
main = "build/worker/shim.mjs"

# Default (development)
[vars]
ENVIRONMENT = "development"

# Staging environment
[env.staging]
name = "my-mcp-server-staging"
[env.staging.vars]
ENVIRONMENT = "staging"

# Production environment
[env.production]
name = "my-mcp-server-prod"
[[env.production.routes]]
pattern = "mcp.example.com/*"
zone_id = "your-zone-id"
[env.production.vars]
ENVIRONMENT = "production"
```

Deploy to specific environment:

```bash
# Deploy to staging
wrangler deploy --env staging

# Deploy to production
wrangler deploy --env production
```

## Monitoring and Debugging

### Real-Time Logs

Stream logs from your worker:

```bash
# Tail logs in real-time
wrangler tail

# Filter by status
wrangler tail --status error

# Filter by search term
wrangler tail --search "tool_call"
```

### Structured Logging

Use console methods that appear in logs:

```rust
use worker::console_log;

async fn handler(req: Request) -> Result<Response> {
    console_log!("Request received: {} {}", req.method(), req.path());

    let start = Date::now();

    // Process request...

    let duration = Date::now().as_millis() - start.as_millis();
    console_log!("Request completed in {}ms", duration);

    Ok(response)
}
```

### Analytics

View metrics in Cloudflare dashboard:
- Request count
- Error rate
- CPU time
- Response time percentiles

## Performance Optimization

### Bundle Size

Keep WASM bundles small for faster cold starts:

```toml
# Cargo.toml
[profile.release]
opt-level = "z"        # Optimize for size
lto = true
codegen-units = 1
panic = "abort"
strip = true

[profile.release.package."*"]
opt-level = "z"
```

Typical sizes:
- Minimal MCP server: ~500KB WASM
- With dependencies: 1-3MB WASM

### CPU Time Limits

Workers has CPU time limits:

| Plan | CPU Time Limit |
|------|---------------|
| Free | 10ms |
| Paid | 50ms |

**Important**: This is *CPU time*, not wall-clock time. Waiting for I/O doesn't count.

Optimize CPU-intensive operations:

```rust
// Bad: CPU-intensive in hot path
async fn handler(input: Input) -> Result<Response> {
    let result = expensive_computation(&input.data);  // Uses CPU time
    Ok(Response::from_json(&result)?)
}

// Good: Offload to Durable Objects or external service
async fn handler(input: Input) -> Result<Response> {
    // Light processing in Worker
    let key = hash(&input.data);

    // Heavy computation cached
    let cached = env.kv("CACHE")?.get(&key).text().await?;
    if let Some(result) = cached {
        return Ok(Response::from_json(&result)?);
    }

    // Compute once, cache result
    let result = expensive_computation(&input.data);
    env.kv("CACHE")?.put(&key, &result)?.execute().await?;

    Ok(Response::from_json(&result)?)
}
```

## Limitations

Workers has specific limitations to be aware of:

| Limitation | Details |
|------------|---------|
| No filesystem | No `/tmp`, no file I/O |
| CPU time | 10-50ms per request |
| Memory | 128MB per isolate |
| Request size | 100MB max |
| Subrequest limit | 50 subrequests per request (1000 on paid) |
| No raw sockets | HTTP/HTTPS only via fetch() |

### What Works

- HTTP client requests via `fetch()`
- KV, D1, R2 storage
- Durable Objects for state
- WebSocket connections
- Crypto APIs

### What Doesn't Work

- Raw TCP/UDP sockets
- Native database drivers (use Hyperdrive or HTTP APIs)
- File system operations
- Some Rust crates (see WASM Considerations chapter)

## Connecting Clients

Configure Claude Desktop for Workers:

```json
{
  "mcpServers": {
    "my-workers-server": {
      "transport": "streamable-http",
      "url": "https://my-mcp-server.username.workers.dev/mcp"
    }
  }
}
```

With API key authentication:

```json
{
  "mcpServers": {
    "my-workers-server": {
      "transport": "streamable-http",
      "url": "https://my-mcp-server.username.workers.dev/mcp",
      "headers": {
        "Authorization": "Bearer ${MCP_API_KEY}"
      }
    }
  }
}
```

## Local Development

Test locally before deploying:

```bash
# Start local dev server
wrangler dev

# With local KV/D1/R2 simulation
wrangler dev --local

# Specify port
wrangler dev --port 8787
```

Test MCP locally:

```bash
curl -X POST http://localhost:8787/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/list","params":{},"id":1}'
```

## Summary

Cloudflare Workers deployment provides:

- **Global edge network** - 300+ locations, minimal latency
- **Sub-millisecond cold starts** - V8 isolates, not containers
- **Zero egress fees** - Unlimited outbound bandwidth included
- **Integrated storage** - KV, D1, R2 with simple bindings
- **Simple deployment** - `wrangler deploy` handles everything

Key commands:

```bash
cargo pmcp deploy init --target cloudflare-workers  # Initialize
cargo pmcp deploy --target cloudflare-workers       # Deploy
wrangler tail                                       # View logs
wrangler dev                                        # Local development
wrangler secret put KEY                             # Set secrets
```

**Best suited for:**
- Global APIs with low-latency requirements
- Stateless operations with caching
- MCP servers using D1/KV for data
- High-volume, cost-sensitive deployments

**Consider alternatives when:**
- You need raw database drivers (use Lambda)
- Long-running computations >50ms CPU (use Lambda/Cloud Run)
- Complex native dependencies (use Lambda/Cloud Run)

## Knowledge Check

Test your understanding of Cloudflare Workers deployment:

{{#quiz ../quizzes/ch09-cloudflare.toml}}

---

*Continue to [WASM Considerations](./ch09-01-wasm-considerations.md) →*
