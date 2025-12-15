# OAuth SDK Design: Developer Experience & Provider-Agnostic Architecture

> **Related Documents:**
> - `oauth-design.md` - AWS infrastructure implementation
> - `pmcp-run-oauth-design.md` - Multi-tenant pmcp.run architecture

## Executive Summary

This document describes the OAuth integration design for the PMCP SDK and `cargo-pmcp` CLI, focusing on:

1. **Developer Experience**: Incremental development from zero auth to production OAuth
2. **Provider Agnosticism**: Support for Cognito, Entra, Google, Okta, and generic OIDC
3. **Separation of Concerns**: MCP server code is completely OAuth-agnostic
4. **Configuration-Driven**: OAuth is a deployment concern, not a code concern

## Core Design Principle

```
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                          │
│   "Your MCP server code should NEVER know about OAuth providers,        │
│    tokens, or authentication flows. It only sees AuthContext."          │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Architecture Overview

### Responsibility Split

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           MCP CLIENT                                     │
│                                                                          │
│  Responsibilities:                                                       │
│  • Execute OAuth 2.1 authorization code flow with PKCE                  │
│  • Store tokens securely (access + refresh)                             │
│  • Refresh tokens when expired                                          │
│  • Add Authorization: Bearer <token> header to all requests             │
│                                                                          │
│  Reference: TypeScript SDK's OAuthClientProvider interface              │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ HTTP Request + Bearer Token
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           MCP SERVER                                     │
│                                                                          │
│  Responsibilities:                                                       │
│  • Validate incoming tokens (stateless JWT validation)                  │
│  • Extract claims into AuthContext                                      │
│  • Pass AuthContext to tool/resource handlers                           │
│  • Return 401/403 with appropriate WWW-Authenticate headers            │
│                                                                          │
│  NO Responsibilities:                                                    │
│  • Token storage (client handles this)                                  │
│  • Token refresh (client handles this)                                  │
│  • OAuth flow orchestration (proxy/gateway handles this)                │
│  • Provider-specific logic (configuration handles this)                 │
└─────────────────────────────────────────────────────────────────────────┘
```

### Token Flow (No Server-Side Storage)

```
Request Flow:
┌─────────────────────────────────────────────────────────────────────────┐
│                                                                          │
│  1. MCP Client obtains token from OAuth provider                        │
│     (via authorization code + PKCE flow)                                │
│                                                                          │
│  2. MCP Client sends request:                                           │
│     POST /mcp                                                           │
│     Authorization: Bearer eyJhbG...                                     │
│                                                                          │
│  3. MCP Server validates JWT:                                           │
│     - Fetch JWKS from provider (cached)                                 │
│     - Verify signature                                                  │
│     - Check expiry, issuer, audience                                    │
│     - Extract claims                                                    │
│                                                                          │
│  4. MCP Server creates AuthContext from claims                          │
│                                                                          │
│  5. Handler receives AuthContext (no knowledge of tokens/OAuth)         │
│                                                                          │
│  NO DATABASE READS for token validation                                 │
│  NO TOKEN STORAGE on server side                                        │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## Developer Journey

### Phase 1: Build (No Auth)

Developer focuses purely on MCP functionality:

```rust
// src/tools/greeting.rs
use pmcp::prelude::*;

#[tool(
    name = "greet",
    description = "Greet a user by name"
)]
pub async fn greet(
    #[arg(description = "Name to greet")] name: String,
) -> Result<String, ToolError> {
    Ok(format!("Hello, {}!", name))
}
```

```bash
# Development workflow
cargo pmcp new my-server         # Create new MCP server
cargo pmcp test                  # Run unit tests
cargo pmcp tester                # Interactive testing with mcp-tester
cargo pmcp run                   # Run locally (no auth)

# Deploy for internal testing (no OAuth)
cargo pmcp deploy --target pmcp-run
```

### Phase 2: Add Auth-Aware Logic (Optional)

Developer wants to use user context in their tools. **No OAuth provider configured yet** - uses mock auth for testing:

```rust
// src/tools/user_data.rs
use pmcp::prelude::*;

#[tool(
    name = "get_my_profile",
    description = "Get the current user's profile",
    requires_auth = true,  // Declares this tool requires authentication
)]
pub async fn get_my_profile(
    ctx: ToolContext,      // Contains auth: AuthContext
    db: Data<Database>,
) -> Result<UserProfile, ToolError> {
    // Access user info from AuthContext
    let user_id = &ctx.auth.user_id;

    // Use claims for multi-tenant queries
    let tenant_id = ctx.auth.tenant_id()
        .ok_or(ToolError::BadRequest("Missing tenant"))?;

    // Query database with user context
    let profile = db.get_user_profile(tenant_id, user_id).await?;

    Ok(profile)
}

