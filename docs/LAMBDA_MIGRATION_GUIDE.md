# Lambda Migration Guide: Simplifying MCP Server Boilerplate

This guide is for teams running MCP servers on AWS Lambda using the `StreamableHttpServer` + reqwest proxy pattern. The v2.0 SDK eliminates most of this boilerplate with `pmcp::axum::router()` and built-in security layers.

## What Changes

| Before (v1.x) | After (v2.0) | Impact |
|----------------|-------------|--------|
| `StreamableHttpServerConfig { ... }` (7 fields) | `StreamableHttpServerConfig::stateless()` | One-liner config |
| Hand-rolled CORS (`access-control-allow-origin: *`) | Built-in `CorsLayer` with origin locking | CVE fix (no wildcard CORS) |
| Hand-rolled OPTIONS handler in Lambda | `CorsLayer` handles preflight automatically | Delete ~10 lines per handler |
| reqwest proxy: Lambda → background HTTP → response | `pmcp::axum::router()` + Lambda Web Adapter | Eliminate reqwest entirely |
| `OnceCell` + `ensure_server_started` + `start_http_in_background` | Direct `axum::serve` or Lambda Web Adapter | ~80 fewer lines per Lambda |
| No DNS rebinding protection | `DnsRebindingLayer` validates Host/Origin headers | Security by default |
| No security response headers | `SecurityHeadersLayer` adds nosniff, DENY, no-store | Security by default |

## Migration Path 1: Simplify server-common (minimal change)

If you want to keep the `StreamableHttpServer` pattern but pick up the security improvements, just add the `allowed_origins` field:

### Before (server-common/src/lib.rs)

```rust
let config = StreamableHttpServerConfig {
    session_id_generator: None,
    enable_json_response: true,
    event_store: None,
    on_session_initialized: None,
    on_session_closed: None,
    http_middleware: Some(Arc::new(middleware_chain)),
};
```

### After

```rust
let config = StreamableHttpServerConfig {
    session_id_generator: None,
    enable_json_response: true,
    event_store: None,
    on_session_initialized: None,
    on_session_closed: None,
    http_middleware: Some(Arc::new(middleware_chain)),
    allowed_origins: None, // auto-detects from bind address
};
```

The `start()` method now automatically applies:
- `DnsRebindingLayer` — validates Host and Origin headers
- `SecurityHeadersLayer` — adds X-Content-Type-Options, X-Frame-Options, Cache-Control
- `CorsLayer` — origin-locked CORS (no more wildcard `*`)

**That's it.** One field addition. Your existing `ServerHttpMiddleware` (logging, OAuth) continues to work — Tower layers wrap outside, custom middleware runs inside.

## Migration Path 2: Replace Lambda proxy with pmcp::axum::router() (recommended)

The current Lambda handlers have ~170 lines of proxy boilerplate:
- `OnceCell<String>` for base URL
- `OnceCell<Client>` for reqwest
- `start_http_in_background()` — builds server, binds to localhost, returns address
- `ensure_server_started()` — once-init guard
- `handler()` — extracts method/path/headers/body, forwards via reqwest, copies response back
- Hand-rolled CORS and health check responses

