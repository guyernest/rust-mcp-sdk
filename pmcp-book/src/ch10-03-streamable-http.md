# Chapter 10.3: Streamable HTTP (Server + Client)

Streamable HTTP is PMCP’s preferred transport for servers. It combines JSON requests with Server‑Sent Events (SSE) for notifications, supports both stateless and stateful operation, and uses a single endpoint with content negotiation via the `Accept` header.

## Why Streamable HTTP?

- Single endpoint for requests and notifications
- Works well through proxies and enterprise firewalls
- Optional sessions for stateful, multi-request workflows
- Built with Axum, provided by the SDK

## Server (Axum-based)

Types: `pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig}` (feature: `streamable-http`).

```rust
use pmcp::{Server, ServerCapabilities, ToolHandler, RequestHandlerExtra};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

struct Add;

#[async_trait]
impl ToolHandler for Add {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        let a = args["a"].as_f64().unwrap_or(0.0);
        let b = args["b"].as_f64().unwrap_or(0.0);
        Ok(json!({"result": a + b}))
    }
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = Server::builder()
        .name("streamable-http-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("add", Add)
        .build()?;

    let server = Arc::new(Mutex::new(server));
    let addr: SocketAddr = ([0,0,0,0], 8080).into();

    // Default: stateful with SSE support
    let http = StreamableHttpServer::new(addr, server.clone());
    let (bound, _handle) = http.start().await?;
    println!("Streamable HTTP listening on {}", bound);
    Ok(())
}
```

### Stateless vs Stateful

```rust
// Stateless (serverless-friendly): no session tracking
let cfg = StreamableHttpServerConfig {
    session_id_generator: None,
    enable_json_response: false, // prefer SSE for notifications
    event_store: None,
    on_session_initialized: None,
    on_session_closed: None,
};
let http = StreamableHttpServer::with_config(addr, server, cfg);
```

## Protocol Details

Headers enforced by the server:
- `mcp-protocol-version`: Protocol version (e.g., `2024-11-05`)
- `Accept`: Must include `application/json` or `text/event-stream`
- `mcp-session-id`: Present in stateful mode
- `Last-Event-Id`: For SSE resumption

Accept rules:
- `Accept: application/json` → JSON responses only
- `Accept: text/event-stream` → SSE stream for notifications

## Client (Streamable HTTP)

Types: `pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig}` (feature: `streamable-http`).

```rust
use pmcp::{Client, ClientCapabilities};
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use url::Url;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let cfg = StreamableHttpTransportConfig {
        url: Url::parse("http://localhost:8080")?,
        extra_headers: vec![],
        auth_provider: None,
        session_id: None,
        enable_json_response: true, // or false to use SSE
        on_resumption_token: None,
    };

    let transport = StreamableHttpTransport::new(cfg);
    let mut client = Client::new(transport);
    let _info = client.initialize(ClientCapabilities::minimal()).await?;
    Ok(())
}
```

## Examples

- `examples/22_streamable_http_server_stateful.rs` – Stateful mode with SSE notifications
- `examples/23_streamable_http_server_stateless.rs` – Stateless/serverless-friendly configuration
- `examples/24_streamable_http_client.rs` – Client connecting to both modes

## Feature Flags

```toml
[dependencies]
pmcp = { version = "1.7", features = ["streamable-http"] }
```

