# OAuth 2.0 Fundamentals

This chapter covers the OAuth 2.0 concepts essential for implementing authentication in MCP servers. We focus on the patterns most relevant to enterprise deployments.

**Good news for MCP developers:** You don't need to build token management from scratch. Popular MCP clients—Claude Code, ChatGPT, Cursor, and others—already handle the complexity of OAuth for you. They securely store tokens, automatically refresh them when expired, and manage the entire authentication flow. Your job as an MCP server developer is simpler: validate the tokens these clients send you.

**What this means for users:** Users authenticate once (through your enterprise SSO), and then work uninterrupted for weeks or months until the refresh token expires (typically 30-90 days). No repeated logins, no token copying, no credential management. The MCP client handles everything silently in the background.

## OAuth 2.0 Core Concepts

### Roles in OAuth 2.0

```
┌─────────────────────────────────────────────────────────────────────┐
│                     OAuth 2.0 Roles for MCP                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Resource Owner (User)                                              │
│  ├─ The person using the AI assistant                               │
│  └─ Grants permission for AI to access MCP tools                    │
│                                                                     │
│  Client (MCP Client)                                                │
│  ├─ Claude Code, ChatGPT, Cursor, or custom application             │
│  ├─ Securely stores tokens (locally or server-side)                 │
│  ├─ Automatically refreshes tokens before expiration                │
│  └─ Sends access token with every MCP request                       │
│                                                                     │
│  Authorization Server (Identity Provider)                           │
│  ├─ Cognito, Auth0, Entra ID, Okta                                  │
│  ├─ Authenticates users                                             │
│  └─ Issues access tokens                                            │
│                                                                     │
│  Resource Server (Your MCP Server)                                  │
│  ├─ Validates access tokens                                         │
│  ├─ Enforces scopes                                                 │
│  └─ Provides tools and resources                                    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Grant Types

OAuth 2.0 defines several grant types. For MCP servers, these are most relevant:

#### Authorization Code Grant (Recommended)

The most secure flow for user-facing applications:

```
┌─────────────────────────────────────────────────────────────────────┐
│                 Authorization Code Flow                             │
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
│       "access_token": "eyJhbGc...",                                 │
│       "refresh_token": "def456...",                                 │
│       "token_type": "Bearer",                                       │
│       "expires_in": 3600                                            │
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
│                        JWT Structure                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImtleS0xIn0.           │
│  ────────────────────────────────────────────────────────           │
│                         HEADER (Base64)                             │
│                                                                     │
│  eyJzdWIiOiJ1c2VyMTIzIiwiZW1haWwiOiJhbGljZUBjby5jb20iLCJzY29w...    │
│  ────────────────────────────────────────────────────────────       │
│                         PAYLOAD (Base64)                            │
│                                                                     │
│  SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c                        │
│  ────────────────────────────────────────────────                   │
│                         SIGNATURE                                   │
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

## Understanding Token Lifetimes

If you're new to OAuth, token lifetimes can be confusing. Here's the mental model:

**Think of it like a building security system:**
- **Access token** = Day pass. Gets you through the door today, but expires at midnight. If someone steals it, they only have access until it expires (typically 1 hour for OAuth).
- **Refresh token** = ID badge that lets you print new day passes. Valid for months, but if you lose it (or leave the company), security can deactivate it immediately.

### Why Two Tokens?

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Token Lifetime Strategy                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ACCESS TOKEN (Short-lived: 15-60 minutes)                          │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  ✓ Sent with every API request                                │  │
│  │  ✓ If leaked, damage limited to minutes/hours                 │  │
│  │  ✓ Contains user claims (who, what permissions)               │  │
│  │  ✗ Cannot be revoked (must wait for expiration)               │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  REFRESH TOKEN (Long-lived: 30-90 days)                             │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  ✓ Only sent to the IdP, never to your MCP server             │  │
│  │  ✓ Used to get new access tokens silently                     │  │
│  │  ✓ Can be revoked immediately by administrators               │  │
│  │  ✓ Enables "login once, work for weeks" experience            │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  The combination: Security of short-lived tokens +                  │
│                   Convenience of long sessions                      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### What Users Experience

| Day | What Happens | User Action Required |
|-----|--------------|---------------------|
| Day 1 | User connects MCP server to Claude Code | Login once via SSO |
| Day 2-89 | Access tokens refresh automatically every hour | None - seamless |
| Day 90 | Refresh token expires | Login again via SSO |

**The key insight:** Users authenticate once and work uninterrupted for the refresh token lifetime (often 90 days). MCP clients like Claude Code, ChatGPT, and Cursor handle all the token refresh logic automatically—users never see it happening.

