# WASM MCP Server - Write Once, Run Everywhere

This example demonstrates a single MCP (Model Context Protocol) server implementation in Rust that compiles to WebAssembly and runs on multiple platforms. The same core code deploys to both Cloudflare Workers and Fermyon Spin, showcasing true platform portability.

## 🎯 Overview

**One Implementation, Multiple Deployments:**
- Single Rust codebase (`src/lib.rs`)
- Compiles to WebAssembly
- Deploys to multiple WASI/WASM platforms
- Full MCP protocol support

## 📁 Project Structure

```
wasm-mcp-server/
├── src/
│   └── lib.rs              # Core MCP server implementation (shared)
├── Cargo.toml              # Rust dependencies
├── deployments/            # Platform-specific deployment configs
│   ├── cloudflare/         # Cloudflare Workers deployment
│   │   ├── wrangler.toml   # Cloudflare configuration
│   │   ├── worker-rust.js  # WASM wrapper for Workers
│   │   ├── Makefile        # Build & deploy commands
│   │   └── README.md       # Cloudflare-specific docs
│   └── fermyon-spin/       # Fermyon Spin deployment
│       ├── spin.toml       # Spin configuration
│       ├── Makefile        # Build & deploy commands
│       └── README.md       # Spin-specific docs
└── README.md               # This file
```

## 🚀 Quick Start

### Build the WASM Module

```bash
# Build the core WASM module (once for all platforms)
cargo build --target wasm32-unknown-unknown --release
```

### Deploy to Cloudflare Workers

```bash
cd deployments/cloudflare
make deploy

# Your deployment will be available at: https://<your-worker-name>.workers.dev
```

### Deploy to Fermyon Spin

```bash
cd deployments/fermyon-spin
make deploy

# Your deployment will be available at: https://<your-app-name>.fermyon.app/
```

## 🏗️ Architecture

### Core Implementation (`src/lib.rs`)

The core MCP server uses the `WasmMcpServer` from the pmcp SDK:

```rust
use pmcp::server::wasm_server::{WasmMcpServer, SimpleTool};

// Create server with tools
let server = WasmMcpServer::builder()
    .name("wasm-mcp-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities {
        tools: Some(Default::default()),
        resources: None,
        prompts: None,
    })
    .tool("calculator", SimpleTool::new(...))
    .tool("weather", SimpleTool::new(...))
    .tool("system_info", SimpleTool::new(...))
    .build();
```

### Platform Adapters

Each platform provides a thin adapter layer:

#### Cloudflare Workers
```rust
#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    // Adapt Workers Request/Response to MCP
}
```

#### Fermyon Spin
```rust
#[http_component]
fn handle_request(req: Request) -> Result<impl IntoResponse> {
    // Adapt Spin Request/Response to MCP
}
```

## 🛠️ Available Tools

The server implements three example tools:

### 1. Calculator
Performs arithmetic operations (add, subtract, multiply, divide)

### 2. Weather
Returns mock weather information for a location

### 3. System Info
Reports the runtime environment (Cloudflare vs Fermyon)

## 🧪 Testing

### Using MCP Tester with Scenario Files

The repository includes comprehensive test scenarios that can be run with the `mcp-tester` tool:

```bash
# Test with simple calculator scenario
mcp-tester scenario <deployment-url> test-scenarios/calculator-simple.json

# Test with comprehensive calculator tests (including error cases)
mcp-tester scenario <deployment-url> test-scenarios/calculator-test.yaml

# Test with minimal tool listing
mcp-tester scenario <deployment-url> test-scenarios/minimal-test.json
```

#### Example: Testing Cloudflare Deployment
```bash
# From the rust-mcp-sdk root directory
# Replace <your-worker-name> with your Cloudflare Worker subdomain
./target/release/mcp-tester scenario \
  https://<your-worker-name>.workers.dev \
  examples/wasm-mcp-server/test-scenarios/calculator-test.yaml
```

#### Example: Testing Fermyon Spin Deployment
```bash
# From the rust-mcp-sdk root directory
# Replace <your-app-name> with your Fermyon app URL
./target/release/mcp-tester scenario \
  https://<your-app-name>.fermyon.app/ \
  examples/wasm-mcp-server/test-scenarios/calculator-test.yaml
```

