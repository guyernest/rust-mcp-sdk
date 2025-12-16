//! Generic OIDC identity provider.
//!
//! This module provides a generic OIDC provider implementation that works with
//! any OIDC-compliant identity provider (Google, Auth0, Okta, Azure AD, etc.).
//! JWT validation is delegated to [`JwtValidator`] for code reuse and shared JWKS caching.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::{Error, ErrorCode, Result};
#[cfg(feature = "jwt-auth")]
use crate::server::auth::jwt_validator::{JwtValidator, ValidationConfig};
use crate::server::auth::provider::{
    AuthorizationParams, DcrRequest, DcrResponse, IdentityProvider, OidcDiscovery,
    ProviderCapabilities, TokenExchangeParams, TokenResponse,
};
use crate::server::auth::traits::{AuthContext, ClaimMappings};

/// Cached data with expiration.
struct CachedData<T: std::fmt::Debug> {
    data: T,
    fetched_at: Instant,
    ttl: Duration,
}

impl<T: std::fmt::Debug> std::fmt::Debug for CachedData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedData")
            .field("data", &self.data)
            .field("fetched_at", &self.fetched_at)
            .field("ttl", &self.ttl)
            .finish()
    }
}

impl<T: std::fmt::Debug> CachedData<T> {
    fn new(data: T, ttl: Duration) -> Self {
        Self {
            data,
            fetched_at: Instant::now(),
            ttl,
        }
    }

    fn is_expired(&self) -> bool {
        self.fetched_at.elapsed() > self.ttl
    }
}

/// Configuration for creating a generic OIDC provider.
#[derive(Debug, Clone)]
pub struct GenericOidcConfig {
    /// Unique identifier for this provider.
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// OIDC issuer URL.
    pub issuer: String,
    /// Client ID.
    pub client_id: String,
    /// Client secret (for confidential clients).
    pub client_secret: Option<String>,
    /// Custom claim mappings.
    pub claim_mappings: ClaimMappings,
    /// Cache TTL in seconds.
    pub cache_ttl: Duration,
    /// Clock skew leeway in seconds.
    pub leeway_seconds: u64,
}

impl GenericOidcConfig {
    /// Create a new configuration with required fields.
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        issuer: impl Into<String>,
        client_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            issuer: issuer.into(),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::default(),
            cache_ttl: Duration::from_secs(3600),
            leeway_seconds: 60,
        }
    }

    /// Set client secret (for confidential clients).
    pub fn with_client_secret(mut self, secret: impl Into<String>) -> Self {
        self.client_secret = Some(secret.into());
        self
    }

    /// Set custom claim mappings.
    pub fn with_claim_mappings(mut self, mappings: ClaimMappings) -> Self {
        self.claim_mappings = mappings;
        self
    }

    /// Create configuration for Google Identity.
    pub fn google(client_id: impl Into<String>) -> Self {
        Self {
            id: "google".to_string(),
            display_name: "Google Identity".to_string(),
            issuer: "https://accounts.google.com".to_string(),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::google(),
            cache_ttl: Duration::from_secs(3600),
            leeway_seconds: 60,
        }
    }

    /// Create configuration for Auth0.
    pub fn auth0(domain: impl Into<String>, client_id: impl Into<String>) -> Self {
        let domain = domain.into();
        Self {
            id: "auth0".to_string(),
            display_name: "Auth0".to_string(),
            issuer: format!("https://{}/", domain),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::auth0(),
            cache_ttl: Duration::from_secs(3600),
            leeway_seconds: 60,
        }
    }

    /// Create configuration for Okta.
    pub fn okta(domain: impl Into<String>, client_id: impl Into<String>) -> Self {
        let domain = domain.into();
        Self {
            id: "okta".to_string(),
            display_name: "Okta".to_string(),
            issuer: format!("https://{}", domain),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::okta(),
            cache_ttl: Duration::from_secs(3600),
            leeway_seconds: 60,
        }
    }

    /// Create configuration for Microsoft Entra ID (Azure AD).
    pub fn entra(tenant_id: impl Into<String>, client_id: impl Into<String>) -> Self {
        let tenant_id = tenant_id.into();
        Self {
            id: "entra".to_string(),
            display_name: "Microsoft Entra ID".to_string(),
            issuer: format!("https://login.microsoftonline.com/{}/v2.0", tenant_id),
            client_id: client_id.into(),
            client_secret: None,
            claim_mappings: ClaimMappings::entra(),
            cache_ttl: Duration::from_secs(3600),
            leeway_seconds: 60,
        }
    }
}

