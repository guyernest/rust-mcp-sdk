# cargo-pmcp

Production-grade MCP server development toolkit.

## Overview

`cargo-pmcp` is a comprehensive scaffolding and testing tool for building Model Context Protocol (MCP) servers using the PMCP SDK. It streamlines the entire development workflow from project creation to automated testing.

## Features

- **Project Scaffolding**: Create new MCP server workspaces with best practices built-in
- **Server Management**: Add multiple MCP servers to a single workspace
- **Development Mode**: Hot-reload MCP servers with HTTP transport for rapid development
- **Automated Testing**: Generate and run comprehensive test scenarios for your MCP servers
- **Smart Test Generation**: Automatically creates meaningful test cases with realistic values
- **Load Testing**: Stress-test deployed MCP servers with concurrent virtual users, live progress, and detailed performance reports
- **MCP Apps**: Scaffold interactive widget projects with `cargo pmcp app new`, generate ChatGPT manifests, and build landing pages
- **Widget Preview**: Browser-based preview environment with dual proxy/WASM bridge modes and hot-reload
- **Multi-Target Deployment**: Deploy to AWS Lambda, Google Cloud Run, or Cloudflare Workers with one command
- **Secrets Management**: Multi-provider secret storage (local, pmcp.run, AWS Secrets Manager)
- **WASM Support**: Automatic WASM compilation for edge deployments
- **Infrastructure as Code**: CDK-based AWS deployment with complete stack management
- **Deployment Management**: Logs, metrics, rollback, and destroy capabilities
- **OAuth Authentication**: Production-ready OAuth 2.0 with AWS Cognito, supporting Dynamic Client Registration (DCR)

## Installation

```bash
cargo install cargo-pmcp
```

## Quick Start

### 1. Create a New Workspace

```bash
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace
```

### 2. Add an MCP Server

```bash
cargo pmcp add calculator --tools --resources
```

This creates:
- A new MCP server with example tools and resources
- A `scenarios/calculator/` directory for test scenarios
- Template code ready for customization

### 3. Develop Your Server

```bash
cargo pmcp dev --server calculator
```

This starts the server with hot-reload enabled on `http://0.0.0.0:3000`.

### 4. Test Your Server

Generate test scenarios:

```bash
# In another terminal, with server running
cargo pmcp test --server calculator --generate-scenarios
```

Run tests:

```bash
cargo pmcp test --server calculator
```

## Commands

### `new <name>`

Create a new MCP server workspace.

**Options:**
- `--description` - Optional workspace description

**Example:**
```bash
cargo pmcp new my-workspace --description "My MCP servers"
```

### `add <name>`

Add a new MCP server to the current workspace.

**Options:**
- `--tools` - Include example tool implementations
- `--resources` - Include example resource implementations
- `--prompts` - Include example prompt implementations

**Example:**
```bash
cargo pmcp add calculator --tools --resources
```

### `dev --server <name>`

Start an MCP server in development mode with HTTP transport.

**Options:**
- `--server` - Name of the server to run
- `--port` - Port to listen on (default: 3000)

**Example:**
```bash
cargo pmcp dev --server calculator --port 8080
```

### `test --server <name>`

Test an MCP server using scenario-based testing.

**Prerequisites:**
- Server must be running in another terminal (use `cargo pmcp dev --server <name>`)

**Options:**
- `--server` - Name of the server to test
- `--port` - Port the server is running on (default: 3000)
- `--generate-scenarios` - Generate test scenarios from server schema
- `--detailed` - Show detailed test output

**Example:**
```bash
# Terminal 1: Start server
cargo pmcp dev --server calculator

# Terminal 2: Generate and run tests
cargo pmcp test --server calculator --generate-scenarios
cargo pmcp test --server calculator --detailed
```

### `loadtest`

Load test a deployed MCP server with concurrent virtual users. Measures latency percentiles, throughput, error rates, and can auto-detect breaking points.

**Subcommands:**
- `loadtest run <URL>` - Run a load test against a deployed MCP server
- `loadtest init [URL]` - Generate a starter `loadtest.toml` config (optionally discovers server schema)

**Options (run):**
- `--config` - Path to loadtest config file (default: auto-discovers `.pmcp/loadtest.toml`)
- `--vus` - Override number of virtual users
- `--duration` - Override test duration (seconds)
- `--iterations` - Override iteration count
- `--no-color` - Disable colored output
- `--no-report` - Skip JSON report file generation

