# Authentication and Security

Authentication in MCP is about **trust and accountability**. When an MCP server exposes tools that access user data, modify resources, or perform privileged operations, it must act on behalf of an authenticated user and enforce their permissions. This chapter shows you how to build secure MCP servers using industry-standard OAuth 2.0 and OpenID Connect (OIDC).

## The Philosophy: Don't Reinvent the Wheel

> **MCP servers should behave like web servers** - they validate access tokens sent with requests and enforce authorization policies. They should **not** implement custom authentication flows.

### Why This Matters

**Security is hard.** Every custom authentication system introduces risk:
- Password storage vulnerabilities
- Token generation weaknesses
- Session management bugs
- Timing attack vulnerabilities
- Missing rate limiting
- Inadequate audit logging

**Your organization already solved this.** Most organizations have:
- Single Sign-On (SSO) systems
- OAuth 2.0 / OIDC providers (Auth0, Okta, Keycloak, Azure AD)
- Established user directories (LDAP, Active Directory)
- Proven authorization policies
- Security auditing and compliance

**MCP should integrate, not duplicate.** Instead of building a new authentication system, MCP servers should:
1. **Receive access tokens** from MCP clients
2. **Validate tokens** against your existing OAuth provider
3. **Extract user identity** from validated tokens
4. **Enforce permissions** based on scopes and roles
5. **Act on behalf of the user** with their privileges

This approach provides:
- ✅ Centralized user management
- ✅ Consistent security policies across all applications
- ✅ Audit trails showing which user performed which action
- ✅ SSO integration (login once, access all MCP servers)
- ✅ Industry-standard security reviewed by experts

## How MCP Authentication Works

MCP uses the same pattern as securing REST APIs:

```
┌─────────────┐                 ┌─────────────┐                 ┌─────────────┐
│             │                 │             │                 │             │
│  MCP Client │                 │ MCP Server  │                 │   OAuth     │
│             │                 │             │                 │  Provider   │
└──────┬──────┘                 └──────┬──────┘                 └──────┬──────┘
       │                               │                               │
       │  1. User authenticates        │                               │
       ├──────────────────────────────────────────────────────────────>│
       │      (OAuth flow happens externally)                          │
       │                               │                               │
       │<─────────────────────────────────────────────────────────────┤
       │  2. Receive access token      │                               │
       │                               │                               │
       │  3. Call tool with token      │                               │
       ├──────────────────────────────>│                               │
       │  Authorization: Bearer <token>│                               │
       │                               │                               │
       │                               │  4. Validate token            │
       │                               ├──────────────────────────────>│
       │                               │                               │
       │                               │<──────────────────────────────┤
       │                               │  5. Token valid + user info   │
       │                               │                               │
       │  6. Result (as authenticated  │                               │
       │<─────────────────────────────┤│                               │
       │     user)                     │                               │
```

**Key principles:**

1. **OAuth flow is external** - The client handles authorization code flow, token exchange, and refresh
2. **MCP server validates tokens** - Server checks tokens against OAuth provider for each request
3. **User context is propagated** - Tools know which user is calling them
4. **Permissions are enforced** - Scopes and roles control what each user can do

## OAuth 2.0 and OIDC Primer

Before diving into code, let's understand the key concepts.

### OAuth 2.0: Delegated Authorization

OAuth 2.0 lets users grant applications limited access to their resources without sharing passwords.

**Key terms:**
- **Resource Owner**: The user who owns the data
- **Client**: The application (MCP client) requesting access
- **Authorization Server**: Issues tokens after authenticating the user (e.g., Auth0, Okta)
- **Resource Server**: Protects user resources, validates tokens (your MCP server)
- **Access Token**: Short-lived token proving authorization (typically JWT)
- **Refresh Token**: Long-lived token used to get new access tokens
- **Scope**: Permission level (e.g., "read:data", "write:data", "admin")

### OpenID Connect (OIDC): Identity Layer

OIDC extends OAuth 2.0 to provide user identity information.

**Additional concepts:**
- **ID Token**: JWT containing user identity claims (name, email, etc.)
- **UserInfo Endpoint**: Returns additional user profile information
- **Discovery**: `.well-known/openid-configuration` endpoint for provider metadata

### Authorization Code Flow (Recommended)

This is the most secure flow for MCP clients:

1. **Client redirects user to authorization endpoint**
   ```
   https://auth.example.com/authorize?
     response_type=code&
     client_id=mcp-client-123&
     redirect_uri=http://localhost:3000/callback&
     scope=openid profile read:tools write:tools&
     state=random-state-value
   ```

2. **User authenticates and grants permission**

3. **Authorization server redirects back with code**
   ```
   http://localhost:3000/callback?code=auth_code_xyz&state=random-state-value
   ```

