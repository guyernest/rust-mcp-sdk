# Token Validation

This chapter covers the practical implementation of JWT token validation in Rust MCP servers. Proper validation is critical for security.

## Multi-Layer Security: Understanding Where to Validate

Before diving into implementation, understand that security happens at multiple layers. You don't have to implement everything in your MCP server—you can leverage existing security in your backend systems.

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Security Layers in MCP                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  LAYER 1: MCP Server Access                                         │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  Question: Can this user reach the MCP server at all?         │  │
│  │  Validated: Token signature, expiration, issuer, audience     │  │
│  │  Claims used: sub, iss, aud, exp                              │  │
│  │  Result: 401 Unauthorized if invalid                          │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                           │                                         │
│                           ▼                                         │
│  LAYER 2: Tool-Level Authorization                                  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  Question: Can this user call this specific tool?             │  │
│  │  Validated: Scopes match tool requirements                    │  │
│  │  Claims used: scope, permissions, roles, groups               │  │
│  │  Result: 403 Forbidden if insufficient permissions            │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                           │                                         │
│                           ▼                                         │
│  LAYER 3: Data-Level Security (Backend Systems)                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  Question: What data can this user see/modify?                │  │
│  │  Validated by: Database, API, or data platform                │  │
│  │  Examples:                                                    │  │
│  │  • PostgreSQL Row-Level Security (RLS)                        │  │
│  │  • GraphQL field-level authorization                          │  │
│  │  • API gateway per-resource policies                          │  │
│  │  • Data warehouse column masking                              │  │
│  │  Result: Filtered/masked data or 403 from backend             │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Why Multiple Layers?

Each layer catches different security concerns:

| Layer | What It Catches | Example |
|-------|-----------------|---------|
| **Layer 1: Server Access** | Invalid/expired tokens, wrong IdP, attacks | Stolen token from different app |
| **Layer 2: Tool Authorization** | Users calling tools they shouldn't | Analyst trying to use admin tools |
| **Layer 3: Data Security** | Users accessing data they shouldn't | User A reading User B's records |

### What You Control vs. What You Delegate

**Your MCP server handles Layers 1 & 2:**
- Validate the token is legitimate (Layer 1)
- Check scopes match tool requirements (Layer 2)
- Pass user identity to backend systems

**Backend systems handle Layer 3:**
- Databases enforce row-level security using the user ID you provide
- APIs check permissions on each resource
- Data platforms apply column masking based on user roles

**The advantage:** You don't reinvent data security. If your database already has RLS policies, or your API already checks permissions, your MCP server just passes through the authenticated user identity and lets the backend do what it already does.

### Practical Example: The Three Layers in Action

```rust
// LAYER 1: Happens in middleware before your tool code runs
// The request already has a validated token at this point

#[derive(TypedTool)]
#[tool(name = "query_sales", description = "Query sales data")]
pub struct QuerySales;

impl QuerySales {
    pub async fn run(
        &self,
        input: QueryInput,
        context: &ToolContext,
    ) -> Result<SalesData> {
        let auth = context.auth()?;

        // LAYER 2: Check tool-level scope
        // "Can this user call this tool at all?"
        auth.require_scope("read:sales")?;

        // LAYER 3: Pass identity to database, let RLS handle row filtering
        // "What sales records can this user see?"
        let results = self.database
            .query(&input.sql)
            .with_user_context(&auth.user_id, &auth.org_id)  // Database uses this for RLS
            .await?;

        // The database only returns rows this user is allowed to see
        // We didn't write that logic—the database handles it

        Ok(results)
    }
}
```

### Layer 3 Examples in Different Systems

**PostgreSQL Row-Level Security:**
```sql
-- Policy defined once in database, enforced automatically
CREATE POLICY sales_team_only ON sales
    FOR SELECT
    USING (team_id = current_setting('app.team_id')::uuid);

-- MCP server just sets the context
SET app.team_id = 'team-123';  -- From JWT claims
SELECT * FROM sales;  -- Only sees their team's data
```

