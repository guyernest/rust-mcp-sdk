# OAuth 2.0 Fundamentals

This chapter covers the OAuth 2.0 concepts essential for implementing authentication in MCP servers. We focus on the patterns most relevant to enterprise deployments.

## OAuth 2.0 Core Concepts

### Roles in OAuth 2.0

```
┌─────────────────────────────────────────────────────────────────────┐
│                     OAuth 2.0 Roles for MCP                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Resource Owner (User)                                              │
│  ├─ The person using the AI assistant                              │
│  └─ Grants permission for AI to access MCP tools                   │
│                                                                     │
│  Client (MCP Client)                                                │
│  ├─ Claude Desktop, ChatGPT, or custom application                 │
│  ├─ Requests access on behalf of the user                          │
│  └─ Receives and uses access tokens                                │
│                                                                     │
│  Authorization Server (Identity Provider)                           │
│  ├─ Cognito, Auth0, Entra ID, Okta                                 │
│  ├─ Authenticates users                                            │
│  └─ Issues access tokens                                           │
│                                                                     │
│  Resource Server (Your MCP Server)                                  │
│  ├─ Validates access tokens                                        │
│  ├─ Enforces scopes                                                │
│  └─ Provides tools and resources                                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Grant Types

OAuth 2.0 defines several grant types. For MCP servers, these are most relevant:

#### Authorization Code Grant (Recommended)

The most secure flow for user-facing applications:

```
┌─────────────────────────────────────────────────────────────────────┐
│                 Authorization Code Flow                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. User clicks "Connect to MCP Server"                             │
│     │                                                               │
│     ▼                                                               │
│  2. Client redirects to Authorization Server                        │
│     GET /authorize?                                                 │
│       response_type=code&                                           │
│       client_id=abc&                                                │
│       redirect_uri=https://client/callback&                         │
│       scope=read:tools write:tools&                                 │
│       state=random123                                               │
│     │                                                               │
│     ▼                                                               │
│  3. User logs in and consents                                       │
│     │                                                               │
│     ▼                                                               │
│  4. Authorization Server redirects back with code                   │
│     GET https://client/callback?                                    │
│       code=AUTH_CODE_HERE&                                          │
│       state=random123                                               │
│     │                                                               │
│     ▼                                                               │
│  5. Client exchanges code for tokens (server-side)                  │
│     POST /token                                                     │
│       grant_type=authorization_code&                                │
│       code=AUTH_CODE_HERE&                                          │
│       client_id=abc&                                                │
│       client_secret=xyz&                                            │
│       redirect_uri=https://client/callback                          │
│     │                                                               │
│     ▼                                                               │
│  6. Authorization Server returns tokens                             │
│     {                                                               │
│       "access_token": "eyJhbGc...",                                │
│       "refresh_token": "def456...",                                │
│       "token_type": "Bearer",                                      │
│       "expires_in": 3600                                           │
│     }                                                               │
│     │                                                               │
│     ▼                                                               │
│  7. Client calls MCP Server with access token                       │
│     POST /mcp                                                       │
│       Authorization: Bearer eyJhbGc...                              │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

#### Client Credentials Grant (Server-to-Server)

For automated systems without user interaction:

```rust
// Server-to-server authentication
let token_response = reqwest::Client::new()
    .post("https://auth.example.com/oauth/token")
    .form(&[
        ("grant_type", "client_credentials"),
        ("client_id", &config.client_id),
        ("client_secret", &config.client_secret),
        ("scope", "read:tools"),
    ])
    .send()
    .await?
    .json::<TokenResponse>()
    .await?;

// Use the token
let mcp_response = client
    .post("https://mcp.example.com/mcp")
    .bearer_auth(&token_response.access_token)
    .json(&mcp_request)
    .send()
    .await?;
```

## JSON Web Tokens (JWT)

OAuth 2.0 access tokens are typically JWTs. Understanding their structure is essential for validation.

### JWT Structure

```
┌─────────────────────────────────────────────────────────────────────┐
│                        JWT Structure                                 │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImtleS0xIn0.            │
│  ────────────────────────────────────────────────────────            │
│                         HEADER (Base64)                              │
│                                                                     │
│  eyJzdWIiOiJ1c2VyMTIzIiwiZW1haWwiOiJhbGljZUBjby5jb20iLCJzY29w...      │
│  ────────────────────────────────────────────────────────────        │
│                         PAYLOAD (Base64)                             │
│                                                                     │
│  SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c                         │
│  ────────────────────────────────────────────────                    │
│                         SIGNATURE                                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Header

```json
{
  "alg": "RS256",      // Signing algorithm
  "typ": "JWT",        // Token type
  "kid": "key-123"     // Key ID for signature verification
}
```

Common algorithms:
- **RS256** - RSA signature with SHA-256 (recommended)
- **RS384** - RSA with SHA-384
- **ES256** - ECDSA with P-256 curve
- **HS256** - HMAC with SHA-256 (avoid for access tokens)

### Payload (Claims)

```json
{
  // Standard claims
  "iss": "https://auth.example.com/",           // Issuer
  "sub": "auth0|user123",                        // Subject (user ID)
  "aud": "https://mcp.example.com",             // Audience
  "exp": 1700000000,                            // Expiration time
  "iat": 1699996400,                            // Issued at
  "nbf": 1699996400,                            // Not before

  // Common custom claims
  "email": "alice@company.com",
  "name": "Alice Smith",
  "scope": "read:tools write:tools",
  "permissions": ["read:customers", "write:orders"],
  "org_id": "org_abc123",
  "roles": ["developer", "data-analyst"]
}
```

### Essential Claims for MCP

| Claim | Purpose | Example |
|-------|---------|---------|
| `sub` | User identifier | `auth0|user123` |
| `iss` | Token issuer (IdP) | `https://cognito...` |
| `aud` | Intended audience | `mcp-server-prod` |
| `exp` | Expiration time | `1700000000` |
| `scope` | Granted permissions | `read:tools write:data` |
| `email` | User email (optional) | `alice@co.com` |