#[tool(
    name = "query_external_api",
    description = "Query an external API on behalf of the user",
    requires_auth = true,
)]
pub async fn query_external_api(
    ctx: ToolContext,
    #[arg(description = "Query to execute")] query: String,
) -> Result<ApiResponse, ToolError> {
    // Forward the token to downstream service
    let token = ctx.auth.token.as_ref()
        .ok_or(ToolError::Unauthorized("Token required for API access"))?;

    let response = reqwest::Client::new()
        .post("https://api.example.com/query")
        .bearer_auth(token)  // Forward user's token
        .json(&QueryRequest { query })
        .send()
        .await?;

    Ok(response.json().await?)
}
```

#### Testing with Mock Auth

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::testing::MockAuthContext;

    #[tokio::test]
    async fn test_get_my_profile() {
        // Create mock auth context
        let auth = MockAuthContext::builder()
            .user_id("user-123")
            .tenant_id("tenant-abc")
            .scopes(vec!["read:profile"])
            .claim("email", "user@example.com")
            .build();

        let ctx = ToolContext::with_auth(auth);
        let db = setup_test_db().await;

        let result = get_my_profile(ctx, Data::new(db)).await.unwrap();
        assert_eq!(result.user_id, "user-123");
    }

    #[tokio::test]
    async fn test_requires_tenant() {
        // Auth without tenant_id should fail
        let auth = MockAuthContext::builder()
            .user_id("user-123")
            // No tenant_id
            .build();

        let ctx = ToolContext::with_auth(auth);
        let result = get_my_profile(ctx, Data::new(db)).await;

        assert!(matches!(result, Err(ToolError::BadRequest(_))));
    }
}
```

#### Local Development with Mock Auth

```toml
# pmcp.toml
[profile.dev]
[profile.dev.auth]
type = "mock"
default_user_id = "dev-user"
default_tenant_id = "dev-tenant"
default_scopes = ["read:profile", "write:profile"]

[profile.dev.auth.claims]
email = "dev@example.com"
name = "Developer"
```

```bash
# Run locally with mock auth
cargo pmcp run --profile dev

# mcp-tester will automatically use mock auth
cargo pmcp tester --profile dev
```

### Phase 3: Deploy with OAuth

Now the developer wants real authentication. **Configuration only - no code changes:**

```bash
# Initialize OAuth with Cognito
cargo pmcp oauth init --provider cognito

# Interactive wizard:
# ? AWS Region: us-east-1
# ? Create new User Pool or use existing? Create new
# ? User Pool name: my-server-users
# ? Enable social sign-in?
#   [x] GitHub
#   [ ] Google
#
# ✓ Created User Pool: us-east-1_xxxxx
# ✓ Created App Client
# ✓ Updated pmcp.toml with OAuth configuration
# ✓ Updated deployment configuration
#
# Next steps:
#   1. Deploy: cargo pmcp deploy --target pmcp-run
#   2. Configure your MCP client with the OAuth endpoints
```

#### What `oauth init` Does

1. **Creates OAuth resources** (User Pool, App Client) OR connects to existing
2. **Updates pmcp.toml** with provider configuration
3. **Updates deployment configuration** (CDK stack, etc.)
4. **Generates client configuration** for MCP clients

```toml
# pmcp.toml - after oauth init
[package]
name = "my-server"
version = "0.1.0"

[profile.production]
[profile.production.auth]
type = "jwt"
issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
audience = "xxxxx"  # App client ID

# OAuth endpoints for MCP clients
[profile.production.auth.oauth]
authorization_endpoint = "https://my-server-users.auth.us-east-1.amazoncognito.com/oauth2/authorize"
token_endpoint = "https://my-server-users.auth.us-east-1.amazoncognito.com/oauth2/token"

# Claim mappings (provider-specific → standard)
[profile.production.auth.claim_mappings]
tenant_id = "custom:tenant_id"  # Cognito custom attribute
email = "email"
name = "name"
```

```bash
# Deploy with OAuth enabled
cargo pmcp deploy --target pmcp-run --profile production
```

### Phase 4: Switch Providers

Developer wants to use organization's Entra ID instead of Cognito:

```bash
# Switch OAuth provider (keeps all code the same!)
cargo pmcp oauth switch --provider entra

# Interactive wizard:
# ? Azure Tenant ID: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
# ? App Registration Client ID: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
#
# ✓ Updated pmcp.toml with Entra configuration
# ✓ Claim mappings updated (Entra uses different claim names)
#
# Note: Your MCP server code doesn't change!

# Re-deploy with new provider
cargo pmcp deploy --target pmcp-run --profile production
```