4. **Client exchanges code for tokens** (backend, not browser)
   ```bash
   POST https://auth.example.com/token
   Content-Type: application/x-www-form-urlencoded

   grant_type=authorization_code&
   code=auth_code_xyz&
   client_id=mcp-client-123&
   client_secret=client_secret_abc&
   redirect_uri=http://localhost:3000/callback
   ```

5. **Receive tokens**
   ```json
   {
     "access_token": "eyJhbGc...",
     "token_type": "Bearer",
     "expires_in": 3600,
     "refresh_token": "refresh_xyz...",
     "scope": "openid profile read:tools write:tools"
   }
   ```

6. **Client uses access token in MCP requests**
   ```json
   {
     "jsonrpc": "2.0",
     "id": 1,
     "method": "tools/call",
     "params": {
       "name": "read_data",
       "arguments": {"key": "user-123"}
     },
     "_meta": {
       "authorization": {
         "type": "bearer",
         "token": "eyJhbGc..."
       }
     }
   }
   ```

**Important:** Steps 1-5 happen **outside** the MCP server. The MCP server only sees step 6 (requests with tokens).

**Best practice for public clients:** Use **Authorization Code + PKCE** (Proof Key for Code Exchange) instead of implicit flow. PKCE prevents authorization code interception attacks even without client secrets.

## Building a Secure MCP Server

Let's build an MCP server that validates OAuth tokens and enforces permissions.

### Step 1: Configure OAuth Provider Integration

PMCP provides built-in OAuth provider integration:

```rust
use pmcp::server::auth::{InMemoryOAuthProvider, OAuthClient, OAuthProvider, GrantType, ResponseType};
use std::sync::Arc;
use std::collections::HashMap;

// Create OAuth provider (points to your real OAuth server)
let oauth_provider = Arc::new(
    InMemoryOAuthProvider::new("https://auth.example.com")
);

// Register your MCP client application
let client = OAuthClient {
    client_id: "mcp-client-123".to_string(),
    client_secret: Some("your-client-secret".to_string()),
    client_name: "My MCP Client".to_string(),
    redirect_uris: vec!["http://localhost:3000/callback".to_string()],
    grant_types: vec![
        GrantType::AuthorizationCode,
        GrantType::RefreshToken,
    ],
    response_types: vec![ResponseType::Code],
    scopes: vec![
        "openid".to_string(),
        "profile".to_string(),
        "read:tools".to_string(),
        "write:tools".to_string(),
        "admin".to_string(),
    ],
    metadata: HashMap::new(),
};

let registered_client = oauth_provider.register_client(client).await?;
```

**Note:** `InMemoryOAuthProvider` is for development. In production, integrate with your real OAuth provider:
- **Auth0**: Use Auth0 Management API
- **Okta**: Use Okta API
- **Keycloak**: Use Keycloak Admin API
- **Azure AD**: Use Microsoft Graph API
- **Custom**: Implement `OAuthProvider` trait

### Step 2: Create Authentication Middleware

Middleware validates tokens and extracts user context:

```rust
use pmcp::server::auth::middleware::{AuthMiddleware, BearerTokenMiddleware, ScopeMiddleware};
use std::sync::Arc;

// Scope middleware - enforces required scopes
// Create fresh instances for each middleware (BearerTokenMiddleware doesn't implement Clone)
let read_middleware = Arc::new(ScopeMiddleware::any(
    Box::new(BearerTokenMiddleware::new(oauth_provider.clone())),
    vec!["read:tools".to_string()],
));

let write_middleware = Arc::new(ScopeMiddleware::all(
    Box::new(BearerTokenMiddleware::new(oauth_provider.clone())),
    vec!["write:tools".to_string()],
));

let admin_middleware = Arc::new(ScopeMiddleware::all(
    Box::new(BearerTokenMiddleware::new(oauth_provider.clone())),
    vec!["admin".to_string()],
));
```

**Scope enforcement:**
- `ScopeMiddleware::any()` - Requires **at least one** of the specified scopes
- `ScopeMiddleware::all()` - Requires **all** of the specified scopes

### Step 3: Protect Tools with Authentication

Tools check authentication via middleware:

```rust
use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra};
use pmcp::error::{Error, ErrorCode};
use serde_json::{json, Value};
use tracing::info;
use chrono::Utc;

/// Public tool - no authentication required
struct GetServerTimeTool;

#[async_trait]
impl ToolHandler for GetServerTimeTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra)
        -> pmcp::Result<Value>
    {
        Ok(json!({
            "time": Utc::now().to_rfc3339(),
            "timezone": "UTC"
        }))
    }
}

/// Protected tool - requires authentication and 'read:tools' scope
struct ReadUserDataTool {
    auth_middleware: Arc<dyn AuthMiddleware>,
}

#[async_trait]
impl ToolHandler for ReadUserDataTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra)
        -> pmcp::Result<Value>
    {
        // Authenticate the request
        let auth_context = self.auth_middleware
            .authenticate(extra.auth_info.as_ref())
            .await?;

        info!("User {} accessing read_user_data", auth_context.subject);

        // Extract parameters
        let user_id = args.get("user_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_params("Missing 'user_id'"))?;

        // Authorization check: users can only read their own data
        if auth_context.subject != user_id && !auth_context.has_scope("admin") {
            return Err(Error::protocol(
                ErrorCode::PERMISSION_DENIED,
                format!("User {} cannot access data for user {}",
                    auth_context.subject, user_id)
            ));
        }

        // Fetch data (as the authenticated user)
        let data = fetch_user_data(user_id).await?;

        Ok(json!({
            "user_id": user_id,
            "data": data,
            "accessed_by": auth_context.subject,
            "scopes": auth_context.scopes
        }))
    }
}
```

