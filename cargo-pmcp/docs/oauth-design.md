# OAuth Design for cargo-pmcp

Production-ready OAuth 2.0 authentication for MCP servers deployed with cargo-pmcp.

## Design Principles

1. **MCP server developer is OAuth-agnostic** - No authentication code required in server logic
2. **Layered security** - Platform auth (API Gateway + Cognito) handles everything
3. **Stateless token validation** - JWT + JWKS, no token storage tables
4. **Dynamic Client Registration** - MCP clients self-register via RFC 7591
5. **Low cost** - Minimal Lambda invocations, JWKS caching
6. **Future-proof** - Easy migration to AWS AgentCore when ready

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              API Gateway                                     │
│                                                                             │
│  ┌───────────────────┐  ┌────────────────┐  ┌───────────────────────────┐  │
│  │ /.well-known/*    │  │ /oauth2/*      │  │ /mcp/*                    │  │
│  │ (No Auth)         │  │ (No Auth)      │  │ (JWT Authorizer)          │  │
│  │                   │  │                │  │                           │  │
│  │ • openid-config   │  │ • /register    │  │ Lambda Authorizer         │  │
│  │ • jwks.json       │  │ • /authorize   │  │ (stateless JWT check)     │  │
│  │                   │  │ • /token       │  │                           │  │
│  └─────────┬─────────┘  └───────┬────────┘  └─────────────┬─────────────┘  │
└────────────┼────────────────────┼─────────────────────────┼─────────────────┘
             │                    │                         │
             │                    ▼                         ▼
             │         ┌─────────────────────┐   ┌─────────────────────────┐
             │         │  OAuth Proxy        │   │  User's MCP Server      │
             │         │  Lambda             │   │  Lambda                 │
             │         │                     │   │                         │
             │         │  Handles:           │   │  • Zero OAuth code      │
             │         │  • DCR → Cognito    │   │  • User context via     │
             │         │  • Token exchange   │   │    request headers      │
             │         │  • Authorize redirect│  │  • Optional: pmcp-auth  │
             │         └──────────┬──────────┘   └─────────────────────────┘
             │                    │
             │                    ▼
             │         ┌─────────────────────────────────────────┐
             │         │              DynamoDB                    │
             │         │                                          │
             │         │  ClientRegistrationTable (ONLY TABLE)   │
             │         │  ┌─────────────────────────────────┐    │
             │         │  │ PK: client_id                   │    │
             │         │  │ • client_secret                 │    │
             │         │  │ • client_name                   │    │
             │         │  │ • redirect_uris                 │    │
             │         │  │ • is_public                     │    │
             │         │  │ • created_at                    │    │
             │         │  │ • server_id (links to MCP srv)  │    │
             │         │  └─────────────────────────────────┘    │
             │         └─────────────────────────────────────────┘
             │                    │
             └────────────────────┼────────────────────────────────┐
                                  ▼                                │
                       ┌─────────────────────┐                     │
                       │      Cognito        │◀────────────────────┘
                       │    User Pool        │   (JWKS for validation)
                       │                     │
                       │  • User identity    │
                       │  • Token issuance   │
                       │  • Hosted UI        │
                       │  • Social logins    │
                       └─────────────────────┘
```

## Key Design Decisions

### Single DynamoDB Table

Only `ClientRegistrationTable` is needed. No token storage tables because:

- Token validation is stateless (JWT + JWKS)
- MCP clients send tokens on every request
- Cognito handles token issuance and refresh
- The OAuth proxy forwards to Cognito for token exchange

### Who Registers Clients?

**MCP Clients (Primary)** - Via Dynamic Client Registration:
- Claude Desktop, ChatGPT, Cursor auto-register when user adds server URL
- User just provides server URL, client handles OAuth transparently

**Testing & CI/CD (Secondary)**:
- `cargo pmcp test` uses built-in test client
- CI/CD pipelines use pre-registered service accounts

**Admin Portal (Future)**:
- Organization admins manage/revoke clients
- Audit client usage

### Stateless Token Validation

```
Request Flow:
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│  Request 1: POST /mcp                                           │
│  Authorization: Bearer eyJhbG...                                │
│  → Validate JWT signature (JWKS cached in Lambda)               │
│  → Check expiry                                                 │
│  → Extract claims (sub, scopes)                                 │
│  → Process MCP request                                          │
│                                                                  │
│  Request 2: POST /mcp                                           │
│  Authorization: Bearer eyJhbG... (same or refreshed token)      │
│  → Same validation (no state from Request 1)                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

Benefits:
✓ No DynamoDB reads for token validation
✓ Infinite horizontal scaling
✓ No token table costs
✓ Cognito handles token lifecycle
✓ JWKS cached in Lambda (1 hour TTL)
```

## OAuth Flows

### 1. OpenID Connect Discovery

MCP clients discover OAuth endpoints automatically:

```
GET https://api.example.com/.well-known/openid-configuration

Response:
{
  "issuer": "https://api.example.com",
  "authorization_endpoint": "https://xxx.amazoncognito.com/oauth2/authorize",
  "token_endpoint": "https://api.example.com/oauth2/token",
  "registration_endpoint": "https://api.example.com/oauth2/register",
  "jwks_uri": "https://cognito-idp.{region}.amazonaws.com/{pool-id}/.well-known/jwks.json",
  "scopes_supported": ["openid", "email", "mcp/read", "mcp/write"],
  "grant_types_supported": ["authorization_code", "refresh_token"],
  "code_challenge_methods_supported": ["S256"]
}
```

### 2. Dynamic Client Registration (RFC 7591)

```
POST https://api.example.com/oauth2/register
Content-Type: application/json

{
  "client_name": "Claude Desktop",
  "redirect_uris": ["http://localhost:8080/callback"],
  "grant_types": ["authorization_code", "refresh_token"],
  "token_endpoint_auth_method": "none"
}

Response:
{
  "client_id": "abc123xyz",
  "client_name": "Claude Desktop",
  "token_endpoint_auth_method": "none"
}
```

**Public vs Confidential Clients:**
- Public clients (Claude Desktop, native apps): No client_secret, auth method = "none"
- Confidential clients (server-side): client_secret generated, auth method = "client_secret_post"

Auto-detection based on client_name patterns:
```rust
fn is_public_client(client_name: &str) -> bool {
    client_name.contains("claude") ||
    client_name.contains("desktop") ||
    client_name.contains("cursor") ||
    client_name.contains("mcp-inspector") ||
    client_name.contains("chatgpt")
}
```

### 3. Authorization Code Flow with PKCE

```
1. Client redirects user to:
   GET /oauth2/authorize
   ?client_id=abc123
   &redirect_uri=http://localhost:8080/callback
   &code_challenge=xxx (PKCE S256)
   &scope=openid email mcp/read
   &state=random123

2. OAuth Proxy redirects to Cognito Hosted UI

3. User authenticates (email/password, GitHub, etc.)

4. Cognito redirects back:
   GET http://localhost:8080/callback?code=AUTH_CODE&state=random123

5. Client exchanges code for tokens:
   POST /oauth2/token
   grant_type=authorization_code
   &code=AUTH_CODE
   &code_verifier=xxx (PKCE)
   &client_id=abc123

6. Response:
   {
     "access_token": "eyJhbG...",
     "token_type": "Bearer",
     "expires_in": 3600,
     "refresh_token": "xxx"
   }
```

### 4. MCP Request with Token

```
POST https://api.example.com/mcp
Authorization: Bearer eyJhbG...
Content-Type: application/json

{"jsonrpc": "2.0", "method": "tools/list", "id": 1}

Flow:
1. API Gateway invokes Lambda Authorizer
2. Authorizer validates JWT signature using cached JWKS
3. Authorizer extracts claims, returns Allow policy + context
4. API Gateway forwards request to MCP Server Lambda
5. MCP Server processes request (no OAuth code!)
6. Response returned to client
```

## CLI Commands

### Initialize with OAuth

```bash
$ cargo pmcp deploy init --target aws-lambda

? Enable OAuth authentication? (Y/n) Y

? OAuth provider:
  ❯ cognito     - AWS Cognito (recommended)
    oidc        - External OIDC provider (future)

? Cognito setup:
  ❯ Create new  - New User Pool for this server
    Existing    - Use existing User Pool
    Shared      - Use organization's shared pool

? User Pool name: my-mcp-users

? Enable social sign-in?
  [x] GitHub
  [ ] Google
  [ ] Apple

Creating OAuth infrastructure...
  ✓ Cognito User Pool: us-west-2_Abc123XyZ
  ✓ Resource Server: mcp (scopes: mcp/read, mcp/write)
  ✓ OAuth Proxy Lambda: my-server-oauth-proxy
  ✓ Authorizer Lambda: my-server-authorizer
  ✓ ClientRegistrationTable: my-server-clients

OAuth configuration saved to pmcp.toml
```

### Deploy with OAuth

```bash
$ cargo pmcp deploy

Building MCP server...
  ✓ Compiled: target/lambda/my-server/bootstrap

Deploying...
  ✓ Lambda: my-server-mcp
  ✓ API Gateway routes configured:
      GET  /.well-known/openid-configuration  (public)
      POST /oauth2/register                   (public, DCR)
      GET  /oauth2/authorize                  (public, redirect)
      POST /oauth2/token                      (public, exchange)
      POST /mcp                               (protected)
      POST /mcp/{proxy+}                      (protected)

Your MCP server is live:

  ┌─────────────────────────────────────────────────────────────┐
  │  Endpoint: https://abc123.execute-api.us-west-2.amazonaws.com│
  │  OAuth:    Enabled (Cognito)                                │
  │                                                             │
  │  MCP Clients can discover OAuth via:                        │
  │  https://abc123.execute-api.../.well-known/openid-configuration
  └─────────────────────────────────────────────────────────────┘
```

### Test with OAuth

```bash
$ cargo pmcp test --server myserver

Testing MCP server at https://abc123.execute-api...

? OAuth authentication required. Login method:
  ❯ Browser    - Open browser for OAuth login
    Token      - Paste existing access token

Opening browser for authentication...
  ✓ Authentication successful (user: guy@example.com)

Running MCP tests...
  ✓ tools/list (3 tools found)
  ✓ tools/call: get_weather (200ms)
  ✓ resources/list (2 resources)

All tests passed!
```

### View Registered Clients

```bash
$ cargo pmcp oauth clients

Registered OAuth Clients:

  CLIENT ID              NAME                REGISTERED        TYPE
  abc123def456          Claude Desktop       2025-12-01        public
  xyz789ghi012          ChatGPT              2025-12-02        public
  mcp-tester-ci         CI Pipeline          2025-11-28        confidential

3 clients registered via Dynamic Client Registration

# Note: Clients register themselves via /oauth2/register
# Use Cognito Console or Admin API to revoke clients
```

## Configuration

### pmcp.toml

```toml
[package]
name = "my-mcp-server"
version = "0.1.0"

[deploy]
target = "aws-lambda"
region = "us-west-2"

[deploy.oauth]
enabled = true
provider = "cognito"

[deploy.oauth.cognito]
# Created by `cargo pmcp deploy init`, or reference existing
user_pool_id = "us-west-2_Abc123XyZ"

# Resource server identifier (for scopes)
resource_server_id = "mcp"

# Social identity providers
social_providers = ["github"]

# Token TTLs (managed by Cognito)
access_token_ttl = "1h"
refresh_token_ttl = "30d"

[deploy.oauth.scopes]
"mcp/read" = "Read access to MCP tools and resources"
"mcp/write" = "Write access to MCP tools"
"mcp/admin" = "Administrative operations"

[deploy.oauth.dcr]
enabled = true

# Auto-detect public clients
public_client_patterns = [
    "claude",
    "desktop",
    "cursor",
    "mcp-inspector",
    "chatgpt"
]

# Default scopes for new clients
default_scopes = ["openid", "email", "mcp/read"]
```

## Shared Infrastructure

For organizations with multiple MCP servers:

```
Option A: Per-Server (Simple)          Option B: Shared (Enterprise)
─────────────────────────────          ────────────────────────────

┌─────────┐ ┌─────────┐ ┌─────────┐              ┌─────────────────┐
│Server A │ │Server B │ │Server C │              │ Shared Cognito  │
│+ OAuth  │ │+ OAuth  │ │+ OAuth  │              │ + OAuth Proxy   │
│+ Cognito│ │+ Cognito│ │+ Cognito│              │ + Authorizer    │
└─────────┘ └─────────┘ └─────────┘              └────────┬────────┘
                                                          │
                                       ┌──────────────────┼──────────────────┐
                                       ▼                  ▼                  ▼
                                  ┌─────────┐        ┌─────────┐        ┌─────────┐
                                  │Server A │        │Server B │        │Server C │
                                  │(no OAuth│        │(no OAuth│        │(no OAuth│
                                  │ infra)  │        │ infra)  │        │ infra)  │
                                  └─────────┘        └─────────┘        └─────────┘
```

### Shared Infrastructure Setup

```bash
# Initialize shared infrastructure (once per organization)
$ cargo pmcp oauth infra init --shared --name acme-corp

Creating shared OAuth infrastructure...
  ✓ Cognito User Pool: us-west-2_AcmeCorp
  ✓ OAuth Proxy Lambda: acme-corp-oauth-proxy
  ✓ Authorizer Lambda: acme-corp-authorizer
  ✓ ClientRegistrationTable: acme-corp-clients

Configuration saved to ~/.pmcp/shared-oauth/acme-corp.toml

# Individual servers use shared infra
$ cargo pmcp deploy init --oauth shared:acme-corp

Using shared OAuth infrastructure: acme-corp
  ✓ Authorizer: acme-corp-authorizer
  ✓ Adding resource server scopes: my-server/read, my-server/write
```

## Optional: pmcp-auth Middleware

For servers that need user context in tools (defense-in-depth):

```rust
use pmcp::prelude::*;
use pmcp_auth::{AuthContext, OAuthMiddleware};

#[derive(McpServer)]
struct MyServer {
    auth: Option<OAuthMiddleware>,
}

impl MyServer {
    fn new() -> Self {
        Self {
            // Auto-configures if OAUTH_* env vars present
            auth: OAuthMiddleware::from_env().ok(),
        }
    }
}

#[tool]
impl MyServer {
    /// Tool that uses user context
    #[tool(description = "Get user-specific data")]
    async fn get_my_data(&self, ctx: &RequestContext) -> Result<UserData> {
        if let Some(auth) = &self.auth {
            let user = auth.validate(ctx.bearer_token()?).await?;

            // Check scopes
            user.require_scope("mcp/read")?;

            // Use user context
            let data = self.db.get_user_data(&user.user_id).await?;
            Ok(data)
        } else {
            // No middleware = trust API Gateway validation
            Ok(default_data())
        }
    }
}
```

## Cost Estimate

Per 1M MCP requests/month:

| Component | Requests | Cost |
|-----------|----------|------|
| OAuth Proxy Lambda | ~10K (auth flows only) | ~$0.02 |
| Token Validator Lambda | ~200K (cached 5min) | ~$0.40 |
| MCP Server Lambda | 1M | ~$2.00 |
| API Gateway | 1M | ~$3.50 |
| DynamoDB (clients table) | ~10K R/W | ~$0.05 |
| Cognito | 1K MAU | $0 (50K free) |
| **Total** | | **~$6/month** |
| **OAuth overhead** | | **~$0.50/month** |

## Migration Path

### Phase 1: Cognito + Lambda (Current)
- OAuth Proxy Lambda
- Token Validator Lambda Authorizer
- DynamoDB ClientRegistrationTable
- `cargo pmcp deploy init --oauth cognito`

### Phase 2: pmcp-auth Crate
- Optional in-server middleware
- Defense-in-depth validation
- User context extraction

### Phase 3: AWS AgentCore (Future)
- Replace OAuth Proxy with AgentCore Gateway
- Keep DynamoDB (client registration persists)
- Keep token format (JWT compatible)
- MCP server code: ZERO changes

### Phase 4: Multi-Platform (Future)
- Cloudflare Workers: Edge JWT validation
- Cloud Run: IAP or sidecar
- Generic OIDC: Auth0, Okta, Azure AD

## Reference Implementation

See the Interactive Fiction Cloud project for a working example:
- OAuth Proxy Lambda implementation
- Token Validator Lambda Authorizer
- Amplify backend configuration
- DynamoDB table schema

## Security Considerations

1. **JWKS Caching**: 1-hour TTL prevents token validation failures
2. **PKCE Required**: S256 code challenge for all clients
3. **Short-lived Access Tokens**: 15-60 minute expiry
4. **Stateless Validation**: No session database attack surface
5. **Scope Enforcement**: Resource server scopes in Cognito
6. **Client Auto-Detection**: Public clients don't receive secrets

## FAQ

**Q: Do I need to write OAuth code in my MCP server?**
A: No. API Gateway + Lambda Authorizer handles all authentication. Your server just processes MCP requests.

**Q: How do clients register?**
A: MCP clients (Claude, ChatGPT, etc.) automatically register via Dynamic Client Registration when users add your server URL.

**Q: Where are tokens stored?**
A: Nowhere on the server side. Tokens are JWTs validated statelessly. MCP clients store their own tokens.

**Q: How do I revoke a client?**
A: Use the Cognito Console or AWS CLI to delete the app client. Or delete from ClientRegistrationTable.

**Q: Can I use my own identity provider?**
A: Future support for `--oauth oidc` with external providers (Auth0, Okta, etc.).