**GraphQL with field-level auth:**
```graphql
type Customer {
  id: ID!
  name: String!
  email: String! @auth(requires: "read:pii")      # Only users with PII scope
  ssn: String @auth(requires: "admin:sensitive")  # Only admins
}
```

**API Gateway policies:**
```yaml
# AWS API Gateway resource policy
/customers/{customerId}:
  GET:
    auth:
      # User can only access customers in their organization
      condition: $context.authorizer.org_id == $resource.org_id
```

### Choosing Where to Implement Security

| Security Concern | Best Layer | Reasoning |
|-----------------|------------|-----------|
| "Is this token valid?" | Layer 1 (MCP Server) | Must happen first |
| "Can user call this tool?" | Layer 2 (MCP Server) | Scope-based, defined in IdP |
| "Can user see this row?" | Layer 3 (Database) | Database knows data relationships |
| "Can user see this field?" | Layer 3 (API/GraphQL) | Field sensitivity is data concern |
| "What columns should be masked?" | Layer 3 (Data Platform) | Masking rules are data governance |

**The principle:** Implement security as close to the data as possible. Your MCP server is the front door (Layers 1 & 2), but the data systems are the vault (Layer 3).

## The Validation Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                    JWT Validation Pipeline                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Incoming Request                                                   │
│       │                                                             │
│       ▼                                                             │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 1. EXTRACT TOKEN                                            │    │
│  │    Authorization: Bearer eyJhbGciOiJS...                    │    │
│  └─────────────────────────────┬───────────────────────────────┘    │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 2. DECODE HEADER (without verification)                     │    │
│  │    { "alg": "RS256", "kid": "key-123" }                     │    │
│  └─────────────────────────────┬───────────────────────────────┘    │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 3. FETCH PUBLIC KEY (from JWKS, cached)                     │    │
│  │    Match key by "kid" from header                           │    │
│  └─────────────────────────────┬───────────────────────────────┘    │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 4. VERIFY SIGNATURE                                         │    │
│  │    RSA/ECDSA verification using public key                  │    │
│  └─────────────────────────────┬───────────────────────────────┘    │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 5. VALIDATE CLAIMS                                          │    │
│  │    • exp (expiration)                                       │    │
│  │    • nbf (not before)                                       │    │
│  │    • iss (issuer)                                           │    │
│  │    • aud (audience)                                         │    │
│  └─────────────────────────────┬───────────────────────────────┘    │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │ 6. EXTRACT USER INFO                                        │    │
│  │    sub, email, scopes → AuthContext                         │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Core Dependencies

```toml
# Cargo.toml
[dependencies]
jsonwebtoken = "9"           # JWT encoding/decoding
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["sync"] }
thiserror = "1"
tracing = "0.1"
```

## Token Extractor

First, extract the token from the Authorization header:

```rust
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode, header},
    response::{IntoResponse, Response},
};

pub struct BearerToken(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or(AuthError::MissingToken)?;

        let auth_str = auth_header
            .to_str()
            .map_err(|_| AuthError::InvalidHeader)?;

        if !auth_str.starts_with("Bearer ") {
            return Err(AuthError::InvalidScheme);
        }

        let token = auth_str[7..].trim().to_string();

        if token.is_empty() {
            return Err(AuthError::MissingToken);
        }

        Ok(BearerToken(token))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing authorization token")]
    MissingToken,

    #[error("Invalid authorization header")]
    InvalidHeader,

    #[error("Invalid authorization scheme, expected Bearer")]
    InvalidScheme,

    #[error("Token validation failed: {0}")]
    ValidationFailed(String),

    #[error("Insufficient permissions")]
    InsufficientScope,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AuthError::MissingToken | AuthError::InvalidHeader | AuthError::InvalidScheme => {
                (StatusCode::UNAUTHORIZED, self.to_string())
            }
            AuthError::ValidationFailed(_) => {
                (StatusCode::UNAUTHORIZED, self.to_string())
            }
            AuthError::InsufficientScope => {
                (StatusCode::FORBIDDEN, self.to_string())
            }
        };

        let body = serde_json::json!({
            "error": "authentication_error",
            "message": message
        });

        (status, axum::Json(body)).into_response()
    }
}
```

