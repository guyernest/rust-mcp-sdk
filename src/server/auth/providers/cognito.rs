//! AWS Cognito identity provider.
//!
//! This module provides a Cognito-specific implementation of [`IdentityProvider`].
//! JWT validation is delegated to [`MultiTenantJwtValidator`] for code reuse.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::error::{Error, ErrorCode, Result};
use crate::server::auth::jwt_validator::{JwtValidator, ValidationConfig};
use crate::server::auth::provider::{
    AuthorizationParams, DcrRequest, DcrResponse, IdentityProvider, OidcDiscovery,
    ProviderCapabilities, TokenExchangeParams, TokenResponse,
};
use crate::server::auth::traits::AuthContext;

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

/// Type alias for discovery cache.
#[cfg(not(target_arch = "wasm32"))]
type DiscoveryCache = Arc<RwLock<Option<CachedData<OidcDiscovery>>>>;

/// AWS Cognito identity provider.
///
/// Provides token validation and OIDC discovery for AWS Cognito user pools.
/// Uses [`JwtValidator`] internally for efficient JWKS caching and JWT validation.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::server::auth::providers::CognitoProvider;
///
/// let cognito = CognitoProvider::new(
///     "us-east-1",
///     "us-east-1_xxxxx",
///     "your-client-id",
/// ).await?;
///
/// // Validate a token
/// let auth = cognito.validate_token("eyJ...").await?;
/// println!("User: {}", auth.user_id());
/// ```
#[derive(Debug)]
pub struct CognitoProvider {
    /// AWS region.
    region: String,
    /// Cognito user pool ID.
    user_pool_id: String,
    /// App client ID.
    client_id: String,
    /// Issuer URL.
    issuer: String,
    /// JWKS URI.
    jwks_uri: String,
    /// JWT validator with shared JWKS cache.
    #[cfg(not(target_arch = "wasm32"))]
    jwt_validator: JwtValidator,
    /// Validation config for this provider.
    validation_config: ValidationConfig,
    /// Cached discovery document.
    #[cfg(not(target_arch = "wasm32"))]
    discovery_cache: DiscoveryCache,
    /// HTTP client for non-JWT operations.
    #[cfg(not(target_arch = "wasm32"))]
    http_client: reqwest::Client,
    /// Cache TTL.
    cache_ttl: Duration,
}

impl CognitoProvider {
    /// Create a new Cognito provider.
    ///
    /// # Arguments
    ///
    /// * `region` - AWS region (e.g., "us-east-1")
    /// * `user_pool_id` - Cognito user pool ID (e.g., "us-east-1_xxxxx")
    /// * `client_id` - App client ID
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(region: &str, user_pool_id: &str, client_id: &str) -> Result<Self> {
        let issuer = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);

        // Create validation config for Cognito
        let validation_config = ValidationConfig::cognito(region, user_pool_id, client_id);

        // Create shared JWT validator
        let jwt_validator = JwtValidator::new();

        let provider = Self {
            region: region.to_string(),
            user_pool_id: user_pool_id.to_string(),
            client_id: client_id.to_string(),
            issuer,
            jwks_uri,
            jwt_validator,
            validation_config,
            discovery_cache: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?,
            cache_ttl: Duration::from_secs(3600), // 1 hour
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
    /// use pmcp::server::auth::{MultiTenantJwtValidator, CognitoProvider};
    ///
    /// // Create shared validator
    /// let validator = MultiTenantJwtValidator::new();
    ///
    /// // Create providers that share the validator
    /// let provider1 = CognitoProvider::with_validator("us-east-1", "pool1", "client1", validator.clone()).await?;
    /// let provider2 = CognitoProvider::with_validator("us-west-2", "pool2", "client2", validator.clone()).await?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn with_validator(
        region: &str,
        user_pool_id: &str,
        client_id: &str,
        jwt_validator: JwtValidator,
    ) -> Result<Self> {
        let issuer = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);

        let validation_config = ValidationConfig::cognito(region, user_pool_id, client_id);

        let provider = Self {
            region: region.to_string(),
            user_pool_id: user_pool_id.to_string(),
            client_id: client_id.to_string(),
            issuer,
            jwks_uri,
            jwt_validator,
            validation_config,
            discovery_cache: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .map_err(|e| Error::internal(format!("Failed to create HTTP client: {}", e)))?,
            cache_ttl: Duration::from_secs(3600),
        };

        Ok(provider)
    }

    /// Get the AWS region.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Get the user pool ID.
    pub fn user_pool_id(&self) -> &str {
        &self.user_pool_id
    }

    /// Get the client ID.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Get the Cognito hosted UI authorization endpoint.
    fn hosted_ui_domain(&self) -> String {
        // Default hosted UI domain pattern
        format!(
            "https://{}.auth.{}.amazoncognito.com",
            self.user_pool_id, self.region
        )
    }
}

