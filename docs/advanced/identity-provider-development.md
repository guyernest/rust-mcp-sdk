# Identity Provider Development Guide

This guide explains how to create custom identity provider plugins for PMCP SDK's OAuth/OIDC authentication system.

## Overview

PMCP SDK provides an extensible `IdentityProvider` trait that allows you to integrate with any OAuth/OIDC identity provider. The SDK includes built-in providers for:

- **GenericOidcProvider** - Works with any OIDC-compliant provider
- **CognitoProvider** - AWS Cognito with optimized claim mappings

Third-party providers can be developed as separate crates (e.g., `pmcp-auth-google`, `pmcp-auth-auth0`) or implemented directly in your application.

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                         PMCP SDK                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐│
│  │                    IdentityProvider Trait                           ││
│  │  (Plugin interface for external identity providers)                 ││
│  └─────────────────────────────────────────────────────────────────────┘│
│                              │                                          │
│         ┌────────────────────┼────────────────────┐                     │
│         │                    │                    │                     │
│         ▼                    ▼                    ▼                     │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐             │
│  │   Cognito   │      │ GenericOidc │      │   Custom    │             │
│  │  Provider   │      │  Provider   │      │  Provider   │             │
│  └─────────────┘      └─────────────┘      └─────────────┘             │
│                              │                                          │
│                    Works with any OIDC provider:                        │
│                    Google, Auth0, Azure AD, Okta...                     │
└─────────────────────────────────────────────────────────────────────────┘
```

## When to Create a Custom Provider

Use **GenericOidcProvider** when:
- Your provider is fully OIDC-compliant
- Standard claim mappings are sufficient
- You don't need provider-specific optimizations

Create a **custom provider** when:
- The provider has non-standard OIDC behavior
- You need custom claim extraction logic
- You want provider-specific optimizations (caching, endpoints)
- The provider doesn't support standard OIDC discovery

## The IdentityProvider Trait

```rust
use async_trait::async_trait;
use pmcp::server::auth::provider::{
    IdentityProvider, ProviderCapabilities, OidcDiscovery,
    AuthorizationParams, TokenExchangeParams, TokenResponse,
    DcrRequest, DcrResponse,
};
use pmcp::server::auth::AuthContext;
use pmcp::error::Result;

#[async_trait]
pub trait IdentityProvider: Send + Sync + Debug {
    // Required: Provider identity
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn issuer(&self) -> &str;

    // Required: Token validation
    async fn validate_token(&self, token: &str) -> Result<AuthContext>;

    // Required: OIDC discovery
    async fn discovery(&self) -> Result<OidcDiscovery>;
    async fn jwks(&self) -> Result<serde_json::Value>;

    // Optional: Authorization flow (with default implementations)
    async fn authorization_url(&self, params: AuthorizationParams) -> Result<String>;
    async fn exchange_code(&self, params: TokenExchangeParams) -> Result<TokenResponse>;
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse>;

    // Optional: Dynamic Client Registration
    async fn register_client(&self, request: DcrRequest) -> Result<DcrResponse>;

    // Optional: Token management
    async fn revoke_token(&self, token: &str) -> Result<()>;
    async fn introspect_token(&self, token: &str) -> Result<AuthContext>;

    // Optional: UserInfo
    async fn user_info(&self, access_token: &str) -> Result<serde_json::Value>;
}
```

## Step-by-Step Implementation

### Step 1: Define Your Provider Struct

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use pmcp::server::auth::provider::{
    IdentityProvider, ProviderCapabilities, OidcDiscovery,
    AuthorizationParams, TokenExchangeParams, TokenResponse,
};
use pmcp::server::auth::traits::{AuthContext, ClaimMappings};
use pmcp::error::{Error, Result};

/// Cached data with expiration.
struct CachedData<T> {
    data: T,
    fetched_at: Instant,
    ttl: Duration,
}

impl<T> CachedData<T> {
    fn new(data: T, ttl: Duration) -> Self {
        Self { data, fetched_at: Instant::now(), ttl }
    }

    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

/// My custom identity provider.
#[derive(Debug)]
pub struct MyProvider {
    /// Provider identifier.
    id: &'static str,
    /// Display name.
    display_name: &'static str,
    /// Issuer URL.
    issuer: String,
    /// Client ID.
    client_id: String,
    /// Client secret (optional for public clients).
    client_secret: Option<String>,
    /// Custom claim mappings.
    claim_mappings: ClaimMappings,
    /// Cached JWKS.
    jwks_cache: Arc<RwLock<Option<CachedData<HashMap<String, JwkKey>>>>>,
    /// Cached discovery document.
    discovery_cache: Arc<RwLock<Option<CachedData<OidcDiscovery>>>>,
    /// HTTP client.
    http_client: reqwest::Client,
    /// Cache TTL.
    cache_ttl: Duration,
}
```