All of this exists because Lambda needs to proxy HTTP to the MCP server. With `pmcp::axum::router()`, you can use [Lambda Web Adapter](https://github.com/awslabs/aws-lambda-web-adapter) to serve Axum directly — no proxy needed.

### Before: calculator-lambda/src/main.rs (~177 lines)

```rust
use lambda_http::{run, service_fn, Body, Error, Request, Response};
use once_cell::sync::OnceCell;
use reqwest::Client;

static BASE_URL: OnceCell<String> = OnceCell::new();
static HTTP: OnceCell<Client> = OnceCell::new();

async fn build_server() -> pmcp::Result<pmcp::Server> { ... }
async fn start_http_in_background(...) -> pmcp::Result<SocketAddr> { ... }
async fn ensure_server_started() -> Result<String, Error> { ... }

async fn handler(event: Request) -> Result<Response<Body>, Error> {
    // 20 lines: health check with hand-rolled CORS
    // 10 lines: OPTIONS preflight with hand-rolled CORS
    // 40 lines: reqwest proxy (copy headers, copy body, forward, copy response)
    // 5 lines: inject access-control-allow-origin: * on response
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()...init();
    run(service_fn(handler)).await
}
```

### After: calculator-lambda/src/main.rs (~25 lines)

```rust
use pmcp::axum::{router, AllowedOrigins, RouterConfig};
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .init();

    let server = mcp_calculator_core::build_calculator_server()
        .await
        .expect("Failed to build server");

    let server = Arc::new(Mutex::new(server));

    // One line: secure MCP server with DNS rebinding protection + CORS
    let app = router(server);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("Failed to bind");

    tracing::info!("Calculator MCP server started on 127.0.0.1:8080");
    axum::serve(listener, app).await.expect("Server failed");
}
```

**Removed:** `OnceCell`, `reqwest::Client`, `ensure_server_started`, `start_http_in_background`, hand-rolled CORS, hand-rolled OPTIONS, proxy logic. **~150 lines deleted per Lambda.**

**Deploy with Lambda Web Adapter** — add this to your Dockerfile:

```dockerfile
COPY --from=public.ecr.aws/awsguru/aws-lambda-web-adapter:0.8 /lambda-adapter /opt/extensions/lambda-adapter
ENV PORT=8080
```

The Lambda Web Adapter forwards Lambda events to your Axum server's HTTP port. No reqwest proxy needed.

### With custom allowed origins (production)

```rust
let app = router_with_config(server, RouterConfig {
    allowed_origins: Some(AllowedOrigins::explicit(vec![
        "https://myapp.example.com".to_string(),
        "https://admin.example.com".to_string(),
    ])),
    ..Default::default()
});
```

## Migration Path 3: Simplify server-common/run_http() (container/ECS)

For the non-Lambda path (containers, ECS, direct binary), `run_http()` in server-common can also use `pmcp::axum::router()`:

### Before: server-common run_http() (~100 lines)

```rust
pub async fn run_http(server: Server, server_name: &str, ...) -> Result<...> {
    init_logging();
    let auth_provider = init_auth_provider().await;
    let port = resolve_port();
    let addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port);
    let server = Arc::new(Mutex::new(server));

    let mut middleware_chain = ServerHttpMiddlewareChain::new();
    middleware_chain.add(Arc::new(ServerHttpLoggingMiddleware::new()));
    // ... auth middleware setup ...

    let config = StreamableHttpServerConfig {
        session_id_generator: None,
        enable_json_response: true,
        event_store: None,
        on_session_initialized: None,
        on_session_closed: None,
        http_middleware: Some(Arc::new(middleware_chain)),
    };

    let http_server = StreamableHttpServer::with_config(addr, server, config);
    let (bound_addr, server_handle) = http_server.start().await?;
    // ... health check loop ...
    // ... tokio::select! ...
}
```

### After: server-common run_http() (~40 lines)

```rust
use pmcp::axum::{router_with_config, AllowedOrigins, RouterConfig};
use pmcp::server::streamable_http_server::StreamableHttpServerConfig;

pub async fn run_http(server: Server, server_name: &str, ...) -> Result<...> {
    init_logging();
    let port = resolve_port();
    let server = Arc::new(Mutex::new(server));

    // Auth middleware still works — Tower layers wrap outside it
    let mut middleware_chain = ServerHttpMiddlewareChain::new();
    middleware_chain.add(Arc::new(ServerHttpLoggingMiddleware::new()));
    // ... auth middleware setup ...

    let app = router_with_config(server, RouterConfig {
        allowed_origins: None, // auto-detects localhost aliases
        server_config: StreamableHttpServerConfig {
            enable_json_response: true,
            http_middleware: Some(Arc::new(middleware_chain)),
            ..Default::default()
        },
        ..Default::default()
    });

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    let bound_addr = listener.local_addr()?;

    tracing::info!(address = %bound_addr, server_name = %server_name, "Server started");
    axum::serve(listener, app).await?;
    Ok(())
}
```

**What changed:**
- `StreamableHttpServer::with_config()` + `.start()` → `router_with_config()` + `axum::serve()`
- CORS is automatic (origin-locked, no wildcard)
- DNS rebinding protection is automatic
- Security headers are automatic
- `ServerHttpMiddleware` (logging, OAuth) still works unchanged

## Type Construction DX (bonus cleanup)

While migrating, also simplify your type construction. The v2.0 SDK adds constructors to all protocol types:

### Before

```rust
let resource = ResourceInfo {
    uri: "file://test.txt".to_string(),
    name: "test.txt".to_string(),
    title: None,
    description: Some("A test file".to_string()),
    mime_type: Some("text/plain".to_string()),
    icons: None,
    annotations: None,
    meta: None,
};

let content = Content::Text { text: "Hello".to_string() };

let message = PromptMessage {
    role: Role::User,
    content: Content::Text { text: "Hello".to_string() },
};
```

### After

```rust
let resource = ResourceInfo::new("file://test.txt", "test.txt")
    .with_description("A test file")
    .with_mime_type("text/plain");

let content = Content::text("Hello");

let message = PromptMessage::user(Content::text("Hello"));
```

No `.to_string()` noise. No explicit `None` padding. Forward-compatible — new optional fields in future SDK versions won't break your code.

## Security Improvements (automatic)

After migration, you get these security features **with zero configuration**:

| Feature | Protection |
|---------|-----------|
| DNS rebinding | `DnsRebindingLayer` validates Host + Origin headers, returns 403 on mismatch |
| CORS locking | Origin-locked CORS replaces wildcard `*` — fixes CVE-2025-66414 pattern |
| Security headers | `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Cache-Control: no-store` |
| Owner isolation | Task operations scoped to authenticated owner (via `AuthContext`) |

The wildcard `access-control-allow-origin: *` that your Lambda handlers currently inject is a known CVE pattern (CVE-2025-66414 in TypeScript SDK, CVE-2025-66416 in Python SDK). After migration, CORS is handled by the SDK's Tower layer — **delete all hand-rolled CORS headers from your Lambda handlers.**

## Checklist

- [ ] Add `allowed_origins: None` to `StreamableHttpServerConfig` struct literals (or use `..Default::default()`)
- [ ] Delete hand-rolled `access-control-allow-origin: *` headers from Lambda handlers
- [ ] Delete hand-rolled OPTIONS handler from Lambda handlers
- [ ] (Optional) Replace reqwest proxy with `pmcp::axum::router()` + Lambda Web Adapter
- [ ] (Optional) Migrate type construction to `::new()` + `.with_*()` pattern
- [ ] (Optional) Replace `run_http()` internals with `router_with_config()` + `axum::serve()`
- [ ] Verify: `grep -r "access-control-allow-origin.*\*"` returns zero matches