**Security features demonstrated:**
1. **Authentication** - Validates token and extracts user identity
2. **Authorization** - Checks if user has permission for this specific action
3. **Audit logging** - Records who accessed what
4. **Scope validation** - Ensures required permissions are present

**Note:** Authentication happens once at the request boundary. The `RequestHandlerExtra.auth_context` is populated by middleware and passed to tools, avoiding re-authentication for each tool call and reducing logging noise.

### Step 4: Build and Run the Server

```rust
use pmcp::server::Server;
use pmcp::types::capabilities::ServerCapabilities;

let server = Server::builder()
    .name("secure-mcp-server")
    .version("1.0.0")
    .capabilities(ServerCapabilities {
        tools: Some(Default::default()),
        ..Default::default()
    })
    // Public tool - no auth
    .tool("get_server_time", GetServerTimeTool)
    // Protected tools - require auth + scopes
    .tool("read_user_data", ReadUserDataTool {
        auth_middleware: read_middleware,
    })
    .tool("write_user_data", WriteUserDataTool {
        auth_middleware: write_middleware,
    })
    .tool("admin_operation", AdminOperationTool {
        auth_middleware: admin_middleware,
    })
    .build()?;

// Run server
server.run_stdio().await?;
```

## OIDC Discovery

OIDC providers expose a discovery endpoint that provides all the metadata your client needs:

```rust
use pmcp::client::auth::OidcDiscoveryClient;
use std::time::Duration;
use tracing::info;

// Discover provider configuration
let discovery_client = OidcDiscoveryClient::with_settings(
    5,                          // max retries
    Duration::from_secs(1),     // retry delay
);

let metadata = discovery_client
    .discover("https://auth.example.com")
    .await?;

info!("Authorization endpoint: {}", metadata.authorization_endpoint);
info!("Token endpoint: {}", metadata.token_endpoint);
info!("Supported scopes: {:?}", metadata.scopes_supported);
```

**What you get from discovery:**
- `issuer` - Provider's identifier
- `authorization_endpoint` - Where to redirect users for login
- `token_endpoint` - Where to exchange codes for tokens
- `jwks_uri` - Public keys for validating JWT signatures
- `userinfo_endpoint` - Where to fetch user profile data
- `scopes_supported` - Available permission scopes
- `grant_types_supported` - Supported OAuth flows
- `token_endpoint_auth_methods_supported` - Client authentication methods

This eliminates hardcoded URLs and ensures compatibility with provider changes.

## Token Validation Best Practices

Proper token validation is critical for security.

### Validate JWT Tokens

If using JWT access tokens, validate:

```rust
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use std::collections::HashSet;
use pmcp::error::Error;

async fn validate_jwt_token(token: &str, jwks_uri: &str) -> Result<TokenClaims, Error> {
    // 1. Fetch JWKS (public keys) from provider (cache with TTL based on Cache-Control)
    let jwks = fetch_jwks_cached(jwks_uri).await?;

    // 2. Decode header to get key ID
    let header = jsonwebtoken::decode_header(token)?;
    let kid = header.kid.ok_or_else(|| Error::validation("Missing kid in JWT header"))?;

    // 3. Enforce expected algorithm (reject "none" and unexpected algs)
    if header.alg != Algorithm::RS256 {
        return Err(Error::validation(format!("Unsupported algorithm: {:?}", header.alg)));
    }

    // 4. Find matching key (refresh JWKS on miss for key rotation)
    let key = match jwks.find_key(&kid) {
        Some(k) => k,
        None => {
            // Refresh JWKS in case of key rotation
            let fresh_jwks = fetch_jwks(jwks_uri).await?;
            fresh_jwks.find_key(&kid)
                .ok_or_else(|| Error::validation(format!("Unknown signing key: {}", kid)))?
        }
    };

    // 5. Validate signature and claims
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;  // Check expiration
    validation.validate_nbf = true;  // Check "not before"
    validation.leeway = 60;          // Allow 60s clock skew tolerance
    validation.set_audience(&["mcp-server"]);  // Validate audience
    validation.set_issuer(&["https://auth.example.com"]);  // Validate issuer

    let token_data = decode::<TokenClaims>(
        token,
        &DecodingKey::from_rsa_components(&key.n, &key.e)?,
        &validation,
    )?;

    Ok(token_data.claims)
}
```