**Example:**
```bash
# Generate starter config with server schema discovery
cargo pmcp loadtest init https://my-server.example.com

# Run load test (uses .pmcp/loadtest.toml)
cargo pmcp loadtest run https://my-server.example.com

# Quick test with CLI overrides
cargo pmcp loadtest run https://my-server.example.com --vus 20 --duration 60
```

**Features:**
- TOML-based scenario config with weighted MCP operation mix (tools/call, resources/read, prompts/get)
- HdrHistogram latency percentiles with coordinated omission correction
- k6-style live terminal progress and colorized summary report
- Stage-driven load shaping with ramp-up/hold/ramp-down phases
- Automatic breaking point detection
- Per-tool metrics breakdown
- Schema-versioned JSON reports for CI/CD pipelines

### `app`

Scaffold and manage MCP Apps projects with interactive widgets.

**Subcommands:**
- `app new <name>` - Create a new MCP Apps project with widget scaffolding
- `app manifest --url <URL>` - Generate a ChatGPT-compatible action manifest
- `app landing` - Generate a standalone demo landing page
- `app build --url <URL>` - Generate both manifest and landing page

**Options (new):**
- `--path` - Directory to create project in (defaults to current directory)

**Options (manifest):**
- `--url` - Server URL (required)
- `--logo` - Logo URL
- `--output` - Output directory (default: `dist`)

**Options (landing):**
- `--widget` - Widget to showcase (defaults to first alphabetically)
- `--output` - Output directory (default: `dist`)

**Example:**
```bash
# Create a new MCP Apps project
cargo pmcp app new my-widget-app
cd my-widget-app

# Develop with hot-reload preview (see `preview` command)
cargo run &
cargo pmcp preview --url http://localhost:3000 --open

# Build for production
cargo pmcp app build --url https://my-server.example.com
```

### `preview`

Launch a browser-based preview environment for testing MCP server widgets. Simulates the ChatGPT Apps runtime with dual proxy/WASM bridge modes.

**Options:**
- `--url` - URL of the running MCP server (required)
- `--port` - Port for the preview server (default: 8765)
- `--open` - Open browser automatically
- `--tool` - Auto-select this tool on start
- `--theme` - Initial theme: `light` or `dark` (default: light)
- `--locale` - Initial locale (default: en-US)
- `--widgets-dir` - Path to widgets directory for file-based authoring with hot-reload

**Example:**
```bash
# Basic preview with auto-open
cargo pmcp preview --url http://localhost:3000 --open

# File-based widget authoring with hot-reload
cargo pmcp preview --url http://localhost:3000 --widgets-dir ./widgets --open

# Dark theme, auto-select a specific tool
cargo pmcp preview --url http://localhost:3000 --theme dark --tool chess_board
```

### `deploy`

Deploy your MCP server to production environments.

**Supported Targets:**
- `aws-lambda` - Deploy to AWS Lambda with API Gateway
- `google-cloud-run` - Deploy to Google Cloud Run serverless containers
- `cloudflare-workers` - Deploy to Cloudflare Workers edge network

**Subcommands:**
- `deploy init` - Initialize deployment configuration
- `deploy` - Build and deploy to the configured target
- `deploy logs` - View deployment logs
- `deploy destroy` - Remove deployment (with optional --clean)

**AWS Lambda Example:**
```bash
# Initialize (one-time setup)
cargo pmcp deploy init --target aws-lambda --region us-east-1

# Deploy
cargo pmcp deploy --target aws-lambda

# View logs
cargo pmcp deploy logs --tail --target aws-lambda

# Destroy
cargo pmcp deploy destroy --target aws-lambda --clean
```

**Cloudflare Workers Example:**
```bash
# Initialize (one-time setup)
cargo pmcp deploy init --target cloudflare-workers

# Deploy
cargo pmcp deploy --target cloudflare-workers

# View logs
cargo pmcp deploy logs --tail --target cloudflare-workers

# Destroy
cargo pmcp deploy destroy --target cloudflare-workers --clean
```