## JWKS Fetcher with Caching

Fetch and cache public keys from the IdP:

```rust
use jsonwebtoken::jwk::{JwkSet, Jwk};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

pub struct JwksClient {
    jwks_uri: String,
    client: reqwest::Client,
    cache: Arc<RwLock<Option<CachedJwks>>>,
    cache_duration: Duration,
}

struct CachedJwks {
    jwks: JwkSet,
    fetched_at: Instant,
}

impl JwksClient {
    pub fn new(jwks_uri: String) -> Self {
        Self {
            jwks_uri,
            client: reqwest::Client::new(),
            cache: Arc::new(RwLock::new(None)),
            cache_duration: Duration::from_secs(3600), // 1 hour
        }
    }

    pub async fn get_key(&self, kid: &str) -> Result<Jwk, AuthError> {
        let jwks = self.get_jwks().await?;

        jwks.keys
            .iter()
            .find(|k| k.common.key_id.as_deref() == Some(kid))
            .cloned()
            .ok_or_else(|| AuthError::ValidationFailed(
                format!("Key not found: {}", kid)
            ))
    }

    async fn get_jwks(&self) -> Result<JwkSet, AuthError> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = &*cache {
                if cached.fetched_at.elapsed() < self.cache_duration {
                    return Ok(cached.jwks.clone());
                }
            }
        }

        // Fetch fresh JWKS
        let jwks = self.fetch_jwks().await?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(CachedJwks {
                jwks: jwks.clone(),
                fetched_at: Instant::now(),
            });
        }

        Ok(jwks)
    }

    async fn fetch_jwks(&self) -> Result<JwkSet, AuthError> {
        tracing::debug!("Fetching JWKS from {}", self.jwks_uri);

        self.client
            .get(&self.jwks_uri)
            .send()
            .await
            .map_err(|e| AuthError::ValidationFailed(format!("JWKS fetch failed: {}", e)))?
            .json::<JwkSet>()
            .await
            .map_err(|e| AuthError::ValidationFailed(format!("JWKS parse failed: {}", e)))
    }

    /// Force refresh the cache (call on key rotation)
    pub async fn refresh(&self) -> Result<(), AuthError> {
        let jwks = self.fetch_jwks().await?;
        let mut cache = self.cache.write().await;
        *cache = Some(CachedJwks {
            jwks,
            fetched_at: Instant::now(),
        });
        Ok(())
    }
}
```

## JWT Validator

The core validation logic:

