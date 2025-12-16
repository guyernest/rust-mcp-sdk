//! Configuration types for token validation.
//!
//! This module provides configuration-driven token validator selection.
//! The validator type is determined by configuration (environment, config file),
//! not by code changes.

use super::ClaimMappings;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Token validator configuration.
///
/// This enum determines which token validation strategy is used.
/// The configuration can come from `pmcp.toml`, environment variables,
/// or be specified programmatically.
///
/// # Configuration-Driven Selection
///
/// The validator is selected based on the `type` field in configuration:
///
/// ```toml
/// # pmcp.toml - JWT validation
/// [profile.production.auth]
/// type = "jwt"
/// issuer = "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
/// audience = "your-app-client-id"
///
/// # pmcp.toml - Mock validation (development)
/// [profile.dev.auth]
/// type = "mock"
/// default_user_id = "dev-user"
/// default_scopes = ["read", "write"]
/// ```
///
/// # Environment Variable Override
///
/// ```bash
/// PMCP_AUTH_TYPE=jwt
/// PMCP_AUTH_ISSUER=https://issuer.example.com
/// PMCP_AUTH_AUDIENCE=my-audience
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TokenValidatorConfig {
    /// Validate JWTs locally using JWKS.
    ///
    /// This is the recommended configuration for production deployments.
    /// Tokens are validated stateless using the provider's public keys.
    #[serde(rename = "jwt")]
    Jwt(JwtValidatorConfig),

    /// Validate via token introspection endpoint (RFC 7662).
    ///
    /// Use this when the OAuth server doesn't provide JWKS or when
    /// you need real-time token revocation checking.
    #[serde(rename = "introspection")]
    Introspection(IntrospectionValidatorConfig),

    /// Validate via external proxy (like Lambda authorizer).
    ///
    /// Use this when token validation is handled by an API gateway
    /// or external service.
    #[serde(rename = "proxy")]
    Proxy(ProxyValidatorConfig),

    /// Mock authentication for development/testing.
    ///
    /// Always returns a successful authentication with configurable
    /// user ID, scopes, and claims. **Never use in production.**
    #[serde(rename = "mock")]
    Mock(MockValidatorConfig),

    /// No authentication (development only).
    ///
    /// Allows all requests without authentication.
    /// **Never use in production.**
    #[serde(rename = "none")]
    #[default]
    Disabled,
}

impl TokenValidatorConfig {
    /// Create a JWT validator configuration.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::TokenValidatorConfig;
    ///
    /// let config = TokenValidatorConfig::jwt(
    ///     "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx",
    ///     "your-app-client-id",
    /// );
    /// ```
    pub fn jwt(issuer: impl Into<String>, audience: impl Into<String>) -> Self {
        Self::Jwt(JwtValidatorConfig {
            issuer: issuer.into(),
            audience: audience.into(),
            jwks_uri: None,
            algorithms: default_algorithms(),
            jwks_cache_ttl: default_jwks_ttl(),
            claim_mappings: ClaimMappings::default(),
            leeway_seconds: 60,
        })
    }

    /// Create a mock validator configuration for testing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::TokenValidatorConfig;
    ///
    /// let config = TokenValidatorConfig::mock("test-user");
    /// ```
    pub fn mock(default_user_id: impl Into<String>) -> Self {
        Self::Mock(MockValidatorConfig {
            default_user_id: default_user_id.into(),
            default_tenant_id: None,
            default_scopes: vec!["read".to_string(), "write".to_string()],
            default_client_id: Some("mock-client".to_string()),
            claims: serde_json::Value::Object(serde_json::Map::new()),
            always_authenticated: true,
        })
    }

    /// Create a disabled (no-auth) configuration.
    pub fn disabled() -> Self {
        Self::Disabled
    }

    /// Check if this configuration requires authentication.
    pub fn requires_auth(&self) -> bool {
        !matches!(self, Self::Disabled)
    }
}