/// Type alias for discovery cache.
#[cfg(not(target_arch = "wasm32"))]
type DiscoveryCache = Arc<RwLock<Option<CachedData<OidcDiscovery>>>>;

/// Generic OIDC identity provider.
///
/// Works with any OIDC-compliant identity provider by auto-discovering
/// endpoints from the OIDC discovery document. JWT validation is delegated
/// to [`JwtValidator`] for efficient shared JWKS caching.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::auth::providers::{GenericOidcProvider, GenericOidcConfig};
///
/// // Create provider for Google
/// let config = GenericOidcConfig::google("your-client-id");
/// let google = GenericOidcProvider::new(config).await?;
///
/// // Or create a custom provider
/// let custom_config = GenericOidcConfig::new(
///     "my-provider",
///     "My Identity Provider",
///     "https://auth.example.com",
///     "my-client-id",
/// );
/// let provider = GenericOidcProvider::new(custom_config).await?;
/// ```
pub struct GenericOidcProvider {
    /// Provider configuration.
    config: GenericOidcConfig,
    /// Provider ID (leaked string for 'static lifetime).
    id: &'static str,
    /// Display name (leaked string for 'static lifetime).
    display_name: &'static str,
    /// JWT validator with shared JWKS cache.
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    jwt_validator: JwtValidator,
    /// Validation config (built after discovery to get JWKS URI).
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    validation_config: ValidationConfig,
    /// Cached discovery document.
    #[cfg(not(target_arch = "wasm32"))]
    discovery_cache: DiscoveryCache,
    /// HTTP client.
    #[cfg(not(target_arch = "wasm32"))]
    http_client: reqwest::Client,
}

impl std::fmt::Debug for GenericOidcProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericOidcProvider")
            .field("id", &self.id)
            .field("display_name", &self.display_name)
            .field("issuer", &self.config.issuer)
            .field("client_id", &self.config.client_id)
            .finish()
    }
}

impl GenericOidcProvider {
    /// Create a new generic OIDC provider.
    ///
    /// This constructor fetches the OIDC discovery document to determine the JWKS URI.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(config: GenericOidcConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?;

        // Leak strings for static lifetime (these are typically created once per app)
        let id: &'static str = Box::leak(config.id.clone().into_boxed_str());
        let display_name: &'static str = Box::leak(config.display_name.clone().into_boxed_str());

        // Fetch discovery to get JWKS URI
        let discovery = fetch_discovery_doc(&http_client, &config.issuer).await?;

        // Cache the discovery document
        let discovery_cache = Arc::new(RwLock::new(Some(CachedData::new(
            discovery.clone(),
            config.cache_ttl,
        ))));

        let provider = Self {
            #[cfg(feature = "jwt-auth")]
            jwt_validator: JwtValidator::new(),
            #[cfg(feature = "jwt-auth")]
            validation_config: ValidationConfig::new(
                &config.issuer,
                &discovery.jwks_uri,
                &config.client_id,
            )
            .with_leeway(config.leeway_seconds)
            .with_claim_mappings(config.claim_mappings.clone()),
            config,
            id,
            display_name,
            discovery_cache,
            http_client,
        };