```toml
# pmcp.toml - after switching to Entra
[profile.production.auth]
type = "jwt"
issuer = "https://login.microsoftonline.com/xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx/v2.0"
audience = "api://my-mcp-server"

[profile.production.auth.oauth]
authorization_endpoint = "https://login.microsoftonline.com/xxxxxxxx/oauth2/v2.0/authorize"
token_endpoint = "https://login.microsoftonline.com/xxxxxxxx/oauth2/v2.0/token"

# Claim mappings (Entra uses different names)
[profile.production.auth.claim_mappings]
tenant_id = "tid"           # Entra uses 'tid' for tenant
user_id = "oid"             # Entra uses 'oid' for user object ID
email = "preferred_username" # Entra puts email here
name = "name"
```

## Core SDK Types

### AuthContext

The **only** auth type your MCP code ever sees:

```rust
/// Authentication context passed to MCP handlers.
/// Provider-agnostic - works with Cognito, Entra, Okta, or any OIDC provider.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuthContext {
    /// Is this request authenticated?
    pub authenticated: bool,

    /// User identifier (sub claim, normalized via claim_mappings)
    pub user_id: String,

    /// Client/app that made the request
    pub client_id: String,

    /// Granted scopes
    pub scopes: Vec<String>,

    /// Token expiration (Unix timestamp)
    pub expires_at: Option<i64>,

    /// All claims from the token (for advanced use cases)
    pub claims: serde_json::Value,

    /// Raw token (for forwarding to downstream services)
    #[serde(skip_serializing)]
    pub token: Option<String>,
}

impl AuthContext {
    /// Get a typed claim value
    pub fn claim<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.claims.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get email (handles different claim names across providers)
    pub fn email(&self) -> Option<&str> {
        self.claims.get("email")
            .or_else(|| self.claims.get("preferred_username"))
            .and_then(|v| v.as_str())
    }

    /// Get tenant ID (handles different claim names across providers)
    pub fn tenant_id(&self) -> Option<&str> {
        self.claims.get("tenant_id")
            .or_else(|| self.claims.get("tid"))           // Entra
            .or_else(|| self.claims.get("custom:tenant")) // Cognito custom
            .and_then(|v| v.as_str())
    }

    /// Check if a scope is present
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Require a scope, return error if missing
    pub fn require_scope(&self, scope: &str) -> Result<(), AuthError> {
        if self.has_scope(scope) {
            Ok(())
        } else {
            Err(AuthError::InsufficientScope {
                required: scope.to_string(),
                present: self.scopes.clone(),
            })
        }
    }

    /// Require authentication
    pub fn require_auth(&self) -> Result<&Self, AuthError> {
        if self.authenticated {
            Ok(self)
        } else {
            Err(AuthError::Unauthenticated)
        }
    }
}
```

### TokenValidator Trait

Pluggable token validation - implementations are selected by configuration:

```rust
/// Token validation trait - implementations are configuration-driven
#[async_trait]
pub trait TokenValidator: Send + Sync + 'static {
    /// Validate a token and extract claims into AuthContext
    async fn validate(&self, token: &str) -> Result<AuthContext, TokenValidationError>;

    /// Refresh JWKS cache (if applicable)
    async fn refresh_keys(&self) -> Result<(), TokenValidationError> {
        Ok(()) // Default: no-op
    }
}

/// Errors that can occur during token validation
#[derive(Debug, thiserror::Error)]
pub enum TokenValidationError {
    #[error("Token is missing or malformed")]
    MalformedToken,

    #[error("Token signature is invalid")]
    InvalidSignature,

    #[error("Token has expired")]
    Expired,

    #[error("Token issuer '{0}' does not match expected")]
    InvalidIssuer(String),

    #[error("Token audience does not match")]
    InvalidAudience,

    #[error("Required scope '{required}' not present (have: {present:?})")]
    InsufficientScope { required: String, present: Vec<String> },

    #[error("JWKS fetch failed: {0}")]
    JwksFetchError(String),

    #[error("Key ID '{0}' not found in JWKS")]
    UnknownKeyId(String),

    #[error("Validation failed: {0}")]
    Other(String),
}
```

### Configuration Types

Configuration determines which validator is used - no hardcoded providers:

```rust
/// Token validator configuration (from pmcp.toml or environment)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TokenValidatorConfig {
    /// Validate JWTs locally using JWKS
    #[serde(rename = "jwt")]
    Jwt {
        /// OIDC issuer URL (used to derive JWKS URL if not specified)
        issuer: String,
        /// Expected audience (typically client ID)
        audience: String,
        /// Optional: explicit JWKS URI (otherwise derived from issuer)
        jwks_uri: Option<String>,
        /// Algorithms to accept (default: ["RS256"])
        #[serde(default = "default_algorithms")]
        algorithms: Vec<String>,
        /// JWKS cache TTL in seconds (default: 3600)
        #[serde(default = "default_jwks_ttl")]
        jwks_cache_ttl: u64,
    },

    /// Validate via token introspection endpoint (RFC 7662)
    #[serde(rename = "introspection")]
    Introspection {
        /// Introspection endpoint URL
        url: String,
        /// Credentials for authenticating to introspection endpoint
        client_id: Option<String>,
        client_secret: Option<String>,
    },

    /// Validate via external proxy (like Lambda authorizer)
    #[serde(rename = "proxy")]
    Proxy {
        /// URL of the validation proxy
        url: String,
        /// Headers to forward to proxy
        #[serde(default)]
        forward_headers: Vec<String>,
    },

    /// Mock authentication for development/testing
    #[serde(rename = "mock")]
    Mock {
        default_user_id: String,
        #[serde(default)]
        default_tenant_id: Option<String>,
        #[serde(default)]
        default_scopes: Vec<String>,
        #[serde(default)]
        claims: serde_json::Value,
    },

    /// No authentication (development only)
    #[serde(rename = "none")]
    Disabled,
}

/// Claim mappings - translate provider-specific claims to standard names
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClaimMappings {
    /// Claim name for user ID (default: "sub")
    #[serde(default = "default_user_id_claim")]
    pub user_id: String,
    /// Claim name for tenant ID
    pub tenant_id: Option<String>,
    /// Claim name for email
    pub email: Option<String>,
    /// Claim name for display name
    pub name: Option<String>,
    /// Additional custom mappings
    #[serde(flatten)]
    pub custom: HashMap<String, String>,
}

fn default_algorithms() -> Vec<String> {
    vec!["RS256".to_string()]
}

fn default_jwks_ttl() -> u64 {
    3600
}

fn default_user_id_claim() -> String {
    "sub".to_string()
}
```

## Validator Implementations

### JWT Validator (Stateless)

```rust
/// JWT validator using JWKS - works with any OIDC provider
pub struct JwtValidator {
    issuer: String,
    audience: String,
    jwks_uri: String,
    algorithms: Vec<Algorithm>,
    jwks: Arc<RwLock<CachedJwks>>,
    claim_mappings: ClaimMappings,
}

struct CachedJwks {
    keys: HashMap<String, DecodingKey>,
    fetched_at: Instant,
    ttl: Duration,
}

impl JwtValidator {
    /// Create from configuration
    pub async fn from_config(
        config: &JwtValidatorConfig,
        claim_mappings: ClaimMappings,
    ) -> Result<Self, Error> {
        let jwks_uri = config.jwks_uri.clone()
            .unwrap_or_else(|| format!("{}/.well-known/jwks.json", config.issuer));

        let jwks = Self::fetch_jwks(&jwks_uri).await?;

        Ok(Self {
            issuer: config.issuer.clone(),
            audience: config.audience.clone(),
            jwks_uri,
            algorithms: parse_algorithms(&config.algorithms),
            jwks: Arc::new(RwLock::new(CachedJwks {
                keys: jwks,
                fetched_at: Instant::now(),
                ttl: Duration::from_secs(config.jwks_cache_ttl),
            })),
            claim_mappings,
        })
    }

    async fn fetch_jwks(uri: &str) -> Result<HashMap<String, DecodingKey>, Error> {
        let response = reqwest::get(uri).await?;
        let jwks: JwksResponse = response.json().await?;

        let mut keys = HashMap::new();
        for key in jwks.keys {
            if let Some(kid) = &key.kid {
                let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)?;
                keys.insert(kid.clone(), decoding_key);
            }
        }

        Ok(keys)
    }

    async fn get_key(&self, kid: &str) -> Result<DecodingKey, TokenValidationError> {
        // Check cache
        {
            let cache = self.jwks.read().await;
            if cache.fetched_at.elapsed() < cache.ttl {
                if let Some(key) = cache.keys.get(kid) {
                    return Ok(key.clone());
                }
            }
        }

        // Refresh and retry
        self.refresh_keys().await?;

        let cache = self.jwks.read().await;
        cache.keys.get(kid)
            .cloned()
            .ok_or_else(|| TokenValidationError::UnknownKeyId(kid.to_string()))
    }
}

#[async_trait]
impl TokenValidator for JwtValidator {
    async fn validate(&self, token: &str) -> Result<AuthContext, TokenValidationError> {
        // 1. Decode header to get 'kid'
        let header = decode_header(token)
            .map_err(|_| TokenValidationError::MalformedToken)?;
        let kid = header.kid
            .ok_or(TokenValidationError::MalformedToken)?;

        // 2. Get key from JWKS (cached)
        let key = self.get_key(&kid).await?;

        // 3. Build validation parameters
        let mut validation = Validation::new(self.algorithms[0].clone());
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        // 4. Decode and verify
        let token_data = decode::<serde_json::Value>(token, &key, &validation)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenValidationError::Expired,
                jsonwebtoken::errors::ErrorKind::InvalidIssuer => {
                    TokenValidationError::InvalidIssuer(self.issuer.clone())
                }
                jsonwebtoken::errors::ErrorKind::InvalidAudience => TokenValidationError::InvalidAudience,
                _ => TokenValidationError::InvalidSignature,
            })?;

        // 5. Extract claims using mappings
        let claims = token_data.claims;

        Ok(AuthContext {
            authenticated: true,
            user_id: self.extract_claim(&claims, &self.claim_mappings.user_id)
                .unwrap_or_default(),
            client_id: claims.get("azp")
                .or_else(|| claims.get("client_id"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            scopes: parse_scopes(&claims),
            expires_at: claims.get("exp").and_then(|v| v.as_i64()),
            claims,
            token: Some(token.to_string()),
        })
    }

    async fn refresh_keys(&self) -> Result<(), TokenValidationError> {
        let new_keys = Self::fetch_jwks(&self.jwks_uri).await
            .map_err(|e| TokenValidationError::JwksFetchError(e.to_string()))?;

        let mut cache = self.jwks.write().await;
        cache.keys = new_keys;
        cache.fetched_at = Instant::now();

        Ok(())
    }
}
```

