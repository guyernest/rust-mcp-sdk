//! Mock token validator for development and testing.
//!
//! This module provides a mock token validator that always returns a configurable
//! authentication context. **Never use in production.**
//!
//! # Example
//!
//! ```rust
//! use pmcp::server::auth::{MockValidator, AuthContext};
//!
//! // Create a mock validator for testing
//! let validator = MockValidator::new("test-user")
//!     .with_tenant_id("test-tenant")
//!     .with_scopes(vec!["read", "write"])
//!     .with_claim("email", "test@example.com");
//! ```

use super::config::MockValidatorConfig;
use super::traits::{AuthContext, TokenValidator};
use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;

/// Mock token validator for development and testing.
///
/// This validator always returns a successful authentication result with
/// the configured user ID, scopes, and claims. It's useful for:
///
/// - Unit testing tools that require authentication
/// - Local development without setting up OAuth
/// - Integration testing with controlled auth contexts
///
/// # Warning
///
/// **Never use `MockValidator` in production.** It bypasses all security
/// checks and accepts any token (or no token at all).
///
/// # Example
///
/// ```rust
/// use pmcp::server::auth::{MockValidator, TokenValidator};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let validator = MockValidator::new("dev-user")
///     .with_scopes(vec!["read", "write", "admin"])
///     .with_claim("email", "dev@example.com")
///     .with_claim("name", "Developer");
///
/// // Any token will work
/// let auth = validator.validate("any-token").await?;
/// assert_eq!(auth.user_id(), "dev-user");
/// assert!(auth.has_scope("admin"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MockValidator {
    /// User ID to return.
    user_id: String,
    /// Tenant ID to return.
    tenant_id: Option<String>,
    /// Scopes to grant.
    scopes: Vec<String>,
    /// Client ID.
    client_id: Option<String>,
    /// Additional claims.
    claims: HashMap<String, serde_json::Value>,
    /// Whether to always authenticate (true) or require token presence (false).
    always_authenticated: bool,
}

impl MockValidator {
    /// Create a new mock validator with the given user ID.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::MockValidator;
    ///
    /// let validator = MockValidator::new("test-user-123");
    /// ```
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            tenant_id: None,
            scopes: vec!["read".to_string(), "write".to_string()],
            client_id: Some("mock-client".to_string()),
            claims: HashMap::new(),
            always_authenticated: true,
        }
    }

    /// Create a mock validator from configuration.
    pub fn from_config(config: MockValidatorConfig) -> Self {
        let mut claims = HashMap::new();

        // Convert claims from serde_json::Value
        if let Some(obj) = config.claims.as_object() {
            for (key, value) in obj {
                claims.insert(key.clone(), value.clone());
            }
        }

        // Add tenant_id to claims if configured
        if let Some(ref tenant_id) = config.default_tenant_id {
            claims.insert(
                "tenant_id".to_string(),
                serde_json::Value::String(tenant_id.clone()),
            );
        }

        Self {
            user_id: config.default_user_id,
            tenant_id: config.default_tenant_id,
            scopes: config.default_scopes,
            client_id: config.default_client_id,
            claims,
            always_authenticated: config.always_authenticated,
        }
    }

    /// Set the tenant ID.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::MockValidator;
    ///
    /// let validator = MockValidator::new("user")
    ///     .with_tenant_id("tenant-abc");
    /// ```
    pub fn with_tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        let tenant = tenant_id.into();
        self.tenant_id = Some(tenant.clone());
        self.claims
            .insert("tenant_id".to_string(), serde_json::Value::String(tenant));
        self
    }

    /// Set the scopes to grant.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::MockValidator;
    ///
    /// let validator = MockValidator::new("user")
    ///     .with_scopes(vec!["read", "write", "admin"]);
    /// ```
    pub fn with_scopes<I, S>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Set the client ID.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::MockValidator;
    ///
    /// let validator = MockValidator::new("user")
    ///     .with_client_id("my-test-client");
    /// ```
    pub fn with_client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    /// Add a claim.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::MockValidator;
    ///
    /// let validator = MockValidator::new("user")
    ///     .with_claim("email", "user@example.com")
    ///     .with_claim("roles", serde_json::json!(["admin", "user"]));
    /// ```
    pub fn with_claim(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.claims.insert(key.into(), value.into());
        self
    }

    /// Set whether to always authenticate regardless of token presence.
    ///
    /// When `true` (default), validation succeeds even without a token.
    /// When `false`, validation fails if no token is provided.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::MockValidator;
    ///
    /// // Require a token (any token works, but must be present)
    /// let validator = MockValidator::new("user")
    ///     .require_token();
    /// ```
    pub fn require_token(mut self) -> Self {
        self.always_authenticated = false;
        self
    }

    /// Build the mock auth context.
    fn build_context(&self, token: Option<&str>) -> AuthContext {
        let mut claims = self.claims.clone();

        // Add email if not present
        if !claims.contains_key("email") {
            claims.insert(
                "email".to_string(),
                serde_json::Value::String(format!("{}@mock.local", self.user_id)),
            );
        }

        // Add name if not present
        if !claims.contains_key("name") {
            claims.insert(
                "name".to_string(),
                serde_json::Value::String(format!("Mock User {}", self.user_id)),
            );
        }

        AuthContext {
            subject: self.user_id.clone(),
            scopes: self.scopes.clone(),
            claims,
            token: token.map(String::from),
            client_id: self.client_id.clone(),
            expires_at: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ),
            authenticated: true,
        }
    }
}

impl Default for MockValidator {
    fn default() -> Self {
        Self::new("mock-user")
    }
}