        Ok(provider)
    }

    /// Create a provider with a shared JWT validator.
    ///
    /// Use this when you want multiple providers to share the same JWKS cache.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use pmcp::server::auth::{MultiTenantJwtValidator, GenericOidcProvider, GenericOidcConfig};
    ///
    /// // Create shared validator
    /// let validator = MultiTenantJwtValidator::new();
    ///
    /// // Create providers that share the validator
    /// let google_config = GenericOidcConfig::google("google-client-id");
    /// let google = GenericOidcProvider::with_validator(google_config, validator.clone()).await?;
    ///
    /// let auth0_config = GenericOidcConfig::auth0("tenant.auth0.com", "auth0-client-id");
    /// let auth0 = GenericOidcProvider::with_validator(auth0_config, validator.clone()).await?;
    /// ```
    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    pub async fn with_validator(
        config: GenericOidcConfig,
        jwt_validator: JwtValidator,
    ) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?;

        let id: &'static str = Box::leak(config.id.clone().into_boxed_str());
        let display_name: &'static str = Box::leak(config.display_name.clone().into_boxed_str());

        // Fetch discovery to get JWKS URI
        let discovery = fetch_discovery_doc(&http_client, &config.issuer).await?;

        let discovery_cache = Arc::new(RwLock::new(Some(CachedData::new(
            discovery.clone(),
            config.cache_ttl,
        ))));

        Ok(Self {
            jwt_validator,
            validation_config: ValidationConfig::new(
                &config.issuer,
                &discovery.jwks_uri,
                &config.client_id,
            )
            .with_leeway(config.leeway_seconds)
            .with_claim_mappings(config.claim_mappings.clone()),
            config,
            id,
            display_name,
            discovery_cache,
            http_client,
        })
    }

    /// Get the client ID.
    pub fn client_id(&self) -> &str {
        &self.config.client_id
    }

    /// Fetch and cache the OIDC discovery document.
    #[cfg(not(target_arch = "wasm32"))]
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

        // Fetch discovery document
        let discovery = fetch_discovery_doc(&self.http_client, &self.config.issuer).await?;

        // Cache the discovery document
        {
            let mut cache = self.discovery_cache.write().await;
            *cache = Some(CachedData::new(discovery.clone(), self.config.cache_ttl));
        }

        Ok(discovery)
    }

    /// Determine capabilities from discovery document.
    ///
    /// This method can be used to detect provider capabilities dynamically
    /// by inspecting the OIDC discovery document.
    #[cfg(not(target_arch = "wasm32"))]
    #[allow(dead_code)]
    async fn detect_capabilities(&self) -> ProviderCapabilities {
        let Ok(discovery) = self.fetch_discovery().await else {
            return ProviderCapabilities::basic_oidc();
        };

        ProviderCapabilities {
            oidc: true,
            dcr: discovery.registration_endpoint.is_some(),
            pkce: discovery
                .code_challenge_methods_supported
                .iter()
                .any(|m| m == "S256"),
            refresh_tokens: discovery
                .grant_types_supported
                .iter()
                .any(|g| g == "refresh_token"),
            revocation: discovery.revocation_endpoint.is_some(),
            introspection: discovery.introspection_endpoint.is_some(),
            custom_scopes: !discovery.scopes_supported.is_empty(),
            device_flow: discovery
                .grant_types_supported
                .iter()
                .any(|g| g == "urn:ietf:params:oauth:grant-type:device_code"),
        }
    }
}

/// Fetch OIDC discovery document (helper function).
#[cfg(not(target_arch = "wasm32"))]
async fn fetch_discovery_doc(http_client: &reqwest::Client, issuer: &str) -> Result<OidcDiscovery> {
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    tracing::debug!("Fetching OIDC discovery from {}", discovery_url);

    let response = http_client
        .get(&discovery_url)
        .send()
        .await
        .map_err(|e| Error::internal(format!("Failed to fetch discovery: {}", e)))?;

    if !response.status().is_success() {
        return Err(Error::internal(format!(
            "Discovery endpoint returned status {}",
            response.status()
        )));
    }

    response
        .json()
        .await
        .map_err(|e| Error::internal(format!("Failed to parse discovery: {}", e)))
}