/// JWT validator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtValidatorConfig {
    /// OIDC issuer URL (used to derive JWKS URL if not specified).
    ///
    /// Examples:
    /// - Cognito: `https://cognito-idp.{region}.amazonaws.com/{userPoolId}`
    /// - Entra: `https://login.microsoftonline.com/{tenantId}/v2.0`
    /// - Google: `https://accounts.google.com`
    /// - Okta: `https://{domain}.okta.com`
    pub issuer: String,

    /// Expected audience (typically client ID).
    pub audience: String,

    /// Optional: explicit JWKS URI (otherwise derived from issuer).
    ///
    /// If not provided, defaults to `{issuer}/.well-known/jwks.json`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jwks_uri: Option<String>,

    /// Algorithms to accept (default: `["RS256"]`).
    #[serde(default = "default_algorithms")]
    pub algorithms: Vec<String>,

    /// JWKS cache TTL in seconds (default: 3600).
    #[serde(default = "default_jwks_ttl")]
    pub jwks_cache_ttl: u64,

    /// Claim mappings for provider-specific claim names.
    #[serde(default)]
    pub claim_mappings: ClaimMappings,

    /// Clock skew leeway in seconds for expiration checking.
    #[serde(default = "default_leeway")]
    pub leeway_seconds: u64,
}

impl JwtValidatorConfig {
    /// Get the JWKS URI, deriving from issuer if not explicitly set.
    pub fn jwks_uri(&self) -> String {
        self.jwks_uri.clone().unwrap_or_else(|| {
            format!(
                "{}/.well-known/jwks.json",
                self.issuer.trim_end_matches('/')
            )
        })
    }

    /// Get the JWKS cache TTL as a Duration.
    pub fn cache_ttl(&self) -> Duration {
        Duration::from_secs(self.jwks_cache_ttl)
    }

    /// Create configuration for AWS Cognito.
    pub fn cognito(region: &str, user_pool_id: &str, client_id: &str) -> Self {
        Self {
            issuer: format!(
                "https://cognito-idp.{}.amazonaws.com/{}",
                region, user_pool_id
            ),
            audience: client_id.to_string(),
            jwks_uri: None,
            algorithms: default_algorithms(),
            jwks_cache_ttl: default_jwks_ttl(),
            claim_mappings: ClaimMappings::cognito(),
            leeway_seconds: default_leeway(),
        }
    }

    /// Create configuration for Microsoft Entra ID.
    pub fn entra(tenant_id: &str, audience: &str) -> Self {
        Self {
            issuer: format!("https://login.microsoftonline.com/{}/v2.0", tenant_id),
            audience: audience.to_string(),
            jwks_uri: Some(format!(
                "https://login.microsoftonline.com/{}/discovery/v2.0/keys",
                tenant_id
            )),
            algorithms: default_algorithms(),
            jwks_cache_ttl: default_jwks_ttl(),
            claim_mappings: ClaimMappings::entra(),
            leeway_seconds: default_leeway(),
        }
    }

    /// Create configuration for Google Identity.
    pub fn google(client_id: &str) -> Self {
        Self {
            issuer: "https://accounts.google.com".to_string(),
            audience: client_id.to_string(),
            jwks_uri: Some("https://www.googleapis.com/oauth2/v3/certs".to_string()),
            algorithms: default_algorithms(),
            jwks_cache_ttl: default_jwks_ttl(),
            claim_mappings: ClaimMappings::google(),
            leeway_seconds: default_leeway(),
        }
    }

    /// Create configuration for Okta.
    pub fn okta(domain: &str, audience: &str) -> Self {
        Self {
            issuer: format!("https://{}", domain),
            audience: audience.to_string(),
            jwks_uri: Some(format!("https://{}/oauth2/v1/keys", domain)),
            algorithms: default_algorithms(),
            jwks_cache_ttl: default_jwks_ttl(),
            claim_mappings: ClaimMappings::okta(),
            leeway_seconds: default_leeway(),
        }
    }

    /// Create configuration for Auth0.
    pub fn auth0(domain: &str, audience: &str) -> Self {
        Self {
            issuer: format!("https://{}/", domain),
            audience: audience.to_string(),
            jwks_uri: Some(format!("https://{}/.well-known/jwks.json", domain)),
            algorithms: default_algorithms(),
            jwks_cache_ttl: default_jwks_ttl(),
            claim_mappings: ClaimMappings::auth0(),
            leeway_seconds: default_leeway(),
        }
    }
}