### Mock Validator (Development/Testing)

```rust
/// Mock validator for development and testing
pub struct MockValidator {
    config: MockValidatorConfig,
}

#[async_trait]
impl TokenValidator for MockValidator {
    async fn validate(&self, _token: &str) -> Result<AuthContext, TokenValidationError> {
        // Always returns the configured mock context
        Ok(AuthContext {
            authenticated: true,
            user_id: self.config.default_user_id.clone(),
            client_id: "mock-client".to_string(),
            scopes: self.config.default_scopes.clone(),
            expires_at: Some(chrono::Utc::now().timestamp() + 3600),
            claims: self.config.claims.clone(),
            token: None,
        })
    }
}
```

## Server Integration

### ServerBuilder Integration

```rust
impl ServerBuilder {
    /// Configure authentication from config file/environment
    pub fn with_auth_config(mut self, config: TokenValidatorConfig) -> Self {
        self.auth_config = Some(config);
        self
    }

    /// Configure authentication with custom validator
    pub fn with_token_validator<V: TokenValidator>(mut self, validator: V) -> Self {
        self.token_validator = Some(Arc::new(validator));
        self
    }

    /// Configure claim mappings
    pub fn with_claim_mappings(mut self, mappings: ClaimMappings) -> Self {
        self.claim_mappings = mappings;
        self
    }

    /// Require authentication for all requests
    pub fn require_auth(mut self) -> Self {
        self.require_auth = true;
        self
    }

    /// Require specific scopes for all requests
    pub fn require_scopes(mut self, scopes: Vec<String>) -> Self {
        self.required_scopes = scopes;
        self
    }
}
```

### Request Handling

```rust
impl Server {
    async fn handle_request(
        &self,
        request: McpRequest,
        auth_header: Option<&str>,
    ) -> Result<McpResponse, Error> {
        // Extract and validate token if auth is configured
        let auth_context = match (&self.token_validator, auth_header) {
            (Some(validator), Some(header)) => {
                let token = extract_bearer_token(header)?;
                let auth = validator.validate(&token).await?;

                // Check required scopes if configured
                for scope in &self.required_scopes {
                    auth.require_scope(scope)?;
                }

                Some(auth)
            }
            (Some(_), None) if self.require_auth => {
                return Err(Error::Unauthorized("Authentication required"));
            }
            _ => None,
        };

        // Create context with auth
        let ctx = RequestContext {
            auth: auth_context.unwrap_or_default(),
            ..Default::default()
        };

        // Dispatch to handler
        self.handler.handle(request, ctx).await
    }
}
```

## CLI Commands

### Complete Command Reference

