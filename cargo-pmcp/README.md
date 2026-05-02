# cargo-pmcp

Production-grade MCP server development toolkit.

## Overview

`cargo-pmcp` is a comprehensive scaffolding and development tool for building Model Context Protocol (MCP) servers using the PMCP SDK. It streamlines the entire lifecycle from project creation to production deployment.

## Features

- **Project Scaffolding** - Create workspaces and add servers with best-practice templates
- **Development Mode** - Build and run servers with HTTP transport and live logs
- **Client Connection** - One-command setup for Claude Code, Cursor, and MCP Inspector
- **Automated Testing** - Generate and run scenario-based tests from server capabilities
- **Load Testing** - Stress-test with concurrent virtual users, latency histograms, and CI/CD reports
- **Security Pentesting** - 32 attack checks across 7 categories with SARIF output for GitHub Security tab
- **Schema Management** - Export, validate, and diff schemas from live MCP servers
- **Workflow Validation** - Catch structural errors in workflows before runtime
- **MCP Apps** - Scaffold widget projects, generate ChatGPT manifests, and build landing pages
- **Widget Preview** - Browser-based preview with dual proxy/WASM bridge modes and hot-reload
- **Multi-Target Deployment** - Deploy to AWS Lambda, Google Cloud Run, Cloudflare Workers, or pmcp.run
- **Declarative IAM** - Declare AWS permissions (DynamoDB, S3, SecretsManager, …) in `.pmcp/deploy.toml` via the `[iam]` section. Translated into `mcpFunction.addToRolePolicy(...)` calls in the generated CDK stack at deploy time. See [docs/IAM.md](./docs/IAM.md) for the how-to guide, or [DEPLOYMENT.md § IAM Declarations](./DEPLOYMENT.md#iam-declarations-iam-section) for the schema reference.
- **Secrets Management** - Multi-provider secret storage (local, pmcp.run, AWS Secrets Manager)
- **OAuth Authentication** - Production-ready OAuth 2.0 with AWS Cognito, Dynamic Client Registration, and SSO
- **Landing Pages** - Create, develop, and deploy landing pages for server discovery
- **Workspace Diagnostics** - Validate project structure, toolchain, and server connectivity

## Installation

```bash
cargo install cargo-pmcp
```

## End-to-End Example

Walk through the full lifecycle using the `complete` template calculator server.

### 1. Create workspace and add a server

```bash
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace
cargo pmcp add server calculator --template complete
```

### 2. Start the dev server

```bash
cargo pmcp dev --server calculator
```

Server starts on `http://0.0.0.0:3000` with live logs.

### 3. Connect to Claude Code

In another terminal:

```bash
cargo pmcp connect --server calculator --client claude-code
```

Now ask Claude: *"Multiply 7 and 8"* or *"Solve x^2 - 5x + 6 = 0"*.

### 4. Generate and run tests

```bash
# Generate test scenarios from server capabilities
cargo pmcp test generate --server calculator

# Run the tests
cargo pmcp test run --server calculator --detailed
```

### 5. Load test

```bash
# Generate starter config with schema discovery
cargo pmcp loadtest init https://my-server.example.com

# Run load test
cargo pmcp loadtest run https://my-server.example.com --vus 20 --duration 60
```

### 6. Security pentest

```bash
# Quick scan — MCP-specific checks (prompt injection, tool poisoning, session security)
cargo pmcp pentest http://localhost:3000

# Deep scan — all 7 categories including transport, auth, exfiltration, protocol abuse
cargo pmcp pentest http://localhost:3000 --profile deep

# SARIF output for GitHub Security tab
cargo pmcp pentest http://localhost:3000 --profile deep --format sarif -o results.sarif

# Filter to specific categories
cargo pmcp pentest http://localhost:3000 --category pi,tp,ss

# CI gate — fail on medium or higher findings
cargo pmcp pentest http://localhost:3000 --fail-on medium
```

### 7. Deploy

```bash
# Initialize for AWS Lambda with OAuth
cargo pmcp deploy init --target aws-lambda --oauth cognito

# Deploy
cargo pmcp deploy --target aws-lambda
```

### 8. Monitor

```bash
cargo pmcp deploy logs --tail
cargo pmcp deploy metrics --period 24h
cargo pmcp deploy test --verbose
```

## Commands

| Command | Description | Reference |
|---------|-------------|-----------|
| `new` | Create a new MCP workspace | [docs/commands/new.md](docs/commands/new.md) |
| `add` | Add server, tool, or workflow to workspace | [docs/commands/add.md](docs/commands/add.md) |
| `dev` | Start development server with HTTP transport | [docs/commands/dev.md](docs/commands/dev.md) |
| `connect` | Connect server to Claude Code, Cursor, or Inspector | [docs/commands/connect.md](docs/commands/connect.md) |
| `test` | Run, generate, upload, and download test scenarios | [docs/commands/test.md](docs/commands/test.md) |
| `loadtest` | Load test with virtual users and performance reports | [docs/commands/loadtest.md](docs/commands/loadtest.md) |
| `pentest` | Security penetration testing with 32 checks across 7 categories | [src/pentest/README.md](src/pentest/README.md) |
| `doctor` | Workspace diagnostics — toolchain, dependencies, connectivity | |
| `schema` | Export, validate, and diff MCP server schemas | [docs/commands/schema.md](docs/commands/schema.md) |
| `validate` | Validate workflows and server components | [docs/commands/validate.md](docs/commands/validate.md) |
| `deploy` | Deploy to AWS Lambda, Cloud Run, Workers, pmcp.run | [docs/commands/deploy.md](docs/commands/deploy.md) |
| `secret` | Manage secrets across local, pmcp.run, and AWS | [docs/commands/secret.md](docs/commands/secret.md) |
| `app` | Scaffold MCP Apps projects with widgets | [docs/commands/app.md](docs/commands/app.md) |
| `preview` | Browser-based widget preview with hot-reload | [docs/commands/preview.md](docs/commands/preview.md) |
| `landing` | Create and deploy server landing pages | [docs/commands/landing.md](docs/commands/landing.md) |

## App Validation

`cargo pmcp test apps URL` validates MCP App metadata on a running server. It cross-references tools that declare `ui.resourceUri` against the resources they reference, validates MIME types, and (in strict modes) statically inspects the widget HTML for required protocol handler wiring.

### Modes

| Mode | Severity | Description |
|------|----------|-------------|
| `standard` (default) | Warning | Permissive — emits ONE summary Warning row per widget (MCP Apps is optional in the spec). |
| `chatgpt` | Error | Strict for ChatGPT compatibility (checks `openai/*` `_meta` keys). **For widget validation specifically, this mode is a no-op** (no widget-related rows emitted; preserves prior behavior). |
| `claude-desktop` | Error | Strict for Claude Desktop / Claude.ai. Statically inspects each widget HTML body fetched via `resources/read` for the `@modelcontextprotocol/ext-apps` import, the `new App({...})` constructor, the four required protocol handlers (`onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`), and the `app.connect()` call. Missing signals emit one Error row each. Honors `--tool` to restrict the check to a single tool's widget. |

```bash
# Default (permissive) mode
cargo pmcp test apps http://localhost:3000

# ChatGPT compatibility mode
cargo pmcp test apps http://localhost:3000 --mode chatgpt

# Claude Desktop / Claude.ai pre-deploy gate (strict static widget inspection)
cargo pmcp test apps http://localhost:3000 --mode claude-desktop

# Restrict the strict check to a single tool's widget via --tool
cargo pmcp test apps http://localhost:3000 --mode claude-desktop --tool open_dashboard
```

### Why --mode claude-desktop

Claude Desktop and Claude.ai silently tear down the entire MCP connection when a widget is missing required protocol handlers — the widget appears for a moment, then everything dies with no actionable error. `--mode claude-desktop` catches this class of failure pre-deploy by statically inspecting the widget HTML body for the SDK import, the App constructor, all four required handlers, and the `connect()` call. Missing signals are emitted as Error rows that link to the relevant section of the MCP Apps guide:

[src/server/mcp_apps/GUIDE.md#handlers-before-connect](https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect).

### MIME profile (`;profile=mcp-app`)

A common Claude Desktop failure point is omitting the `;profile=mcp-app` MIME parameter on widget resources. Claude Desktop uses this MIME parameter to identify resources that should be rendered as MCP App widgets vs plain HTML resources. Verify your server registers widget resources with `mime_type: "text/html;profile=mcp-app"` (or equivalent — `UIResource::html_mcp_app()` and `UIResourceContents::html()` both produce this MIME type by default).

### Vite singlefile minification

If your widget bundle is produced by Vite singlefile in production, the validator's regex strategy is designed to survive that minifier; if you encounter false-negatives, file an issue with a bundled HTML for empirical analysis.

> Note: The legacy widget examples under `examples/mcp-apps-chess/`, `examples/mcp-apps-dataviz/`, and `examples/mcp-apps-map/` use the older postMessage channel and will fail `--mode claude-desktop`. For a Claude-Desktop-ready widget, see `crates/mcp-tester/examples/validate_widget_pair.rs` and the corrected fixture at `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html`.

## Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output for debugging |
| `--no-color` | | Suppress colored output (also respects `NO_COLOR` env and non-TTY) |
| `--quiet` | | Suppress all non-error output (verbose wins if both are set) |

## OAuth Authentication

Enable OAuth 2.0 for your MCP server with zero code changes. Supports AWS Cognito, Microsoft Entra ID, Google, Okta, and Auth0.

**Infrastructure setup:**
```bash
cargo pmcp deploy init --target aws-lambda --oauth cognito
```

**Server-side (provider-agnostic):**
```rust
use pmcp::server::auth::AuthContext;

fn handle_tool_call(auth: &AuthContext) -> Result<Value, Error> {
    auth.require_auth()?;
    auth.require_scope("read:data")?;
    let user_id = auth.user_id();
    Ok(json!({ "user": user_id }))
}
```

**Switch providers via config only (no code changes):**
```toml
[profile.production.auth]
type = "jwt"
issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
audience = "your-app-client-id"
```

For detailed OAuth architecture, see [docs/oauth-design.md](docs/oauth-design.md) and [docs/oauth-sdk-design.md](docs/oauth-sdk-design.md).

## Secrets Management

Secrets are environment variables that your MCP server needs at runtime. They are resolved at deploy time from `.env` files and shell environment variables, then injected into the deployment target.

### Local Development

Create a `.env` file in your project root with `KEY=VALUE` pairs:

```text
ANTHROPIC_API_KEY=sk-ant-...
DATABASE_URL=postgresql://localhost/mydb
ANALYTICS_KEY=ua-12345
```

`cargo pmcp dev <server>` automatically loads `.env` into the server process. Shell environment variables take precedence over `.env` values when both define the same key.

### Declaring Secrets

Declare required secrets in `pmcp.toml` so the deploy pipeline knows what to resolve:

```toml
[[secrets.definitions]]
name = "ANTHROPIC_API_KEY"
description = "Anthropic API key for LLM calls"
required = true
env_var = "ANTHROPIC_API_KEY"
obtain_url = "https://console.anthropic.com/settings/keys"
```

### Deployment Integration

`cargo pmcp deploy` resolves secrets from your environment and `.env` file, then reports which are found and which are missing:

- **AWS Lambda:** Resolved secrets are injected as Lambda environment variables. Missing secrets produce a warning but do not block deployment.
- **pmcp.run:** The CLI performs a diagnostic check only -- secrets are never sent from your machine. For missing secrets, it shows the exact `cargo pmcp secret set` command to store each secret in pmcp.run's managed Secrets Manager. Actual env var injection happens server-side.

Missing secrets are warnings, not deployment blockers.

### Runtime Access

Server code reads secrets via the `pmcp::secrets` module:

```rust
use pmcp::secrets;

// Optional secret
if let Some(key) = secrets::get("ANALYTICS_KEY") {
    configure_analytics(&key);
}

// Required secret (returns actionable error if missing)
let api_key = secrets::require("ANTHROPIC_API_KEY")?;
```

The `require()` function returns an error message that includes the exact `cargo pmcp secret set` command to fix it.

### Secret Providers

| Provider | Storage | Commands |
|----------|---------|----------|
| Local | File-based at `.pmcp/secrets/` | `cargo pmcp secret list`, `set`, `get` |
| pmcp.run | Managed Secrets Manager | `cargo pmcp secret set --target pmcp` |

## CI/CD Integration

`cargo-pmcp` supports OAuth 2.0 client credentials flow for automated deployments.

```bash
export PMCP_CLIENT_ID="your-client-id"
export PMCP_CLIENT_SECRET="your-client-secret"
cargo pmcp deploy --target pmcp-run
```

### GitHub Actions

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
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-pmcp
      - name: Deploy
        env:
          PMCP_CLIENT_ID: ${{ secrets.PMCP_CLIENT_ID }}
          PMCP_CLIENT_SECRET: ${{ secrets.PMCP_CLIENT_SECRET }}
        run: cargo pmcp deploy --target pmcp-run
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `PMCP_CLIENT_ID` | Cognito App Client ID (for client_credentials flow) |
| `PMCP_CLIENT_SECRET` | Cognito App Client Secret |
| `PMCP_ACCESS_TOKEN` | Direct access token (alternative to client credentials) |
| `PMCP_ID_TOKEN` | Optional ID token (when using direct access token) |

## Requirements

- Rust 1.70 or later

## License

MIT

## Contributing

See the main PMCP SDK repository for contributing guidelines.