**Google Cloud Run Example:**
```bash
# Prerequisites: gcloud CLI installed and authenticated
# gcloud auth login
# gcloud config set project PROJECT_ID

# Initialize (one-time setup - generates Dockerfile)
cargo pmcp deploy init --target google-cloud-run

# Deploy (builds Docker image and deploys to Cloud Run)
cargo pmcp deploy --target google-cloud-run

# View logs
cargo pmcp deploy logs --tail --target google-cloud-run

# Destroy
cargo pmcp deploy destroy --target google-cloud-run --clean
```

**Configuration Options (Google Cloud Run):**
- `CLOUD_RUN_REGION` - Deployment region (default: us-central1)
- `CLOUD_RUN_MEMORY` - Memory limit (default: 512Mi)
- `CLOUD_RUN_CPU` - CPU allocation (default: 1)
- `CLOUD_RUN_MAX_INSTANCES` - Max instances (default: 10)
- `CLOUD_RUN_ALLOW_UNAUTHENTICATED` - Allow public access (default: true)

**Additional Commands:**
- `deploy metrics --period 24h` - View deployment metrics
- `deploy test --verbose` - Test the deployment
- `deploy outputs --format json` - Show deployment outputs

### `secret`

Manage secrets for MCP servers across multiple providers.

**Supported Providers:**
- `local` - Local filesystem storage for development (default)
- `pmcp` - pmcp.run managed platform for production
- `aws` - AWS Secrets Manager for self-hosted deployments

**Secret Naming Convention:**
Secrets are namespaced by server ID to avoid conflicts:
```
{server-id}/{SECRET_NAME}

Examples:
  chess/ANTHROPIC_API_KEY
  london-tube/TFL_APP_KEY
  my-api/DATABASE_URL
```

**Subcommands:**

```bash
# List secrets for a server
cargo pmcp secret list --server myserver

# Set a secret interactively (recommended - hidden input)
cargo pmcp secret set myserver/API_KEY --prompt

# Set from environment variable
cargo pmcp secret set myserver/API_KEY --env MY_API_KEY

# Set from file
cargo pmcp secret set myserver/API_KEY --file ./secret.txt

# Generate a random secret
cargo pmcp secret set myserver/SESSION_SECRET --generate --length 64

# Get a secret value
cargo pmcp secret get myserver/API_KEY

# Delete a secret
cargo pmcp secret delete myserver/API_KEY

# Show provider status
cargo pmcp secret providers
```

**Target Selection:**
```bash
# Explicit target
cargo pmcp secret list --server myserver --target pmcp
cargo pmcp secret list --server myserver --target local
cargo pmcp secret list --server myserver --target aws

# Auto-detection (checks pmcp.run auth, then AWS, then local)
cargo pmcp secret list --server myserver
```

**Verbose Mode:**
```bash
# Enable verbose output for debugging
cargo pmcp -v secret set myserver/KEY --prompt --target pmcp
```

**Security Features:**
- Secret values use `secrecy` crate with automatic memory zeroization
- Local secrets stored with file permissions 0600
- Debug/Display output shows `[REDACTED]` instead of actual values
- Warns when outputting secrets to terminal

### OAuth Authentication

Enable OAuth 2.0 authentication for your MCP server. Supports multiple providers: AWS Cognito, Microsoft Entra ID, Google, Okta, and Auth0.

#### Infrastructure Setup (cargo-pmcp)

**Initialize with OAuth:**
```bash
# Create deployment with OAuth enabled
cargo pmcp deploy init --target aws-lambda --oauth cognito

# This creates:
# - Cognito User Pool with optional social logins
# - OAuth Proxy Lambda (handles DCR, authorize, token endpoints)
# - Token Validator Lambda Authorizer (stateless JWT validation)
# - ClientRegistrationTable in DynamoDB
```

**OAuth Options:**
- `--oauth cognito` - Use AWS Cognito (recommended for AWS deployments)
- `--oauth oidc` - Use external OIDC provider (future)
- `--oauth shared:<name>` - Use organization's shared OAuth infrastructure

**How It Works:**
1. MCP clients discover OAuth endpoints via `/.well-known/openid-configuration`
2. Clients self-register using Dynamic Client Registration (RFC 7591)
3. Users authenticate via Cognito Hosted UI (supports social logins, MFA)
4. Clients send Bearer tokens on every MCP request
5. API Gateway validates tokens using Lambda Authorizer (stateless JWT)
6. Your MCP server code requires zero OAuth logic