### Step 2: Implement Constructor

```rust
impl MyProvider {
    /// Create a new provider instance.
    pub async fn new(
        issuer: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Result<Self> {
        let issuer = issuer.into();

        let provider = Self {
            id: "my-provider",
            display_name: "My Provider",
            issuer,
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::default(),
            jwks_cache: Arc::new(RwLock::new(None)),
            discovery_cache: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| Error::internal(format!("HTTP client error: {}", e)))?,
            cache_ttl: Duration::from_secs(3600),
        };

        // Pre-fetch discovery and JWKS on startup (recommended)
        provider.fetch_discovery().await?;
        provider.refresh_jwks().await?;

        Ok(provider)
    }

    /// Set client secret for confidential clients.
    pub fn with_client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// Set custom claim mappings.
    pub fn with_claim_mappings(mut self, mappings: ClaimMappings) -> Self {
        self.claim_mappings = mappings;
        self
    }
}
```

### Step 3: Implement JWKS Caching

```rust
impl MyProvider {
    /// Fetch and cache the OIDC discovery document.
    async fn fetch_discovery(&self) -> Result<OidcDiscovery> {
        // Check cache first
        {
            let cache = self.discovery_cache.read().await;
            if let Some(ref cached) = *cache {
                if !cached.is_expired() {
                    return Ok(cached.data.clone());
                }
            }
        }

        // Fetch from provider
        let discovery_url = format!("{}/.well-known/openid-configuration", self.issuer);
        let response = self.http_client
            .get(&discovery_url)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Discovery fetch failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Discovery endpoint returned {}",
                response.status()
            )));
        }

        let discovery: OidcDiscovery = response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Discovery parse failed: {}", e)))?;

        // Update cache
        {
            let mut cache = self.discovery_cache.write().await;
            *cache = Some(CachedData::new(discovery.clone(), self.cache_ttl));
        }

        Ok(discovery)
    }

    /// Refresh the JWKS cache.
    async fn refresh_jwks(&self) -> Result<()> {
        let discovery = self.fetch_discovery().await?;

        let response = self.http_client
            .get(&discovery.jwks_uri)
            .send()
            .await
            .map_err(|e| Error::internal(format!("JWKS fetch failed: {}", e)))?;

        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| Error::internal(format!("JWKS parse failed: {}", e)))?;

        // Index keys by kid for O(1) lookup
        let keys_map: HashMap<String, JwkKey> = jwks
            .keys
            .into_iter()
            .map(|k| (k.kid.clone(), k))
            .collect();

        let mut cache = self.jwks_cache.write().await;
        *cache = Some(CachedData::new(keys_map, self.cache_ttl));

        Ok(())
    }

    /// Get a key by ID, refreshing cache if key not found.
    async fn get_key(&self, kid: &str) -> Result<JwkKey> {
        // Try cache first
        {
            let cache = self.jwks_cache.read().await;
            if let Some(ref cached) = *cache {
                if let Some(key) = cached.data.get(kid) {
                    return Ok(key.clone());
                }
            }
        }

        // Key not found, refresh and retry (key rotation)
        self.refresh_jwks().await?;

        let cache = self.jwks_cache.read().await;
        cache
            .as_ref()
            .and_then(|c| c.data.get(kid))
            .cloned()
            .ok_or_else(|| Error::auth(format!("Unknown key ID: {}", kid)))
    }
}
```

### Step 4: Implement Token Validation

