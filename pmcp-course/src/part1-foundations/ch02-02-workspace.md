# Building and Running

Now that you've seen the quick start, let's understand what `cargo pmcp` created and how to work with it effectively.

## The Workspace Structure

When you ran `cargo pmcp new my-mcp-servers`, it created a Cargo workspace:

```
my-mcp-servers/
├── Cargo.toml              # Workspace manifest
├── pmcp.toml               # PMCP configuration
├── server-common/          # Shared infrastructure code
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
└── servers/                # Your MCP servers live here
    └── calculator/
        ├── Cargo.toml
        └── src/
            ├── main.rs
            └── tools/
                ├── mod.rs
                └── calculator.rs
```

### Why a Workspace?

A Cargo workspace lets you manage multiple related packages together. For MCP development, this provides:

| Benefit | How It Helps |
|---------|--------------|
| **Shared dependencies** | All servers use the same versions of pmcp, serde, etc. |
| **Common code** | `server-common` is shared across all servers |
| **Single build** | `cargo build` compiles everything together |
| **Consistent tooling** | One `cargo fmt`, one `cargo clippy` for all |

As you build more MCP servers, they all go in the `servers/` directory and share the common infrastructure.

## The Workspace Manifest

The root `Cargo.toml` defines the workspace:

```toml
[workspace]
resolver = "2"
members = [
    "server-common",
    "servers/*",
]

[workspace.dependencies]
pmcp = "1.8"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
async-trait = "0.1"
```

**Key points:**

- `members` includes `server-common` and all packages under `servers/`
- `[workspace.dependencies]` defines shared dependency versions
- Individual packages inherit these with `dependency.workspace = true`

## The PMCP Configuration

The `pmcp.toml` file configures cargo-pmcp behavior:

```toml
[workspace]
name = "my-mcp-servers"
default_server = "calculator"

[servers.calculator]
package = "calculator"
port = 3000

[deploy]
default_target = "lambda"
```

This tells `cargo pmcp dev` which server to run by default and on which port.

## Server-Common: Shared Infrastructure

The `server-common` crate provides HTTP server bootstrap code that all your MCP servers share:

```rust
// server-common/src/lib.rs
use pmcp::server::streamable_http_server::{
    StreamableHttpServer, 
    StreamableHttpServerConfig
};
use pmcp::Server;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Start an HTTP server for the given MCP server
pub async fn serve_http(
    server: Server,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let server = Arc::new(Mutex::new(server));
    
    let config = StreamableHttpServerConfig {
        session_id_generator: None,   // Stateless mode
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: None,
    };
    
    let http_server = StreamableHttpServer::with_config(addr, server, config);
    let (bound_addr, handle) = http_server.start().await?;
    
    tracing::info!("MCP server listening on http://{}/mcp", bound_addr);
    
    handle.await?;
    Ok(())
}
```

By centralizing this code, you:
- Update HTTP handling once, all servers benefit
- Keep server code focused on business logic
- Ensure consistent configuration across servers

## Running Your Server

### Development Mode

Use `cargo pmcp dev` for local development:

```bash
# Run the default server (from pmcp.toml)
cargo pmcp dev

# Run a specific server
cargo pmcp dev calculator

# Run on a different port
cargo pmcp dev calculator --port 8080
```

Development mode includes:
- Hot reloading (rebuilds on file changes)
- Verbose logging
- Pretty-printed output

### Production Build

For production, build a release binary:

```bash
cargo build --release --package calculator
```

The binary is at `target/release/calculator` (~5-15MB, no runtime dependencies).

Run it directly:

```bash
./target/release/calculator
```

Or with environment configuration:

```bash
RUST_LOG=info PORT=3000 ./target/release/calculator
```

## Adding More Servers

Add a new server to your workspace:

```bash
cargo pmcp add server inventory --template basic
```

This creates `servers/inventory/` with the standard structure. Your workspace now has:

```
servers/
├── calculator/
└── inventory/
```

Both servers share `server-common` and workspace dependencies.

## Available Templates

`cargo pmcp add server` supports several templates:

| Template | Description |
|----------|-------------|
| `basic` | Minimal server with one example tool |
| `calculator` | Math operations with typed inputs/outputs |
| `database` | Database query patterns with connection pooling |
| `crud` | Create/Read/Update/Delete operations |
| `authenticated` | OAuth-protected server template |

Use `--template` to specify:

```bash
cargo pmcp add server users --template crud
cargo pmcp add server reports --template database
```

## Building All Servers

Build everything in the workspace:

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Check without building (faster)
cargo check
```

## Testing

Run tests across the workspace:

```bash
# All tests
cargo test

# Tests for a specific server
cargo test --package calculator

# With output
cargo test -- --nocapture
```

## Code Quality

The workspace supports standard Rust quality tools:

```bash
# Format all code
cargo fmt

# Lint all code
cargo clippy

# Both (recommended before commits)
cargo fmt && cargo clippy
```

## Summary

| Command | Purpose |
|---------|---------|
| `cargo pmcp new <name>` | Create a new workspace |
| `cargo pmcp add server <name>` | Add a server to the workspace |
| `cargo pmcp dev [server]` | Run in development mode |
| `cargo build --release` | Build for production |
| `cargo test` | Run all tests |
| `cargo fmt && cargo clippy` | Code quality checks |

---

*Next, let's look inside the calculator server to understand how tools are defined.*

*Continue to [The Calculator Server](./ch02-03-calculator.md) →*