**Critical validations:**
- ✅ **Algorithm** - Enforce expected algorithm (RS256), reject "none" or unexpected algs
- ✅ **Signature** - Verify token was issued by trusted provider
- ✅ **Expiration** (`exp`) - Reject expired tokens (with clock skew tolerance)
- ✅ **Not before** (`nbf`) - Reject tokens used too early (with clock skew tolerance)
- ✅ **Issuer** (`iss`) - Verify token is from expected provider
- ✅ **Audience** (`aud`) - Verify token is intended for this server
- ✅ **Scope** - Check required permissions are present
- ✅ **Key rotation** - Refresh JWKS on key ID miss, cache keys respecting Cache-Control

**JWT vs Opaque Tokens:**
- **JWT tokens** - Validate locally using provider's JWKs (public keys). No provider call needed per request after JWKS fetch.
- **Opaque tokens** - Use introspection endpoint, requires provider call per validation (use caching).

### Token Introspection

For opaque (non-JWT) tokens, use introspection:

```rust
use pmcp::error::{Error, ErrorCode};

async fn introspect_token(token: &str, introspection_endpoint: &str)
    -> Result<IntrospectionResponse, Error>
{
    let client = reqwest::Client::new();

    let response = client
        .post(introspection_endpoint)
        .basic_auth("client-id", Some("client-secret"))
        .form(&[("token", token)])
        .send()
        .await?
        .json::<IntrospectionResponse>()
        .await?;

    if !response.active {
        return Err(Error::protocol(
            ErrorCode::AUTHENTICATION_REQUIRED,
            "Token is not active".to_string()
        ));
    }

    Ok(response)
}
```

### Cache Validation Results

Token validation can be expensive (network calls, crypto operations). Cache validated tokens:

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

struct TokenCache {
    cache: RwLock<HashMap<String, (TokenClaims, Instant)>>,
    ttl: Duration,
    jwks_uri: String,
}

impl TokenCache {
    async fn get_or_validate(&self, token: &str) -> Result<TokenClaims, Error> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some((claims, cached_at)) = cache.get(token) {
                if cached_at.elapsed() < self.ttl {
                    return Ok(claims.clone());
                }
            }
        }

        // Validate and cache
        let claims = validate_jwt_token(token, &self.jwks_uri).await?;

        let mut cache = self.cache.write().await;
        cache.insert(token.to_string(), (claims.clone(), Instant::now()));

        Ok(claims)
    }
}
```

**Cache considerations:**
- ✅ Use short TTL (e.g., 5 minutes) to limit exposure of revoked tokens
- ✅ Clear cache on server restart
- ✅ Consider using Redis for distributed caching
- ❌ Don't cache expired tokens
- ❌ Don't cache tokens with critical operations

## Authorization Patterns

Authentication (who you are) is different from authorization (what you can do).

### Pattern 1: Scope-Based Authorization

Scopes define coarse-grained permissions:

```rust
use serde_json::json;

async fn handle_request(&self, args: Value, extra: RequestHandlerExtra)
    -> pmcp::Result<Value>
{
    let auth_ctx = self.auth_middleware
        .authenticate(extra.auth_info.as_ref())
        .await?;

    // Check scopes using has_scope() method
    if !auth_ctx.has_scope("write:data") {
        return Err(Error::protocol(
            ErrorCode::PERMISSION_DENIED,
            "Missing required scope: write:data".to_string()
        ));
    }

    // Proceed with operation
    Ok(json!({"status": "authorized"}))
}
```

### Pattern 2: Role-Based Access Control (RBAC)

Roles group permissions:

```rust
#[derive(Debug, Clone)]
enum Role {
    User,
    Manager,
    Admin,
}

impl Role {
    fn from_claims(claims: &TokenClaims) -> Vec<Role> {
        claims.roles
            .iter()
            .filter_map(|r| match r.as_str() {
                "user" => Some(Role::User),
                "manager" => Some(Role::Manager),
                "admin" => Some(Role::Admin),
                _ => None,
            })
            .collect()
    }

    fn can_delete_users(&self) -> bool {
        matches!(self, Role::Admin)
    }

    fn can_approve_requests(&self) -> bool {
        matches!(self, Role::Manager | Role::Admin)
    }
}

// In handler
let roles = Role::from_claims(&auth_ctx.claims);