```rust
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rsa::{pkcs1::DecodeRsaPublicKey, Pkcs1v15Sign, RsaPublicKey};
use sha2::{Digest, Sha256};

impl MyProvider {
    /// Validate a JWT token.
    async fn validate_jwt(&self, token: &str) -> Result<AuthContext> {
        // Split token into parts
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(Error::auth("Invalid JWT format"));
        }

        let (header_b64, payload_b64, signature_b64) = (parts[0], parts[1], parts[2]);

        // Decode and parse header
        let header_json = URL_SAFE_NO_PAD
            .decode(header_b64)
            .map_err(|_| Error::auth("Invalid JWT header encoding"))?;
        let header: serde_json::Value = serde_json::from_slice(&header_json)
            .map_err(|_| Error::auth("Invalid JWT header"))?;

        // Get key ID from header
        let kid = header["kid"]
            .as_str()
            .ok_or_else(|| Error::auth("Missing kid in JWT header"))?;

        // Get the signing key
        let key = self.get_key(kid).await?;

        // Verify signature
        self.verify_signature(header_b64, payload_b64, signature_b64, &key)?;

        // Decode and parse payload
        let payload_json = URL_SAFE_NO_PAD
            .decode(payload_b64)
            .map_err(|_| Error::auth("Invalid JWT payload encoding"))?;
        let claims: HashMap<String, serde_json::Value> = serde_json::from_slice(&payload_json)
            .map_err(|_| Error::auth("Invalid JWT payload"))?;

        // Validate standard claims
        self.validate_claims(&claims)?;

        // Extract AuthContext using claim mappings
        self.extract_auth_context(&claims)
    }

    fn verify_signature(
        &self,
        header_b64: &str,
        payload_b64: &str,
        signature_b64: &str,
        key: &JwkKey,
    ) -> Result<()> {
        // Decode RSA components
        let n_bytes = URL_SAFE_NO_PAD
            .decode(&key.n)
            .map_err(|_| Error::auth("Invalid key modulus"))?;
        let e_bytes = URL_SAFE_NO_PAD
            .decode(&key.e)
            .map_err(|_| Error::auth("Invalid key exponent"))?;

        // Construct RSA public key
        let public_key = RsaPublicKey::new(
            rsa::BigUint::from_bytes_be(&n_bytes),
            rsa::BigUint::from_bytes_be(&e_bytes),
        )
        .map_err(|_| Error::auth("Invalid RSA key"))?;

        // Calculate message hash
        let message = format!("{}.{}", header_b64, payload_b64);
        let mut hasher = Sha256::new();
        hasher.update(message.as_bytes());
        let hash = hasher.finalize();

        // Decode signature
        let signature = URL_SAFE_NO_PAD
            .decode(signature_b64)
            .map_err(|_| Error::auth("Invalid signature encoding"))?;

        // Verify
        public_key
            .verify(Pkcs1v15Sign::new::<Sha256>(), &hash, &signature)
            .map_err(|_| Error::auth("Invalid token signature"))
    }

    fn validate_claims(&self, claims: &HashMap<String, serde_json::Value>) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check expiration
        if let Some(exp) = claims.get("exp").and_then(|v| v.as_u64()) {
            if now > exp + 60 {  // 60 second leeway
                return Err(Error::auth("Token expired"));
            }
        }

        // Check not-before
        if let Some(nbf) = claims.get("nbf").and_then(|v| v.as_u64()) {
            if now < nbf.saturating_sub(60) {
                return Err(Error::auth("Token not yet valid"));
            }
        }

        // Check issuer
        if let Some(iss) = claims.get("iss").and_then(|v| v.as_str()) {
            if iss != self.issuer {
                return Err(Error::auth(format!("Invalid issuer: {}", iss)));
            }
        }

        // Check audience (if client_id is set)
        if let Some(aud) = claims.get("aud") {
            let valid_aud = match aud {
                serde_json::Value::String(s) => s == &self.client_id,
                serde_json::Value::Array(arr) => {
                    arr.iter().any(|v| v.as_str() == Some(&self.client_id))
                }
                _ => false,
            };
            if !valid_aud {
                return Err(Error::auth("Invalid audience"));
            }
        }

        Ok(())
    }

    fn extract_auth_context(
        &self,
        claims: &HashMap<String, serde_json::Value>,
    ) -> Result<AuthContext> {
        // Normalize claims using mappings
        let normalized = self.claim_mappings.normalize(claims);

        // Extract user ID (required)
        let user_id = normalized
            .get("sub")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::auth("Missing subject claim"))?
            .to_string();

        let mut auth = AuthContext::new(user_id);
        auth.authenticated = true;
        auth.provider = Some(self.id.to_string());

        // Extract optional claims
        if let Some(email) = normalized.get("email").and_then(|v| v.as_str()) {
            auth.email = Some(email.to_string());
        }

        if let Some(name) = normalized.get("name").and_then(|v| v.as_str()) {
            auth.name = Some(name.to_string());
        }

        // Extract scopes
        if let Some(scope) = normalized.get("scope").and_then(|v| v.as_str()) {
            auth.scopes = scope.split_whitespace().map(String::from).collect();
        }

        // Extract groups/roles
        if normalized.contains_key("groups") {
            if let Some(groups) = normalized.get("groups").and_then(|v| v.as_array()) {
                auth.groups = groups
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect();
            }
        }

        Ok(auth)
    }
}
```

