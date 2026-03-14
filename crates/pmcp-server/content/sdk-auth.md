# Authentication

PMCP supports OAuth 2.0, API key, and JWT authentication through middleware.

## OAuth 2.0 Middleware

```rust
use pmcp::server::auth::{OAuthMiddleware, OAuthConfig};

let oauth = OAuthMiddleware::new(OAuthConfig {
    issuer_url: "https://auth.example.com".into(),
    audience: Some("my-api".into()),
    ..Default::default()
});

server_builder.middleware(oauth);
```

The middleware validates Bearer tokens on incoming requests and rejects
unauthenticated calls with a 401 response.

## API Key Authentication

```rust
use pmcp::server::auth::ApiKeyMiddleware;

let api_key = ApiKeyMiddleware::new("X-API-Key", vec![
    "sk-live-abc123".into(),
]);

server_builder.middleware(api_key);
```

API keys are checked against the provided header name. Multiple valid
keys can be configured for key rotation.

## JWT Validation

```rust
use pmcp::server::auth::JwtMiddleware;

let jwt = JwtMiddleware::builder()
    .issuer("https://auth.example.com")
    .audience("my-api")
    .jwks_url("https://auth.example.com/.well-known/jwks.json")
    .build()?;

server_builder.middleware(jwt);
```

## Middleware Chain

Combine multiple auth methods:

```rust
server_builder
    .middleware(rate_limiter)
    .middleware(oauth)
    .middleware(logging);
```

Middleware executes in registration order. The first middleware to reject
a request short-circuits the chain.

## Accessing Auth Info

Inside a tool handler, access the authenticated identity:

```rust
async fn call(&self, input: Self::Input, extra: RequestHandlerExtra)
    -> pmcp::Result<pmcp::CallToolResult>
{
    // Auth info is available via extra if middleware populated it
    let _meta = extra.meta();
    // Perform authorized operation
    Ok(pmcp::CallToolResult::text("done"))
}
```