if !roles.iter().any(|r| r.can_approve_requests()) {
    return Err(Error::protocol(
        ErrorCode::PERMISSION_DENIED,
        "Requires Manager or Admin role".to_string()
    ));
}
```

### Pattern 3: Attribute-Based Access Control (ABAC)

Fine-grained, context-aware permissions:

```rust
async fn can_access_resource(
    user_id: &str,
    resource_id: &str,
    operation: &str,
    context: &RequestContext,
) -> Result<bool, Error> {
    // Check resource ownership
    let resource = fetch_resource(resource_id).await?;
    if resource.owner_id == user_id {
        return Ok(true);  // Owners can do anything
    }

    // Check sharing permissions
    if resource.is_shared_with(user_id) {
        let permissions = resource.get_user_permissions(user_id);
        if permissions.contains(&operation.to_string()) {
            return Ok(true);
        }
    }

    // Check organization membership
    if context.organization_id == resource.organization_id {
        let org_role = get_org_role(user_id, &context.organization_id).await?;
        if org_role.can_perform(operation) {
            return Ok(true);
        }
    }

    Ok(false)
}
```

### Pattern 4: Least Privilege Principle

Always grant minimum necessary permissions:

```rust
// ❌ Bad: Overly permissive
if auth_ctx.has_scope("admin") {
    // Admin can do anything
    perform_operation(&args).await?;
}

// ✅ Good: Specific permission checks
match operation {
    "read" => {
        require_scope(&auth_ctx, "read:data")?;
        read_data(&args).await?
    }
    "write" => {
        require_scope(&auth_ctx, "write:data")?;
        write_data(&args).await?
    }
    "delete" => {
        require_scope(&auth_ctx, "delete:data")?;
        require_ownership(&auth_ctx, &resource)?;
        delete_data(&args).await?
    }
    _ => return Err(Error::invalid_params("Unknown operation")),
}
```

## Security Best Practices

### 1. Use HTTPS in Production

**Always** use TLS/HTTPS for:
- OAuth authorization endpoints
- Token endpoints
- MCP server endpoints
- Any endpoint transmitting tokens

```rust
// ❌ NEVER in production
let oauth_provider = InMemoryOAuthProvider::new("http://auth.example.com");

// ✅ Always use HTTPS
let oauth_provider = InMemoryOAuthProvider::new("https://auth.example.com");
```

### 2. Validate Redirect URIs

Prevent authorization code interception:

```rust
fn validate_redirect_uri(client: &OAuthClient, redirect_uri: &str) -> Result<(), Error> {
    if !client.redirect_uris.contains(&redirect_uri.to_string()) {
        return Err(Error::protocol(
            ErrorCode::INVALID_REQUEST,
            "Invalid redirect_uri".to_string()
        ));
    }

    // Must be HTTPS in production
    if !redirect_uri.starts_with("https://") && !is_localhost(redirect_uri) {
        return Err(Error::validation("redirect_uri must use HTTPS"));
    }

    Ok(())
}
```

### 3. Use Short-Lived Access Tokens

```rust
// ✅ Good: Short-lived access tokens
let token = TokenResponse {
    access_token: generate_token(),
    expires_in: Some(900),  // 15 minutes
    refresh_token: Some(generate_refresh_token()),
    // ...
};

// ❌ Bad: Long-lived access tokens
let token = TokenResponse {
    access_token: generate_token(),
    expires_in: Some(86400 * 30),  // 30 days - too long!
    // ...
};
```

### 4. Implement Rate Limiting

Prevent brute force and DoS attacks:

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use pmcp::error::{Error, ErrorCode};

struct RateLimiter {
    requests: RwLock<HashMap<String, Vec<Instant>>>,
    max_requests: usize,
    window: Duration,
}

impl RateLimiter {
    async fn check(&self, user_id: &str, client_id: Option<&str>) -> Result<(), Error> {
        let mut requests = self.requests.write().await;

        // Key by user_id + client_id for better granularity
        let key = match client_id {
            Some(cid) => format!("{}:{}", user_id, cid),
            None => user_id.to_string(),
        };

        let user_requests = requests.entry(key).or_default();

        // Remove old requests outside window (sliding window)
        user_requests.retain(|&time| time.elapsed() < self.window);

        // Check limit
        if user_requests.len() >= self.max_requests {
            return Err(Error::protocol(
                ErrorCode::RATE_LIMITED,
                format!("Rate limit exceeded: {} requests per {:?}",
                    self.max_requests, self.window)
            ));
        }

        user_requests.push(Instant::now());
        Ok(())
    }
}
```

**Rate limiting strategies:**
- **Sliding window** (shown above) - Fair, tracks exact request times
- **Token bucket** - Allows bursts, good for API quotas
- **IP-based** - Additional layer at edge/reverse proxy for DoS protection
- **Multi-key** - Combine user_id + client_id + IP for comprehensive control

### 5. Audit Logging

Log all authentication and authorization events:

```rust
use tracing::{info, warn, error};

async fn authenticate_request(&self, auth_info: &AuthInfo)
    -> Result<AuthContext, Error>
{
    match self.validate_token(&auth_info.token).await {
        Ok(claims) => {
            info!(
                user = %claims.sub,
                scopes = ?claims.scope,
                client = %claims.aud,
                "Authentication successful"
            );
            Ok(AuthContext::from_claims(claims))
        }
        Err(e) => {
            warn!(
                error = %e,
                token_prefix = %&auth_info.token[..10],
                "Authentication failed"
            );
            Err(e)
        }
    }
}

async fn authorize_action(&self, user: &str, action: &str, resource: &str)
    -> Result<(), Error>
{
    if !self.has_permission(user, action, resource).await? {
        error!(
            user = %user,
            action = %action,
            resource = %resource,
            "Authorization denied"
        );
        return Err(Error::protocol(
            ErrorCode::PERMISSION_DENIED,
            "Insufficient permissions".to_string()
        ));
    }

    info!(
        user = %user,
        action = %action,
        resource = %resource,
        "Authorization granted"
    );

    Ok(())
}
```

### 6. Secure Token Storage (Client-Side)

**For MCP clients:**

```rust
// ✅ Good: Use OS keychain/credential manager
use keyring::Entry;

let entry = Entry::new("mcp-client", "access_token")?;
entry.set_password(&access_token)?;

// Later...
let token = entry.get_password()?;

// ❌ Bad: Store in plaintext files
std::fs::write("token.txt", access_token)?;  // NEVER DO THIS
```

### 7. Handle Token Refresh

Implement automatic token refresh:

```rust
use tokio::sync::RwLock;
use std::time::Instant;
use std::time::Duration;
use std::sync::Arc;
use pmcp::server::auth::OAuthProvider;
use pmcp::error::Error;

struct TokenManager {
    access_token: RwLock<String>,
    refresh_token: String,
    expires_at: RwLock<Instant>,
    oauth_provider: Arc<dyn OAuthProvider>,  // Use provider, not client struct
}

impl TokenManager {
    async fn get_valid_token(&self) -> Result<String, Error> {
        // Check if token is about to expire (refresh 5 minutes early)
        let expires_at = *self.expires_at.read().await;
        if Instant::now() + Duration::from_secs(300) >= expires_at {
            self.refresh().await?;
        }

        Ok(self.access_token.read().await.clone())
    }

    async fn refresh(&self) -> Result<(), Error> {
        // Use OAuthProvider's refresh_token method
        let new_tokens = self.oauth_provider
            .refresh_token(&self.refresh_token)
            .await?;

        *self.access_token.write().await = new_tokens.access_token;
        *self.expires_at.write().await = Instant::now()
            + Duration::from_secs(new_tokens.expires_in.unwrap_or(3600) as u64);

        Ok(())
    }
}
```

## Running the Examples

### OAuth Server Example

Demonstrates bearer token validation and scope-based authorization:

```bash
cargo run --example 16_oauth_server
```

**What it shows:**
- Public tools (no auth required)
- Protected tools (auth required)
- Scope-based authorization (read, write, admin)
- Token validation with middleware
- Audit logging

### OIDC Discovery Example

Demonstrates OIDC provider integration:

```bash
cargo run --example 20_oidc_discovery
```

**What it shows:**
- OIDC discovery from provider metadata
- Authorization code exchange
- Token refresh flows
- Retry logic for network errors
- Transport isolation for security

## Integration Checklist

When integrating authentication into your MCP server:

- [ ] **Choose OAuth provider** - Auth0, Okta, Keycloak, Azure AD, etc.
- [ ] **Register MCP server** - Create OAuth client in provider
- [ ] **Configure scopes** - Define permission levels (read, write, admin, etc.)
- [ ] **Implement token validation** - JWT validation or introspection
- [ ] **Add middleware** - Use `BearerTokenMiddleware` and `ScopeMiddleware`
- [ ] **Protect tools** - Add auth checks to tool handlers
- [ ] **Enforce authorization** - Check scopes, roles, or attributes
- [ ] **Enable HTTPS** - Use TLS for all endpoints
- [ ] **Implement rate limiting** - Prevent abuse
- [ ] **Add audit logging** - Track who did what
- [ ] **Document scopes** - Tell users what permissions they need
- [ ] **Test authorization** - Verify permission enforcement works
- [ ] **Handle token expiration** - Implement refresh logic (client-side)

## Multi-Tenant Considerations

For multi-tenant MCP servers, enforce tenant boundaries:

```rust
use pmcp::error::{Error, ErrorCode};

async fn validate_tenant_access(auth_ctx: &AuthContext, resource_id: &str)
    -> Result<(), Error>
{
    // Extract tenant ID from token claims
    let token_tenant = auth_ctx.claims.get("tenant_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Error::validation("Missing tenant_id claim"))?;

    // Fetch resource and check tenant ownership
    let resource = fetch_resource(resource_id).await?;

    if resource.tenant_id != token_tenant {
        return Err(Error::protocol(
            ErrorCode::PERMISSION_DENIED,
            format!("Resource {} does not belong to tenant {}",
                resource_id, token_tenant)
        ));
    }

    Ok(())
}
```