### Step 5: Implement the Trait

```rust
use async_trait::async_trait;

#[async_trait]
impl IdentityProvider for MyProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            oidc: true,
            dcr: false,  // Set true if your provider supports DCR
            pkce: true,
            refresh_tokens: true,
            revocation: true,
            introspection: false,
            custom_scopes: true,
            device_flow: false,
        }
    }

    fn issuer(&self) -> &str {
        &self.issuer
    }

    async fn validate_token(&self, token: &str) -> Result<AuthContext> {
        self.validate_jwt(token).await
    }

    async fn discovery(&self) -> Result<OidcDiscovery> {
        self.fetch_discovery().await
    }

    async fn jwks(&self) -> Result<serde_json::Value> {
        let cache = self.jwks_cache.read().await;
        if let Some(ref cached) = *cache {
            return Ok(serde_json::to_value(&cached.data)
                .map_err(|e| Error::internal(format!("JWKS serialization failed: {}", e)))?);
        }
        Err(Error::internal("JWKS not cached"))
    }

    async fn authorization_url(&self, params: AuthorizationParams) -> Result<String> {
        let discovery = self.fetch_discovery().await?;

        let mut url = url::Url::parse(&discovery.authorization_endpoint)
            .map_err(|e| Error::internal(format!("Invalid auth endpoint: {}", e)))?;

        {
            let mut query = url.query_pairs_mut();
            query.append_pair("client_id", &self.client_id);
            query.append_pair("redirect_uri", &params.redirect_uri);
            query.append_pair("response_type", "code");
            query.append_pair("scope", &params.scopes.join(" "));
            query.append_pair("state", &params.state);

            if let Some(ref nonce) = params.nonce {
                query.append_pair("nonce", nonce);
            }

            if let Some(ref challenge) = params.code_challenge {
                query.append_pair("code_challenge", challenge);
                query.append_pair(
                    "code_challenge_method",
                    params.code_challenge_method.as_deref().unwrap_or("S256"),
                );
            }

            for (key, value) in &params.extra {
                query.append_pair(key, value);
            }
        }

        Ok(url.to_string())
    }

    async fn exchange_code(&self, params: TokenExchangeParams) -> Result<TokenResponse> {
        let discovery = self.fetch_discovery().await?;

        let mut form = HashMap::new();
        form.insert("grant_type", "authorization_code".to_string());
        form.insert("code", params.code);
        form.insert("redirect_uri", params.redirect_uri);
        form.insert("client_id", self.client_id.clone());

        if let Some(secret) = &self.client_secret {
            form.insert("client_secret", secret.clone());
        }

        if let Some(verifier) = params.code_verifier {
            form.insert("code_verifier", verifier);
        }

        let response = self.http_client
            .post(&discovery.token_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token exchange failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::auth(format!("Token exchange failed: {}", error_text)));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Token response parse failed: {}", e)))
    }

    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        let discovery = self.fetch_discovery().await?;

        let mut form = HashMap::new();
        form.insert("grant_type", "refresh_token");
        form.insert("refresh_token", refresh_token);
        form.insert("client_id", &self.client_id);

        if let Some(ref secret) = self.client_secret {
            form.insert("client_secret", secret);
        }

        let response = self.http_client
            .post(&discovery.token_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token refresh failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::auth("Token refresh failed"));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Refresh response parse failed: {}", e)))
    }

    async fn revoke_token(&self, token: &str) -> Result<()> {
        let discovery = self.fetch_discovery().await?;

        let Some(revocation_endpoint) = discovery.revocation_endpoint else {
            // Provider doesn't support revocation
            return Ok(());
        };

        let mut form = HashMap::new();
        form.insert("token", token);
        form.insert("client_id", &self.client_id);

        if let Some(ref secret) = self.client_secret {
            form.insert("client_secret", secret);
        }

        self.http_client
            .post(&revocation_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token revocation failed: {}", e)))?;

        Ok(())
    }

    async fn user_info(&self, access_token: &str) -> Result<serde_json::Value> {
        let discovery = self.fetch_discovery().await?;

        let Some(userinfo_endpoint) = discovery.userinfo_endpoint else {
            return Err(Error::protocol(
                pmcp::error::ErrorCode::INVALID_REQUEST,
                "Provider doesn't support UserInfo endpoint",
            ));
        };

        let response = self.http_client
            .get(&userinfo_endpoint)
            .header("Authorization", format!("Bearer {}", access_token))
            .send()
            .await
            .map_err(|e| Error::internal(format!("UserInfo fetch failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::auth("UserInfo request failed"));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("UserInfo parse failed: {}", e)))
    }
}
```

