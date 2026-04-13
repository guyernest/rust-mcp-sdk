# pmcp

High-quality Rust SDK for the Model Context Protocol (MCP), providing both
client and server implementations with full TypeScript SDK compatibility,
multiple transport options (stdio, HTTP streaming, WebSocket), and built-in
authentication support.

pmcp is designed as a zero-compromise production SDK: every feature is behind
an explicit cargo feature flag so downstream builds stay lean, every public API
ships with a doctest that runs under `cargo test --doc`, and the transport
layer is interchangeable so the same `Server` handles stdio for local
assistants, HTTP streaming for cloud deployments, and WebSocket for browser
clients without rewriting your handlers.

The crate is organized around three primitives that mirror the MCP
specification: **tools** (callable functions with typed arguments and
structured output), **prompts** (parameterized templates with auto-generated
argument schemas), and **resources** (URI-addressable content with optional
change notifications). Builder APIs wire your handlers to each primitive, and
the `#[mcp_tool]`, `#[mcp_prompt]`, and `#[mcp_server]` attribute macros
(behind the `macros` feature) eliminate the per-handler boilerplate entirely
by deriving schemas from your Rust types.

## Quick Start

Add pmcp to `Cargo.toml` with the features you need. The `full` meta-feature
turns everything on for prototyping; production builds should cherry-pick
individual features to keep compile time and binary size under control.

```toml
[dependencies]
pmcp = { version = "2.3", features = ["full"] }
tokio = { version = "1.46", features = ["full"] }
```

### Client Example

Connect to an MCP server over stdio, initialize the session, and enumerate the
tools that server exposes. This is the minimum end-to-end client flow — wire it
into your own assistant or test harness as-is, then layer on tool calls and
prompt fetches as your use case grows.

```rust,no_run
use pmcp::{Client, StdioTransport, ClientCapabilities};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
// Create a client with stdio transport
let transport = StdioTransport::new();
let mut client = Client::new(transport);

// Initialize the connection
let server_info = client.initialize(ClientCapabilities::default()).await?;

// List available tools
let tools = client.list_tools(None).await?;
# Ok(())
# }
```

### Server Example

Expose a single tool backed by a custom `ToolHandler` implementation and run it
over stdio. The same `Server::builder()` chain works with any transport; just
call a different `run_*` method or hand the server to the `pmcp::axum::router`
helper behind the `streamable-http` feature.

```rust,no_run
use pmcp::{Server, ServerCapabilities, ToolHandler};
use async_trait::async_trait;
use serde_json::Value;

struct MyTool;

#[async_trait]
impl ToolHandler for MyTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> Result<Value, pmcp::Error> {
        Ok(serde_json::json!({"result": "success"}))
    }
}

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities::default())
    .tool("my-tool", MyTool)
    .build()?;

// Run with stdio transport
server.run_stdio().await?;
# Ok(())
# }
```

## Cargo Features

pmcp uses cargo features to keep the default build minimal and to let you opt
into exactly the transports, integrations, and helpers you need. The table
below documents every user-facing feature, what it gives you, and what it pulls
into your dependency tree so you can weigh capability against binary size
before turning a flag on.

<!-- update when Cargo.toml [features] changes -->

| Feature | Description | Enables |
|---------|-------------|---------|
| `default` | Enabled by default; structured logging via `tracing-subscriber` | `tracing-subscriber` |
| `full` | Everything below — single switch for prototyping and tests | All individual features listed below |
| `composition` | Compose multiple MCP servers into one streamable-HTTP endpoint | (via `streamable-http`) |
| `http` | HTTP transport primitives (Hyper server) | `hyper`, `hyper-util`, `bytes` |
| `http-client` | Async HTTP client (reqwest, rustls backend) | `reqwest` |
| `jwt-auth` | JWT-based authentication helpers | `jsonwebtoken` + `http-client` |
| `logging` | Structured logging via the `tracing` ecosystem | `tracing-subscriber` |
| `macros` | Attribute proc macros (`#[mcp_tool]`, `#[mcp_server]`, `#[mcp_prompt]`) | `pmcp-macros`, `schemars` |
| `mcp-apps` | `ChatGPT` Apps / MCP-UI / SEP-1865 interactive UI types | UI types (code-only, no extra deps) |
| `oauth` | OAuth 2.0 CLI helper for local token flows | `webbrowser`, `dirs`, `rand` + `http-client` |
| `rayon` | Parallel iterator support for batch operations | `rayon` |
| `resource-watcher` | File-system watcher for MCP resource notifications | `notify`, `glob-match` |
| `schema-generation` | Generate JSON Schema from Rust types | `schemars` |
| `simd` | SIMD-optimized JSON parsing (code-only, uses target-feature detection) | SIMD JSON parsing (no extra deps) |
| `sse` | Server-Sent Events streaming transport | `bytes` + `http-client` |
| `streamable-http` | HTTP streaming transport with SSE and Axum integration | `hyper`, `hyper-util`, `hyper-rustls`, `rustls`, `axum`, `tower`, `tower-http`, `futures-util`, `bytes` |
| `validation` | JSON Schema and struct-level validation | `jsonschema`, `garde` |
| `websocket` | WebSocket transport via `tokio-tungstenite` | `tokio-tungstenite` |

### Choosing features

Most deployments need one transport plus optional auth and validation. A
Cloudflare Workers or Lambda HTTP server typically wants
`streamable-http + jwt-auth + validation + macros`; a desktop assistant that
speaks stdio only needs the defaults plus `macros` and `schema-generation`; a
browser-facing server wants `websocket + validation`. Turn on `full` during
local prototyping and narrow the list before you ship.

### Transport notes

Feature flags divide cleanly along transport lines. `http` gives you the raw
Hyper primitives for custom server wiring; `streamable-http` layers Axum,
Tower middleware, and SSE framing on top for the common case of a production
HTTP MCP endpoint (DNS rebinding protection, CORS, security headers are all
built in). `sse` is a lighter-weight option when you only need server-push
streaming without the full Axum router. `websocket` is native tungstenite for
long-lived browser-facing connections. `composition` lets you merge several
`Server` instances under one `streamable-http` endpoint so a single process
can publish tools from multiple subsystems without per-subsystem HTTP
plumbing.

### Authentication notes

`jwt-auth` and `oauth` are additive: `jwt-auth` validates incoming bearer
tokens against a JWKS or static key, and `oauth` ships a desktop CLI flow that
obtains and refreshes tokens for clients. Both sit on top of `http-client`
(reqwest) which you can also enable on its own for plain outbound HTTP calls
from handlers.

## Learn More

- **API docs:** <https://docs.rs/pmcp>
- **Book:** <https://paiml.github.io/pmcp/book/> — architecture, transport
  selection, OAuth flows, and deployment recipes.
- **Course:** <https://paiml.github.io/pmcp/course/> — hands-on exercises
  covering tools, prompts, resources, and the full MCP protocol lifecycle.
- **Repository:** <https://github.com/paiml/rust-mcp-sdk> — issues, PRs,
  changelog, and examples directory with dozens of runnable MCP servers.
- **TypeScript parity:** pmcp tracks the reference TypeScript MCP SDK closely;
  wire-level messages are compatible, so a pmcp server works with any
  TypeScript client and vice versa.

## License

MIT
