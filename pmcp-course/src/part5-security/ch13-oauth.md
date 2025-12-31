# OAuth for MCP

Enterprise MCP servers must authenticate users properly. API keys are not sufficient. This chapter covers OAuth 2.0 implementation.

## Why OAuth, Not API Keys

Many tutorials show API key authentication:

```bash
# DON'T DO THIS in production
curl -H "X-API-Key: sk_live_abc123" http://mcp-server/tools
```

**Problems with API keys:**

| Issue | Impact |
|-------|--------|
| No user identity | Can't audit who did what |
| Hard to rotate | Changing keys breaks all clients |
| No granular permissions | Key has full access or none |
| Easy to leak | Shows up in logs, git history |
| No federation | Can't integrate with enterprise IdP |

**OAuth 2.0 solves these:**

| Feature | Benefit |
|---------|---------|
| User identity | JWT contains user info |
| Token expiration | Automatic rotation |
| Scopes | Fine-grained permissions |
| Standard protocol | Works with existing IdPs |
| Audit trail | Every request tied to a user |

## OAuth 2.0 for MCP: Quick Overview

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   MCP       │     │   MCP       │     │  Identity   │
│   Client    │────▶│   Server    │────▶│  Provider   │
│ (ChatGPT)   │◀────│  (Your App) │◀────│ (Cognito)   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │
       │  1. Connect       │                   │
       ├──────────────────▶│                   │
       │                   │  2. Redirect to   │
       │◀──────────────────┤     IdP login     │
       │                   │                   │
       │  3. User logs in  │                   │
       ├───────────────────┼──────────────────▶│
       │                   │                   │
       │  4. Auth code     │                   │
       │◀──────────────────┼───────────────────│
       │                   │                   │
       │  5. Exchange for  │                   │
       │     access token  │                   │
       ├──────────────────▶│  6. Validate      │
       │                   ├──────────────────▶│
       │                   │◀──────────────────│
       │  7. Tool calls    │                   │
       │     with token    │                   │
       ├──────────────────▶│  8. Verify JWT    │
       │◀──────────────────│                   │
```

## Adding OAuth to Your Server

### Using cargo pmcp

The easiest way to add OAuth:

```bash
# Initialize OAuth with Cognito
cargo pmcp deploy init --target pmcp-run --oauth cognito

# Or with Auth0
cargo pmcp deploy init --target pmcp-run --oauth auth0
```

This generates the necessary configuration and middleware.

### Manual Setup

For more control, add OAuth manually:

```rust
// src/main.rs
use pmcp::prelude::*;
use pmcp::server::auth::{JwtValidator, ValidationConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Configure JWT validation
    let jwt_config = ValidationConfig::cognito(
        "us-east-1",           // AWS region
        "us-east-1_xxxxxx",    // User pool ID
        "your-client-id",      // App client ID
    );

    let validator = JwtValidator::new()
        .with_config(jwt_config);

    // Build server with OAuth middleware
    let server = ServerBuilder::new("secure-server", "1.0.0")
        .with_auth(validator)
        .with_tool(tools::SecureTool)
        .build()?;

    server_common::create_http_server(server)
        .serve("0.0.0.0:3000")
        .await
}
```

### The OAuth Middleware

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

        // Store user info in context for tools to access
        context.set_auth(AuthContext::from_claims(claims));

        Ok(None)  // Continue to handler
    }
}
```

### Accessing User Info in Tools

```rust
#[derive(TypedTool)]
#[tool(name = "get_my_data", description = "Get data for the authenticated user")]
pub struct GetMyData;

impl GetMyData {
    pub async fn run(
        &self,
        _input: (),
        context: &ToolContext,
    ) -> Result<UserData> {
        // Get authenticated user from context
        let auth = context.auth()
            .ok_or_else(|| PmcpError::unauthorized("Not authenticated"))?;

        let user_id = auth.user_id();
        let email = auth.email();
        let scopes = auth.scopes();

        // Check for required scope
        if !scopes.contains(&"read:data".to_string()) {
            return Err(PmcpError::forbidden("Missing read:data scope"));
        }

        // Fetch user's data
        let data = self.database.get_user_data(user_id).await?;

        Ok(data)
    }
}
```

## Token Validation

### JWT Structure

A JWT token contains:

```json
{
  "header": {
    "alg": "RS256",
    "kid": "key-id-123"
  },
  "payload": {
    "sub": "user-123",
    "email": "user@example.com",
    "scope": "read:tools write:tools",
    "iss": "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxx",
    "aud": "client-id",
    "exp": 1699999999,
    "iat": 1699996399
  },
  "signature": "..."
}
```

### Validation Steps