**Multi-tenant best practices:**
- Include `tenant_id` in access token claims
- Validate tenant context for ALL resource access
- Use database row-level security when possible
- Audit cross-tenant access attempts
- Consider separate OAuth clients per tenant

## Common Pitfalls to Avoid

❌ **Don't implement custom auth flows** - Use established OAuth providers

❌ **Don't store tokens in plaintext** - Use secure storage (keychain, vault)

❌ **Don't skip token validation** - Always validate signature, expiration, audience, algorithm

❌ **Don't use long-lived access tokens** - Keep them short (15-60 minutes)

❌ **Don't accept HTTP in production** - Always use HTTPS

❌ **Don't leak tokens in logs** - Redact tokens in log messages

❌ **Don't bypass authorization** - Always check permissions, even for "trusted" users

❌ **Don't trust client-provided identity** - Always validate server-side

❌ **Don't use implicit flow** - Use Authorization Code + PKCE instead

❌ **Don't ignore tenant boundaries** - Always validate tenant context in multi-tenant apps

## Provider-Agnostic Authentication SDK

PMCP provides a provider-agnostic authentication abstraction. Your MCP server code should never know about OAuth providers, tokens, or authentication flows - it only sees `AuthContext`.

### The Core Principle

**OAuth is a deployment concern, not a code concern.** Your server code uses the same `AuthContext` API regardless of whether tokens come from Cognito, Entra, Google, Okta, or Auth0.

### AuthContext: The Only Auth Type You Need

```rust
use pmcp::server::auth::AuthContext;

fn handle_request(auth: &AuthContext) -> Result<String, &'static str> {
    // Require authentication
    auth.require_auth()?;

    // Check scopes
    auth.require_scope("read:data")?;

    // Access user info (provider-agnostic)
    let user_id = auth.user_id();
    let email = auth.email().unwrap_or("unknown");
    let tenant = auth.tenant_id();

    // Check group membership
    if auth.in_group("admins") {
        // Admin-only logic
    }

    Ok(format!("Hello, {} ({})", email, user_id))
}
```

**Key helper methods:**

| Method | Description |
|--------|-------------|
| `user_id()` | Standard user identifier (from `sub` claim) |
| `email()` | Email address (handles Cognito, Entra, Google differences) |
| `tenant_id()` | Tenant ID (handles `tid`, `custom:tenant`, `org_id` variations) |
| `groups()` | Group membership (handles `groups`, `cognito:groups`, `roles`) |
| `name()` | Display name |
| `claim<T>(key)` | Get any typed claim value |
| `require_auth()` | Returns error if not authenticated |
| `require_scope(s)` | Returns error if scope missing |
| `in_group(g)` | Check group membership |

### ClaimMappings: Provider-Specific Translations

Different providers use different claim names. `ClaimMappings` handles the translation:

```rust
use pmcp::server::auth::ClaimMappings;

// Built-in presets for major providers
let cognito_mappings = ClaimMappings::cognito();
let entra_mappings = ClaimMappings::entra();
let google_mappings = ClaimMappings::google();
let okta_mappings = ClaimMappings::okta();
let auth0_mappings = ClaimMappings::auth0();

// Or create custom mappings
let custom = ClaimMappings {
    user_id: "sub".to_string(),
    tenant_id: Some("organization_id".to_string()),
    email: Some("email_address".to_string()),
    groups: Some("user_roles".to_string()),
    ..Default::default()
};
```

**Provider claim name differences:**

| Standard | Cognito | Entra ID | Google | Okta | Auth0 |
|----------|---------|----------|--------|------|-------|
| `user_id` | sub | oid | sub | uid | sub |
| `tenant_id` | `custom:tenant` | tid | N/A | `org_id` | `org_id` |
| email | email | `preferred_username` | email | email | email |
| groups | `cognito:groups` | groups | N/A | groups | roles |

### TokenValidatorConfig: Configuration-Driven Validation

Configure validators via TOML or code - no code changes to switch providers:

```rust
use pmcp::server::auth::TokenValidatorConfig;

// JWT validation (recommended for production)
let jwt_config = TokenValidatorConfig::jwt(
    "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx",
    "your-app-client-id",
);

// Mock validation (for development/testing)
let mock_config = TokenValidatorConfig::mock("dev-user");

// Disabled (development only)
let disabled = TokenValidatorConfig::disabled();
```

**Configuration via `pmcp.toml`:**

```toml
# Production profile
[profile.production.auth]
type = "jwt"
issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
audience = "your-app-client-id"

# Development profile
[profile.dev.auth]
type = "mock"
default_user_id = "dev-user"
default_scopes = ["read", "write", "admin"]
```