```bash
# ============================================================================
# PHASE 1: BUILD (No Auth)
# ============================================================================

cargo pmcp new my-server              # Create new MCP server project
cargo pmcp build                      # Build the server
cargo pmcp test                       # Run unit tests
cargo pmcp tester                     # Interactive testing with mcp-tester
cargo pmcp run                        # Run locally (no auth)

# ============================================================================
# PHASE 2: AUTH-AWARE DEVELOPMENT
# ============================================================================

cargo pmcp run --profile dev          # Run with mock auth
cargo pmcp tester --profile dev       # Test with mock auth
cargo pmcp tester --auth-token "xxx"  # Test with specific token

# ============================================================================
# PHASE 3: ADD OAUTH
# ============================================================================

# Initialize with specific provider
cargo pmcp oauth init --provider cognito
cargo pmcp oauth init --provider entra
cargo pmcp oauth init --provider google
cargo pmcp oauth init --provider okta
cargo pmcp oauth init --provider oidc \
    --issuer "https://your-provider.com" \
    --client-id "xxx"

# View current OAuth configuration
cargo pmcp oauth status

# Test token validation locally
cargo pmcp oauth test --token "eyJ..."

# Validate configuration
cargo pmcp oauth validate

# ============================================================================
# PHASE 4: MANAGE OAUTH
# ============================================================================

# Switch to different provider
cargo pmcp oauth switch --provider entra

# Update configuration
cargo pmcp oauth configure \
    --issuer "https://new-issuer.com" \
    --audience "new-audience"

# Remove OAuth (go back to no auth)
cargo pmcp oauth remove

# Rotate secrets (provider-specific)
cargo pmcp oauth rotate-secrets

# List registered clients (if using DCR)
cargo pmcp oauth clients

# ============================================================================
# DEPLOYMENT
# ============================================================================

# Deploy with specific profile
cargo pmcp deploy --target pmcp-run --profile production
cargo pmcp deploy --target aws-lambda --profile staging

# Deploy with OAuth enabled
cargo pmcp deploy --target pmcp-run --oauth

# View deployment OAuth info
cargo pmcp deploy info --show-oauth
```

### `oauth init` Wizard Flow

```bash
$ cargo pmcp oauth init

? Select OAuth provider:
  > AWS Cognito (recommended for AWS deployments)
    Microsoft Entra ID (Azure AD)
    Google Identity
    Okta
    Auth0
    Generic OIDC

# --- AWS Cognito ---
? AWS Region: us-east-1
? Create new User Pool or use existing?
  > Create new User Pool
    Use existing User Pool
    Use shared organization pool

? User Pool name: my-server-users

? Enable social sign-in?
  [x] GitHub
  [ ] Google
  [ ] Apple
  [ ] Facebook

? Default scopes for new clients:
  [x] openid
  [x] email
  [x] profile
  [ ] mcp/admin

Creating OAuth infrastructure...
  ✓ Created User Pool: us-east-1_xxxxx
  ✓ Created App Client: xxxxx
  ✓ Created Resource Server with MCP scopes
  ✓ Updated pmcp.toml
  ✓ Updated deployment configuration

OAuth initialized successfully!

Configuration saved to: pmcp.toml
Profile: production

Next steps:
  1. Review configuration: cargo pmcp oauth status
  2. Deploy: cargo pmcp deploy --target pmcp-run --profile production
  3. Test: cargo pmcp oauth test --token "<your-token>"
```

## Configuration Reference

### Complete pmcp.toml Example

```toml
[package]
name = "my-mcp-server"
version = "0.1.0"
description = "My awesome MCP server"

# ============================================================================
# DEVELOPMENT PROFILE (mock auth)
# ============================================================================
[profile.dev]
[profile.dev.auth]
type = "mock"
default_user_id = "dev-user-123"
default_tenant_id = "dev-tenant"
default_scopes = ["openid", "email", "mcp/read", "mcp/write"]

[profile.dev.auth.claims]
email = "developer@example.com"
name = "Local Developer"
roles = ["admin", "developer"]

# ============================================================================
# STAGING PROFILE (Cognito)
# ============================================================================
[profile.staging]
[profile.staging.auth]
type = "jwt"
issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_STAGING"
audience = "staging-client-id"
jwks_cache_ttl = 3600

[profile.staging.auth.oauth]
authorization_endpoint = "https://staging.auth.us-east-1.amazoncognito.com/oauth2/authorize"
token_endpoint = "https://staging.auth.us-east-1.amazoncognito.com/oauth2/token"
registration_endpoint = "https://api.staging.example.com/oauth2/register"
scopes_supported = ["openid", "email", "mcp/read", "mcp/write"]

[profile.staging.auth.claim_mappings]
user_id = "sub"
tenant_id = "custom:tenant_id"
email = "email"
name = "name"

# ============================================================================
# PRODUCTION PROFILE (Entra ID)
# ============================================================================
[profile.production]
[profile.production.auth]
type = "jwt"
issuer = "https://login.microsoftonline.com/xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx/v2.0"
audience = "api://my-mcp-server"
algorithms = ["RS256"]
jwks_cache_ttl = 3600

[profile.production.auth.oauth]
authorization_endpoint = "https://login.microsoftonline.com/xxxxxxxx/oauth2/v2.0/authorize"
token_endpoint = "https://login.microsoftonline.com/xxxxxxxx/oauth2/v2.0/token"
scopes_supported = ["openid", "email", "profile", "api://my-mcp-server/mcp.read"]

[profile.production.auth.claim_mappings]
user_id = "oid"        # Entra uses 'oid' for user object ID
tenant_id = "tid"      # Entra uses 'tid' for Azure tenant
email = "preferred_username"
name = "name"

# ============================================================================
# DEPLOYMENT CONFIGURATION
# ============================================================================
[deploy]
default_target = "pmcp-run"
default_profile = "production"

[deploy.pmcp-run]
region = "us-east-1"
```