```rust
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct JwtValidatorConfig {
    pub issuer: String,
    pub audience: String,
    pub jwks_uri: String,
    pub algorithms: Vec<Algorithm>,
    pub leeway_seconds: u64,
}

impl JwtValidatorConfig {
    /// Create config for AWS Cognito
    pub fn cognito(region: &str, user_pool_id: &str, client_id: &str) -> Self {
        let issuer = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);

        Self {
            issuer,
            audience: client_id.to_string(),
            jwks_uri,
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }

    /// Create config for Auth0
    pub fn auth0(domain: &str, audience: &str) -> Self {
        Self {
            issuer: format!("https://{}/", domain),
            audience: audience.to_string(),
            jwks_uri: format!("https://{}/.well-known/jwks.json", domain),
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }

    /// Create config for Microsoft Entra ID
    pub fn entra(tenant_id: &str, client_id: &str) -> Self {
        Self {
            issuer: format!("https://login.microsoftonline.com/{}/v2.0", tenant_id),
            audience: client_id.to_string(),
            jwks_uri: format!(
                "https://login.microsoftonline.com/{}/discovery/v2.0/keys",
                tenant_id
            ),
            algorithms: vec![Algorithm::RS256],
            leeway_seconds: 60,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: ClaimAudience,
    pub exp: u64,
    pub iat: u64,
    #[serde(default)]
    pub nbf: Option<u64>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaimAudience {
    Single(String),
    Multiple(Vec<String>),
}

impl ClaimAudience {
    pub fn contains(&self, audience: &str) -> bool {
        match self {
            ClaimAudience::Single(s) => s == audience,
            ClaimAudience::Multiple(v) => v.iter().any(|a| a == audience),
        }
    }
}

pub struct JwtValidator {
    config: JwtValidatorConfig,
    jwks_client: JwksClient,
}

impl JwtValidator {
    pub fn new(config: JwtValidatorConfig) -> Self {
        let jwks_client = JwksClient::new(config.jwks_uri.clone());
        Self { config, jwks_client }
    }

    pub async fn validate(&self, token: &str) -> Result<Claims, AuthError> {
        // 1. Decode header to get key ID
        let header = decode_header(token)
            .map_err(|e| AuthError::ValidationFailed(format!("Invalid header: {}", e)))?;

        let kid = header.kid
            .ok_or_else(|| AuthError::ValidationFailed("Missing kid in header".into()))?;

        // 2. Verify algorithm is allowed
        if !self.config.algorithms.contains(&header.alg) {
            return Err(AuthError::ValidationFailed(format!(
                "Algorithm not allowed: {:?}",
                header.alg
            )));
        }

        // 3. Fetch public key
        let jwk = self.jwks_client.get_key(&kid).await?;

        // 4. Create decoding key
        let decoding_key = DecodingKey::from_jwk(&jwk)
            .map_err(|e| AuthError::ValidationFailed(format!("Invalid JWK: {}", e)))?;

        // 5. Set up validation
        let mut validation = Validation::new(header.alg);
        validation.set_issuer(&[&self.config.issuer]);
        validation.set_audience(&[&self.config.audience]);
        validation.leeway = self.config.leeway_seconds;

        // 6. Decode and validate
        let token_data = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| AuthError::ValidationFailed(format!("Validation failed: {}", e)))?;

        let claims = token_data.claims;

        // 7. Additional audience check (handles array audiences)
        if !claims.aud.contains(&self.config.audience) {
            return Err(AuthError::ValidationFailed("Invalid audience".into()));
        }

        tracing::debug!(
            user_id = %claims.sub,
            email = ?claims.email,
            "Token validated successfully"
        );

        Ok(claims)
    }
}
```

## Auth Context for Tools

Make authentication available to tools. The auth context carries not just identity, but all the claims needed for Layer 2 (scope checking) and Layer 3 (passing to backend systems):

```rust
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub scopes: HashSet<String>,
}

impl AuthContext {
    pub fn from_claims(claims: &Claims) -> Self {
        let scopes = claims.scope
            .as_ref()
            .map(|s| s.split_whitespace().map(String::from).collect())
            .or_else(|| {
                claims.permissions.as_ref().map(|p| p.iter().cloned().collect())
            })
            .unwrap_or_default();

        Self {
            user_id: claims.sub.clone(),
            email: claims.email.clone(),
            name: claims.name.clone(),
            scopes,
        }
    }

    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.contains(scope) ||
        // Check wildcards: "write:*" matches "write:data"
        self.scopes.iter().any(|s| {
            s.ends_with(":*") && scope.starts_with(&s[..s.len()-1])
        })
    }

    pub fn require_scope(&self, scope: &str) -> Result<(), AuthError> {
        if self.has_scope(scope) {
            Ok(())
        } else {
            Err(AuthError::InsufficientScope)
        }
    }

    pub fn require_any_scope(&self, scopes: &[&str]) -> Result<(), AuthError> {
        if scopes.iter().any(|s| self.has_scope(s)) {
            Ok(())
        } else {
            Err(AuthError::InsufficientScope)
        }
    }
}
```

### Extended Auth Context for Backend Passthrough

For Layer 3 security, you often need to pass additional claims to backend systems. Extend the context with organization, team, and role information:

```rust
#[derive(Debug, Clone)]
pub struct AuthContext {
    // Identity (Layer 1)
    pub user_id: String,
    pub email: Option<String>,
    pub name: Option<String>,

    // Scopes for tool authorization (Layer 2)
    pub scopes: HashSet<String>,

    // Organization context for backend systems (Layer 3)
    pub org_id: Option<String>,
    pub team_id: Option<String>,
    pub roles: Vec<String>,
    pub groups: Vec<String>,

    // Raw claims for custom backend needs
    pub custom_claims: serde_json::Value,
}

impl AuthContext {
    /// Get the context as headers for HTTP backend calls
    pub fn as_headers(&self) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            ("X-User-ID", self.user_id.clone()),
        ];

        if let Some(ref org) = self.org_id {
            headers.push(("X-Org-ID", org.clone()));
        }
        if let Some(ref team) = self.team_id {
            headers.push(("X-Team-ID", team.clone()));
        }
        if let Some(ref email) = self.email {
            headers.push(("X-User-Email", email.clone()));
        }

        headers
    }

    /// Get context for database session variables (PostgreSQL RLS)
    pub fn as_db_session_vars(&self) -> Vec<(&'static str, String)> {
        let mut vars = vec![
            ("app.user_id", self.user_id.clone()),
        ];

        if let Some(ref org) = self.org_id {
            vars.push(("app.org_id", org.clone()));
        }
        if let Some(ref team) = self.team_id {
            vars.push(("app.team_id", team.clone()));
        }

        vars
    }
}
```

## Middleware Integration

Integrate validation into your HTTP server:

```rust
use axum::{
    middleware::{self, Next},
    extract::State,
    http::Request,
    body::Body,
};
use std::sync::Arc;

pub type SharedValidator = Arc<JwtValidator>;

pub async fn auth_middleware(
    State(validator): State<SharedValidator>,
    BearerToken(token): BearerToken,
    mut request: Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, AuthError> {
    // Validate the token
    let claims = validator.validate(&token).await?;

    // Create auth context
    let auth_context = AuthContext::from_claims(&claims);

    // Add to request extensions for handlers to access
    request.extensions_mut().insert(auth_context);

    Ok(next.run(request).await)
}

// Usage in router
pub fn create_router(validator: SharedValidator) -> Router {
    Router::new()
        .route("/mcp", post(mcp_handler))
        .layer(middleware::from_fn_with_state(
            validator.clone(),
            auth_middleware
        ))
        .with_state(validator)
}

// Access in handler
async fn mcp_handler(
    Extension(auth): Extension<AuthContext>,
    Json(request): Json<McpRequest>,
) -> impl IntoResponse {
    tracing::info!(user = %auth.user_id, "Processing MCP request");
    // ...
}
```

## Handling Key Rotation

IdPs rotate signing keys periodically. Handle this gracefully:

```rust
impl JwtValidator {
    pub async fn validate_with_retry(&self, token: &str) -> Result<Claims, AuthError> {
        match self.validate(token).await {
            Ok(claims) => Ok(claims),
            Err(AuthError::ValidationFailed(msg)) if msg.contains("Key not found") => {
                // Key might have rotated, refresh JWKS and retry
                tracing::info!("Key not found, refreshing JWKS");
                self.jwks_client.refresh().await?;
                self.validate(token).await
            }
            Err(e) => Err(e),
        }
    }
}
```

## Testing Validation