### JwtValidator: Stateless Token Validation

The `JwtValidator` fetches and caches JWKS from your OAuth provider:

```rust
use pmcp::server::auth::{JwtValidator, TokenValidator};
use pmcp::server::auth::config::JwtValidatorConfig;

// Create validator for AWS Cognito
let config = JwtValidatorConfig::cognito(
    "us-east-1",
    "us-east-1_xxxxx",
    "client-id"
);
let validator = JwtValidator::new(config).await?;

// Validate a token
let auth_context = validator.validate("eyJhbGci...").await?;
println!("User: {}", auth_context.user_id());
```

**Provider-specific configurations:**

```rust
// AWS Cognito
let config = JwtValidatorConfig::cognito("us-east-1", "pool-id", "client-id");

// Microsoft Entra ID (Azure AD)
let config = JwtValidatorConfig::entra("tenant-id", "api://my-api");

// Google Identity
let config = JwtValidatorConfig::google("client-id.apps.googleusercontent.com");

// Okta
let config = JwtValidatorConfig::okta("dev-123456.okta.com", "api://default");

// Auth0
let config = JwtValidatorConfig::auth0("myapp.auth0.com", "https://myapi/");
```

**Feature flag:** Requires the `jwt-auth` feature:

```toml
[dependencies]
pmcp = { version = "1.8", features = ["jwt-auth"] }
```

### MockValidator: Development and Testing

Use `MockValidator` for local development and unit testing:

```rust
use pmcp::server::auth::{MockValidator, MockAuthContextBuilder, TokenValidator};

// Create a mock validator
let validator = MockValidator::new("test-user")
    .with_tenant_id("test-tenant")
    .with_scopes(vec!["read", "write", "admin"])
    .with_claim("email", "test@example.com");

// Any token works in mock mode
let auth = validator.validate("any-token").await?;
assert_eq!(auth.user_id(), "test-user");
assert!(auth.has_scope("admin"));

// Or build contexts directly for unit tests
let auth = MockAuthContextBuilder::new()
    .user_id("unit-test-user")
    .tenant_id("tenant-abc")
    .scopes(vec!["read"])
    .claim("email", "test@example.com")
    .build();
```

### Developer Journey: From No Auth to Production OAuth

The SDK enables incremental development:

**Phase 1: Build with no auth**
```rust
// Just implement your tools - no auth code needed
async fn my_tool(args: Value) -> Result<Value> {
    // Business logic only
}
```

**Phase 2: Add auth-aware logic**
```rust
// Use MockValidator or AuthContext::anonymous() for testing
async fn my_tool(args: Value, auth: &AuthContext) -> Result<Value> {
    let user_id = auth.user_id();  // Works with mock or real auth
    // Business logic uses user context
}
```

**Phase 3: Deploy with real OAuth**
```bash
# Configure in pmcp.toml or environment variables
PMCP_AUTH_TYPE=jwt
PMCP_AUTH_ISSUER=https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx
PMCP_AUTH_AUDIENCE=your-client-id
```

**Phase 4: Switch providers (no code changes!)**
```bash
# Just change configuration
PMCP_AUTH_ISSUER=https://login.microsoftonline.com/tenant-id/v2.0
PMCP_AUTH_AUDIENCE=api://my-api
```

## Key Takeaways

1. **Reuse existing OAuth infrastructure** - Don't reinvent authentication
2. **MCP servers = web servers** - Same token validation patterns apply
3. **OAuth flows are external** - MCP server only validates tokens
4. **Act on behalf of users** - Use access tokens to enforce user permissions
5. **Validate everything** - Signature, expiration, audience, scopes
6. **Log security events** - Track authentication and authorization
7. **Use HTTPS always** - Protect tokens in transit
8. **Keep tokens short-lived** - Use refresh tokens for long sessions
9. **Enforce least privilege** - Grant minimum necessary permissions
10. **Test security** - Verify authorization works correctly
11. **Use provider-agnostic code** - Your MCP code should only see `AuthContext`
12. **Configure, don't code** - Switch OAuth providers via configuration

Authentication done right makes MCP servers secure, auditable, and integrated with your organization's existing identity infrastructure. By following OAuth 2.0 and OIDC standards, you get enterprise-grade security without reinventing the wheel.

## Further Reading

- [OAuth 2.0 RFC 6749](https://datatracker.ietf.org/doc/html/rfc6749)
- [OpenID Connect Core 1.0](https://openid.net/specs/openid-connect-core-1_0.html)
- [OIDC Discovery](https://openid.net/specs/openid-connect-discovery-1_0.html)
- [JWT Best Practices](https://datatracker.ietf.org/doc/html/rfc8725)
- [PMCP Auth API Documentation](https://docs.rs/pmcp/latest/pmcp/server/auth/)
- Example: `examples/16_oauth_server.rs`
- Example: `examples/20_oidc_discovery.rs`
