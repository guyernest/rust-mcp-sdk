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
- **Multi-Target Deployment**: Deploy to AWS Lambda, Google Cloud Run, or Cloudflare Workers with one command
- **WASM Support**: Automatic WASM compilation for edge deployments
- **Infrastructure as Code**: CDK-based AWS deployment with complete stack management
- **Deployment Management**: Logs, metrics, secrets, rollback, and destroy capabilities
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
- `deploy secrets set KEY --from-env ENV_VAR` - Manage secrets
- `deploy test --verbose` - Test the deployment
- `deploy outputs --format json` - Show deployment outputs

### OAuth Authentication

Enable OAuth 2.0 authentication for your MCP server with AWS Cognito. MCP clients (like Claude Desktop, ChatGPT, Cursor) automatically discover and use OAuth via the standard OpenID Connect discovery endpoint.

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

**View Registered Clients:**
```bash
cargo pmcp oauth clients
```

**Test with OAuth:**
```bash
# Opens browser for authentication, then runs tests
cargo pmcp test --server myserver
```

For detailed OAuth architecture and configuration, see [docs/oauth-design.md](docs/oauth-design.md).

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
8. **Deploy with OAuth**:
   - AWS Lambda: `cargo pmcp deploy init --target aws-lambda --oauth cognito && cargo pmcp deploy`
   - Google Cloud Run: `cargo pmcp deploy init --target google-cloud-run && cargo pmcp deploy`
   - Cloudflare Workers: `cargo pmcp deploy init --target cloudflare-workers && cargo pmcp deploy`
9. **Monitor**: `cargo pmcp deploy logs --tail` and `cargo pmcp deploy metrics`
10. **Iterate**: Make changes and repeat from step 4

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