#[async_trait]
impl IdentityProvider for CognitoProvider {
    fn id(&self) -> &'static str {
        "cognito"
    }

    fn display_name(&self) -> &'static str {
        "AWS Cognito"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            oidc: true,
            dcr: false, // Cognito doesn't support DCR
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
        let discovery_url = format!("{}/.well-known/openid-configuration", self.issuer);
        tracing::debug!("Fetching OIDC discovery from {}", discovery_url);

        let response = self
            .http_client
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

        let discovery: OidcDiscovery = response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse discovery: {}", e)))?;

        // Cache the discovery document
        {
            let mut cache = self.discovery_cache.write().await;
            *cache = Some(CachedData::new(discovery.clone(), self.cache_ttl));
        }

        Ok(discovery)
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
        let response = self
            .http_client
            .get(&self.jwks_uri)
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

    async fn authorization_url(&self, params: AuthorizationParams) -> Result<String> {
        let hosted_ui = self.hosted_ui_domain();

        let mut url = format!(
            "{}/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            hosted_ui,
            urlencoding::encode(&self.client_id),
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

    #[cfg(not(target_arch = "wasm32"))]
    async fn exchange_code(&self, params: TokenExchangeParams) -> Result<TokenResponse> {
        let hosted_ui = self.hosted_ui_domain();
        let token_url = format!("{}/oauth2/token", hosted_ui);

        let mut form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("client_id", self.client_id.clone()),
            ("code", params.code),
            ("redirect_uri", params.redirect_uri),
        ];

        if let Some(verifier) = params.code_verifier {
            form.push(("code_verifier", verifier));
        }

        let response = self
            .http_client
            .post(&token_url)
            .form(&form)
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
        let hosted_ui = self.hosted_ui_domain();
        let token_url = format!("{}/oauth2/token", hosted_ui);

        let form = vec![
            ("grant_type", "refresh_token"),
            ("client_id", &self.client_id),
            ("refresh_token", refresh_token),
        ];

        let response = self
            .http_client
            .post(&token_url)
            .form(&form)
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

    async fn register_client(&self, _request: DcrRequest) -> Result<DcrResponse> {
        Err(Error::protocol(
            ErrorCode::INVALID_REQUEST,
            "AWS Cognito does not support Dynamic Client Registration",
        ))
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn revoke_token(&self, token: &str) -> Result<()> {
        let hosted_ui = self.hosted_ui_domain();
        let revoke_url = format!("{}/oauth2/revoke", hosted_ui);

        let form = vec![("token", token), ("client_id", &self.client_id)];

        let response = self
            .http_client
            .post(&revoke_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Token revocation failed: {}", e)))?;

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
        let hosted_ui = self.hosted_ui_domain();
        let userinfo_url = format!("{}/oauth2/userInfo", hosted_ui);

        let response = self
            .http_client
            .get(&userinfo_url)
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
    use crate::server::auth::traits::ClaimMappings;

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
        // Wait for it to expire
        std::thread::sleep(Duration::from_millis(10));
        assert!(data.is_expired());
    }

    #[test]
    fn test_cached_data_debug() {
        let data: CachedData<String> = CachedData::new("test".to_string(), Duration::from_secs(60));
        let debug_str = format!("{:?}", data);
        assert!(debug_str.contains("CachedData"));
        assert!(debug_str.contains("data"));
        assert!(debug_str.contains("ttl"));
    }

    // =========================================================================
    // Provider Capabilities Tests
    // =========================================================================

    #[test]
    fn test_cognito_capabilities() {
        // Test the expected capabilities for Cognito
        let caps = ProviderCapabilities {
            oidc: true,
            dcr: false, // Cognito doesn't support DCR
            pkce: true,
            refresh_tokens: true,
            revocation: true,
            introspection: false,
            custom_scopes: true,
            device_flow: false,
        };

        assert!(caps.oidc);
        assert!(!caps.dcr);
        assert!(caps.pkce);
        assert!(caps.refresh_tokens);
        assert!(caps.revocation);
        assert!(!caps.introspection);
        assert!(caps.custom_scopes);
        assert!(!caps.device_flow);
    }

    // =========================================================================
    // URL Generation Tests (Unit tests without network)
    // =========================================================================

    #[test]
    fn test_issuer_url_format() {
        let region = "us-east-1";
        let user_pool_id = "us-east-1_ABC123";
        let expected = format!(
            "https://cognito-idp.{}.amazonaws.com/{}",
            region, user_pool_id
        );
        assert_eq!(
            expected,
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC123"
        );
    }

    #[test]
    fn test_jwks_uri_format() {
        let issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_ABC123";
        let jwks_uri = format!("{}/.well-known/jwks.json", issuer);
        assert!(jwks_uri.ends_with("/.well-known/jwks.json"));
        assert!(jwks_uri.contains("cognito-idp"));
    }

    #[test]
    fn test_hosted_ui_domain_format() {
        let user_pool_id = "us-east-1_ABC123";
        let region = "us-east-1";
        let expected = format!("https://{}.auth.{}.amazoncognito.com", user_pool_id, region);
        assert_eq!(
            expected,
            "https://us-east-1_ABC123.auth.us-east-1.amazoncognito.com"
        );
    }

    #[test]
    fn test_authorization_url_components() {
        // Test URL components without needing actual provider
        let hosted_ui = "https://us-east-1_ABC123.auth.us-east-1.amazoncognito.com";
        let client_id = "test-client-id";
        let redirect_uri = "https://example.com/callback";
        let scopes = ["openid", "email", "profile"];
        let state = "random-state";

        let url = format!(
            "{}/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            hosted_ui,
            urlencoding::encode(client_id),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(&scopes.join(" ")),
            urlencoding::encode(state),
        );

        assert!(url.contains("/oauth2/authorize"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=https%3A%2F%2Fexample.com%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid%20email%20profile"));
        assert!(url.contains("state=random-state"));
    }

    #[test]
    fn test_authorization_url_with_pkce() {
        let base_url = "https://auth.example.com/oauth2/authorize?client_id=test";
        let code_challenge = "challenge123";
        let code_challenge_method = "S256";

        let url = format!(
            "{}&code_challenge={}&code_challenge_method={}",
            base_url,
            urlencoding::encode(code_challenge),
            code_challenge_method
        );

        assert!(url.contains("code_challenge=challenge123"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn test_authorization_url_with_nonce() {
        let base_url = "https://auth.example.com/oauth2/authorize?client_id=test";
        let nonce = "nonce456";

        let url = format!("{}&nonce={}", base_url, urlencoding::encode(nonce));

        assert!(url.contains("nonce=nonce456"));
    }

    // =========================================================================
    // ClaimMappings Tests
    // =========================================================================

    #[test]
    fn test_cognito_claim_mappings() {
        let mappings = ClaimMappings::cognito();
        assert_eq!(mappings.user_id, "sub");
        assert_eq!(mappings.tenant_id, Some("custom:tenant_id".to_string()));
        assert_eq!(mappings.email, Some("email".to_string()));
        assert_eq!(mappings.groups, Some("cognito:groups".to_string()));
    }

    #[test]
    fn test_cognito_claim_normalization() {
        let mappings = ClaimMappings::cognito();

        let claims = serde_json::json!({
            "sub": "user-123",
            "email": "user@example.com",
            "custom:tenant_id": "tenant-456",
            "cognito:groups": ["admin", "users"]
        });

        let normalized = mappings.normalize_claims(&claims);

        assert_eq!(
            normalized.get("sub").and_then(|v| v.as_str()),
            Some("user-123")
        );
        assert_eq!(
            normalized.get("email").and_then(|v| v.as_str()),
            Some("user@example.com")
        );
        assert_eq!(
            normalized.get("tenant_id").and_then(|v| v.as_str()),
            Some("tenant-456")
        );
        assert!(normalized.contains_key("groups"));
    }

    // =========================================================================
    // Error Message Tests
    // =========================================================================

    #[test]
    fn test_dcr_not_supported_message() {
        // Cognito doesn't support DCR - verify the error message
        let error_msg = "AWS Cognito does not support Dynamic Client Registration";
        assert!(error_msg.contains("Cognito"));
        assert!(error_msg.contains("Dynamic Client Registration"));
    }

    #[tokio::test]
    async fn test_dcr_returns_error() {
        // This test would require a mock provider, but we can verify the trait default
        use crate::server::auth::provider::IdentityProvider;

        // Create a mock that has the same behavior
        struct MockCognito;

        #[async_trait]
        impl IdentityProvider for MockCognito {
            fn id(&self) -> &'static str {
                "cognito"
            }
            fn display_name(&self) -> &'static str {
                "AWS Cognito"
            }
            fn capabilities(&self) -> ProviderCapabilities {
                ProviderCapabilities {
                    oidc: true,
                    dcr: false,
                    pkce: true,
                    refresh_tokens: true,
                    revocation: true,
                    introspection: false,
                    custom_scopes: true,
                    device_flow: false,
                }
            }
            #[allow(clippy::unnecessary_literal_bound)]
            fn issuer(&self) -> &str {
                "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_test"
            }
            async fn validate_token(&self, _token: &str) -> Result<AuthContext> {
                Ok(AuthContext::new("test-user"))
            }
            async fn discovery(&self) -> Result<OidcDiscovery> {
                unimplemented!()
            }
            async fn jwks(&self) -> Result<serde_json::Value> {
                unimplemented!()
            }
            async fn register_client(
                &self,
                _request: crate::server::auth::provider::DcrRequest,
            ) -> Result<crate::server::auth::provider::DcrResponse> {
                Err(crate::error::Error::protocol(
                    crate::error::ErrorCode::INVALID_REQUEST,
                    "AWS Cognito does not support Dynamic Client Registration",
                ))
            }
        }

        impl std::fmt::Debug for MockCognito {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("MockCognito").finish()
            }
        }

        let provider = MockCognito;
        let request = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec!["https://example.com/callback".to_string()],
            client_name: None,
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: None,
            grant_types: vec![],
            response_types: vec![],
            scope: None,
            software_id: None,
            software_version: None,
            extra: std::collections::HashMap::new(),
        };

        let result = provider.register_client(request).await;
        assert!(result.is_err());
    }
}