#### Server-Side SDK Integration

Your MCP server code is **provider-agnostic**. It only interacts with `AuthContext`, never with OAuth providers directly:

```rust
use pmcp::server::auth::AuthContext;

fn handle_tool_call(auth: &AuthContext) -> Result<Value, Error> {
    // Require authentication
    auth.require_auth()?;

    // Check scopes
    auth.require_scope("read:data")?;

    // Access user info (works with any OAuth provider)
    let user_id = auth.user_id();
    let email = auth.email().unwrap_or("unknown");
    let tenant = auth.tenant_id();

    // Your business logic here
    Ok(json!({ "user": user_id }))
}
```

**Key `AuthContext` methods:**

| Method | Description |
|--------|-------------|
| `user_id()` | User identifier (from `sub` claim) |
| `email()` | Email (handles provider differences) |
| `tenant_id()` | Tenant ID (handles `tid`, `custom:tenant`, `org_id`) |
| `groups()` | Group membership |
| `require_auth()` | Error if not authenticated |
| `require_scope(s)` | Error if scope missing |

#### Configuration-Driven Validation

Configure token validation via `pmcp.toml` - **no code changes to switch providers**:

```toml
# Production: JWT validation against Cognito
[profile.production.auth]
type = "jwt"
issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
audience = "your-app-client-id"

# Development: Mock authentication
[profile.dev.auth]
type = "mock"
default_user_id = "dev-user"
default_scopes = ["read", "write", "admin"]
```

**Switch to Microsoft Entra ID (no code changes):**
```toml
[profile.production.auth]
type = "jwt"
issuer = "https://login.microsoftonline.com/tenant-id/v2.0"
audience = "api://my-api"
```

#### Developer Workflow

1. **Phase 1: Build** - Implement tools with no auth code
2. **Phase 2: Test** - Use `MockValidator` for local development
3. **Phase 3: Deploy** - Configure OAuth via `pmcp.toml`
4. **Phase 4: Switch** - Change providers via configuration only

**View Registered Clients:**
```bash
cargo pmcp oauth clients
```

**Test with OAuth:**
```bash
# Opens browser for authentication, then runs tests
cargo pmcp test --server myserver
```

For detailed OAuth architecture and SDK design, see:
- [docs/oauth-design.md](docs/oauth-design.md) - Infrastructure design
- [docs/oauth-sdk-design.md](docs/oauth-sdk-design.md) - SDK integration

### CI/CD Integration

For automated deployments in CI/CD pipelines (GitHub Actions, GitLab CI, AWS CodeBuild, etc.), `cargo-pmcp` supports OAuth 2.0 client credentials flow (machine-to-machine authentication).

#### Setup

1. **Create a Cognito App Client** with `client_credentials` grant enabled:
   - In AWS Console: Cognito → User Pools → App Clients → Create
   - Enable "Client credentials" under OAuth 2.0 grant types
   - Note the Client ID and generate a Client Secret

2. **Store credentials securely** in your CI/CD environment:
   - AWS CodeBuild: Use AWS Secrets Manager
   - GitHub Actions: Use encrypted secrets
   - GitLab CI: Use CI/CD variables (masked)

3. **Set environment variables** in your CI/CD job:
   ```bash
   export PMCP_CLIENT_ID="your-client-id"
   export PMCP_CLIENT_SECRET="your-client-secret"
   ```

#### Environment Variables

| Variable | Description |
|----------|-------------|
| `PMCP_CLIENT_ID` | Cognito App Client ID (for client_credentials flow) |
| `PMCP_CLIENT_SECRET` | Cognito App Client Secret |
| `PMCP_ACCESS_TOKEN` | Direct access token (alternative to client credentials) |
| `PMCP_ID_TOKEN` | Optional ID token (when using direct access token) |

#### Example: GitHub Actions

```yaml
name: Deploy MCP Server
on:
  push:
    branches: [main]

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Install cargo-pmcp
        run: cargo install cargo-pmcp

      - name: Deploy to pmcp.run
        env:
          PMCP_CLIENT_ID: ${{ secrets.PMCP_CLIENT_ID }}
          PMCP_CLIENT_SECRET: ${{ secrets.PMCP_CLIENT_SECRET }}
        run: cargo pmcp deploy --target pmcp-run
```