### Available Test Scenarios

1. **`calculator-simple.json`** - Basic calculator operations
   - Tests addition, multiplication, division, and subtraction
   - Validates correct results for each operation

2. **`calculator-test.yaml`** - Comprehensive calculator test suite
   - Tests all arithmetic operations with various inputs
   - Tests negative numbers and decimals
   - Tests error handling (division by zero, invalid operations, missing parameters)
   - Tests large numbers and edge cases

3. **`minimal-test.json`** - Minimal connectivity test
   - Simply lists available tools
   - Quick smoke test for deployment health

### Manual Testing with curl

You can also test deployments manually:

```bash
# Initialize connection
curl -X POST <deployment-url> \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'

# List available tools
curl -X POST <deployment-url> \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"2","method":"tools/list","params":{}}'

# Call a tool
curl -X POST <deployment-url> \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"3","method":"tools/call","params":{"name":"calculator","arguments":{"operation":"add","a":5,"b":3}}}'
```

### Building the MCP Tester

If you need to build the mcp-tester tool:

```bash
# From the rust-mcp-sdk root directory
cargo build --release --package mcp-server-tester

# The binary will be at: ./target/release/mcp-tester
```

## 📊 Deployment Comparison

| Platform | Build Target | Runtime | Global Edge | Cold Start | State Management |
|----------|-------------|---------|-------------|------------|------------------|
| **Cloudflare Workers** | wasm32-unknown-unknown | V8 Isolates | ✅ Yes (200+ locations) | 50-200ms | KV, Durable Objects |
| **Fermyon Spin** | wasm32-wasip1 | Wasmtime | ❌ No (single region) | 100-300ms | Built-in SQLite |

## 🔧 Building for Each Platform

### Cloudflare Workers

```bash
cd deployments/cloudflare

# Uses wasm-pack for reliable builds
wasm-pack build --target web --out-dir pkg --no-opt

# Deploy with wrangler
wrangler deploy
```

### Fermyon Spin

```bash
cd deployments/fermyon-spin

# Build with Spin's toolchain
spin build

# Deploy to Fermyon Cloud
spin deploy
```

## 📝 Key Implementation Details

### Request Handling

The core server is stateless and handles each request independently:

1. Parse JSON-RPC request
2. Route based on method (`initialize`, `tools/list`, `tools/call`)
3. Process with `WasmMcpServer`
4. Return JSON-RPC response

### Compatibility Fixes

- **Missing capabilities field**: Auto-adds empty `{}` if not present
- **CORS support**: Enabled for browser-based clients
- **Error handling**: Graceful degradation with proper error codes

## 🎯 Benefits of This Approach

1. **Code Reuse**: Single implementation for all platforms
2. **Type Safety**: Rust's compile-time guarantees
3. **Performance**: Native WASM execution speed
4. **Portability**: Deploy anywhere WASM runs
5. **Maintainability**: Fix once, deploy everywhere

## 🚀 Adding New Platforms

To deploy to a new WASM platform:

1. Create `deployments/<platform>/` folder
2. Add platform-specific config files
3. Write thin adapter layer for request/response
4. Use the same core `WasmMcpServer` implementation

## 📚 Documentation

- [**Production Deployment Guide**](DEPLOYMENT.md) - Cloudflare configuration, security best practices, and layered architecture
- [Fermyon Spin Deployment Guide](deployments/fermyon-spin/README.md)
- [Architecture Overview](deployments/cloudflare/ARCHITECTURE.md)
- [MCP Protocol Specification](https://modelcontextprotocol.io/)

## 🤝 Contributing

When adding features:
1. Implement in core `src/lib.rs`
2. Test on all platforms
3. Update documentation

## 📄 License

MIT

---

**Example Deployments for Testing:**
You can test the MCP protocol with these example deployments:
- 🌐 Cloudflare Example: https://mcp-sdk-worker.guy-ernest.workers.dev
- 🔄 Fermyon Example: https://mcp-fermyon-spin-3juc7zc4.fermyon.app/

Note: These are example deployments for testing. Deploy your own instances using the instructions above.