# Chapter 1: Installation & Setup

Getting started with PMCP is straightforward. This chapter will guide you through installing PMCP, setting up your development environment, and verifying everything works correctly.

## System Requirements

PMCP supports all major platforms:

- **Linux** (Ubuntu 20.04+, RHEL 8+, Arch Linux)
- **macOS** (10.15+)
- **Windows** (Windows 10+)
- **WebAssembly** (for browser environments)

**Minimum Requirements:**
- Rust 1.82+
- 2GB RAM
- 1GB disk space

## Installation Methods

### Method 1: Using Cargo (Recommended)

Add PMCP to your `Cargo.toml`:

```toml
[dependencies]
pmcp = "2.0"
```

Or use `cargo add`:

```bash
cargo add pmcp
```

### Method 2: From Source

Clone and build from source for the latest features:

```bash
git clone https://github.com/paiml/pmcp.git
cd pmcp
cargo build --release
```

### Method 3: Pre-built Binaries

Download pre-built binaries from the [releases page](https://github.com/paiml/pmcp/releases):

```bash
# Linux/macOS
curl -L https://github.com/paiml/pmcp/releases/latest/download/pmcp-linux.tar.gz | tar xz

# Windows PowerShell  
Invoke-WebRequest -Uri "https://github.com/paiml/pmcp/releases/latest/download/pmcp-windows.zip" -OutFile "pmcp.zip"
Expand-Archive pmcp.zip
```

## Feature Flags

PMCP uses feature flags to minimize binary size. Choose the features you need:

```toml
[dependencies]
pmcp = { version = "2.0", features = ["full"] }
```

### Available Features

| Feature | Description | Dependencies |
|---------|-------------|--------------|
| `default` | Core functionality + validation | `jsonschema`, `garde` |
| `full` | All features enabled | All dependencies |
| `websocket` | WebSocket transport | `tokio-tungstenite` |
| `http` | HTTP transport | `hyper`, `hyper-util` |
| `streamable-http` | Streamable HTTP server | `axum`, `tokio-stream` |
| `sse` | Server-Sent Events | `bytes`, `tokio-util` |
| `validation` | Input validation | `jsonschema`, `garde` |
| `resource-watcher` | File system watching | `notify`, `glob-match` |
| `wasm` | WebAssembly support | `wasm-bindgen` |

### Common Configurations

**Minimal client:**
```toml
pmcp = { version = "2.0", features = ["validation"] }
```

**WebSocket server:**
```toml  
pmcp = { version = "2.0", features = ["websocket", "validation"] }
```

**Production server:**
```toml
pmcp = { version = "2.0", features = ["full"] }
```

## Development Environment Setup

### Install Required Tools

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add required components
rustup component add rustfmt clippy llvm-tools-preview

# Install development tools
cargo install cargo-nextest cargo-llvm-cov cargo-audit
```

### IDE Configuration

**Visual Studio Code:**
```bash
# Install Rust extension
code --install-extension rust-lang.rust-analyzer
```

**vim/neovim:**
```vim
" Add to your config
Plug 'rust-lang/rust.vim'
Plug 'neoclide/coc.nvim'
```

**JetBrains IntelliJ/CLion:**
- Install the Rust plugin from the marketplace

## Verification

### Quick Test

Create a new project and verify PMCP works:

```bash
cargo new pmcp-test
cd pmcp-test
```

Add to `Cargo.toml`:
```toml
[dependencies]
pmcp = "2.0"
tokio = { version = "1.0", features = ["full"] }
```

Replace `src/main.rs`:
```rust
use pmcp::{Client, ClientCapabilities};

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    println!("PMCP version: {}", pmcp::VERSION);
    
    // Test client creation
    let client = Client::builder()
        .name("test-client")
        .version("1.0.0")
        .capabilities(ClientCapabilities::default())
        .build()?;
    
    println!("✅ PMCP client created successfully!");
    println!("Client name: {}", client.name());
    
    Ok(())
}
```

Run the test:
```bash
cargo run
```

Expected output:
```
PMCP version: 2.0.0
✅ PMCP client created successfully!
Client name: test-client
```

### Run Examples

Test with the included examples:

```bash
# Clone the repository
git clone https://github.com/paiml/pmcp.git
cd pmcp

# Run basic server example  
cargo run --example 02_server_basic --features full

# In another terminal, run client example
cargo run --example 01_client_initialize --features full
```

### Performance Benchmark

Verify performance with built-in benchmarks:

```bash
cargo bench --all-features
```

Expected results (approximate):
```
simple_protocol_parse    time: [12.5 ns 12.8 ns 13.2 ns]
json_serialization      time: [1.85 μs 1.89 μs 1.94 μs]  
websocket_roundtrip     time: [45.2 μs 46.1 μs 47.3 μs]
```

## Common Issues

### Compilation Errors

**Issue**: Missing features
```
error[E0432]: unresolved import `pmcp::WebSocketTransport`
```

**Solution**: Enable required features:
```toml
pmcp = { version = "2.0", features = ["websocket"] }
```

**Issue**: MSRV (Minimum Supported Rust Version)
```
error: package `pmcp v2.0.0` cannot be built because it requires rustc 1.82 or newer
```

**Solution**: Update Rust:
```bash
rustup update stable
```

### Runtime Issues

**Issue**: Port already in use
```
Error: Address already in use (os error 98)
```

**Solution**: Use a different port:
```rust
server.bind("127.0.0.1:0").await?; // Let OS choose port
```

**Issue**: Permission denied
```
Error: Permission denied (os error 13)
```

**Solution**: Use unprivileged port (>1024):
```rust  
server.bind("127.0.0.1:8080").await?;
```

### Performance Issues

**Issue**: High memory usage
```
Memory usage: 2.3GB for simple server
```

**Solution**: Disable debug symbols in release:
```toml
[profile.release]
debug = false
strip = true
```

## Next Steps

Now that PMCP is installed and working, you're ready to:

1. **Build your first server** - Chapter 2 walks through creating a basic MCP server
2. **Create a client** - Chapter 3 shows how to connect and interact with servers  
3. **Explore examples** - Check out the `examples/` directory for real-world patterns

## Getting Help

If you encounter issues:

- **Documentation**: [https://docs.rs/pmcp](https://docs.rs/pmcp)
- **Examples**: [https://github.com/paiml/pmcp/tree/main/examples](https://github.com/paiml/pmcp/tree/main/examples)
- **Issues**: [https://github.com/paiml/pmcp/issues](https://github.com/paiml/pmcp/issues)
- **Discussions**: [https://github.com/paiml/pmcp/discussions](https://github.com/paiml/pmcp/discussions)

You're all set! Let's start building with PMCP.