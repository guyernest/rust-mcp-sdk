# Chapter 10.2: HTTP Transport (Client)

PMCP’s HTTP transport is a client-side implementation that sends JSON-RPC requests over HTTP and can optionally subscribe to Server-Sent Events (SSE) for notifications. For server deployments, see Streamable HTTP in Chapter 10.3.

## When to Use HTTP Transport

- Simple request/response interactions against an HTTP MCP endpoint
- Optional SSE notifications via a separate endpoint
- Firewall-friendly and proxy-compatible

If you control the server, prefer Streamable HTTP (single endpoint with JSON and SSE, plus session support).

## Features and Types

- `HttpTransport`, `HttpConfig` (feature: `http`)
- Optional connection pooling and configurable timeouts
- Optional `sse_endpoint` for notifications

## Basic Client Example

```rust
use pmcp::{Client, ClientCapabilities, HttpTransport, HttpConfig};
use url::Url;

#[tokio::main]
async fn main() -> pmcp::Result<()> {
    // Create HTTP transport
    let transport = HttpTransport::with_url(Url::parse("http://localhost:8080")?)?;

    // Optionally connect to SSE notifications if your server exposes one
    // transport.connect_sse().await?;

    // Build client and initialize
    let mut client = Client::new(transport);
    let _info = client.initialize(ClientCapabilities::minimal()).await?;

    // Use the client as usual (list tools, call tools, etc.)
    Ok(())
}
```

## Configuration

```rust
use pmcp::HttpConfig;
use url::Url;
use std::time::Duration;

let cfg = HttpConfig {
    base_url: Url::parse("https://api.example.com/mcp")?,
    sse_endpoint: Some("/events".into()), // or None if not using SSE
    timeout: Duration::from_secs(30),
    headers: vec![("Authorization".into(), "Bearer <token>".into())],
    enable_pooling: true,
    max_idle_per_host: 10,
};
```

## Feature Flags

```toml
[dependencies]
pmcp = { version = "1.7", features = ["http"] }
```

## Notes

- This client can target generic HTTP-style MCP servers.
- For PMCP’s recommended server implementation, see Streamable HTTP (Axum-based) in Chapter 10.3.

