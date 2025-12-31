::: exercise
id: ch08-01-lambda-deployment
difficulty: intermediate
time: 45 minutes
:::

Deploy your MCP server to AWS Lambda using cargo-pmcp. This is your first
cloud deployment, connecting local development to production infrastructure.

::: objectives
thinking:
  - Why serverless architecture is ideal for MCP servers
  - How Lambda Web Adapter enables HTTP-based MCP servers
  - Cold start optimization strategies for Rust binaries
doing:
  - Initialize deployment with cargo pmcp deploy init
  - Configure Lambda settings (memory, timeout, VPC)
  - Optimize binary size for faster cold starts
  - Deploy and verify with curl tests
:::

::: discussion
- What happens when you close your laptop? How do AI clients access your MCP server?
- Why is "same code locally and in Lambda" important for debugging?
- Where should database credentials come from - environment variables or Secrets Manager?
:::

## Prerequisites

Before starting, verify you have:
- AWS CLI configured (`aws sts get-caller-identity`)
- cargo-lambda installed (`cargo install cargo-lambda`)
- AWS CDK installed (`npm install -g aws-cdk`)

## Step 1: Initialize Deployment

```bash
# From your project directory
cargo pmcp deploy init --target aws-lambda

# This creates .pmcp/ with CDK configuration
```

## Step 2: Configure deploy.toml

Edit `.pmcp/deploy.toml`:

```toml
[lambda]
memory_mb = 256
timeout_seconds = 30
architecture = "arm64"  # Cheaper and cargo-lambda handles it

[secrets]
enabled = true
prefix = "my-mcp-server"

# Enable if connecting to RDS/private resources
[vpc]
enabled = false
```

## Step 3: Optimize Binary Size

Add to your `Cargo.toml`:

```toml
[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Link-time optimization
strip = true        # Strip debug symbols
codegen-units = 1   # Better optimization
```

## Step 4: Deploy

```bash
# Build and deploy
cargo pmcp deploy

# Get the endpoint URL
cargo pmcp deploy outputs
```

## Step 5: Verify

```bash
# Test the deployed endpoint
curl -X POST https://your-endpoint.execute-api.region.amazonaws.com/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}'
```

::: hints
level_1: "Run 'aws sts get-caller-identity' to verify AWS credentials are configured correctly."
level_2: "If you get timeout errors, increase timeout_seconds in deploy.toml to 60."
level_3: "Use lazy initialization with OnceCell for database connections to reduce cold start time."
:::

## Success Criteria

- [ ] cargo pmcp deploy init creates .pmcp/ directory
- [ ] deploy.toml configured with appropriate memory and timeout
- [ ] Cargo.toml has release profile optimizations
- [ ] cargo pmcp deploy completes without errors
- [ ] curl test to /mcp endpoint returns valid MCP response
- [ ] CloudWatch logs show successful invocation

---

*This exercise connects to [Remote Testing](../../part4-testing/ch12-remote-testing.md) for testing deployed servers.*
