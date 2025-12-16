//! Proxy authentication provider that delegates to upstream OAuth servers.

use super::traits::{AuthContext, AuthProvider, TokenValidator};
use crate::error::{Error, ErrorCode, Result};
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Token validation function type.
pub type TokenValidatorFn =
    Box<dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<AuthContext>> + Send>> + Send + Sync>;

/// Proxy provider configuration.
#[derive(Clone, Debug)]
pub struct ProxyProviderConfig {
    /// Upstream OAuth server URL for token validation.
    pub upstream_url: String,

    /// Optional introspection endpoint (defaults to `{upstream_url}/introspect`).
    pub introspection_endpoint: Option<String>,

    /// Client ID for introspection requests.
    pub client_id: Option<String>,

    /// Client secret for introspection requests.
    pub client_secret: Option<String>,

    /// Whether to cache token validation results.
    pub enable_cache: bool,

    /// Cache TTL in seconds (default 300).
    pub cache_ttl: u64,
}

impl Default for ProxyProviderConfig {
    fn default() -> Self {
        Self {
            upstream_url: String::new(),
            introspection_endpoint: None,
            client_id: None,
            client_secret: None,
            enable_cache: true,
            cache_ttl: 300,
        }
    }
}

/// Proxy authentication provider that delegates to an upstream OAuth server.
/// This is similar to the TypeScript SDK's `ProxyProvider` pattern.
pub struct ProxyProvider {
    config: ProxyProviderConfig,
    token_validator: Option<TokenValidatorFn>,
    validator: Option<Arc<dyn TokenValidator>>,
}

impl std::fmt::Debug for ProxyProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyProvider")
            .field("config", &self.config)
            .field("token_validator", &self.token_validator.is_some())
            .field("validator", &self.validator.is_some())
            .finish()
    }
}

impl ProxyProvider {
    /// Create a new proxy provider with the given configuration.
    pub fn new(config: ProxyProviderConfig) -> Self {
        Self {
            config,
            token_validator: None,
            validator: None,
        }
    }

    /// Create a proxy provider with just an upstream URL.
    pub fn with_upstream(upstream_url: impl Into<String>) -> Self {
        Self::new(ProxyProviderConfig {
            upstream_url: upstream_url.into(),
            ..Default::default()
        })
    }

    /// Set a custom token validation function.
    /// This allows applications to implement custom validation logic.
    pub fn with_validator_fn<F, Fut>(mut self, validator: F) -> Self
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AuthContext>> + Send + 'static,
    {
        self.token_validator = Some(Box::new(move |token| Box::pin(validator(token))));
        self
    }

    /// Set a token validator implementation.
    pub fn with_validator(mut self, validator: Arc<dyn TokenValidator>) -> Self {
        self.validator = Some(validator);
        self
    }

    /// Set the introspection endpoint.
    pub fn introspection_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.config.introspection_endpoint = Some(endpoint.into());
        self
    }

    /// Set client credentials for introspection.
    pub fn client_credentials(
        mut self,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        self.config.client_id = Some(client_id.into());
        self.config.client_secret = Some(client_secret.into());
        self
    }

    /// Enable or disable token caching.
    pub fn cache(mut self, enable: bool) -> Self {
        self.config.enable_cache = enable;
        self
    }

    /// Extract bearer token from authorization header.
    fn extract_bearer_token(authorization_header: Option<&str>) -> Option<String> {
        authorization_header?
            .strip_prefix("Bearer ")
            .map(|s| s.to_string())
    }

    /// Validate token using the configured method.
    async fn validate_token_internal(&self, token: String) -> Result<AuthContext> {
        // Use custom validator function if provided
        if let Some(ref validator_fn) = self.token_validator {
            return validator_fn(token).await;
        }

        // Use validator implementation if provided
        if let Some(ref validator) = self.validator {
            return validator.validate(&token).await;
        }

        // Fall back to introspection endpoint
        self.introspect_token(token).await
    }

    /// Introspect token using the upstream server.
    async fn introspect_token(&self, _token: String) -> Result<AuthContext> {
        // This would make an HTTP request to the introspection endpoint
        // For now, return a placeholder implementation
        // Real implementation would use reqwest or similar HTTP client
        //
        // The implementation would:
        // 1. POST to introspection_endpoint with token
        // 2. Include client credentials if configured
        // 3. Parse the introspection response
        // 4. Convert to AuthContext

        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Token introspection not yet implemented. Please provide a custom validator.",
        ))
    }
}

#[async_trait]
impl AuthProvider for ProxyProvider {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>> {
        // Extract bearer token from Authorization header
        let Some(token) = Self::extract_bearer_token(authorization_header) else {
            return Ok(None); // No auth provided
        };

        // Validate the token
        match self.validate_token_internal(token).await {
            Ok(auth_context) => {
                // Check if token is expired
                if auth_context.is_expired() {
                    return Err(Error::protocol(ErrorCode::INVALID_REQUEST, "Token expired"));
                }
                Ok(Some(auth_context))
            },
            Err(e) => Err(e),
        }
    }

    fn auth_scheme(&self) -> &'static str {
        "Bearer"
    }
}

#[async_trait]
impl TokenValidator for ProxyProvider {
    async fn validate(&self, token: &str) -> Result<AuthContext> {
        self.validate_token_internal(token.to_string()).await
    }
}

/// No-op authentication provider for development/testing.
#[derive(Debug, Clone)]
pub struct NoOpAuthProvider;

#[async_trait]
impl AuthProvider for NoOpAuthProvider {
    async fn validate_request(
        &self,
        _authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>> {
        // Always return a valid auth context for development with all common scopes
        Ok(Some(AuthContext {
            subject: "dev-user".to_string(),
            scopes: vec![
                "read".to_string(),
                "write".to_string(),
                "admin".to_string(),
                "mcp:tools:use".to_string(),
            ],
            claims: Default::default(),
            token: None,
            client_id: Some("dev-client".to_string()),
            expires_at: None,
            authenticated: true,
        }))
    }

    fn is_required(&self) -> bool {
        false // Auth not required in dev mode
    }
}

/// Optional authentication provider that makes auth optional.
#[derive(Debug)]
pub struct OptionalAuthProvider<P: AuthProvider> {
    inner: P,
}

impl<P: AuthProvider> OptionalAuthProvider<P> {
    /// Wrap an auth provider to make authentication optional.
    pub fn new(provider: P) -> Self {
        Self { inner: provider }
    }
}

#[async_trait]
impl<P: AuthProvider> AuthProvider for OptionalAuthProvider<P> {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>> {
        // Try to validate, but don't fail if no auth is provided
        self.inner.validate_request(authorization_header).await
    }

    fn auth_scheme(&self) -> &'static str {
        self.inner.auth_scheme()
    }

    fn is_required(&self) -> bool {
        false // Make auth optional
    }
}
