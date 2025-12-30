# Token Validation

This chapter covers the practical implementation of JWT token validation in Rust MCP servers. Proper validation is critical for security.

## The Validation Pipeline

```
┌─────────────────────────────────────────────────────────────────────┐
│                    JWT Validation Pipeline                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Incoming Request                                                   │
│       │                                                             │
│       ▼                                                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ 1. EXTRACT TOKEN                                             │   │
│  │    Authorization: Bearer eyJhbGciOiJS...                     │   │
│  └─────────────────────────────┬───────────────────────────────┘   │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ 2. DECODE HEADER (without verification)                      │   │
│  │    { "alg": "RS256", "kid": "key-123" }                     │   │
│  └─────────────────────────────┬───────────────────────────────┘   │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ 3. FETCH PUBLIC KEY (from JWKS, cached)                      │   │
│  │    Match key by "kid" from header                            │   │
│  └─────────────────────────────┬───────────────────────────────┘   │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ 4. VERIFY SIGNATURE                                          │   │
│  │    RSA/ECDSA verification using public key                   │   │
│  └─────────────────────────────┬───────────────────────────────┘   │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ 5. VALIDATE CLAIMS                                           │   │
│  │    • exp (expiration)                                        │   │
│  │    • nbf (not before)                                        │   │
│  │    • iss (issuer)                                            │   │
│  │    • aud (audience)                                          │   │
│  └─────────────────────────────┬───────────────────────────────┘   │
│                                │                                    │
│                                ▼                                    │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │ 6. EXTRACT USER INFO                                         │   │
│  │    sub, email, scopes → AuthContext                          │   │
│  └─────────────────────────────────────────────────────────────┘   │
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

Make authentication available to tools:

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

## Summary

Token validation requires:

1. **Extract token** - Parse Authorization header
2. **Decode header** - Get algorithm and key ID
3. **Fetch JWKS** - Cache public keys from IdP
4. **Verify signature** - Use correct algorithm and key
5. **Validate claims** - Check iss, aud, exp, nbf
6. **Extract context** - Make user info available to tools

Common pitfalls to avoid:

- Not caching JWKS (DoS risk)
- Not handling key rotation
- Accepting any algorithm
- Skipping audience verification
- Ignoring clock skew

The next chapter covers integration with specific identity providers.

---

*Continue to [Identity Provider Integration](../ch14-providers.md) →*