## Custom Claim Mappings

Different providers use different claim names. Use `ClaimMappings` to normalize them:

```rust
use pmcp::server::auth::traits::ClaimMappings;

// Create custom mappings for your provider
let mappings = ClaimMappings::new()
    .map_claim("sub", "user_id")           // Map user_id -> sub
    .map_claim("email", "email_address")   // Map email_address -> email
    .map_claim("name", "display_name")     // Map display_name -> name
    .map_claim("groups", "roles");         // Map roles -> groups

let provider = MyProvider::new("https://my-idp.example.com", "client-id")
    .await?
    .with_claim_mappings(mappings);
```

### Built-in Mappings

The SDK provides pre-configured mappings for common providers:

```rust
// For Google
ClaimMappings::google()

// For Auth0
ClaimMappings::auth0()

// For Okta
ClaimMappings::okta()

// For Microsoft Entra ID (Azure AD)
ClaimMappings::entra()

// For AWS Cognito
ClaimMappings::cognito()
```

## Using GenericOidcProvider

For most OIDC-compliant providers, use `GenericOidcProvider` instead of building from scratch:

```rust
use pmcp::server::auth::providers::{GenericOidcConfig, GenericOidcProvider};

// Google
let google = GenericOidcProvider::new(
    GenericOidcConfig::google("your-google-client-id")
).await?;

// Auth0
let auth0 = GenericOidcProvider::new(
    GenericOidcConfig::auth0("your-tenant.auth0.com", "your-client-id")
        .with_client_secret("your-client-secret")
).await?;

// Okta
let okta = GenericOidcProvider::new(
    GenericOidcConfig::okta("your-domain.okta.com", "your-client-id")
).await?;

// Microsoft Entra ID
let entra = GenericOidcProvider::new(
    GenericOidcConfig::entra("your-tenant-id", "your-client-id")
        .with_client_secret("your-client-secret")
).await?;

// Custom provider
let custom = GenericOidcProvider::new(
    GenericOidcConfig::new(
        "my-provider",
        "My Custom Provider",
        "https://idp.example.com",
        "client-id",
    )
    .with_client_secret("client-secret")
    .with_claim_mappings(ClaimMappings::new()
        .map_claim("groups", "custom_groups_claim"))
).await?;
```

## Using CognitoProvider

For AWS Cognito, use the specialized `CognitoProvider`:

```rust
use pmcp::server::auth::providers::CognitoProvider;

let cognito = CognitoProvider::new(
    "us-east-1",           // AWS region
    "us-east-1_xxxxx",     // User pool ID
    "your-client-id",      // App client ID
).await?;

// Validate a token
let auth_context = cognito.validate_token("eyJ...").await?;
println!("User: {}", auth_context.user_id());
println!("Email: {:?}", auth_context.email);
println!("Groups: {:?}", auth_context.groups);
```

## Registering Providers

Use `ProviderRegistry` to manage multiple providers:

```rust
use pmcp::server::auth::provider::ProviderRegistry;

let mut registry = ProviderRegistry::new();

// Register providers
registry.register(google);
registry.register(auth0);
registry.register(cognito);

// Look up by ID
if let Some(provider) = registry.get("google") {
    let auth = provider.validate_token(token).await?;
}

// List all providers
for id in registry.list() {
    println!("Provider: {}", id);
}
```

