# Deployment Overview

In Part 1 and Part 2, we built MCP servers that run locally on your development machine. These local servers are perfect for developers who want AI assistants integrated into their IDEs, accessing files, running tests, and querying local databases. But what happens when you want to share your MCP server with your entire organization?

This chapter introduces **remote MCP deployments** - taking your server from a local process to a production service that anyone in your organization can access.

## Why Remote Deployments?

### The Developer vs Business User Gap

Local MCP servers have a fundamental limitation: they require technical setup on each user's machine.

```
┌─────────────────────────────────────────────────────────────────┐
│                     LOCAL MCP SERVER                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│    Developer's Machine                                          │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │  IDE (VS Code, Cursor, etc.)                            │  │
│    │       │                                                 │  │
│    │       ▼                                                 │  │
│    │  MCP Server Process                                     │  │
│    │       │                                                 │  │
│    │       ▼                                                 │  │
│    │  Local Database / Files / APIs                          │  │
│    └─────────────────────────────────────────────────────────┘  │
│                                                                 │
│    ✅ Works great for developers                                │
│    ❌ Requires local setup, Rust toolchain, database access     │
│    ❌ Each developer runs their own instance                    │
│    ❌ No centralized access control or monitoring               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

For a sales team to query CRM data through Claude, or for analysts to access business metrics, they shouldn't need to:
- Install Rust and compile the server
- Configure database credentials on their laptop
- Manage their own server process
- Troubleshoot connection issues

**Remote deployment solves this** by making your MCP server a managed service:

```
┌─────────────────────────────────────────────────────────────────┐
│                    REMOTE MCP SERVER                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│    Cloud Platform (AWS, GCP, Cloudflare)                        │
│    ┌─────────────────────────────────────────────────────────┐  │
│    │  MCP Server (managed)                                   │  │
│    │       │                                                 │  │
│    │       ▼                                                 │  │
│    │  Production Database / Internal APIs                    │  │
│    └─────────────────────────────────────────────────────────┘  │
│            ▲                                                    │
│            │ HTTPS                                              │
│    ┌───────┴───────┬───────────────┬───────────────┐            │
│    │               │               │               │            │
│    ▼               ▼               ▼               ▼            │
│  Developer      Analyst        Sales Rep      Support Agent    │
│  (Claude.ai)   (Claude.ai)    (Claude.ai)    (Claude.ai)       │
│                                                                 │
│    ✅ No local setup required                                   │
│    ✅ Centralized access control (OAuth, SSO)                   │
│    ✅ Server is close to the data (low latency)                 │
│    ✅ IT/Ops team manages the deployment                        │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Data Proximity: Network Latency Matters

MCP servers often need to access databases, internal APIs, and file systems. When your server runs **near the data it accesses**, everything is faster:

| Scenario | Network Latency | Impact on 10 DB Queries |
|----------|-----------------|-------------------------|
| Server in same AWS VPC as RDS | ~1ms | ~10ms total |
| Server in same region, different VPC | ~5ms | ~50ms total |
| Server on user's laptop, DB in cloud | ~50-200ms | ~500-2000ms total |

For an MCP server that queries a database multiple times per tool call, running remotely in the same network as your data can be **100x faster** than running locally.

### Operational Management

Remote deployments enable proper operational practices:

- **Access Control**: Authenticate users via OAuth, SSO, or API keys
- **Audit Logging**: Track who accessed what data and when
- **Monitoring**: CloudWatch, Datadog, or built-in metrics
- **Scaling**: Handle multiple concurrent users automatically
- **Updates**: Deploy new versions without user action
- **Security**: Keep database credentials server-side, never on user machines

## Deployment Targets

PMCP supports three primary deployment targets, each optimized for different use cases:

### AWS Lambda (Serverless)

**Best for**: Most production deployments, pay-per-use, AWS-native environments

```bash
cargo pmcp deploy init --target aws-lambda
cargo pmcp deploy
```

AWS Lambda runs your MCP server as a serverless function, triggered by HTTP requests through API Gateway.

**Architecture:**
```
┌──────────────┐    ┌───────────────┐    ┌─────────────────────┐
│  API Gateway │───▶│  Lambda       │───▶│  RDS / DynamoDB /   │
│  (HTTPS)     │    │  (your server)│    │  S3 / Internal APIs │
└──────────────┘    └───────────────┘    └─────────────────────┘
```

**Why Rust Excels on Lambda:**

