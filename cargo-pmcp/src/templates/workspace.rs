//! Workspace template generator

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate workspace files (Cargo.toml, Makefile, README.md)
pub fn generate(workspace_dir: &Path, name: &str) -> Result<()> {
    generate_cargo_toml(workspace_dir, name)?;
    generate_makefile(workspace_dir, name)?;
    generate_readme(workspace_dir, name)?;
    generate_gitignore(workspace_dir)?;

    println!("  {} Generated workspace files", "✓".green());
    Ok(())
}

fn generate_cargo_toml(workspace_dir: &Path, _name: &str) -> Result<()> {
    let content = r#"[workspace]
resolver = "2"
members = [
    "crates/server-common",
    # Add server crates here via: cargo pmcp add server <name>
]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
authors = ["Your Name <you@example.com>"]

[workspace.dependencies]
# MCP SDK (using local path for development - change to git for production)
pmcp = { path = "/Users/guy/Development/mcp/sdk/rust-mcp-sdk", features = ["streamable-http", "schema-generation"] }

# HTTP transport
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["trace", "cors"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = { version = "1.0", features = ["preserve_order"] }

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
async-trait = "0.1"

# Error handling
anyhow = "1"
thiserror = "1"

# Validation
validator = { version = "0.18", features = ["derive"] }

[profile.release]
opt-level = "z"     # Optimize for size
lto = true          # Enable link-time optimization
codegen-units = 1   # Better optimization
strip = true        # Strip symbols for smaller binaries
"#;

    fs::write(workspace_dir.join("Cargo.toml"), content).context("Failed to create Cargo.toml")?;

    Ok(())
}

fn generate_makefile(workspace_dir: &Path, _name: &str) -> Result<()> {
    let content = r#".PHONY: help build test quality-gate dev deploy clean

help:
	@echo "Available commands:"
	@echo "  make build         - Build all servers"
	@echo "  make test          - Run all tests"
	@echo "  make quality-gate  - Run format, clippy, and tests"
	@echo "  make dev           - Start development server with hot reload"
	@echo "  make deploy        - Deploy to production"
	@echo "  make clean         - Clean build artifacts"

build:
	cargo build --release

test:
	cargo test --all-features

quality-gate:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test --all-features

dev:
	@echo "Starting development server..."
	@echo "Use: cargo pmcp dev --server <name>"

deploy:
	@echo "Deploying to production..."
	@echo "Use: cargo pmcp deploy --server <name> --target lambda"

clean:
	cargo clean
"#;

    fs::write(workspace_dir.join("Makefile"), content).context("Failed to create Makefile")?;

    Ok(())
}

fn generate_readme(workspace_dir: &Path, name: &str) -> Result<()> {
    let content = format!(
        r#"# {}

Production MCP workspace built with [PMCP SDK](https://github.com/paiml/rust-mcp-sdk).

## Quick Start

### Prerequisites

Install mcp-tester for automated testing:
```bash
cargo install mcp-tester
```

### Development

```bash
# Add your first server
cargo-pmcp add server calculator --template minimal

# Generate and run tests
cargo-pmcp test --server calculator --generate-scenarios

# Start development server
cargo run --bin calculator-server

# Run quality checks
make quality-gate
```

## Project Structure

```
{}/
├── crates/
│   ├── server-common/     # Shared HTTP bootstrap (80 LOC)
│   ├── mcp-calculator-core/  # Calculator business logic
│   └── calculator-server/    # Calculator binary (6 LOC)
├── scenarios/             # Test scenarios (YAML)
├── lambda/                # Lambda deployment configs
├── Cargo.toml             # Workspace manifest
└── Makefile               # Build/test/deploy commands
```

## Development Workflow

### 1. Add a new server
```bash
cargo-pmcp add server myserver --template minimal
```

### 2. Generate and run tests
```bash
# Generate test scenarios using mcp-tester (requires: cargo install mcp-tester)
cargo-pmcp test --server myserver --generate-scenarios

# Or just run existing scenarios
cargo-pmcp test --server myserver
```

### 3. Run quality checks
```bash
make quality-gate  # fmt + clippy + tests
```

### 4. Build and run
```bash
# Development
cargo run --bin myserver-server

# Production
cargo build --release --bin myserver-server
```

## Server Pattern

Each server has two crates:

- **mcp-{name}-core** (library): Business logic, tools, resources, workflows
- **{name}-server** (binary): Just 6 lines calling `server_common::run_http()`

This pattern:
- Shares HTTP bootstrap across all servers (DRY)
- Makes binaries trivial (easy to audit)
- Enables unit testing without HTTP complexity
- Scales from 1 to 100 servers

## Configuration

Servers use environment variables:

```bash
RUST_LOG=info              # Logging level
MCP_HTTP_PORT=3000         # Port (or PORT)
MCP_ALLOWED_ORIGINS=*      # CORS origins
```

## Testing

### Automated Testing with mcp-tester

Install mcp-tester:
```bash
cargo install mcp-tester
```

Generate test scenarios automatically:
```bash
cargo-pmcp test --server calculator --generate-scenarios
```

This will:
1. Build your server
2. Start it temporarily
3. Use mcp-tester to discover all tools and capabilities
4. Generate comprehensive test scenarios in `scenarios/calculator/generated.yaml`
5. Run the scenarios and show results

### Manual Testing

Run unit tests:
```bash
cargo test -p mcp-calculator-core
```

Start server and test with curl:
```bash
cargo run --bin calculator-server &
curl -X POST http://0.0.0.0:3000 \\
  -H "Content-Type: application/json" \\
  -H "Accept: application/json" \\
  -d '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\",\"params\":{{}}}}'
```

## Quality Standards

- **Zero tolerance for defects**: All commits pass quality gates
- **80%+ test coverage**: Property tests + unit tests + integration tests
- **Type safety**: schemars JsonSchema with validation
- **Production middleware**: Client tracking, redaction, request IDs

## Resources

- [PMCP SDK Documentation](https://github.com/paiml/rust-mcp-sdk)
- [MCP Specification](https://spec.modelcontextprotocol.io)
- [Example Servers](https://github.com/paiml/rust-mcp-sdk/tree/main/examples)

## License

MIT OR Apache-2.0
"#,
        name, name
    );

    fs::write(workspace_dir.join("README.md"), content).context("Failed to create README.md")?;

    Ok(())
}

fn generate_gitignore(workspace_dir: &Path) -> Result<()> {
    let content = r#"# Rust
target/
Cargo.lock

# IDE
.idea/
.vscode/
*.swp
*.swo

# OS
.DS_Store
Thumbs.db

# Environment
.env
.env.local

# Logs
*.log

# CDK
lambda/cdk.out/
lambda/.cdk.staging/
lambda/node_modules/
"#;

    fs::write(workspace_dir.join(".gitignore"), content).context("Failed to create .gitignore")?;

    Ok(())
}