## Scopes and Permissions

### Defining Scopes

Scopes define what the client can do. Design them around your MCP capabilities:

```rust
// Scope definitions for an MCP server
pub enum Scope {
    // Tool access
    ReadTools,      // "read:tools" - List and describe tools
    ExecuteTools,   // "execute:tools" - Call tools

    // Resource access
    ReadResources,  // "read:resources" - Read resources
    WriteResources, // "write:resources" - Modify resources

    // Admin operations
    AdminAudit,     // "admin:audit" - View audit logs
    AdminUsers,     // "admin:users" - Manage users
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ReadTools => "read:tools",
            Self::ExecuteTools => "execute:tools",
            Self::ReadResources => "read:resources",
            Self::WriteResources => "write:resources",
            Self::AdminAudit => "admin:audit",
            Self::AdminUsers => "admin:users",
        }
    }
}
```

### Checking Scopes in Tools

```rust
#[derive(TypedTool)]
#[tool(name = "execute_query", description = "Run a database query")]
pub struct ExecuteQuery;

impl ExecuteQuery {
    pub async fn run(
        &self,
        input: QueryInput,
        context: &ToolContext,
    ) -> Result<QueryResult> {
        let auth = context.auth()?;

        // Check for required scope
        auth.require_scope("execute:tools")?;

        // For write operations, check additional scope
        if is_write_query(&input.sql) {
            auth.require_scope("write:resources")?;
        }

        // Execute query...
        self.database.execute(&input.sql).await
    }
}
```

### Scope Hierarchy

Design scopes with hierarchy for flexibility:

```
admin:*          → Full admin access
├── admin:audit  → Read audit logs
├── admin:users  → Manage users
└── admin:config → Modify configuration

write:*          → Full write access
├── write:tools  → Execute modifying tools
└── write:data   → Modify resources

read:*           → Full read access
├── read:tools   → List and describe tools
└── read:data    → Read resources
```

```rust
impl AuthContext {
    pub fn has_scope(&self, required: &str) -> bool {
        self.scopes.iter().any(|s| {
            s == required ||
            // Check wildcard: "write:*" matches "write:tools"
            (s.ends_with(":*") && required.starts_with(&s[..s.len()-1]))
        })
    }
}
```

## Token Refresh

Access tokens are short-lived. Clients use refresh tokens to get new ones:

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Token Refresh Flow                               │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Initial State:                                                     │
│  ┌───────────────────────────────────────────────────────────┐     │
│  │  access_token: eyJhbGc... (expires in 1 hour)             │     │
│  │  refresh_token: def456... (expires in 30 days)            │     │
│  └───────────────────────────────────────────────────────────┘     │
│                                                                     │
│  When access token expires:                                         │
│                                                                     │
│  Client → Authorization Server                                      │
│  POST /token                                                        │
│    grant_type=refresh_token&                                        │
│    refresh_token=def456...&                                         │
│    client_id=abc&                                                   │
│    client_secret=xyz                                                │
│                                                                     │
│  Authorization Server → Client                                      │
│  {                                                                  │
│    "access_token": "NEW_TOKEN...",                                 │
│    "refresh_token": "NEW_REFRESH...",  // May be rotated           │
│    "expires_in": 3600                                              │
│  }                                                                  │
│                                                                     │
│  Note: Some IdPs rotate refresh tokens on each use                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Client-Side Token Management

