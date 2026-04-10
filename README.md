# PMCP - Pragmatic Model Context Protocol
<!-- QUALITY BADGES START -->
[![Quality Gate](https://img.shields.io/badge/Quality%20Gate-failing-red)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
[![TDG Score](https://img.shields.io/badge/TDG%20Score-0.00-brightgreen)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
[![Complexity](https://img.shields.io/badge/Complexity-clean-brightgreen)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
[![Technical Debt](https://img.shields.io/badge/Tech%20Debt-0h-brightgreen)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
<!-- QUALITY BADGES END -->

[![CI](https://github.com/paiml/pmcp/actions/workflows/ci.yml/badge.svg)](https://github.com/paiml/pmcp/actions/workflows/ci.yml)
[![Quality Gate](https://img.shields.io/badge/Quality%20Gate-passing-brightgreen)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
[![TDG Score](https://img.shields.io/badge/TDG%20Score-0.76-green)](https://github.com/paiml/rust-mcp-sdk/actions/workflows/quality-badges.yml)
[![Coverage](https://img.shields.io/badge/coverage-52%25-yellow.svg)](https://github.com/paiml/pmcp)
[![Crates.io](https://img.shields.io/crates/v/pmcp.svg)](https://crates.io/crates/pmcp)
[![Documentation](https://docs.rs/pmcp/badge.svg)](https://docs.rs/pmcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust 1.83+](https://img.shields.io/badge/rust-1.83+-orange.svg)](https://www.rust-lang.org)
[![MCP Compatible](https://img.shields.io/badge/MCP-v2025--11--25-blue.svg)](https://modelcontextprotocol.io)

> **Production-grade Rust implementation of the [Model Context Protocol](https://modelcontextprotocol.io) (MCP) - 16x faster than TypeScript, built with Toyota Way quality principles**

## Overview

PMCP is a complete MCP ecosystem for Rust, providing everything you need to build, test, and deploy production-grade MCP servers:

- **🦀 pmcp SDK** - High-performance Rust crate with full MCP protocol support
- **⚡ cargo-pmcp** - CLI toolkit for scaffolding, testing, and development
- **📚 pmcp-book** - Comprehensive reference guide with 27 chapters
- **🎓 pmcp-course** - Hands-on course with quizzes and exercises
- **🤖 AI Agents** - Kiro and Claude Code configurations for AI-assisted development

**Why PMCP?**
- **Performance**: 16x faster than TypeScript SDK, 50x lower memory
- **Safety**: Rust's type system + zero `unwrap()` in production code
- **Quality**: Toyota Way principles - zero technical debt tolerance
- **Complete**: SDK, tooling, documentation, and AI assistance in one ecosystem

## Quick Start

Choose your path based on experience and preference:

### 🚀 Path 1: AI-Assisted (Fastest - Recommended for Rapid Prototyping)

Build production-ready MCP servers with AI assistance in minutes:

**Prerequisites:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update

# Install cargo-pmcp
cargo install cargo-pmcp
```

**Install Claude Code AI Agent:**
```bash
# Install the mcp-developer subagent (user-level - works across all projects)
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md

# Restart Claude Code
```

**Build your server:**
```
You: "Create a weather forecast MCP server with tools for getting current conditions and 5-day forecasts"

Claude Code: [Invokes mcp-developer subagent]

I'll create a production-ready weather MCP server using cargo-pmcp.

$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace
$ cargo pmcp add server weather --template minimal

[Implements type-safe tools with validation]
[Adds comprehensive tests and observability]
[Validates quality gates]

✅ Production-ready server complete with 85% test coverage!
```

**What you get**: Production-ready code following Toyota Way principles, with comprehensive tests, structured logging, metrics collection, and zero clippy warnings.

**Learn more**: [AI-Assisted Development Course](https://paiml.github.io/rust-mcp-sdk/course/part6-ai-dev/ch15-ai-assisted.html) | [AI Agents README](ai-agents/README.md)

---

### ⚡ Path 2: cargo-pmcp Toolkit (Recommended for Manual Development)

Scaffold and build servers using the cargo-pmcp CLI:

**Installation:**
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update

# Install cargo-pmcp
cargo install cargo-pmcp
```

**Create a server:**
```bash
# Create workspace
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace

# Add a server using a template
cargo pmcp add server myserver --template minimal

# Start development server with hot-reload
cargo pmcp dev --server myserver

# Generate and run tests
cargo pmcp test --server myserver --generate-scenarios
cargo pmcp test --server myserver

# Build for production
cargo build --release
```

**Available templates:**
- `minimal` - Empty structure for custom servers
- `calculator` - Arithmetic operations (learning)
- `complete_calculator` - Full-featured reference implementation
- `sqlite_explorer` - Database browser pattern

**Learn more**: [cargo-pmcp Guide](cargo-pmcp/README.md)

---

### 🦀 Path 3: pmcp SDK Directly (For Fine-Grained Control)

Use the pmcp crate directly for maximum control:

**Installation:**
```toml
[dependencies]
pmcp = "2.0"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
schemars = "0.8"  # For type-safe tools
```

**Type-safe server example:**
```rust
use pmcp::{ServerBuilder, TypedTool, RequestHandlerExtra, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
struct WeatherArgs {
    #[schemars(description = "City name")]
    city: String,

    #[schemars(description = "Number of days (1-5)")]
    days: Option<u8>,
}

#[derive(Debug, Serialize, JsonSchema)]
struct WeatherOutput {
    temperature: f64,
    conditions: String,
}

async fn get_weather(args: WeatherArgs, _extra: RequestHandlerExtra) -> pmcp::Result<WeatherOutput> {
    // Validate
    if args.city.is_empty() {
        return Err(Error::validation("City cannot be empty"));
    }

    let days = args.days.unwrap_or(1);
    if !(1..=5).contains(&days) {
        return Err(Error::validation("Days must be 1-5"));
    }

    // Call weather API...
    Ok(WeatherOutput {
        temperature: 72.0,
        conditions: "Sunny".to_string(),
    })
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = ServerBuilder::new()
        .name("weather-server")
        .version("1.0.0")
        .tool("get-weather", TypedTool::new("get-weather", |args, extra| {
            Box::pin(get_weather(args, extra))
        }).with_description("Get weather forecast for a city"))
        .build()?;

    server.run_stdio().await?;
    Ok(())
}
```

**Learn more**: [pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/) | [pmcp-course](https://paiml.github.io/rust-mcp-sdk/course/) | [API Documentation](https://docs.rs/pmcp)

---

## PMCP Ecosystem Components

### 🦀 pmcp SDK (The Crate)

High-performance Rust implementation of the MCP protocol.

**Key Features:**
- **Type-Safe Tools**: Automatic JSON schema generation from Rust types
- **Multiple Transports**: stdio, HTTP/SSE, WebSocket, WASM
- **OAuth Support**: Full auth context pass-through
- **Workflows**: Multi-step orchestration with array indexing support
- **MCP Apps**: Rich HTML UI widgets with live preview and browser DevTools
- **MCP Tasks**: Shared client/server state with task lifecycle management
- **Tower Middleware**: DNS rebinding protection, CORS, security headers
- **Performance**: 16x faster than TypeScript, SIMD-accelerated parsing
- **Quality**: Zero `unwrap()`, comprehensive error handling

**Latest Version:** `pmcp = "2.0"`

**Documentation:**
- [API Reference](https://docs.rs/pmcp)
- [pmcp-book](https://paiml.github.io/rust-mcp-sdk/)
- [Examples](examples/)

---

### ⚡ cargo-pmcp (CLI Toolkit)

Full-lifecycle development toolkit — from scaffolding to production deployment.

```bash
cargo install cargo-pmcp
```

```bash
cargo pmcp new my-workspace             # Scaffold a new workspace
cargo pmcp add server my-server         # Add a server with best-practice template
cargo pmcp dev --server my-server       # Dev server with hot-reload
cargo pmcp test --server my-server      # Auto-generated scenario tests
cargo pmcp loadtest run                 # Load test with latency percentiles
cargo pmcp pentest run                  # Security audit (32 checks, SARIF output)
cargo pmcp preview --open               # Browser-based widget preview
cargo pmcp deploy --target aws-lambda   # Deploy to AWS Lambda, GCR, or Cloudflare
cargo pmcp deploy logs --tail           # Stream production logs
```

Covers the full development lifecycle: scaffolding, dev mode, testing, load testing, security pentesting, MCP Apps preview, schema management, multi-target deployment, secrets, and OAuth setup.

**Full command reference**: [cargo-pmcp Guide](cargo-pmcp/README.md)

---

### 📚 pmcp-book (Reference Guide)

27-chapter comprehensive reference guide to building MCP servers with pmcp.

**📖 [Read Online](https://paiml.github.io/rust-mcp-sdk/book/)**

**Coverage:**
- **Getting Started**: Installation, first server, quick start tutorial
- **Core Concepts**: Tools, resources, prompts, error handling
- **Advanced Features**: Auth, transports, middleware, progress tracking
- **Real-World**: Production servers, testing, deployment, performance
- **Examples & Patterns**: Complete examples and design patterns
- **TypeScript Migration**: Complete compatibility guide
- **Advanced Topics**: Custom transports, AI-assisted development

**Local development:**
```bash
make book-serve    # Serve at http://localhost:3000
make book-open     # Build and open in browser
```

---

### 🎓 pmcp-course (Hands-On Learning)

Interactive course with quizzes, exercises, and real-world projects for mastering MCP development.

**🎓 [Start the Course](https://paiml.github.io/rust-mcp-sdk/course/)**

**Course Structure:**
- **Part I: Foundations** - MCP concepts, first server, typed tools
- **Part II: Core Concepts** - Tools, resources, prompts, validation
- **Part III: Deployment** - AWS Lambda, Cloudflare Workers, Google Cloud Run
- **Part IV: Testing** - Local testing, CI/CD, regression testing
- **Part V: Security** - OAuth 2.0, identity providers, multi-tenant
- **Part VI: AI-Assisted Dev** - Claude Code, feedback loops, collaboration
- **Part VII: Observability** - Middleware, logging, metrics
- **Part VIII: Advanced** - Server composition, MCP Apps (experimental)

**Features:**
- Interactive quizzes after each chapter
- Hands-on exercises with solutions
- Real-world project examples
- Best practices from production servers

**Local development:**
```bash
cd pmcp-course && mdbook serve    # Serve at http://localhost:3000
```

---

### 🤖 ai-agents (AI-Assisted Development)

AI agent configurations that teach Kiro and Claude Code how to build MCP servers.

**Supported AI Assistants:**

**Kiro (Steering Files)** - 10,876 lines of persistent MCP expertise
- Always-active knowledge in every conversation
- Comprehensive testing and observability guidance
- [Installation Guide](ai-agents/kiro/mcp-developer-power/)

**Claude Code (Subagent)** - ~750 lines of focused MCP knowledge
- On-demand invocation for MCP tasks
- Quick scaffolding and implementation
- [Installation Guide](ai-agents/claude-code/)

**What AI agents know:**
- MCP protocol concepts and patterns
- cargo-pmcp workflow (never creates files manually)
- Type-safe tool implementation
- Testing strategies (unit, integration, property, fuzz)
- Production observability (logging, metrics)
- Toyota Way quality standards

**Community implementations welcome:**
- GitHub Copilot, Cursor, Cline, and others
- [Contribution Guide](ai-agents/README.md)

**Learn more**: [AI-Assisted Development Course](https://paiml.github.io/rust-mcp-sdk/course/part6-ai-dev/ch15-ai-assisted.html) | [ai-agents/](ai-agents/)

---

### 🎨 MCP Apps (Rich UI Widgets)

Build rich HTML UI widgets served from MCP servers — works with ChatGPT, Claude, and other MCP clients.

**What it does:**
- **Preview**: Live widget preview with dual proxy/WASM bridge modes
- **Author**: File-based widgets in `widgets/` directory with hot-reload
- **Scaffold**: `cargo pmcp app new` generates a complete MCP Apps project
- **Publish**: ChatGPT-compatible manifest and standalone demo landing pages
- **Test**: 20 E2E browser tests via chromiumoxide CDP

**Quick start:**
```bash
# Scaffold a new MCP Apps project
cargo pmcp app new my-widget-app
cd my-widget-app

# Run the server
cargo run

# Preview in browser (separate terminal)
cargo pmcp preview --url http://localhost:3000 --open

# Generate deployment artifacts
cargo pmcp app build --url https://my-server.example.com
```

**Examples:**
- [Chess App](examples/mcp-apps-chess/) — Interactive chess board with move validation
- [Map App](examples/mcp-apps-map/) — Leaflet.js geospatial city explorer
- [Data Viz App](examples/mcp-apps-dataviz/) — Chart.js dashboard with SQL queries

**Learn more**: [Widget Runtime](packages/widget-runtime/) | [Preview Server](crates/mcp-preview/) | [E2E Tests](crates/mcp-e2e-tests/)

---

## Latest Release: v2.0.0

**PMCP v2.0 — aligned with the MCP TypeScript SDK v2.0 release (2026-03-22):**

- **Protocol v2025-11-25**: Full alignment with the latest MCP specification, backward compatible with `2024-11-05`
- **MCP Apps**: Rich interactive HTML UI widgets served from MCP servers — works with ChatGPT, Claude Desktop, and other MCP clients. Live preview with browser-style DevTools (resizable panel, network/events/protocol/bridge tabs)
- **MCP Tasks**: Experimental shared client/server state with DynamoDB-backed task lifecycle management and task variables
- **Conformance Test Suite**: 19-scenario conformance engine across 5 domains with `cargo pmcp test conformance` and `mcp-tester conformance` CLI integration
- **Tower Middleware**: DNS rebinding protection, CORS with origin-locked headers, configurable security headers — production-ready HTTP stack
- **PMCP Server**: MCP server exposing SDK developer tools (test, scaffold, schema export) via Streamable HTTP, deployed on AWS Lambda
- **Uniform Constructor DX**: Default impls, builders, and constructors for all protocol types — dramatically improved ergonomics
- **60+ Examples**: Comprehensive coverage of all SDK features

**Full changelog**: [CHANGELOG.md](CHANGELOG.md)

---

## Core Features

### 🚀 **Transport Layer**
- **stdio**: Standard input/output for CLI integration
- **HTTP/SSE**: Streamable HTTP with Server-Sent Events
- **WebSocket**: Full-duplex with auto-reconnection
- **WASM**: Browser and Cloudflare Workers support

### 🛠️ **Type-Safe Development**
- **Automatic Schema Generation**: From Rust types using `schemars`
- **Compile-Time Validation**: Type-checked tool arguments
- **Runtime Validation**: Against generated JSON schemas
- **Zero Unwraps**: Explicit error handling throughout

### 🔐 **Security & Auth**
- **OAuth 2.0**: Full auth context pass-through
- **OIDC Discovery**: Automatic provider configuration
- **Bearer Tokens**: Standard authentication
- **Path Validation**: Secure file system access

### 🧪 **Testing & Quality**
- **mcp-tester**: Comprehensive server testing tool
- **Scenario Generation**: Auto-generate test cases
- **Property Testing**: Invariant validation
- **Quality Gates**: fmt, clippy, coverage enforcement

### ⚡ **Performance**
- **16x faster** than TypeScript SDK
- **50x lower memory** usage
- **SIMD Parsing**: 10.3x SSE speedup with AVX2/SSE4.2
- **Connection Pooling**: Smart load balancing

### 🏭 **Toyota Way Quality**
- **Zero Technical Debt**: TDG score 0.76
- **Jidoka**: Stop the line on defects
- **Genchi Genbutsu**: Go and see (evidence-based)
- **Kaizen**: Continuous improvement
- **No Unwraps**: Explicit error handling only

---

## Documentation

### 📖 Primary Resources

- **[PMCP Documentation Portal](https://paiml.github.io/rust-mcp-sdk/)** - Landing page for all documentation
- **[pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/)** - Comprehensive reference guide (27 chapters)
- **[pmcp-course](https://paiml.github.io/rust-mcp-sdk/course/)** - Hands-on course with quizzes and exercises
- **[API Reference](https://docs.rs/pmcp)** - Complete API documentation
- **[cargo-pmcp Guide](cargo-pmcp/README.md)** - CLI toolkit documentation

### 📚 Additional Resources

- **[Examples](examples/)** - 200+ working examples
- **[CHANGELOG](CHANGELOG.md)** - Version history
- **[Migration Guides](docs/)** - Upgrade instructions
- **[Contributing](CONTRIBUTING.md)** - How to contribute

### 🎯 Quick Links

- [Quick Start Tutorial](https://paiml.github.io/rust-mcp-sdk/book/ch01_5-quick-start-tutorial.html)
- [Your First Server](https://paiml.github.io/rust-mcp-sdk/book/ch02-first-server.html)
- [Course: Getting Started](https://paiml.github.io/rust-mcp-sdk/course/part1-foundations/ch01-enterprise-case.html)
- [Testing Guide](https://paiml.github.io/rust-mcp-sdk/course/part4-testing/ch11-local-testing.html)
- [Production Deployment](https://paiml.github.io/rust-mcp-sdk/course/part3-deployment/ch07-deployment.html)

---

## Examples

The SDK includes 60+ comprehensive examples covering all features:

```bash
# Basic examples
cargo run --example 01_client_initialize    # Client setup
cargo run --example 02_server_basic         # Basic server
cargo run --example 03_client_tools         # Tool usage

# Type-safe tools (v1.6.0+)
cargo run --example 32_typed_tools --features schema-generation
cargo run --example 33_advanced_typed_tools --features schema-generation

# Advanced features
cargo run --example 09_authentication       # OAuth/Bearer
cargo run --example 13_websocket_transport  # WebSocket
cargo run --example 15_middleware           # Middleware chain

# Testing
cargo run --example 26-server-tester -- test http://localhost:8080

# AI-assisted development
# See ai-agents/README.md for Kiro and Claude Code setup
```

See [examples/README.md](examples/README.md) for complete list.

---

## MCP Server Tester

Comprehensive testing tool for validating MCP server implementations.

**Features:**
- Protocol compliance validation (JSON-RPC 2.0, MCP spec)
- Multi-transport support (HTTP, HTTPS, WebSocket, stdio)
- Tool discovery and testing
- CI/CD ready with JSON output

**Installation:**
```bash
cargo install mcp-server-tester

# Or download pre-built binaries from releases
```

**Usage:**
```bash
# Test a server
mcp-tester test http://localhost:8080

# Protocol compliance check
mcp-tester compliance http://localhost:8080 --strict

# Connection diagnostics
mcp-tester diagnose http://localhost:8080
```

**Learn more**: [examples/26-server-tester/README.md](examples/26-server-tester/README.md)

---

## Quality & Performance

### Toyota Way Principles

PMCP is built following Toyota Production System principles:

- **Jidoka (自働化)**: Automation with human touch
  - Quality gates stop builds on defects
  - Zero tolerance for `unwrap()` in production
  - Comprehensive error handling with context

- **Genchi Genbutsu (現地現物)**: Go and see
  - Evidence-based decisions with metrics
  - PMAT quality analysis (TDG score 0.76)
  - Comprehensive testing (unit, property, fuzz)

- **Kaizen (改善)**: Continuous improvement
  - Regular benchmarking and optimization
  - Performance regression prevention
  - Community-driven enhancements

### Quality Metrics

- **TDG Score**: 0.76 (production-ready)
- **Technical Debt**: Minimal (436h across entire codebase)
- **Complexity**: All functions ≤25 complexity
- **Coverage**: 52% line coverage, 100% function coverage
- **Linting**: Zero clippy warnings in production code

### Performance Benchmarks

```
Metric                  PMCP (Rust)     TypeScript SDK    Improvement
────────────────────────────────────────────────────────────────────
Overall Speed           16x             1x                16x faster
Memory Usage            <10 MB          ~500 MB           50x lower
SSE Parsing             336,921 ev/s    32,691 ev/s       10.3x faster
JSON-RPC Parsing        195,181 docs/s  N/A               SIMD-optimized
Round-trip Latency      <100 μs         ~1-2 ms           10-20x faster
Base64 Operations       252+ MB/s       N/A               Optimized
```

**Run benchmarks:**
```bash
make bench                           # General benchmarks
cargo run --example 32_simd_parsing  # SIMD-specific
```

---

## WebAssembly Support

Full WASM support for browser and edge deployment:

**Targets:**
- **Cloudflare Workers** (wasm32-unknown-unknown)
- **WASI Runtimes** (wasm32-wasi)
- **Browser** (wasm-bindgen)

**Quick start:**
```bash
# Build for Cloudflare Workers
cargo build --target wasm32-unknown-unknown --no-default-features --features wasm

# Deploy
make cloudflare-sdk-deploy
```

**Learn more**: [WASM Guide](docs/WASM_TARGETS.md) | [WASM Example](examples/wasm-mcp-server/)

---

## Development

### Prerequisites
- Rust 1.83.0 or later
- Git

### Setup
```bash
git clone https://github.com/paiml/rust-mcp-sdk
cd rust-mcp-sdk

# Install development tools
make setup

# Run quality checks
make quality-gate
```

### Testing
```bash
make test-all           # All tests
make test-property      # Property tests
make coverage           # Coverage report
make mutants            # Mutation tests
```

### Contributing

We welcome contributions! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Ensure quality gates pass (`make quality-gate`)
4. Commit with conventional commits
5. Push and open a Pull Request

**See**: [CONTRIBUTING.md](CONTRIBUTING.md)

---

## Compatibility

| Feature | TypeScript SDK v2.0 | PMCP v2.0 (Rust) |
|---------|---------------------|------------------|
| Protocol Version | 2025-11-25 | 2025-11-25 (+ 2024-11-05 compat) |
| Transports | stdio, SSE, WebSocket | stdio, SSE, WebSocket, WASM |
| Authentication | OAuth 2.0, Bearer | OAuth 2.0, Bearer, OIDC |
| Tools | ✓ | ✓ (Type-safe + outputSchema) |
| Prompts | ✓ | ✓ (Workflows) |
| Resources | ✓ | ✓ (Subscriptions) |
| Sampling | ✓ | ✓ |
| MCP Apps | ✓ | ✓ (Preview + DevTools) |
| Tower Middleware | N/A | ✓ (DNS rebinding, CORS, security headers) |
| Performance | 1x | 16x faster |
| Memory | Baseline | 50x lower |

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- [Model Context Protocol](https://modelcontextprotocol.io) specification
- [TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk) for reference implementation
- [PAIML MCP Agent Toolkit](https://github.com/paiml/paiml-mcp-agent-toolkit) for quality standards
- Community contributors and early adopters

---

## Links

- **Documentation Portal**: https://paiml.github.io/rust-mcp-sdk/
- **Reference Guide**: https://paiml.github.io/rust-mcp-sdk/book/
- **Course**: https://paiml.github.io/rust-mcp-sdk/course/
- **Crates.io**: https://crates.io/crates/pmcp
- **API Docs**: https://docs.rs/pmcp
- **Issues**: https://github.com/paiml/rust-mcp-sdk/issues
- **Discussions**: https://github.com/paiml/rust-mcp-sdk/discussions

---

**Built with 🦀 Rust and ❤️ following Toyota Way principles**
