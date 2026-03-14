# PMCP Server

An MCP server that exposes PMCP SDK capabilities as tools, resources, and prompts — enabling AI coding assistants to test, scaffold, and reference PMCP documentation through the MCP protocol.

## Install

### Pre-built binary (recommended)

Download the latest release for your platform:

```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/paiml/rust-mcp-sdk/releases/latest/download/pmcp-server-aarch64-apple-darwin -o pmcp-server
chmod +x pmcp-server

# macOS (Intel)
curl -fsSL https://github.com/paiml/rust-mcp-sdk/releases/latest/download/pmcp-server-x86_64-apple-darwin -o pmcp-server
chmod +x pmcp-server

# Linux (x86_64)
curl -fsSL https://github.com/paiml/rust-mcp-sdk/releases/latest/download/pmcp-server-x86_64-unknown-linux-gnu -o pmcp-server
chmod +x pmcp-server

# Linux (ARM64)
curl -fsSL https://github.com/paiml/rust-mcp-sdk/releases/latest/download/pmcp-server-aarch64-unknown-linux-gnu -o pmcp-server
chmod +x pmcp-server

# Windows (PowerShell)
Invoke-WebRequest -Uri https://github.com/paiml/rust-mcp-sdk/releases/latest/download/pmcp-server-x86_64-pc-windows-msvc.exe -OutFile pmcp-server.exe
```

### cargo install

```bash
cargo install pmcp-server
```

### cargo binstall

```bash
cargo binstall pmcp-server
```

### From source

```bash
git clone https://github.com/paiml/rust-mcp-sdk.git
cd rust-mcp-sdk
cargo build --release -p pmcp-server
```

## Usage

```bash
# Start on default port 8080
pmcp-server

# Custom port and host
pmcp-server --port 3000 --host 127.0.0.1

# With debug logging
RUST_LOG=debug pmcp-server
```

Environment variables: `PMCP_SERVER_PORT`, `PMCP_SERVER_HOST`.

## Configure with AI clients

### Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "pmcp": {
      "url": "http://localhost:8080/mcp"
    }
  }
}
```

### Claude Code

```bash
claude mcp add pmcp-server --transport http http://localhost:8080/mcp
```

## What's included

### Tools

| Tool | Description |
|------|-------------|
| `test_check` | Run MCP protocol compliance tests against a remote server |
| `test_generate` | Generate test scenarios from a server's discovered capabilities |
| `test_apps` | Validate MCP Apps metadata structure and cross-references |
| `scaffold` | Generate PMCP project templates (returns JSON, does not write files) |
| `schema_export` | Connect to a server and export tool/resource/prompt schemas as JSON or Rust types |

### Resources (9 documentation URIs)

| URI | Content |
|-----|---------|
| `pmcp://docs/typed-tools` | TypedTool, TypedSyncTool, and TypedToolWithOutput patterns |
| `pmcp://docs/resources` | ResourceHandler trait, URI patterns, and static content |
| `pmcp://docs/prompts` | PromptHandler trait, PromptInfo metadata, and workflow prompts |
| `pmcp://docs/auth` | OAuth, API key, and JWT middleware configuration |
| `pmcp://docs/middleware` | Tool and protocol middleware composition |
| `pmcp://docs/mcp-apps` | Widget UIs, _meta emission, and host integration |
| `pmcp://docs/error-handling` | Error variants, Result patterns, and propagation |
| `pmcp://docs/cli` | cargo-pmcp commands: init, test, preview, deploy |
| `pmcp://docs/best-practices` | Tool design, resource organization, testing, deployment |

### Prompts (7 guided workflows)

| Prompt | Description |
|--------|-------------|
| `quickstart` | Step-by-step guide to create your first PMCP server |
| `create-mcp-server` | Set up a new PMCP workspace with scaffold templates |
| `add-tool` | Add a new tool to an existing server |
| `diagnose` | Diagnostic steps for a running MCP server |
| `setup-auth` | Configure OAuth, API key, or JWT authentication |
| `debug-protocol-error` | Debug MCP protocol and JSON-RPC errors |
| `migrate` | Migrate from TypeScript MCP SDK to PMCP (Rust) |

## Deploy to the cloud

The PMCP server uses stateless streamable HTTP — pure HTTP POST/response with no WebSocket or long-lived connections — making it ideal for serverless deployment. Both options below operate well within their free tiers for low-traffic usage.

### AWS Lambda

Deploy as a native ARM64 binary behind API Gateway using CDK:

```bash
# Prerequisites
cargo install cargo-lambda
npm install -g aws-cdk

# Initialize and deploy
cargo pmcp deploy init --target aws-lambda --region us-east-1
cargo pmcp deploy --target aws-lambda
```

This compiles the server to `aarch64-unknown-linux-musl`, wraps it with a Lambda adapter that translates API Gateway events to HTTP, and provisions the stack via CloudFormation. The free tier includes 1M requests/month and 400K GB-seconds.

After deployment, get your endpoint:

```bash
cargo pmcp deploy outputs --target aws-lambda
```

Then configure your AI client to use the deployed URL instead of `localhost`.

### Google Cloud Run

Deploy as a container to Google's managed serverless platform:

```bash
# Prerequisites
# Install Docker: https://docs.docker.com/get-docker/
# Install gcloud: https://cloud.google.com/sdk/docs/install
gcloud auth login

# Initialize and deploy
cargo pmcp deploy init --target google-cloud-run
cargo pmcp deploy --target google-cloud-run
```

This builds a Docker image with the server binary, pushes it to Google Container Registry, and deploys to Cloud Run. The free tier includes 2M requests/month and 360K GB-seconds.

### Managing deployments

```bash
# View logs
cargo pmcp deploy logs --target aws-lambda --tail

# Run health check
cargo pmcp deploy test --target aws-lambda

# Tear down
cargo pmcp deploy destroy --target aws-lambda --clean
```

## License

MIT