### Unit Tests with Mock Tokens

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn create_test_token(claims: &Claims, key: &str) -> String {
        let encoding_key = EncodingKey::from_secret(key.as_bytes());
        encode(
            &Header::new(Algorithm::HS256),
            claims,
            &encoding_key
        ).unwrap()
    }

    #[test]
    fn test_auth_context_scope_matching() {
        let context = AuthContext {
            user_id: "user123".into(),
            email: Some("user@example.com".into()),
            name: None,
            scopes: ["read:data", "write:*"].iter().map(|s| s.to_string()).collect(),
        };

        assert!(context.has_scope("read:data"));
        assert!(context.has_scope("write:data"));
        assert!(context.has_scope("write:users"));
        assert!(!context.has_scope("admin:users"));
    }

    #[test]
    fn test_bearer_token_extraction() {
        // Test valid header
        let valid = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        assert!(valid.starts_with("Bearer "));

        // Test invalid scheme
        let invalid = "Basic dXNlcjpwYXNz";
        assert!(!invalid.starts_with("Bearer "));
    }
}
```

### Integration Tests with Real IdP

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Run manually with: cargo test -- --ignored
    async fn test_cognito_validation() {
        let config = JwtValidatorConfig::cognito(
            "us-east-1",
            &std::env::var("COGNITO_USER_POOL_ID").unwrap(),
            &std::env::var("COGNITO_CLIENT_ID").unwrap(),
        );

        let validator = JwtValidator::new(config);

        // Get a real token from Cognito (e.g., via test user)
        let token = get_test_token().await;

        let claims = validator.validate(&token).await.unwrap();

        assert!(!claims.sub.is_empty());
        assert!(claims.email.is_some());
    }
}
```

## Error Responses

Return proper OAuth-style errors:

```rust
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error_code, description) = match &self {
            AuthError::MissingToken => (
                StatusCode::UNAUTHORIZED,
                "missing_token",
                "No authorization token provided"
            ),
            AuthError::InvalidHeader | AuthError::InvalidScheme => (
                StatusCode::UNAUTHORIZED,
                "invalid_request",
                "Invalid authorization header format"
            ),
            AuthError::ValidationFailed(msg) => (
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                msg.as_str()
            ),
            AuthError::InsufficientScope => (
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "Token does not have required scope"
            ),
        };

        // Add WWW-Authenticate header for 401 responses
        let mut response = (
            status,
            axum::Json(serde_json::json!({
                "error": error_code,
                "error_description": description
            }))
        ).into_response();

        if status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                header::WWW_AUTHENTICATE,
                format!("Bearer realm=\"mcp\", error=\"{}\"", error_code)
                    .parse()
                    .unwrap()
            );
        }

        response
    }
}
```

## Security Best Practices

### Clock Skew

Allow for clock differences between servers:

```rust
// In JwtValidatorConfig
pub leeway_seconds: u64,  // Typically 60 seconds

// In validation
validation.leeway = self.config.leeway_seconds;
```

### Algorithm Validation

Never accept the `alg` from the token without verification:

```rust
// GOOD: Explicitly allow specific algorithms
let mut validation = Validation::new(Algorithm::RS256);

// BAD: Would allow attacker to switch to "none"
// let validation = Validation::default();
```

### Audience Verification

Always verify the audience matches your server:

```rust
// The token might be valid but intended for a different service
if !claims.aud.contains(&self.config.audience) {
    return Err(AuthError::ValidationFailed("Invalid audience".into()));
}
```

## Passing Identity to Backend Systems

Now that you have the auth context, here's how to pass it to different backend systems for Layer 3 security:

### Database with Row-Level Security

```rust
impl QueryTool {
    pub async fn run(&self, input: QueryInput, context: &ToolContext) -> Result<QueryResult> {
        let auth = context.auth()?;
        auth.require_scope("read:data")?;  // Layer 2

        // Layer 3: Set session variables for PostgreSQL RLS
        let pool = &self.database;
        let conn = pool.acquire().await?;

        // Set user context that RLS policies will use
        for (key, value) in auth.as_db_session_vars() {
            sqlx::query(&format!("SET LOCAL {} = $1", key))
                .bind(&value)
                .execute(&mut *conn)
                .await?;
        }

        // Query executes with RLS automatically filtering rows
        let results = sqlx::query_as::<_, Record>(&input.sql)
            .fetch_all(&mut *conn)
            .await?;

        Ok(QueryResult { records: results })
    }
}
```

### Downstream API Calls