#[async_trait]
impl TokenValidator for MockValidator {
    async fn validate(&self, token: &str) -> Result<AuthContext> {
        // If require_token is set and token is empty, fail
        if !self.always_authenticated && token.is_empty() {
            return Err(crate::error::Error::protocol(
                crate::error::ErrorCode::AUTHENTICATION_REQUIRED,
                "Token required",
            ));
        }

        Ok(self.build_context(Some(token)))
    }
}

/// Builder for creating mock auth contexts in tests.
///
/// This is useful for unit tests where you want to create specific
/// auth contexts without going through the validator.
///
/// # Example
///
/// ```rust
/// use pmcp::server::auth::MockAuthContextBuilder;
///
/// let auth = MockAuthContextBuilder::new()
///     .user_id("test-user")
///     .tenant_id("test-tenant")
///     .scopes(vec!["read", "write"])
///     .claim("email", "test@example.com")
///     .build();
///
/// assert_eq!(auth.user_id(), "test-user");
/// assert_eq!(auth.tenant_id(), Some("test-tenant"));
/// ```
#[derive(Debug, Default)]
pub struct MockAuthContextBuilder {
    user_id: String,
    tenant_id: Option<String>,
    scopes: Vec<String>,
    client_id: Option<String>,
    claims: HashMap<String, serde_json::Value>,
    token: Option<String>,
}

impl MockAuthContextBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            user_id: "mock-user".to_string(),
            scopes: vec!["read".to_string(), "write".to_string()],
            client_id: Some("mock-client".to_string()),
            ..Default::default()
        }
    }

    /// Set the user ID.
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = user_id.into();
        self
    }

    /// Set the tenant ID.
    pub fn tenant_id(mut self, tenant_id: impl Into<String>) -> Self {
        let tenant = tenant_id.into();
        self.tenant_id = Some(tenant.clone());
        self.claims
            .insert("tenant_id".to_string(), serde_json::Value::String(tenant));
        self
    }

    /// Set the scopes.
    pub fn scopes<I, S>(mut self, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Set the client ID.
    pub fn client_id(mut self, client_id: impl Into<String>) -> Self {
        self.client_id = Some(client_id.into());
        self
    }

    /// Add a claim.
    pub fn claim(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.claims.insert(key.into(), value.into());
        self
    }

    /// Set the token.
    pub fn token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Build the auth context.
    pub fn build(self) -> AuthContext {
        let mut claims = self.claims;

        // Add email if not present
        if !claims.contains_key("email") {
            claims.insert(
                "email".to_string(),
                serde_json::Value::String(format!("{}@mock.local", self.user_id)),
            );
        }

        AuthContext {
            subject: self.user_id,
            scopes: self.scopes,
            claims,
            token: self.token,
            client_id: self.client_id,
            expires_at: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 3600,
            ),
            authenticated: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_validator_basic() {
        let validator = MockValidator::new("test-user");
        let auth = validator.validate("any-token").await.unwrap();

        assert_eq!(auth.user_id(), "test-user");
        assert!(auth.authenticated);
        assert!(auth.has_scope("read"));
        assert!(auth.has_scope("write"));
    }

    #[tokio::test]
    async fn test_mock_validator_with_tenant() {
        let validator = MockValidator::new("test-user").with_tenant_id("tenant-123");

        let auth = validator.validate("token").await.unwrap();
        assert_eq!(auth.tenant_id(), Some("tenant-123"));
    }

    #[tokio::test]
    async fn test_mock_validator_with_claims() {
        let validator = MockValidator::new("test-user")
            .with_claim("email", "test@example.com")
            .with_claim("roles", serde_json::json!(["admin"]));

        let auth = validator.validate("token").await.unwrap();
        assert_eq!(auth.email(), Some("test@example.com"));

        let roles: Option<Vec<String>> = auth.claim("roles");
        assert_eq!(roles, Some(vec!["admin".to_string()]));
    }

    #[tokio::test]
    async fn test_mock_validator_custom_scopes() {
        let validator =
            MockValidator::new("test-user").with_scopes(vec!["custom:read", "custom:write"]);

        let auth = validator.validate("token").await.unwrap();
        assert!(auth.has_scope("custom:read"));
        assert!(auth.has_scope("custom:write"));
        assert!(!auth.has_scope("read")); // Default scope not present
    }

    #[tokio::test]
    async fn test_mock_validator_require_token() {
        let validator = MockValidator::new("test-user").require_token();

        // Empty token should fail
        let result = validator.validate("").await;
        assert!(result.is_err());

        // Non-empty token should succeed
        let result = validator.validate("some-token").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_auth_context_builder() {
        let auth = MockAuthContextBuilder::new()
            .user_id("builder-user")
            .tenant_id("builder-tenant")
            .scopes(vec!["scope1", "scope2"])
            .claim("custom", "value")
            .build();

        assert_eq!(auth.user_id(), "builder-user");
        assert_eq!(auth.tenant_id(), Some("builder-tenant"));
        assert!(auth.has_scope("scope1"));
        assert!(auth.has_scope("scope2"));

        let custom: Option<String> = auth.claim("custom");
        assert_eq!(custom, Some("value".to_string()));
    }

    #[test]
    fn test_from_config() {
        let config = MockValidatorConfig {
            default_user_id: "config-user".to_string(),
            default_tenant_id: Some("config-tenant".to_string()),
            default_scopes: vec!["read".to_string()],
            default_client_id: Some("config-client".to_string()),
            claims: serde_json::json!({"custom": "claim"}),
            always_authenticated: true,
        };

        let validator = MockValidator::from_config(config);
        assert_eq!(validator.user_id, "config-user");
        assert_eq!(validator.tenant_id, Some("config-tenant".to_string()));
        assert_eq!(validator.scopes, vec!["read".to_string()]);
    }
}
