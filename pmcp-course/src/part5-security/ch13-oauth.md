# OAuth for MCP

Enterprise MCP servers must authenticate users properly. API keys are not sufficient. This chapter covers OAuth 2.0 implementation.

Authentication answers "who is making this request?" Authorization answers "are they allowed to do this?" OAuth 2.0 provides both, using industry-standard protocols that integrate with existing enterprise identity systems.

**What you'll learn:**
- Why API keys are insufficient for production
- How OAuth 2.0 flow works with MCP
- Implementing JWT validation in your server
- Scope-based authorization for tools
- Testing authenticated endpoints

## Why OAuth, Not API Keys

API keys seem simple—generate a secret, include it in requests, check it on the server. But this simplicity hides serious problems that become critical in production environments.

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

OAuth 2.0 separates authentication (verifying identity) from your application. Users authenticate with a trusted Identity Provider (IdP) like AWS Cognito, Auth0, or Okta. The IdP issues tokens that your server validates. This means you never handle passwords—a significant security advantage.

The flow below shows how an MCP client (like Claude Desktop) authenticates with your server through an IdP:

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

Adding OAuth involves two parts: configuring your Identity Provider (outside your code) and adding validation middleware to your server (in your code). The middleware intercepts every request, extracts the JWT token, validates it, and makes user information available to your tools.

### Using cargo pmcp

The easiest way to add OAuth—this generates the boilerplate configuration and middleware:

```bash
# Initialize OAuth with Cognito
cargo pmcp deploy init --target pmcp-run --oauth cognito

# Or with Auth0
cargo pmcp deploy init --target pmcp-run --oauth auth0
```

This generates the necessary configuration and middleware.

### Manual Setup

For more control or custom IdP configurations, add OAuth manually. The key components are: a `ValidationConfig` that describes your IdP, a `JwtValidator` that uses that config, and middleware that applies validation to every request.

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

The middleware runs before every request handler. It extracts the token from the `Authorization` header, validates it with the IdP's public key, and stores the validated claims in the request context. If validation fails, the request is rejected immediately—your tool code never runs.

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

Once authentication succeeds, your tools can access user information through the context. This enables personalized behavior (fetch this user's data), authorization checks (does this user have permission?), and audit logging (who performed this action?).

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

JWT (JSON Web Token) validation is the core of OAuth security. A JWT is a signed JSON document—the IdP signs it with a private key, and your server verifies it with the corresponding public key. If the signature is valid and the claims are correct, you can trust the token's contents.

**Why this matters:** Anyone can create a JSON document claiming to be "admin". The cryptographic signature proves the IdP created the token, and the claims (expiration, issuer, audience) prove it's valid for your server.

### JWT Structure

A JWT has three parts (header, payload, signature), each Base64-encoded and separated by dots. Understanding this structure helps you debug authentication issues:

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

Each validation step catches a different type of attack or misconfiguration. Skipping any step creates a security vulnerability:

1. **Decode header** → Get the key ID to find the right public key
2. **Fetch JWKS** → Get the IdP's public keys (cached for performance)
3. **Verify signature** → Prove the IdP issued this token
4. **Check expiration** → Reject old tokens (prevents replay attacks)
5. **Check issuer** → Ensure token came from your IdP (prevents cross-tenant attacks)
6. **Check audience** → Ensure token was meant for your app (prevents token reuse)

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

Scopes are permission labels attached to tokens. When a user authenticates, the IdP includes scopes based on their role or permissions. Your tools check these scopes to decide what operations to allow.

**Common scope patterns:**
- `read:resource` / `write:resource` — Read/write separation
- `admin:resource` — Administrative operations
- `resource:action` — Fine-grained actions (e.g., `customers:delete`)

Scopes let you implement least-privilege access: users get only the permissions they need.

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

Multi-tenant MCP servers serve multiple organizations, each with their own IdP. A SaaS product might support customers using Okta, Auth0, or their own enterprise IdP. The server must validate tokens from any of these issuers while ensuring users from one tenant can't access another tenant's data.

The key insight: decode the token's issuer claim first (without full validation), then use the issuer to select the appropriate validator.

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

OAuth errors must be precise—clients need to know whether to retry with a new token (401) or inform the user they lack permissions (403). Getting this wrong frustrates users and makes debugging harder.

**401 Unauthorized** — "I don't know who you are"
- Missing token, expired token, invalid signature
- Client should re-authenticate

**403 Forbidden** — "I know who you are, but you can't do this"
- Valid token but insufficient scopes
- Client should inform user, not retry

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

Testing authenticated endpoints is tricky—you don't want tests depending on a real IdP. The solution: mock validators that simulate authentication without network calls. Your tests can create any user identity and scope combination.

**Testing strategies:**
- **Unit tests:** Mock validator with configurable users/scopes
- **Integration tests:** Test against a local IdP (like Keycloak in Docker)
- **E2E tests:** Test against your staging IdP with test accounts

### Mock Validator for Tests

The mock validator lets you test any authentication scenario without real tokens:

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

These practices come from real-world OAuth incidents. Each addresses a specific attack vector:

1. **Always validate tokens server-side** - Don't trust client claims. Clients can be compromised.
2. **Use short-lived tokens** - 1 hour maximum for access tokens
3. **Implement token refresh** - Don't force users to re-authenticate
4. **Log authentication events** - For security auditing
5. **Use HTTPS only** - Never send tokens over HTTP
6. **Rotate signing keys** - Follow your IdP's key rotation schedule
7. **Validate all claims** - issuer, audience, expiration, etc.

## Exercises

1. **Add OAuth to calculator**: Implement authentication for your calculator server

2. **Implement scope checking**: Create tools that require different scopes

3. **Add audit logging**: Log all authenticated requests with user info

4. **Test with real IdP**: Set up a Cognito user pool and test end-to-end

---

*Continue to [OAuth 2.0 Fundamentals](./ch13-02-oauth-basics.md) →*