/// Token introspection configuration (RFC 7662).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntrospectionValidatorConfig {
    /// Introspection endpoint URL.
    pub url: String,

    /// Client ID for authenticating to introspection endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// Client secret for authenticating to introspection endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// Additional headers to send with introspection requests.
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,

    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    /// Claim mappings for provider-specific claim names.
    #[serde(default)]
    pub claim_mappings: ClaimMappings,
}

/// Proxy validator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyValidatorConfig {
    /// URL of the validation proxy.
    pub url: String,

    /// Headers to forward to proxy.
    #[serde(default)]
    pub forward_headers: Vec<String>,

    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

/// Mock validator configuration for development/testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockValidatorConfig {
    /// Default user ID to return.
    pub default_user_id: String,

    /// Default tenant ID (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_tenant_id: Option<String>,

    /// Default scopes to grant.
    #[serde(default)]
    pub default_scopes: Vec<String>,

    /// Default client ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_client_id: Option<String>,

    /// Additional claims to include.
    #[serde(default)]
    pub claims: serde_json::Value,

    /// Whether to always return authenticated (true) or respect token presence.
    #[serde(default = "default_always_auth")]
    pub always_authenticated: bool,
}

impl Default for MockValidatorConfig {
    fn default() -> Self {
        Self {
            default_user_id: "mock-user".to_string(),
            default_tenant_id: None,
            default_scopes: vec!["read".to_string(), "write".to_string()],
            default_client_id: Some("mock-client".to_string()),
            claims: serde_json::Value::Object(serde_json::Map::new()),
            always_authenticated: true,
        }
    }
}

// Default value functions for serde
fn default_algorithms() -> Vec<String> {
    vec!["RS256".to_string()]
}

fn default_jwks_ttl() -> u64 {
    3600 // 1 hour
}

fn default_leeway() -> u64 {
    60 // 1 minute
}

fn default_timeout() -> u64 {
    10 // 10 seconds
}

fn default_always_auth() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_config_creation() {
        let config = TokenValidatorConfig::jwt(
            "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx",
            "my-client-id",
        );

        match config {
            TokenValidatorConfig::Jwt(jwt) => {
                assert_eq!(
                    jwt.issuer,
                    "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
                );
                assert_eq!(jwt.audience, "my-client-id");
                assert!(jwt.jwks_uri.is_none());
            },
            _ => panic!("Expected JWT config"),
        }
    }

    #[test]
    fn test_mock_config_creation() {
        let config = TokenValidatorConfig::mock("test-user");

        match config {
            TokenValidatorConfig::Mock(mock) => {
                assert_eq!(mock.default_user_id, "test-user");
                assert!(mock.always_authenticated);
            },
            _ => panic!("Expected Mock config"),
        }
    }

    #[test]
    fn test_jwks_uri_derivation() {
        let config = JwtValidatorConfig {
            issuer: "https://issuer.example.com".to_string(),
            audience: "audience".to_string(),
            jwks_uri: None,
            algorithms: default_algorithms(),
            jwks_cache_ttl: default_jwks_ttl(),
            claim_mappings: ClaimMappings::default(),
            leeway_seconds: default_leeway(),
        };

        assert_eq!(
            config.jwks_uri(),
            "https://issuer.example.com/.well-known/jwks.json"
        );
    }

    #[test]
    fn test_provider_specific_configs() {
        let cognito = JwtValidatorConfig::cognito("us-east-1", "us-east-1_xxxxx", "client-id");
        assert!(cognito.issuer.contains("cognito-idp"));

        let entra = JwtValidatorConfig::entra("tenant-id", "api://my-api");
        assert!(entra.issuer.contains("microsoftonline"));

        let google = JwtValidatorConfig::google("client-id.apps.googleusercontent.com");
        assert_eq!(google.issuer, "https://accounts.google.com");
    }

    #[test]
    fn test_config_serialization() {
        let config = TokenValidatorConfig::jwt("https://issuer.example.com", "audience");
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"jwt\""));

        let deserialized: TokenValidatorConfig = serde_json::from_str(&json).unwrap();
        match deserialized {
            TokenValidatorConfig::Jwt(jwt) => {
                assert_eq!(jwt.issuer, "https://issuer.example.com");
            },
            _ => panic!("Expected JWT config"),
        }
    }
}