| Metric | Rust | Python | Node.js |
|--------|------|--------|---------|
| Cold start | ~50-100ms | ~500-1500ms | ~200-500ms |
| Warm latency | ~5-10ms | ~20-50ms | ~15-30ms |
| Memory footprint | ~128MB typical | ~256-512MB | ~256MB |
| Binary size | ~5-15MB | N/A (interpreted) | N/A |

Rust's compiled binaries start almost instantly and use minimal memory. This translates directly to **lower costs** (Lambda charges by GB-seconds) and **better user experience** (faster responses).

**Features:**
- ✅ Pay only for actual usage (no idle costs)
- ✅ Automatic scaling to thousands of concurrent users
- ✅ VPC integration for private database access
- ✅ CDK-based infrastructure as code
- ✅ OAuth support via Cognito

### Cloudflare Workers (Edge + WASM)

**Best for**: Global distribution, sub-millisecond latency, WASM-compatible workloads

```bash
cargo pmcp deploy init --target cloudflare-workers
cargo pmcp deploy --target cloudflare-workers
```

Cloudflare Workers runs your server as WebAssembly on Cloudflare's global edge network.

**Architecture:**
```
    User in Tokyo          User in London         User in New York
         │                      │                       │
         ▼                      ▼                       ▼
    ┌─────────┐            ┌─────────┐            ┌─────────┐
    │ Edge    │            │ Edge    │            │ Edge    │
    │ (Tokyo) │            │ (London)│            │ (NYC)   │
    └─────────┘            └─────────┘            └─────────┘
         │                      │                       │
         └──────────────────────┼───────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  Origin APIs / KV     │
                    │  (if needed)          │
                    └───────────────────────┘
```

**Why Rust Excels on Cloudflare Workers:**

Cloudflare Workers runs WebAssembly (WASM), and Rust has first-class WASM support:

| Metric | Rust → WASM | JavaScript |
|--------|-------------|------------|
| Cold start | ~0-5ms | ~0-5ms |
| CPU efficiency | ~10x faster | Baseline |
| Bundle size | ~500KB-2MB | Varies |
| Memory safety | Compile-time | Runtime |

Rust compiles to highly optimized WASM that runs at near-native speed on the edge.

**Features:**
- ✅ Global edge network (300+ locations)
- ✅ Sub-millisecond cold starts
- ✅ KV storage for caching
- ✅ R2 for object storage
- ✅ D1 for SQLite at the edge

**Considerations:**
- WASM has some limitations (no raw filesystem, limited networking)
- Best for stateless, CPU-bound workloads
- May need to adapt database access patterns

### Google Cloud Run (Containers)

**Best for**: Docker-based workflows, GCP-native environments, long-running requests

```bash
cargo pmcp deploy init --target google-cloud-run
cargo pmcp deploy --target google-cloud-run
```

Cloud Run runs your server as a container, with automatic scaling and HTTPS.

**Architecture:**
```
┌──────────────┐    ┌───────────────────────┐    ┌─────────────────┐
│  Cloud Run   │───▶│  Container            │───▶│  Cloud SQL /    │
│  (HTTPS)     │    │  (your server image)  │    │  Firestore /    │
└──────────────┘    └───────────────────────┘    │  GCS            │
                                                 └─────────────────┘
```

**Why Rust Excels on Cloud Run:**

| Metric | Rust Container | Python Container |
|--------|----------------|------------------|
| Image size | ~10-20MB | ~200-500MB |
| Startup time | ~100-300ms | ~1-3s |
| Memory at idle | ~10-30MB | ~100-200MB |
| Min instances cost | Lower | Higher |

Rust's tiny, statically-linked binaries produce minimal Docker images that start quickly and use less memory.

**Features:**
- ✅ Full Docker compatibility (any dependencies)
- ✅ VPC connector for private networks
- ✅ Request timeout up to 60 minutes
- ✅ Automatic HTTPS with managed certificates
- ✅ Cloud Build integration for CI/CD

## The `cargo pmcp deploy` Command

PMCP provides a unified CLI for all deployment targets:

```bash
# Initialize deployment configuration
cargo pmcp deploy init --target <aws-lambda|cloudflare-workers|google-cloud-run>

# Deploy to the cloud
cargo pmcp deploy [--target <target>]

# View deployment outputs (URL, etc.)
cargo pmcp deploy outputs

# View logs
cargo pmcp deploy logs [--tail]

# Manage secrets
cargo pmcp deploy secrets set <key> [--from-env <ENV_VAR>]
cargo pmcp deploy secrets list

# Destroy deployment
cargo pmcp deploy destroy [--clean]
```

### Target Selection

The `--target` flag specifies which platform to deploy to. If not specified, PMCP reads from `.pmcp/deploy.toml`:

```toml
# .pmcp/deploy.toml
[target]
target_type = "aws-lambda"  # or "cloudflare-workers", "google-cloud-run"

[server]
name = "my-mcp-server"

[aws]
region = "us-east-1"
```

### Deployment Workflow

A typical deployment follows this pattern:

```bash
# 1. Initialize (one-time setup)
cargo pmcp deploy init --target aws-lambda

# 2. Build and deploy
cargo pmcp deploy

# 3. Verify
cargo pmcp deploy outputs
cargo pmcp deploy test

# 4. Monitor
cargo pmcp deploy logs --tail

# 5. Update (re-run deploy with new code)
cargo pmcp deploy

# 6. Rollback if needed
cargo pmcp deploy rollback

# 7. Cleanup when done
cargo pmcp deploy destroy --clean
```

## Choosing a Deployment Target

Use this decision tree to select the right target:

```
┌─────────────────────────────────────────────────────────────────┐
│                   Which deployment target?                      │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
         Need global    Using AWS       Using GCP
         edge latency?  infrastructure? infrastructure?
              │               │               │
              ▼               ▼               ▼
    ┌─────────────────┐ ┌─────────────┐ ┌────────────────┐
    │  Cloudflare     │ │ AWS Lambda  │ │ Google Cloud   │
    │  Workers        │ │             │ │ Run            │
    │                 │ │             │ │                │
    │  Best for:      │ │ Best for:   │ │ Best for:      │
    │  - Global users │ │ - VPC/RDS   │ │ - Cloud SQL    │
    │  - Static data  │ │ - Cognito   │ │ - Long requests│
    │  - Edge compute │ │ - Most apps │ │ - Docker deps  │
    └─────────────────┘ └─────────────┘ └────────────────┘
```

| Factor | AWS Lambda | Cloudflare Workers | Cloud Run |
|--------|------------|-------------------|-----------|
| Cold start | ~50-100ms | ~0-5ms | ~100-300ms |
| Max request duration | 15 min | 30s (50ms CPU) | 60 min |
| Private network | VPC | Limited | VPC Connector |
| Database access | RDS, DynamoDB | D1, external | Cloud SQL |
| Pricing model | Per-request | Per-request | Per-container-second |
| Best for | General purpose | Edge/global | Long-running |

## pmcp.run: Managed Hosting (Coming Soon)

For teams that want the benefits of remote deployment without managing cloud infrastructure, **pmcp.run** is a managed hosting service for PMCP servers.

### Public Hosting

Deploy your MCP server with a single command:

```bash
cargo pmcp deploy --target pmcp-run
```

Your server gets a public URL like `https://api.pmcp.run/your-server/mcp` that anyone can connect to (with proper authentication).

**Benefits:**
- No AWS/GCP/Cloudflare account needed
- Automatic HTTPS, scaling, and monitoring
- OAuth integration out of the box
- Pay-as-you-go pricing

### Enterprise Private Hosting

For organizations with compliance requirements, pmcp.run offers private deployments:

- **Dedicated infrastructure** in your preferred region
- **VPC peering** to connect to your private databases
- **SSO integration** with your identity provider
- **Audit logging** shipped to your SIEM
- **SLA guarantees** for production workloads

Contact sales for enterprise pricing and setup.

### Current Status

The pmcp.run service is currently in development. The deployment target is available for early access:

```bash
# Login to pmcp.run
cargo pmcp deploy login --target pmcp-run

# Deploy
cargo pmcp deploy --target pmcp-run

# View your servers
cargo pmcp deploy outputs --target pmcp-run
```

## Summary

Remote MCP deployments transform your server from a developer tool into an organization-wide service. The key benefits are:

1. **Accessibility**: Business users access AI tools without technical setup
2. **Data Proximity**: Server runs near databases for low-latency queries
3. **Operations**: IT teams manage access control, monitoring, and updates

Rust excels on all deployment targets because of its:
- **Fast cold starts**: No JIT warmup or interpreter startup
- **Low memory usage**: Efficient binaries reduce costs
- **Small artifacts**: Tiny Docker images and WASM bundles
- **Predictable performance**: No garbage collection pauses

In the following chapters, we'll dive deep into each deployment target with hands-on exercises.

## What's Next

- **Chapter 8**: AWS Lambda deep dive with VPC, Cognito, and CDK
- **Chapter 9**: Cloudflare Workers for edge deployment
- **Chapter 10**: Google Cloud Run with Cloud SQL
- **Chapter 11**: Authentication and authorization patterns
- **Chapter 12**: Monitoring, logging, and observability