```rust
impl ApiTool {
    pub async fn run(&self, input: ApiInput, context: &ToolContext) -> Result<ApiResult> {
        let auth = context.auth()?;
        auth.require_scope("read:api")?;  // Layer 2

        // Layer 3: Forward identity headers to downstream API
        let mut request = self.client
            .get(&format!("{}/resource/{}", self.api_base, input.resource_id));

        for (name, value) in auth.as_headers() {
            request = request.header(name, value);
        }

        // Downstream API uses these headers for its own authorization
        let response = request.send().await?;

        if response.status() == StatusCode::FORBIDDEN {
            // Backend denied access - this is Layer 3 rejection
            return Err(McpError::forbidden(
                "You don't have access to this resource"
            ));
        }

        Ok(response.json().await?)
    }
}
```

### GraphQL with Field-Level Security

```rust
impl GraphQLTool {
    pub async fn run(&self, input: GraphQLInput, context: &ToolContext) -> Result<GraphQLResult> {
        let auth = context.auth()?;
        auth.require_scope("read:graphql")?;  // Layer 2

        // Layer 3: GraphQL server handles field-level authorization
        // using the identity we pass in the context
        let response = self.graphql_client
            .query(&input.query)
            .variables(input.variables)
            .header("X-User-ID", &auth.user_id)
            .header("X-User-Scopes", auth.scopes.iter().collect::<Vec<_>>().join(" "))
            .execute()
            .await?;

        // Fields the user can't access come back as null or are omitted
        // based on the GraphQL schema's @auth directives

        Ok(response)
    }
}
```

### The Security Division of Labor

```
┌─────────────────────────────────────────────────────────────────────┐
│              What Each System Is Responsible For                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  YOUR MCP SERVER                                                    │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  ✓ Validate JWT signature and claims (Layer 1)                │  │
│  │  ✓ Check scopes for each tool (Layer 2)                       │  │
│  │  ✓ Extract and forward user identity                          │  │
│  │  ✗ NOT: Per-row or per-field authorization                    │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  YOUR DATABASE                                                      │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  ✓ Row-Level Security policies                                │  │
│  │  ✓ Column-level permissions                                   │  │
│  │  ✓ Data filtering based on user context                       │  │
│  │  ✗ NOT: Token validation (trusts MCP server)                  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  YOUR API LAYER                                                     │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  ✓ Resource-level authorization                               │  │
│  │  ✓ Field masking (PII, sensitive data)                        │  │
│  │  ✓ Rate limiting per user/org                                 │  │
│  │  ✗ NOT: Token validation (trusts MCP server)                  │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                                                                     │
│  RESULT: Each system does what it's best at                         │
│  MCP validates identity → Backend enforces data policies            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Summary

### Token Validation Pipeline

1. **Extract token** - Parse Authorization header
2. **Decode header** - Get algorithm and key ID
3. **Fetch JWKS** - Cache public keys from IdP
4. **Verify signature** - Use correct algorithm and key
5. **Validate claims** - Check iss, aud, exp, nbf
6. **Extract context** - Make user info available to tools

### Multi-Layer Security Model

| Layer | What | Where | Your Responsibility |
|-------|------|-------|---------------------|
| **Layer 1** | Token validation | MCP Server | Implement (this chapter) |
| **Layer 2** | Tool authorization | MCP Server | Check scopes in tools |
| **Layer 3** | Data authorization | Backend systems | Pass identity, delegate to existing systems |

**The key insight:** You don't have to build all security in your MCP server. Validate the token (Layer 1), check scopes (Layer 2), then pass the authenticated identity to your databases and APIs (Layer 3). Let each system do what it's designed for.

### Common Pitfalls to Avoid

- Not caching JWKS (DoS risk)
- Not handling key rotation
- Accepting any algorithm
- Skipping audience verification
- Ignoring clock skew
- Trying to implement row-level security in MCP instead of the database

The next chapter covers integration with specific identity providers.

---

*Continue to [Identity Provider Integration](../ch14-providers.md) →*