### Environment Variable Override

All configuration can be overridden via environment variables:

```bash
# Override auth type
PMCP_AUTH_TYPE=jwt

# Override JWT settings
PMCP_AUTH_ISSUER=https://login.microsoftonline.com/xxx/v2.0
PMCP_AUTH_AUDIENCE=api://my-server
PMCP_AUTH_JWKS_URI=https://login.microsoftonline.com/xxx/discovery/v2.0/keys

# Override claim mappings
PMCP_AUTH_CLAIM_USER_ID=oid
PMCP_AUTH_CLAIM_TENANT_ID=tid
PMCP_AUTH_CLAIM_EMAIL=preferred_username

# Full config as JSON (overrides everything)
PMCP_AUTH_CONFIG='{"type":"jwt","issuer":"...","audience":"..."}'
```

## Provider-Specific Claim Mappings

### Quick Reference Table

| Standard Name | Cognito | Entra ID | Google | Okta | Auth0 |
|---------------|---------|----------|--------|------|-------|
| `user_id` | `sub` | `oid` | `sub` | `uid` | `sub` |
| `tenant_id` | `custom:tenant` | `tid` | N/A | `org_id` | `org_id` |
| `email` | `email` | `preferred_username` | `email` | `email` | `email` |
| `name` | `name` | `name` | `name` | `name` | `name` |
| `groups` | `cognito:groups` | `groups` | N/A | `groups` | `roles` |

### Provider Configuration Examples

#### AWS Cognito

```toml
[profile.production.auth]
type = "jwt"
issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
audience = "your-app-client-id"

[profile.production.auth.claim_mappings]
user_id = "sub"
tenant_id = "custom:tenant_id"
email = "email"
name = "name"
groups = "cognito:groups"
```

#### Microsoft Entra ID

```toml
[profile.production.auth]
type = "jwt"
issuer = "https://login.microsoftonline.com/{tenant-id}/v2.0"
audience = "api://your-app-id"

[profile.production.auth.claim_mappings]
user_id = "oid"
tenant_id = "tid"
email = "preferred_username"
name = "name"
groups = "groups"
```

#### Google Identity

```toml
[profile.production.auth]
type = "jwt"
issuer = "https://accounts.google.com"
audience = "your-client-id.apps.googleusercontent.com"

[profile.production.auth.claim_mappings]
user_id = "sub"
email = "email"
name = "name"
# Note: Google doesn't have tenant concept by default
```

#### Okta

```toml
[profile.production.auth]
type = "jwt"
issuer = "https://your-domain.okta.com"
audience = "api://your-api"

[profile.production.auth.claim_mappings]
user_id = "uid"
tenant_id = "org_id"
email = "email"
name = "name"
groups = "groups"
```

#### Auth0

```toml
[profile.production.auth]
type = "jwt"
issuer = "https://your-domain.auth0.com/"
audience = "https://your-api"

[profile.production.auth.claim_mappings]
user_id = "sub"
tenant_id = "org_id"
email = "email"
name = "name"
groups = "roles"
```

## Testing Guide

### Unit Testing with Mock Auth

```rust
#[cfg(test)]
mod tests {
    use pmcp::testing::{MockAuthContext, TestServer};

    #[tokio::test]
    async fn test_authenticated_tool() {
        let auth = MockAuthContext::builder()
            .user_id("test-user")
            .tenant_id("test-tenant")
            .scopes(vec!["mcp/read", "mcp/write"])
            .claim("email", "test@example.com")
            .build();

        let server = TestServer::new()
            .with_auth(auth)
            .build();

        let response = server.call_tool("my_tool", json!({"arg": "value"})).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_missing_scope() {
        let auth = MockAuthContext::builder()
            .user_id("test-user")
            .scopes(vec!["mcp/read"])  // Missing mcp/write
            .build();

        let server = TestServer::new()
            .with_auth(auth)
            .build();

        let response = server.call_tool("write_tool", json!({})).await;
        assert!(matches!(response, Err(ToolError::InsufficientScope(_))));
    }

    #[tokio::test]
    async fn test_unauthenticated() {
        let server = TestServer::new()
            .require_auth()
            .build();

        let response = server.call_tool("my_tool", json!({})).await;
        assert!(matches!(response, Err(ToolError::Unauthorized(_))));
    }
}
```