## Testing Your Provider

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_identity() {
        let provider = MyProvider::new("https://test.example.com", "test-client")
            .await
            .unwrap();

        assert_eq!(provider.id(), "my-provider");
        assert_eq!(provider.display_name(), "My Provider");
        assert!(provider.capabilities().oidc);
    }

    #[tokio::test]
    async fn test_valid_token() {
        let provider = create_test_provider().await;

        // Use a test JWT (generate with known keys)
        let token = create_test_jwt(&provider);

        let auth = provider.validate_token(&token).await.unwrap();
        assert_eq!(auth.user_id(), "test-user");
        assert!(auth.authenticated);
    }

    #[tokio::test]
    async fn test_expired_token() {
        let provider = create_test_provider().await;

        // Create a JWT with exp in the past
        let expired_token = create_expired_jwt(&provider);

        let result = provider.validate_token(&expired_token).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expired"));
    }

    #[tokio::test]
    async fn test_invalid_signature() {
        let provider = create_test_provider().await;

        // Tamper with a valid token
        let tampered = "eyJ...tampered...";

        let result = provider.validate_token(tampered).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_claim_extraction() {
        let provider = create_test_provider().await;

        let token = create_test_jwt_with_claims(
            &provider,
            &[
                ("sub", "user-123"),
                ("email", "test@example.com"),
                ("name", "Test User"),
            ],
        );

        let auth = provider.validate_token(&token).await.unwrap();
        assert_eq!(auth.user_id(), "user-123");
        assert_eq!(auth.email, Some("test@example.com".to_string()));
        assert_eq!(auth.name, Some("Test User".to_string()));
    }
}
```

### Integration Tests

```rust
#[tokio::test]
#[ignore = "requires real provider credentials"]
async fn test_real_provider_integration() {
    let client_id = std::env::var("PROVIDER_CLIENT_ID").unwrap();
    let issuer = std::env::var("PROVIDER_ISSUER").unwrap();

    let provider = MyProvider::new(issuer, client_id).await.unwrap();

    // Test discovery
    let discovery = provider.discovery().await.unwrap();
    assert!(!discovery.authorization_endpoint.is_empty());

    // Test JWKS
    let jwks = provider.jwks().await.unwrap();
    assert!(jwks.get("keys").is_some());
}
```

## Best Practices

### 1. Cache Aggressively

- Cache OIDC discovery documents (TTL: 24 hours typically)
- Cache JWKS (TTL: 1-24 hours)
- Refresh JWKS when unknown `kid` is encountered (key rotation)

### 2. Handle Key Rotation

```rust
async fn get_key(&self, kid: &str) -> Result<JwkKey> {
    // Try cache
    if let Some(key) = self.try_get_cached_key(kid).await {
        return Ok(key);
    }

    // Key not found - might be rotated, refresh
    self.refresh_jwks().await?;

    // Try again
    self.try_get_cached_key(kid)
        .await
        .ok_or_else(|| Error::auth("Unknown key ID"))
}
```

### 3. Use Proper Error Types

```rust
// Authentication errors (invalid tokens, wrong credentials)
Error::auth("Token expired")

// Protocol errors (missing required fields)
Error::protocol(ErrorCode::INVALID_REQUEST, "Missing redirect_uri")

// Internal errors (network failures, parsing errors)
Error::internal("Failed to fetch JWKS")
```

### 4. Support Clock Skew

```rust
let leeway_seconds = 60;  // Allow 60 seconds of clock skew

if now > exp + leeway_seconds {
    return Err(Error::auth("Token expired"));
}

if now < nbf - leeway_seconds {
    return Err(Error::auth("Token not yet valid"));
}
```

### 5. Validate All Required Claims

- `exp` - Expiration time
- `iss` - Issuer (must match expected)
- `aud` - Audience (must include your client ID)
- `sub` - Subject (user identifier)

### 6. Use PKCE for Public Clients

```rust
let params = AuthorizationParams::new(redirect_uri, state)
    .with_scopes(["openid", "email", "profile"])
    .with_pkce(code_challenge, "S256");

let auth_url = provider.authorization_url(params).await?;
```

## Publishing Your Provider

If creating a reusable crate:

1. Name it `pmcp-auth-{provider}` (e.g., `pmcp-auth-google`)
2. Add PMCP SDK as a dependency
3. Export your provider and configuration types
4. Include comprehensive documentation and examples
5. Add integration tests (can be `#[ignore]`)

```toml
# Cargo.toml
[package]
name = "pmcp-auth-myprovider"
version = "0.1.0"
description = "MyProvider identity provider for PMCP SDK"

[dependencies]
pmcp = "0.3"
async-trait = "0.1"
tokio = { version = "1.0", features = ["sync"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Reference Implementations

See these built-in providers as reference implementations:

- `src/server/auth/providers/generic_oidc.rs` - Generic OIDC provider
- `src/server/auth/providers/cognito.rs` - AWS Cognito provider
