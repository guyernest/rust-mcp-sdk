::: exercise
id: ch13-01-jwt-validation
difficulty: intermediate
time: 50 minutes
:::

Implement secure JWT token validation for your MCP server. This is your first
security implementation, connecting user identity to tool authorization.

::: objectives
thinking:
  - Why API keys are insufficient for enterprise security
  - The JWT structure: header, payload, signature
  - The difference between 401 Unauthorized and 403 Forbidden
doing:
  - Configure JWKS endpoint for public key fetching
  - Build validation middleware that extracts and verifies tokens
  - Propagate user identity to tool handlers
  - Implement scope-based authorization
:::

::: discussion
- What information does your MCP server currently know about who is calling it?
- Why can't you just check username/password on every request?
- What happens if you deploy an API without authentication?
:::

## Step 1: Add Dependencies

In `Cargo.toml`:

```toml
[dependencies]
jsonwebtoken = "9"
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
```

## Step 2: Configure JWT Validation

```rust
use pmcp::server::auth::{JwtValidator, ValidationConfig};

// For AWS Cognito
let config = ValidationConfig::cognito(
    "us-east-1",           // AWS region
    "us-east-1_xxxxxx",    // User pool ID
    "your-client-id",      // App client ID
);

// For other OIDC providers
let config = ValidationConfig::new()
    .issuer("https://your-idp.com")
    .audience("your-client-id")
    .jwks_url("https://your-idp.com/.well-known/jwks.json");

let validator = JwtValidator::new().with_config(config);
```

## Step 3: Build Auth Middleware

```rust
use pmcp::server::auth::{AuthContext, ServerHttpMiddleware};

pub struct OAuthMiddleware {
    validator: JwtValidator,
}

#[async_trait]
impl ServerHttpMiddleware for OAuthMiddleware {
    async fn on_request(
        &self,
        request: &HttpRequest,
        context: &mut ServerHttpContext,
    ) -> Result<Option<HttpResponse>> {
        // Extract Bearer token
        let token = request
            .headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .ok_or_else(|| PmcpError::unauthorized("Missing authorization header"))?;

        // Validate JWT
        let claims = self.validator
            .validate(token)
            .await
            .map_err(|e| PmcpError::unauthorized(format!("Invalid token: {}", e)))?;

        // Store user info in context
        context.set_auth(AuthContext::from_claims(claims));

        Ok(None)  // Continue to handler
    }
}
```

## Step 4: Access User in Tools

```rust
pub async fn admin_tool(
    input: AdminInput,
    context: &ToolContext,
) -> Result<Output> {
    // Get authenticated user
    let auth = context.auth()
        .ok_or_else(|| PmcpError::unauthorized("Not authenticated"))?;

    // Check required scope
    if !auth.has_scope("admin:write") {
        return Err(PmcpError::forbidden("Requires admin:write scope"));
    }

    // Log the action for audit
    tracing::info!(
        user_id = %auth.user_id(),
        action = "admin_operation",
        "Admin tool invoked"
    );

    // ... implement tool
}
```

## Step 5: Add to Server

```rust
let server = ServerBuilder::new("secure-server", "1.0.0")
    .with_auth(OAuthMiddleware::new(validator))
    .with_tool(tools::AdminTool)
    .build()?;
```

## Step 6: Test Authentication

```bash
# Get a token from your IdP (varies by provider)
TOKEN=$(aws cognito-idp initiate-auth ...)

# Test with valid token
curl -X POST http://localhost:3000/mcp \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"admin_tool"},"id":1}'

# Test without token - should return 401
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"admin_tool"},"id":1}'
```

::: hints
level_1: "JWKS is a JSON endpoint that returns public keys. For Cognito: https://cognito-idp.{region}.amazonaws.com/{poolId}/.well-known/jwks.json"
level_2: "Cache JWKS responses with a 1-hour TTL to avoid fetching on every request."
level_3: "Log authentication failures internally but return generic messages to clients to avoid leaking information."
:::

## Success Criteria

- [ ] JwtValidator configured with correct issuer and audience
- [ ] Middleware extracts Bearer token from Authorization header
- [ ] Valid tokens pass, expired/invalid tokens return 401
- [ ] User info accessible in tool handlers via context.auth()
- [ ] Scope checking works for admin-only operations
- [ ] Proper 401/403 error responses

---

*Next: [Identity Providers](../../part5-security/ch14-providers.md) for integrating with Cognito, Auth0, and more.*