#[async_trait]
impl IdentityProvider for GenericOidcProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn capabilities(&self) -> ProviderCapabilities {
        // Return basic capabilities synchronously; full detection requires async
        ProviderCapabilities::basic_oidc()
    }

    #[cfg(target_arch = "wasm32")]
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::basic_oidc()
    }

    fn issuer(&self) -> &str {
        &self.config.issuer
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "jwt-auth"))]
    async fn validate_token(&self, token: &str) -> Result<AuthContext> {
        // Delegate to shared JWT validator
        self.jwt_validator
            .validate(token, &self.validation_config)
            .await
    }

    #[cfg(any(target_arch = "wasm32", not(feature = "jwt-auth")))]
    async fn validate_token(&self, _token: &str) -> Result<AuthContext> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "JWT validation requires the 'jwt-auth' feature and non-WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn discovery(&self) -> Result<OidcDiscovery> {
        self.fetch_discovery().await
    }

    #[cfg(target_arch = "wasm32")]
    async fn discovery(&self) -> Result<OidcDiscovery> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Discovery not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn jwks(&self) -> Result<serde_json::Value> {
        let discovery = self.fetch_discovery().await?;

        let response = self
            .http_client
            .get(&discovery.jwks_uri)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to fetch JWKS: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "JWKS endpoint returned status {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse JWKS: {}", e)))
    }

    #[cfg(target_arch = "wasm32")]
    async fn jwks(&self) -> Result<serde_json::Value> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "JWKS not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn authorization_url(&self, params: AuthorizationParams) -> Result<String> {
        let discovery = self.fetch_discovery().await?;

        let mut url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            discovery.authorization_endpoint,
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&params.redirect_uri),
            urlencoding::encode(&params.scopes.join(" ")),
            urlencoding::encode(&params.state),
        );

        if let Some(nonce) = &params.nonce {
            url.push_str(&format!("&nonce={}", urlencoding::encode(nonce)));
        }

        if let Some(challenge) = &params.code_challenge {
            url.push_str(&format!(
                "&code_challenge={}&code_challenge_method={}",
                urlencoding::encode(challenge),
                params.code_challenge_method.as_deref().unwrap_or("S256")
            ));
        }

        for (key, value) in &params.extra {
            url.push_str(&format!(
                "&{}={}",
                urlencoding::encode(key),
                urlencoding::encode(value)
            ));
        }

        Ok(url)
    }

    #[cfg(target_arch = "wasm32")]
    async fn authorization_url(&self, _params: AuthorizationParams) -> Result<String> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Authorization URL not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn exchange_code(&self, params: TokenExchangeParams) -> Result<TokenResponse> {
        let discovery = self.fetch_discovery().await?;

        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("client_id", self.config.client_id.clone()),
            ("code", params.code),
            ("redirect_uri", params.redirect_uri),
        ];

        if let Some(verifier) = params.code_verifier {
            form.push(("code_verifier", verifier));
        }

        let mut request = self.http_client.post(&discovery.token_endpoint).form(&form);

        // Add client authentication if secret is configured
        if let Some(ref secret) = self.config.client_secret {
            request = request.basic_auth(&self.config.client_id, Some(secret));
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token exchange failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token exchange failed: {}", error_text),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse token response: {}", e)))
    }

    #[cfg(target_arch = "wasm32")]
    async fn exchange_code(&self, _params: TokenExchangeParams) -> Result<TokenResponse> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Code exchange not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        let discovery = self.fetch_discovery().await?;

        let form = vec![
            ("grant_type", "refresh_token"),
            ("client_id", &self.config.client_id),
            ("refresh_token", refresh_token),
        ];

        let mut request = self.http_client.post(&discovery.token_endpoint).form(&form);

        // Add client authentication if secret is configured
        if let Some(ref secret) = self.config.client_secret {
            request = request.basic_auth(&self.config.client_id, Some(secret));
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token refresh failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token refresh failed: {}", error_text),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse token response: {}", e)))
    }

    #[cfg(target_arch = "wasm32")]
    async fn refresh_token(&self, _refresh_token: &str) -> Result<TokenResponse> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "Token refresh not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn register_client(&self, request: DcrRequest) -> Result<DcrResponse> {
        let discovery = self.fetch_discovery().await?;

        let registration_endpoint = discovery.registration_endpoint.ok_or_else(|| {
            Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!(
                    "Provider '{}' does not support Dynamic Client Registration",
                    self.display_name
                ),
            )
        })?;

        let response = self
            .http_client
            .post(&registration_endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::internal(format!("DCR request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("DCR failed: {}", error_text),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse DCR response: {}", e)))
    }

    #[cfg(target_arch = "wasm32")]
    async fn register_client(&self, _request: DcrRequest) -> Result<DcrResponse> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "DCR not available on WASM target",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn revoke_token(&self, token: &str) -> Result<()> {
        let discovery = self.fetch_discovery().await?;

        let Some(revocation_endpoint) = discovery.revocation_endpoint else {
            return Ok(()); // No-op if revocation not supported
        };

        let form = vec![("token", token), ("client_id", &self.config.client_id)];

        let mut request = self.http_client.post(&revocation_endpoint).form(&form);

        // Add client authentication if secret is configured
        if let Some(ref secret) = self.config.client_secret {
            request = request.basic_auth(&self.config.client_id, Some(secret));
        }

        let response = request
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token revocation failed: {}", e)))?;

        // Revocation endpoints typically return 200 even for invalid tokens
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("Token revocation failed: {}", error_text),
            ));
        }

        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    async fn revoke_token(&self, _token: &str) -> Result<()> {
        Ok(()) // No-op on WASM
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn user_info(&self, access_token: &str) -> Result<serde_json::Value> {
        let discovery = self.fetch_discovery().await?;

        let userinfo_endpoint = discovery.userinfo_endpoint.ok_or_else(|| {
            Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!(
                    "Provider '{}' does not support UserInfo endpoint",
                    self.display_name
                ),
            )
        })?;

        let response = self
            .http_client
            .get(&userinfo_endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| Error::internal(format!("UserInfo request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::protocol(
                ErrorCode::INVALID_REQUEST,
                format!("UserInfo request failed: {}", error_text),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse UserInfo response: {}", e)))
    }

    #[cfg(target_arch = "wasm32")]
    async fn user_info(&self, _access_token: &str) -> Result<serde_json::Value> {
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            "UserInfo not available on WASM target",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // GenericOidcConfig Factory Methods Tests
    // =========================================================================

    #[test]
    fn test_google_config() {
        let config = GenericOidcConfig::google("test-client");
        assert_eq!(config.id, "google");
        assert_eq!(config.display_name, "Google Identity");
        assert_eq!(config.issuer, "https://accounts.google.com");
        assert_eq!(config.client_id, "test-client");
        assert!(config.client_secret.is_none());
    }

    #[test]
    fn test_auth0_config() {
        let config = GenericOidcConfig::auth0("example.auth0.com", "test-client");
        assert_eq!(config.id, "auth0");
        assert_eq!(config.display_name, "Auth0");
        assert_eq!(config.issuer, "https://example.auth0.com/");
        assert_eq!(config.client_id, "test-client");
    }

    #[test]
    fn test_okta_config() {
        let config = GenericOidcConfig::okta("example.okta.com", "test-client");
        assert_eq!(config.id, "okta");
        assert_eq!(config.display_name, "Okta");
        assert_eq!(config.issuer, "https://example.okta.com");
        assert_eq!(config.client_id, "test-client");
    }

    #[test]
    fn test_entra_config() {
        let config = GenericOidcConfig::entra("tenant-id", "test-client");
        assert_eq!(config.id, "entra");
        assert_eq!(config.display_name, "Microsoft Entra ID");
        assert_eq!(
            config.issuer,
            "https://login.microsoftonline.com/tenant-id/v2.0"
        );
        assert_eq!(config.client_id, "test-client");
    }

    // =========================================================================
    // GenericOidcConfig Builder Tests
    // =========================================================================

    #[test]
    fn test_config_new() {
        let config = GenericOidcConfig::new(
            "custom",
            "Custom Provider",
            "https://auth.example.com",
            "my-client-id",
        );

        assert_eq!(config.id, "custom");
        assert_eq!(config.display_name, "Custom Provider");
        assert_eq!(config.issuer, "https://auth.example.com");
        assert_eq!(config.client_id, "my-client-id");
        assert!(config.client_secret.is_none());
        assert_eq!(config.cache_ttl, Duration::from_secs(3600));
        assert_eq!(config.leeway_seconds, 60);
    }

    #[test]
    fn test_config_with_client_secret() {
        let config = GenericOidcConfig::new("test", "Test", "https://test.com", "client")
            .with_client_secret("my-secret");

        assert_eq!(config.client_secret, Some("my-secret".to_string()));
    }

    #[test]
    fn test_config_with_claim_mappings() {
        let config = GenericOidcConfig::new("test", "Test", "https://test.com", "client")
            .with_claim_mappings(ClaimMappings::google());

        // Google claim mappings should be applied
        assert!(config.claim_mappings.tenant_id.is_none()); // Google doesn't have tenant
    }

    #[test]
    fn test_config_clone() {
        let config = GenericOidcConfig::google("test-client").with_client_secret("secret");
        let cloned = config.clone();

        assert_eq!(config.id, cloned.id);
        assert_eq!(config.client_secret, cloned.client_secret);
    }

    #[test]
    fn test_config_debug() {
        let config = GenericOidcConfig::google("test-client");
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("GenericOidcConfig"));
        assert!(debug_str.contains("google"));
    }

    // =========================================================================
    // ClaimMappings for Different Providers
    // =========================================================================

    #[test]
    fn test_google_claim_mappings() {
        let mappings = ClaimMappings::google();
        assert_eq!(mappings.user_id, "sub");
        assert!(mappings.tenant_id.is_none()); // Google doesn't have tenant concept
        assert_eq!(mappings.email, Some("email".to_string()));
    }

    #[test]
    fn test_auth0_claim_mappings() {
        let mappings = ClaimMappings::auth0();
        assert_eq!(mappings.user_id, "sub");
        assert_eq!(mappings.tenant_id, Some("org_id".to_string()));
        assert_eq!(mappings.groups, Some("roles".to_string()));
    }

    #[test]
    fn test_okta_claim_mappings() {
        let mappings = ClaimMappings::okta();
        assert_eq!(mappings.user_id, "uid");
        assert_eq!(mappings.tenant_id, Some("org_id".to_string()));
        assert_eq!(mappings.groups, Some("groups".to_string()));
    }

    #[test]
    fn test_entra_claim_mappings() {
        let mappings = ClaimMappings::entra();
        assert_eq!(mappings.user_id, "oid");
        assert_eq!(mappings.tenant_id, Some("tid".to_string()));
        assert_eq!(mappings.email, Some("preferred_username".to_string()));
        assert_eq!(mappings.groups, Some("groups".to_string()));
    }

    // =========================================================================
    // CachedData Tests
    // =========================================================================

    #[test]
    fn test_cached_data_creation() {
        let data: CachedData<String> = CachedData::new("test".to_string(), Duration::from_secs(60));
        assert_eq!(data.data, "test");
        assert!(!data.is_expired());
    }

    #[test]
    fn test_cached_data_expiration() {
        let data: CachedData<String> =
            CachedData::new("test".to_string(), Duration::from_millis(1));
        std::thread::sleep(Duration::from_millis(10));
        assert!(data.is_expired());
    }

    #[test]
    fn test_cached_data_debug() {
        let data: CachedData<String> = CachedData::new("test".to_string(), Duration::from_secs(60));
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("CachedData"));
    }

    // =========================================================================
    // URL Generation Tests (Unit tests without network)
    // =========================================================================

    #[test]
    fn test_discovery_url_format() {
        let issuer = "https://accounts.google.com";
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        assert_eq!(
            discovery_url,
            "https://accounts.google.com/.well-known/openid-configuration"
        );
    }

    #[test]
    fn test_discovery_url_format_with_trailing_slash() {
        let issuer = "https://example.auth0.com/";
        let discovery_url = format!(
            "{}/.well-known/openid-configuration",
            issuer.trim_end_matches('/')
        );
        assert_eq!(
            discovery_url,
            "https://example.auth0.com/.well-known/openid-configuration"
        );
    }

    #[test]
    fn test_authorization_url_components() {
        let authorization_endpoint = "https://accounts.google.com/o/oauth2/v2/auth";
        let client_id = "test-client-id";
        let redirect_uri = "https://example.com/callback";
        let scopes = ["openid", "email", "profile"];
        let state = "random-state";

        let url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            authorization_endpoint,
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&scopes.join(" ")),
            urlencoding::encode(state),
        );

        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid%20email%20profile"));
        assert!(url.contains("state=random-state"));
    }

    #[test]
    fn test_authorization_url_with_pkce() {
        let base_url = "https://auth.example.com/authorize?client_id=test";
        let code_challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        let code_challenge_method = "S256";

        let url = format!(
            "{}&code_challenge={}&code_challenge_method={}",
            base_url,
            urlencoding::encode(code_challenge),
            code_challenge_method
        );

        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_authorization_url_with_nonce() {
        let base_url = "https://auth.example.com/authorize?client_id=test";
        let nonce = "n-0S6_WzA2Mj";

        let url = format!("{}&nonce={}", base_url, urlencoding::encode(nonce));

        assert!(url.contains("nonce=n-0S6_WzA2Mj"));
    }

    // =========================================================================
    // Provider Capabilities Tests
    // =========================================================================

    #[test]
    fn test_basic_oidc_capabilities() {
        let caps = ProviderCapabilities::basic_oidc();
        assert!(caps.oidc);
        assert!(!caps.dcr);
        assert!(caps.pkce);
        assert!(caps.refresh_tokens);
        assert!(!caps.revocation);
        assert!(!caps.introspection);
    }

    // =========================================================================
    // Integration-style Tests (without network)
    // =========================================================================

    #[test]
    fn test_config_chain() {
        // Test fluent API
        let config = GenericOidcConfig::new(
            "custom-provider",
            "Custom Identity Provider",
            "https://identity.example.com",
            "client-123",
        )
        .with_client_secret("secret-456")
        .with_claim_mappings(ClaimMappings::default());

        assert_eq!(config.id, "custom-provider");
        assert_eq!(config.display_name, "Custom Identity Provider");
        assert_eq!(config.issuer, "https://identity.example.com");
        assert_eq!(config.client_id, "client-123");
        assert_eq!(config.client_secret, Some("secret-456".to_string()));
    }

    #[test]
    fn test_claim_normalization_google() {
        let mappings = ClaimMappings::google();

        let claims = serde_json::json!({
            "sub": "google-user-123",
            "email": "user@gmail.com",
            "name": "Test User",
            "picture": "https://example.com/photo.jpg"
        });

        let normalized = mappings.normalize_claims(&claims);

        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("google-user-123")
        );
        assert_eq!(
            normalized.get("email").and_then(|v| v.as_str()),
            Some("user@gmail.com")
        );
        assert_eq!(
            normalized.get("name").and_then(|v| v.as_str()),
            Some("Test User")
        );
    }

    #[test]
    fn test_claim_normalization_entra() {
        let mappings = ClaimMappings::entra();

        let claims = serde_json::json!({
            "oid": "entra-user-456",
            "tid": "tenant-789",
            "preferred_username": "user@contoso.com",
            "name": "Enterprise User",
            "groups": ["group1", "group2"]
        });

        let normalized = mappings.normalize_claims(&claims);

        // oid should be mapped to sub
        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("entra-user-456")
        );
        // tid should be mapped to tenant_id
        assert_eq!(
            normalized.get("tenant_id").and_then(|v| v.as_str()),
            Some("tenant-789")
        );
        // preferred_username should be mapped to email
        assert_eq!(
            normalized.get("email").and_then(|v| v.as_str()),
            Some("user@contoso.com")
        );
        // groups should be normalized
        assert!(normalized.contains_key("groups"));
    }

    #[test]
    fn test_claim_normalization_auth0() {
        let mappings = ClaimMappings::auth0();

        let claims = serde_json::json!({
            "sub": "auth0|user123",
            "org_id": "org_ABC123",
            "email": "user@example.com",
            "roles": ["admin", "user"]
        });

        let normalized = mappings.normalize_claims(&claims);

        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("auth0|user123")
        );
        assert_eq!(
            normalized.get("tenant_id").and_then(|v| v.as_str()),
            Some("org_ABC123")
        );
        assert!(normalized.contains_key("groups"));
    }
}