#### Example: AWS CodeBuild

```yaml
version: 0.2

env:
  secrets-manager:
    PMCP_CLIENT_ID: pmcp-build-credentials:client_id
    PMCP_CLIENT_SECRET: pmcp-build-credentials:client_secret

phases:
  install:
    commands:
      - cargo install cargo-pmcp
  build:
    commands:
      - cargo pmcp deploy --target pmcp-run
```

#### Example: GitLab CI

```yaml
deploy:
  stage: deploy
  image: rust:latest
  variables:
    PMCP_CLIENT_ID: $PMCP_CLIENT_ID
    PMCP_CLIENT_SECRET: $PMCP_CLIENT_SECRET
  script:
    - cargo install cargo-pmcp
    - cargo pmcp deploy --target pmcp-run
  only:
    - main
```

#### How It Works

1. When `PMCP_CLIENT_ID` and `PMCP_CLIENT_SECRET` are set, `cargo-pmcp` uses OAuth 2.0 client_credentials flow
2. It exchanges the credentials for an access token via the Cognito token endpoint
3. The access token is used to authenticate GraphQL API calls to pmcp.run
4. No interactive login or browser-based flow is required

This enables fully automated deployments without storing long-lived credentials or requiring human intervention.

## Test Scenarios

Test scenarios are YAML files that define test steps and assertions for your MCP server.

### Generating Scenarios

The `--generate-scenarios` flag discovers your server's capabilities and generates smart test cases:

```bash
cargo pmcp test --server calculator --generate-scenarios
```

This creates `scenarios/calculator/generated.yaml` with:
- Smart test values (e.g., `add(123, 234) = 357`)
- Realistic assertions
- Tool, resource, and prompt test coverage

### Scenario Format

```yaml
name: "Calculator Test Scenario"
description: "Test calculator operations"
timeout: 60
stop_on_failure: false

steps:
  - name: "Test addition"
    operation:
      type: tool_call
      tool: "add"
      arguments:
        a: 123
        b: 234
    assertions:
      - type: success
      - type: equals
        path: "result"
        value: 357
```

### MCP Response Format

MCP tool responses are wrapped in a `content` array. The actual result is in `content[0].text`:

```json
{
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"result\":357.0,\"operation\":\"123 + 234 = 357\"}"
    }]
  }
}
```

To assert on nested values, use JSON path notation or adjust the generated scenarios.

## Workflow

The typical development workflow:

1. **Create workspace**: `cargo pmcp new my-workspace`
2. **Add server**: `cargo pmcp add myserver --tools`
3. **Implement features**: Edit code in `crates/myserver/`
4. **Start dev server**: `cargo pmcp dev --server myserver`
5. **Generate tests**: `cargo pmcp test --server myserver --generate-scenarios`
6. **Customize tests**: Edit `scenarios/myserver/generated.yaml`
7. **Run tests**: `cargo pmcp test --server myserver`
8. **Load test**: `cargo pmcp loadtest init https://my-server.example.com && cargo pmcp loadtest run https://my-server.example.com`
9. **Add widgets**: `cargo pmcp app new my-widget-app` (or add `widgets/` directory to existing project)
10. **Preview widgets**: `cargo pmcp preview --url http://localhost:3000 --open`
11. **Build for production**: `cargo pmcp app build --url https://my-server.example.com`
12. **Deploy with OAuth**:
    - AWS Lambda: `cargo pmcp deploy init --target aws-lambda --oauth cognito && cargo pmcp deploy`
    - Google Cloud Run: `cargo pmcp deploy init --target google-cloud-run && cargo pmcp deploy`
    - Cloudflare Workers: `cargo pmcp deploy init --target cloudflare-workers && cargo pmcp deploy`
13. **Monitor**: `cargo pmcp deploy logs --tail` and `cargo pmcp deploy metrics`
14. **Iterate**: Make changes and repeat from step 4

## Architecture

- **Workspace**: Top-level Cargo workspace containing multiple MCP servers
- **Server**: Individual MCP server crate with its own capabilities
- **Scenarios**: YAML test definitions for each server
- **Templates**: Code generation templates for consistent server structure

## Requirements

- Rust 1.70 or later
- Cargo

## License

MIT

## Contributing

See the main PMCP SDK repository for contributing guidelines.
