# Chapter 10.1: WebSocket Transport

WebSocket provides a persistent, low-latency channel that’s ideal for interactive tools and near–real-time experiences. PMCP supports WebSocket on the client side out of the box and also includes an optional server transport. For most server deployments, Streamable HTTP is recommended; use WebSocket when full‑duplex, long‑lived connections are required or when integrating with existing WS infrastructure.

## Capabilities at a Glance

- Client: `WebSocketTransport`, `WebSocketConfig` (feature: `websocket`)
- Server (optional): `pmcp::server::transport::websocket::{WebSocketServerTransport, WebSocketServerConfig}` (feature: `websocket`)
- Persistent connection with JSON text frames; ping/pong keepalive

## Client: Connect to a WebSocket Server

```rust
use pmcp::{Client, ClientCapabilities, WebSocketTransport, WebSocketConfig};
use std::time::Duration;
use url::Url;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // Connect to an existing MCP server that speaks WebSocket
    let cfg = WebSocketConfig {
        url: Url::parse("ws://localhost:3000/mcp")?,
        auto_reconnect: true,
        reconnect_delay: Duration::from_secs(1),
        max_reconnect_delay: Duration::from_secs(30),
        max_reconnect_attempts: Some(5),
        ping_interval: Some(Duration::from_secs(30)),
        request_timeout: Duration::from_secs(30),
    };

    let transport = WebSocketTransport::new(cfg);
    transport.connect().await?;

    let mut client = Client::new(transport);
    let _info = client.initialize(ClientCapabilities::minimal()).await?;

    // Use the client normally (list tools, call tools, etc.)
    Ok(())
}
```

See `examples/13_websocket_transport.rs` for a complete walkthrough.

## Server (Optional): Accept WebSocket Connections

PMCP includes a WebSocket server transport for custom scenarios. It yields a `Transport` you can pass to `server.run(...)` after accepting a connection. For most production servers, use Streamable HTTP.

```rust
use pmcp::{Server, ServerCapabilities, ToolHandler, RequestHandlerExtra};
use async_trait::async_trait;
use serde_json::{json, Value};
use pmcp::server::transport::websocket::{WebSocketServerTransport, WebSocketServerConfig};

struct Echo;

#[async_trait]
impl ToolHandler for Echo {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "echo": args }))
    }
}

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    let server = Server::builder()
        .name("ws-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool("echo", Echo)
        .build()?;

    let mut ws = WebSocketServerTransport::new(WebSocketServerConfig::default());
    ws.bind().await?;   // Start listening (default 127.0.0.1:9001)
    ws.accept().await?; // Accept one connection

    // Run server over this transport (handles requests from that connection)
    server.run(ws).await
}
```

See `examples/27_websocket_server_enhanced.rs` for a multi‑client demo and additional capabilities.

## Feature Flags

```toml
[dependencies]
pmcp = { version = "1.7", features = ["websocket"] }
```

## When to Use WebSocket

- Full-duplex, interactive sessions with low latency
- Custom desktop/native apps that prefer persistent connections
- Integration with existing WS gateways or load balancers

Prefer Streamable HTTP for most cloud/server deployments (SSE notifications, session management, and firewall friendliness).