### Integration Testing with Real Tokens

```bash
# Get a test token (provider-specific)
# Cognito:
aws cognito-idp admin-initiate-auth \
    --user-pool-id us-east-1_xxxxx \
    --client-id xxxxx \
    --auth-flow ADMIN_NO_SRP_AUTH \
    --auth-parameters USERNAME=test@example.com,PASSWORD=xxx

# Test with mcp-tester
cargo pmcp tester --auth-token "eyJ..."

# Test token validation
cargo pmcp oauth test --token "eyJ..."
# Output:
# ✓ Token is valid
#
# Claims:
#   user_id: user-123
#   tenant_id: tenant-abc
#   email: user@example.com
#   scopes: openid, email, mcp/read
#   expires_at: 2025-12-14T12:00:00Z
```

## Security Considerations

### Token Validation Best Practices

1. **Always validate signature** - Never trust token contents without verification
2. **Check expiration** - Reject expired tokens
3. **Validate issuer** - Ensure token came from expected provider
4. **Validate audience** - Ensure token was issued for your server
5. **Cache JWKS** - But refresh periodically (1 hour default)
6. **Use HTTPS** - For all OAuth endpoints

### Scope Design Guidelines

```toml
# Recommended scope hierarchy
[scopes]
"mcp/read" = "Read access to tools and resources"
"mcp/write" = "Modify data via tools"
"mcp/admin" = "Administrative operations"
"mcp/dangerous" = "Potentially destructive operations"
```

### Token Forwarding

When forwarding tokens to downstream services:

```rust
#[tool(requires_auth = true)]
async fn call_downstream_api(ctx: ToolContext) -> Result<Response, ToolError> {
    // Only forward token if:
    // 1. The downstream service expects it
    // 2. The token's audience includes the downstream service
    // 3. The token has sufficient scopes

    let token = ctx.auth.token.as_ref()
        .ok_or(ToolError::Unauthorized("Token required"))?;

    // Verify token is intended for downstream service
    if !ctx.auth.claims.get("aud")
        .map(|aud| aud.as_array().map(|a| a.iter().any(|v| v == "downstream-api")))
        .flatten()
        .unwrap_or(false)
    {
        return Err(ToolError::Forbidden("Token not valid for downstream API"));
    }

    // Forward token
    client.post(url)
        .bearer_auth(token)
        .send()
        .await
}
```

## Implementation Roadmap

### Phase 1: Core Types & JWT Validation
- [ ] `AuthContext` struct
- [ ] `TokenValidator` trait
- [ ] `TokenValidatorConfig` enum
- [ ] `JwtValidator` implementation
- [ ] JWKS fetching and caching
- [ ] Claim mappings

### Phase 2: Mock & Testing Support
- [ ] `MockValidator` implementation
- [ ] `MockAuthContext` builder
- [ ] `TestServer` auth integration
- [ ] Testing utilities

### Phase 3: Server Integration
- [ ] `ServerBuilder` auth methods
- [ ] Request middleware
- [ ] Error responses (401/403)
- [ ] `#[tool(requires_auth)]` attribute

### Phase 4: CLI Commands
- [ ] `cargo pmcp oauth init`
- [ ] `cargo pmcp oauth status`
- [ ] `cargo pmcp oauth test`
- [ ] `cargo pmcp oauth switch`
- [ ] Profile support in all commands

### Phase 5: Provider Integrations
- [ ] Cognito setup wizard
- [ ] Entra ID setup wizard
- [ ] Generic OIDC setup
- [ ] Deployment configuration updates

### Phase 6: Documentation & Examples
- [ ] Complete API documentation
- [ ] Example: Basic auth-aware server
- [ ] Example: Multi-tenant server
- [ ] Example: Token forwarding

## References

- [MCP Authentication Specification](https://spec.modelcontextprotocol.io/specification/2025-03-26/basic/authentication/)
- [TypeScript MCP SDK Auth Implementation](https://github.com/modelcontextprotocol/typescript-sdk)
- [OAuth 2.1 Draft Specification](https://datatracker.ietf.org/doc/html/draft-ietf-oauth-v2-1-07)
- [RFC 7519 - JSON Web Token (JWT)](https://datatracker.ietf.org/doc/html/rfc7519)
- [RFC 7591 - Dynamic Client Registration](https://datatracker.ietf.org/doc/html/rfc7591)
- [RFC 7662 - Token Introspection](https://datatracker.ietf.org/doc/html/rfc7662)
- [Cloudflare workers-oauth-provider](https://github.com/cloudflare/workers-oauth-provider/) - Reference implementation