```rust
impl JwtValidator {
    pub async fn validate(&self, token: &str) -> Result<Claims> {
        // 1. Decode header to get key ID
        let header = decode_header(token)?;
        let kid = header.kid.ok_or("Missing key ID")?;

        // 2. Fetch JWKS from IdP (cached)
        let jwks = self.get_jwks().await?;
        let key = jwks.find(&kid).ok_or("Key not found")?;

        // 3. Verify signature
        let claims: Claims = decode(token, &key, &self.validation)?;

        // 4. Check expiration
        if claims.exp < current_time() {
            return Err("Token expired");
        }

        // 5. Check issuer
        if claims.iss != self.config.issuer {
            return Err("Invalid issuer");
        }

        // 6. Check audience
        if claims.aud != self.config.audience {
            return Err("Invalid audience");
        }

        Ok(claims)
    }
}
```

## Scope-Based Authorization

Define tool permissions with scopes:

```rust
#[derive(TypedTool)]
#[tool(
    name = "delete_customer",
    description = "Delete a customer record",
    annotations(destructive = true)
)]
pub struct DeleteCustomer;

impl DeleteCustomer {
    pub async fn run(&self, input: DeleteInput, context: &ToolContext) -> Result<()> {
        let auth = context.auth().ok_or(PmcpError::unauthorized("Not authenticated"))?;

        // Require admin scope for destructive operations
        if !auth.has_scope("admin:customers") {
            return Err(PmcpError::forbidden(
                "This operation requires admin:customers scope"
            ));
        }

        // Log the action for audit
        tracing::info!(
            user_id = %auth.user_id(),
            customer_id = %input.customer_id,
            "Deleting customer"
        );

        self.database.delete_customer(&input.customer_id).await?;

        Ok(())
    }
}
```

## Multi-Tenant Configuration

For servers supporting multiple organizations:

```rust
pub struct MultiTenantValidator {
    validators: HashMap<String, JwtValidator>,
}

impl MultiTenantValidator {
    pub async fn validate(&self, token: &str) -> Result<Claims> {
        // Decode without verification to get issuer
        let unverified = decode_unverified(token)?;
        let issuer = &unverified.iss;

        // Find validator for this tenant
        let validator = self.validators
            .get(issuer)
            .ok_or_else(|| PmcpError::unauthorized("Unknown issuer"))?;

        // Validate with tenant-specific config
        validator.validate(token).await
    }
}
```

## Error Handling

Return proper OAuth errors:

```rust
// 401 Unauthorized - missing or invalid credentials
PmcpError::unauthorized("Invalid or expired token")

// 403 Forbidden - valid credentials but insufficient permissions
PmcpError::forbidden("Insufficient scope for this operation")

// Include WWW-Authenticate header for 401
HttpResponse::unauthorized()
    .header("WWW-Authenticate", "Bearer realm=\"mcp\", error=\"invalid_token\"")
```

## Testing OAuth

### Mock Validator for Tests

```rust
#[cfg(test)]
mod tests {
    use pmcp::server::auth::MockValidator;

    #[tokio::test]
    async fn test_requires_authentication() {
        let server = build_test_server().await;

        // Without token - should fail
        let response = server.call_tool("get_my_data", json!({})).await;
        assert_eq!(response.error.code, -32001);  // Unauthorized

        // With valid token - should succeed
        let response = server
            .with_auth(MockValidator::user("test-user"))
            .call_tool("get_my_data", json!({}))
            .await;
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_requires_admin_scope() {
        let server = build_test_server().await;

        // With regular user - should fail
        let response = server
            .with_auth(MockValidator::user("regular-user"))
            .call_tool("delete_customer", json!({"id": "123"}))
            .await;
        assert_eq!(response.error.code, -32003);  // Forbidden

        // With admin - should succeed
        let response = server
            .with_auth(MockValidator::admin("admin-user"))
            .call_tool("delete_customer", json!({"id": "123"}))
            .await;
        assert!(response.error.is_none());
    }
}
```

## Security Best Practices

1. **Always validate tokens server-side** - Don't trust client claims
2. **Use short-lived tokens** - 1 hour maximum for access tokens
3. **Implement token refresh** - Don't force users to re-authenticate
4. **Log authentication events** - For security auditing
5. **Use HTTPS only** - Never send tokens over HTTP
6. **Rotate signing keys** - Follow your IdP's key rotation schedule
7. **Validate all claims** - issuer, audience, expiration, etc.

## Knowledge Check

Test your understanding of OAuth for MCP:

{{#quiz ../quizzes/ch13-oauth.toml}}

## Practice Ideas

These informal exercises help reinforce the concepts. For structured exercises with starter code and tests, see the chapter exercise pages.

1. **Add OAuth to calculator**: Implement authentication for your calculator server

2. **Implement scope checking**: Create tools that require different scopes

3. **Add audit logging**: Log all authenticated requests with user info

4. **Test with real IdP**: Set up a Cognito user pool and test end-to-end

---

*Continue to [OAuth 2.0 Fundamentals](./ch13-02-oauth-basics.md) →*