```rust
pub struct TokenManager {
    access_token: RwLock<String>,
    refresh_token: RwLock<String>,
    expires_at: RwLock<Instant>,
    client: reqwest::Client,
}

impl TokenManager {
    pub async fn get_valid_token(&self) -> Result<String> {
        // Check if current token is still valid (with buffer)
        let expires_at = *self.expires_at.read().await;
        if Instant::now() + Duration::from_secs(60) < expires_at {
            return Ok(self.access_token.read().await.clone());
        }

        // Token expired or expiring soon, refresh it
        self.refresh().await
    }

    async fn refresh(&self) -> Result<String> {
        let refresh_token = self.refresh_token.read().await.clone();

        let response = self.client
            .post("https://auth.example.com/oauth/token")
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", &refresh_token),
                ("client_id", &self.client_id),
            ])
            .send()
            .await?
            .json::<TokenResponse>()
            .await?;

        // Update stored tokens
        *self.access_token.write().await = response.access_token.clone();
        if let Some(new_refresh) = response.refresh_token {
            *self.refresh_token.write().await = new_refresh;
        }
        *self.expires_at.write().await =
            Instant::now() + Duration::from_secs(response.expires_in);

        Ok(response.access_token)
    }
}
```

## PKCE: Proof Key for Code Exchange

For public clients (mobile apps, SPAs), use PKCE to prevent authorization code interception:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    PKCE Flow                                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Client generates code_verifier (random string)                  │
│     code_verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"   │
│                                                                     │
│  2. Client creates code_challenge (SHA256 hash)                     │
│     code_challenge = BASE64URL(SHA256(code_verifier))               │
│     = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"                 │
│                                                                     │
│  3. Authorization request includes challenge                        │
│     GET /authorize?                                                 │
│       response_type=code&                                           │
│       code_challenge=E9Melhoa...&                                   │
│       code_challenge_method=S256&                                   │
│       ...                                                           │
│                                                                     │
│  4. Token request includes verifier                                 │
│     POST /token                                                     │
│       grant_type=authorization_code&                                │
│       code=AUTH_CODE&                                               │
│       code_verifier=dBjftJeZ...&                                    │
│       ...                                                           │
│                                                                     │
│  5. Server verifies SHA256(verifier) == challenge                   │
│     ✓ Only the original client can exchange the code                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

```rust
use sha2::{Sha256, Digest};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::Rng;

pub fn generate_pkce() -> (String, String) {
    // Generate random verifier (43-128 characters)
    let verifier: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    // Create challenge (SHA256 + Base64URL)
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    (verifier, challenge)
}
```

## OpenID Connect (OIDC)

OIDC adds identity layer on top of OAuth 2.0:

### ID Token

```json
{
  "iss": "https://auth.example.com/",
  "sub": "user123",
  "aud": "client-id",
  "exp": 1700000000,
  "iat": 1699996400,
  "nonce": "random-nonce",

  // OIDC standard claims
  "email": "alice@company.com",
  "email_verified": true,
  "name": "Alice Smith",
  "given_name": "Alice",
  "family_name": "Smith",
  "picture": "https://...",
  "locale": "en-US"
}
```

### Discovery Document

OIDC providers publish configuration at a well-known URL:

```bash
# Cognito
https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxx/.well-known/openid-configuration

# Auth0
https://your-tenant.auth0.com/.well-known/openid-configuration

# Entra ID
https://login.microsoftonline.com/{tenant}/v2.0/.well-known/openid-configuration
```

Response includes endpoints and supported features:

```json
{
  "issuer": "https://auth.example.com/",
  "authorization_endpoint": "https://auth.example.com/authorize",
  "token_endpoint": "https://auth.example.com/oauth/token",
  "userinfo_endpoint": "https://auth.example.com/userinfo",
  "jwks_uri": "https://auth.example.com/.well-known/jwks.json",
  "scopes_supported": ["openid", "profile", "email"],
  "response_types_supported": ["code", "token"],
  "token_endpoint_auth_methods_supported": ["client_secret_post", "client_secret_basic"]
}
```

## Best Practices Summary

### For MCP Server Developers

1. **Always validate tokens** - Never trust client claims
2. **Check all standard claims** - iss, aud, exp, nbf
3. **Use scopes for authorization** - Not just authentication
4. **Cache JWKS** - But handle key rotation
5. **Return proper errors** - 401 vs 403 matters

### Token Lifetimes

| Token Type | Recommended Lifetime | Notes |
|------------|---------------------|-------|
| Access Token | 15-60 minutes | Shorter = more secure |
| Refresh Token | 7-30 days | Balance security vs UX |
| ID Token | 5-15 minutes | Only for initial auth |

### Security Checklist

- [ ] Use HTTPS everywhere
- [ ] Validate token signature
- [ ] Check issuer matches expected
- [ ] Check audience matches your server
- [ ] Check expiration (with clock skew)
- [ ] Use PKCE for public clients
- [ ] Implement token refresh
- [ ] Log authentication events

## Summary

OAuth 2.0 fundamentals for MCP servers:

1. **Roles** - Understand resource owner, client, authorization server, resource server
2. **Grant types** - Authorization Code for users, Client Credentials for servers
3. **JWTs** - Structure, claims, and what to validate
4. **Scopes** - Design around your capabilities
5. **Token refresh** - Handle expiration gracefully
6. **PKCE** - Required for public clients
7. **OIDC** - Adds identity on top of OAuth

The next chapter covers the practical implementation of token validation in Rust.

---

*Continue to [Token Validation](./ch13-03-validation.md) →*