### Administrator Control: Immediate Revocation

Even though refresh tokens last 90 days, administrators can revoke them instantly:

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Token Revocation Scenario                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Monday 9am:  Employee leaves company                               │
│  Monday 9:05am: IT disables account in IdP                          │
│  Monday 9:06am: Employee tries to use MCP server                    │
│                                                                     │
│  What happens:                                                      │
│  1. Claude Code tries to refresh the access token                   │
│  2. IdP rejects: "Refresh token revoked"                            │
│  3. Claude Code prompts for re-authentication                       │
│  4. Employee can't login (account disabled)                         │
│  5. Access denied ✓                                                 │
│                                                                     │
│  Maximum exposure time: Until current access token expires          │
│  (typically 15-60 minutes, not 90 days)                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

This is why access tokens are kept short-lived: even if you can't revoke them directly, you limit the damage window to minutes, not days.

## Token Refresh Flow

Access tokens are short-lived by design. MCP clients use refresh tokens to get new ones automatically:

```
┌─────────────────────────────────────────────────────────────────────┐
│                     Token Refresh Flow                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Initial State:                                                     │
│  ┌───────────────────────────────────────────────────────────┐      │
│  │  access_token: eyJhbGc... (expires in 1 hour)             │      │
│  │  refresh_token: def456... (expires in 30 days)            │      │
│  └───────────────────────────────────────────────────────────┘      │
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
│    "access_token": "NEW_TOKEN...",                                  │
│    "refresh_token": "NEW_REFRESH...",  // May be rotated            │
│    "expires_in": 3600                                               │
│  }                                                                  │
│                                                                     │
│  Note: Some IdPs rotate refresh tokens on each use                  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### How MCP Clients Store Tokens Securely

You might wonder: "If refresh tokens last 90 days, where are they stored?" MCP clients handle this differently depending on their architecture:

| Client | Token Storage | Security Model |
|--------|--------------|----------------|
| **Claude Code** | OS keychain (macOS Keychain, Windows Credential Manager) | Encrypted, per-user, survives restarts |
| **ChatGPT** | Server-side (OpenAI infrastructure) | User never sees tokens, encrypted at rest |
| **Cursor** | OS keychain | Same as Claude Code |
| **Custom apps** | Your responsibility | Use OS keychain or secure enclave |

**The important point:** Users never need to handle tokens directly. They click "Connect," authenticate via SSO, and the client manages everything securely. This is a major advantage over API keys, which users often store in plain text files or environment variables.

### Client-Side Token Management (For Custom Implementations)

If you're building a custom MCP client, here's the pattern for automatic token refresh:

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
│                    PKCE Flow                                        │
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

### Token Lifetimes: What They Mean in Practice

| Token Type | Recommended Lifetime | What This Means |
|------------|---------------------|-----------------|
| Access Token | 15-60 minutes | Max time a stolen token is useful. Refreshed silently by MCP clients. |
| Refresh Token | 30-90 days | How long users work without re-authenticating. Can be revoked anytime by admins. |
| ID Token | 5-15 minutes | Only used once during initial login. Not sent to MCP servers. |

**For MCP server developers:** You only see access tokens. You don't handle refresh tokens—that's between the MCP client and the IdP. Your job is to validate each access token is legitimate and not expired.

**For enterprise administrators:** You control refresh token lifetime in your IdP settings. Longer = better user experience. Shorter = users re-authenticate more often. Either way, you can revoke any user's tokens instantly if needed.

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
5. **Token refresh** - Handled automatically by MCP clients
6. **PKCE** - Required for public clients
7. **OIDC** - Adds identity on top of OAuth

**Key takeaways for the user experience:**

- **Users authenticate once** and work uninterrupted for 30-90 days (refresh token lifetime)
- **MCP clients handle complexity:** Claude Code, ChatGPT, Cursor store tokens securely and refresh them automatically
- **Administrators stay in control:** Tokens can be revoked instantly, regardless of expiration date
- **Security through short access tokens:** Even if something goes wrong, exposure is limited to minutes

**Key takeaways for MCP server developers:**

- **You only validate access tokens** - Refresh handling is the client's job
- **Check every request** - Validate signature, issuer, audience, and expiration
- **Use scopes for authorization** - They define what each user can do
- **Return proper errors** - 401 for invalid tokens, 403 for insufficient permissions

The next chapter covers the practical implementation of token validation in Rust.

---

*Continue to [Token Validation](./ch13-03-validation.md) →*
