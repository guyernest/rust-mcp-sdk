//! Core authentication traits for flexible OAuth/auth integration.
//!
//! This module provides a provider-agnostic authentication abstraction for MCP servers.
//! The core design principle is that **your MCP server code should never know about OAuth
//! providers, tokens, or authentication flows. It only sees `AuthContext`.**
//!
//! # Provider Agnosticism
//!
//! The authentication system supports multiple OAuth providers (Cognito, Entra, Google,
//! Okta, Auth0, etc.) through configuration, not code changes. See [`ClaimMappings`] for
//! how provider-specific claim names are translated to standard names.
//!
//! # Example
//!
//! ```rust
//! use pmcp::server::auth::AuthContext;
//!
//! fn handle_request(auth: &AuthContext) -> Result<String, &'static str> {
//!     // Require authentication
//!     auth.require_auth()?;
//!
//!     // Check scopes
//!     auth.require_scope("read:data")?;
//!
//!     // Access user info (provider-agnostic)
//!     let user_id = auth.user_id();
//!     let email = auth.email().unwrap_or("unknown");
//!
//!     Ok(format!("Hello, {} ({})", email, user_id))
//! }
//! ```

use crate::error::Result;
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication context containing validated user information.
///
/// This is the **only** auth type your MCP code should interact with.
/// It provides a provider-agnostic view of the authenticated user, regardless
/// of whether the token came from Cognito, Entra, Google, Okta, or any other
/// OIDC provider.
///
/// # Provider-Agnostic Access
///
/// Use the helper methods like [`email()`](Self::email), [`tenant_id()`](Self::tenant_id),
/// and [`user_id()`](Self::user_id) instead of directly accessing claims. These methods
/// handle the different claim names used by various OAuth providers.
///
/// # Example
///
/// ```rust
/// use pmcp::server::auth::AuthContext;
///
/// fn get_user_greeting(auth: &AuthContext) -> String {
///     let name = auth.email().unwrap_or(auth.user_id());
///     format!("Welcome, {}!", name)
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthContext {
    /// Subject identifier (user ID from the `sub` claim).
    pub subject: String,

    /// Granted scopes/permissions.
    pub scopes: Vec<String>,

    /// Additional claims from the token.
    /// Use the helper methods like [`email()`](Self::email) for common claims.
    pub claims: HashMap<String, serde_json::Value>,

    /// Original token if available (for forwarding to downstream services).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    /// Client ID that authenticated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// Token expiration timestamp (Unix epoch seconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<u64>,

    /// Whether this context represents an authenticated user.
    #[serde(default)]
    pub authenticated: bool,
}

impl AuthContext {
    /// Create a new authenticated context.
    pub fn new(subject: impl Into<String>) -> Self {
        Self {
            subject: subject.into(),
            authenticated: true,
            ..Default::default()
        }
    }

    /// Create an anonymous (unauthenticated) context.
    pub fn anonymous() -> Self {
        Self {
            subject: "anonymous".to_string(),
            authenticated: false,
            ..Default::default()
        }
    }

    /// Get the user ID (alias for subject).
    ///
    /// This is the standard user identifier, typically from the `sub` claim.
    #[inline]
    pub fn user_id(&self) -> &str {
        &self.subject
    }

    /// Get a typed claim value.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::AuthContext;
    ///
    /// let auth = AuthContext::new("user-123");
    /// let roles: Option<Vec<String>> = auth.claim("roles");
    /// ```
    pub fn claim<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.claims
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Get the email address (handles different claim names across providers).
    ///
    /// This method checks common claim names used by different OAuth providers:
    /// - `email` (Cognito, Google, Okta, Auth0)
    /// - `preferred_username` (Entra ID)
    /// - `upn` (Entra ID UPN)
    pub fn email(&self) -> Option<&str> {
        self.claims
            .get("email")
            .or_else(|| self.claims.get("preferred_username"))
            .or_else(|| self.claims.get("upn"))
            .and_then(|v| v.as_str())
    }

    /// Get the display name.
    ///
    /// Checks common claim names for user's name:
    /// - `name` (most providers)
    /// - `given_name` + `family_name` fallback
    pub fn name(&self) -> Option<&str> {
        self.claims.get("name").and_then(|v| v.as_str())
    }

    /// Get the tenant ID (handles different claim names across providers).
    ///
    /// This method checks common claim names used by different OAuth providers:
    /// - `tenant_id` (normalized)
    /// - `tid` (Entra ID)
    /// - `custom:tenant_id` (Cognito custom attribute)
    /// - `custom:tenant` (Cognito custom attribute)
    /// - `org_id` (Auth0, Okta)
    pub fn tenant_id(&self) -> Option<&str> {
        self.claims
            .get("tenant_id")
            .or_else(|| self.claims.get("tid")) // Entra ID
            .or_else(|| self.claims.get("custom:tenant_id")) // Cognito
            .or_else(|| self.claims.get("custom:tenant")) // Cognito
            .or_else(|| self.claims.get("org_id")) // Auth0, Okta
            .and_then(|v| v.as_str())
    }

    /// Get groups/roles the user belongs to.
    ///
    /// Checks common claim names for group membership:
    /// - `groups` (Entra ID, Okta)
    /// - `cognito:groups` (Cognito)
    /// - `roles` (Auth0)
    pub fn groups(&self) -> Vec<String> {
        self.claims
            .get("groups")
            .or_else(|| self.claims.get("cognito:groups"))
            .or_else(|| self.claims.get("roles"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default()
    }

    /// Check if the context has a specific scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Check if the context has all specified scopes.
    pub fn has_all_scopes(&self, scopes: &[&str]) -> bool {
        scopes.iter().all(|scope| self.has_scope(scope))
    }

    /// Check if the context has any of the specified scopes.
    pub fn has_any_scope(&self, scopes: &[&str]) -> bool {
        scopes.iter().any(|scope| self.has_scope(scope))
    }

    /// Require a scope, returning an error message if missing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::AuthContext;
    ///
    /// fn protected_operation(auth: &AuthContext) -> Result<(), &'static str> {
    ///     auth.require_scope("write:data")?;
    ///     // ... perform operation
    ///     Ok(())
    /// }
    /// ```
    pub fn require_scope(&self, scope: &str) -> std::result::Result<(), &'static str> {
        if self.has_scope(scope) {
            Ok(())
        } else {
            Err("Insufficient scope")
        }
    }

    /// Require authentication, returning an error message if not authenticated.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pmcp::server::auth::AuthContext;
    ///
    /// fn protected_operation(auth: &AuthContext) -> Result<&str, &'static str> {
    ///     auth.require_auth()?;
    ///     Ok(auth.user_id())
    /// }
    /// ```
    pub fn require_auth(&self) -> std::result::Result<(), &'static str> {
        if self.authenticated {
            Ok(())
        } else {
            Err("Authentication required")
        }
    }

    /// Check if the token is expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            expires_at < now
        } else {
            false
        }
    }

    /// Check if the user is in a specific group.
    pub fn in_group(&self, group: &str) -> bool {
        self.groups().iter().any(|g| g == group)
    }
}

/// Claim mappings for translating provider-specific claims to standard names.
///
/// Different OAuth providers use different claim names for the same information.
/// This struct allows configuring the mapping from provider-specific names to
/// standard names used by `AuthContext`.
///
/// # Provider-Specific Claim Names
///
/// | Standard | Cognito | Entra ID | Google | Okta | Auth0 |
/// |----------|---------|----------|--------|------|-------|
/// | `user_id` | sub | oid | sub | uid | sub |
/// | `tenant_id` | `custom:tenant` | tid | N/A | `org_id` | `org_id` |
/// | email | email | `preferred_username` | email | email | email |
/// | groups | `cognito:groups` | groups | N/A | groups | roles |
///
/// # Example
///
/// ```rust
/// use pmcp::server::auth::ClaimMappings;
///
/// // Configure for Entra ID
/// let mappings = ClaimMappings::entra();
/// assert_eq!(mappings.user_id, "oid");
/// assert_eq!(mappings.tenant_id, Some("tid".to_string()));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimMappings {
    /// Claim name for user ID (default: "sub").
    #[serde(default = "default_user_id_claim")]
    pub user_id: String,

    /// Claim name for tenant ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,

    /// Claim name for email.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Claim name for display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Claim name for groups/roles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<String>,

    /// Additional custom mappings.
    #[serde(flatten)]
    pub custom: HashMap<String, String>,
}

fn default_user_id_claim() -> String {
    "sub".to_string()
}

impl Default for ClaimMappings {
    fn default() -> Self {
        Self {
            user_id: default_user_id_claim(),
            tenant_id: None,
            email: Some("email".to_string()),
            name: Some("name".to_string()),
            groups: None,
            custom: HashMap::new(),
        }
    }
}

impl ClaimMappings {
    /// Create claim mappings for AWS Cognito.
    pub fn cognito() -> Self {
        Self {
            user_id: "sub".to_string(),
            tenant_id: Some("custom:tenant_id".to_string()),
            email: Some("email".to_string()),
            name: Some("name".to_string()),
            groups: Some("cognito:groups".to_string()),
            custom: HashMap::new(),
        }
    }

    /// Create claim mappings for Microsoft Entra ID (Azure AD).
    pub fn entra() -> Self {
        Self {
            user_id: "oid".to_string(),
            tenant_id: Some("tid".to_string()),
            email: Some("preferred_username".to_string()),
            name: Some("name".to_string()),
            groups: Some("groups".to_string()),
            custom: HashMap::new(),
        }
    }

    /// Create claim mappings for Google Identity.
    pub fn google() -> Self {
        Self {
            user_id: "sub".to_string(),
            tenant_id: None, // Google doesn't have tenant concept
            email: Some("email".to_string()),
            name: Some("name".to_string()),
            groups: None,
            custom: HashMap::new(),
        }
    }

    /// Create claim mappings for Okta.
    pub fn okta() -> Self {
        Self {
            user_id: "uid".to_string(),
            tenant_id: Some("org_id".to_string()),
            email: Some("email".to_string()),
            name: Some("name".to_string()),
            groups: Some("groups".to_string()),
            custom: HashMap::new(),
        }
    }

    /// Create claim mappings for Auth0.
    pub fn auth0() -> Self {
        Self {
            user_id: "sub".to_string(),
            tenant_id: Some("org_id".to_string()),
            email: Some("email".to_string()),
            name: Some("name".to_string()),
            groups: Some("roles".to_string()),
            custom: HashMap::new(),
        }
    }

    /// Apply these mappings to normalize claims from a token.
    ///
    /// This transforms provider-specific claims into standard names that
    /// `AuthContext` helper methods can find.
    pub fn normalize_claims(
        &self,
        claims: &serde_json::Value,
    ) -> HashMap<String, serde_json::Value> {
        let mut normalized = HashMap::new();

        if let Some(obj) = claims.as_object() {
            // Copy all original claims
            for (key, value) in obj {
                normalized.insert(key.clone(), value.clone());
            }

            // Add normalized mappings
            if let Some(value) = obj.get(&self.user_id) {
                normalized.insert("sub".to_string(), value.clone());
            }
            if let Some(ref tenant_claim) = self.tenant_id {
                if let Some(value) = obj.get(tenant_claim) {
                    normalized.insert("tenant_id".to_string(), value.clone());
                }
            }
            if let Some(ref email_claim) = self.email {
                if let Some(value) = obj.get(email_claim) {
                    normalized.insert("email".to_string(), value.clone());
                }
            }
            if let Some(ref name_claim) = self.name {
                if let Some(value) = obj.get(name_claim) {
                    normalized.insert("name".to_string(), value.clone());
                }
            }
            if let Some(ref groups_claim) = self.groups {
                if let Some(value) = obj.get(groups_claim) {
                    normalized.insert("groups".to_string(), value.clone());
                }
            }

            // Apply custom mappings
            for (standard_name, provider_name) in &self.custom {
                if let Some(value) = obj.get(provider_name) {
                    normalized.insert(standard_name.clone(), value.clone());
                }
            }
        }

        normalized
    }
}

/// Core authentication provider trait.
/// This is the main abstraction that MCP servers use for authentication.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Validate an incoming request and extract authentication context.
    ///
    /// This method receives the authorization header value and should:
    /// 1. Parse the authentication token (e.g., Bearer token)
    /// 2. Validate the token
    /// 3. Return the authentication context if valid
    ///
    /// The `authorization_header` parameter contains the value of the Authorization header,
    /// if present (e.g., "Bearer eyJhbGci...")
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>>;

    /// Get the authentication scheme this provider uses (e.g., "Bearer", "Basic").
    fn auth_scheme(&self) -> &'static str {
        "Bearer"
    }

    /// Check if this provider requires authentication for all requests.
    fn is_required(&self) -> bool {
        true
    }
}

/// Token validator trait for validating access tokens.
#[async_trait]
pub trait TokenValidator: Send + Sync {
    /// Validate an access token and return token information.
    async fn validate(&self, token: &str) -> Result<AuthContext>;

    /// Optionally validate token with additional context (e.g., required scopes).
    async fn validate_with_context(
        &self,
        token: &str,
        required_scopes: Option<&[&str]>,
    ) -> Result<AuthContext> {
        let auth_context = self.validate(token).await?;

        // Check required scopes if specified
        if let Some(scopes) = required_scopes {
            if !auth_context.has_all_scopes(scopes) {
                return Err(crate::error::Error::protocol(
                    crate::error::ErrorCode::INVALID_REQUEST,
                    "Insufficient scopes",
                ));
            }
        }

        Ok(auth_context)
    }
}

/// Session management trait for stateful authentication.
#[async_trait]
pub trait SessionManager: Send + Sync {
    /// Create a new session and return the session ID.
    async fn create_session(&self, auth: AuthContext) -> Result<String>;

    /// Get session by ID.
    async fn get_session(&self, session_id: &str) -> Result<Option<AuthContext>>;

    /// Update an existing session.
    async fn update_session(&self, session_id: &str, auth: AuthContext) -> Result<()>;

    /// Invalidate a session.
    async fn invalidate_session(&self, session_id: &str) -> Result<()>;

    /// Clean up expired sessions (optional background task).
    async fn cleanup_expired(&self) -> Result<usize> {
        Ok(0) // Default no-op implementation
    }
}

/// Tool authorization trait for fine-grained access control.
#[async_trait]
pub trait ToolAuthorizer: Send + Sync {
    /// Check if the authenticated context can access a specific tool.
    async fn can_access_tool(&self, auth: &AuthContext, tool_name: &str) -> Result<bool>;

    /// Get required scopes for a tool.
    async fn required_scopes_for_tool(&self, tool_name: &str) -> Result<Vec<String>>;
}

/// Simple scope-based tool authorizer.
#[derive(Debug, Clone)]
pub struct ScopeBasedAuthorizer {
    tool_scopes: HashMap<String, Vec<String>>,
    default_scopes: Vec<String>,
}

impl ScopeBasedAuthorizer {
    /// Create a new scope-based authorizer.
    pub fn new() -> Self {
        Self {
            tool_scopes: HashMap::new(),
            default_scopes: vec!["mcp:tools:use".to_string()],
        }
    }

    /// Add required scopes for a tool.
    pub fn require_scopes<S, I>(mut self, tool_name: impl Into<String>, scopes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let scopes_vec = scopes.into_iter().map(|s| s.as_ref().to_string()).collect();
        self.tool_scopes.insert(tool_name.into(), scopes_vec);
        self
    }

    /// Set default required scopes for all tools.
    pub fn default_scopes(mut self, scopes: Vec<String>) -> Self {
        self.default_scopes = scopes;
        self
    }
}

#[async_trait]
impl ToolAuthorizer for ScopeBasedAuthorizer {
    async fn can_access_tool(&self, auth: &AuthContext, tool_name: &str) -> Result<bool> {
        let required_scopes = self
            .tool_scopes
            .get(tool_name)
            .unwrap_or(&self.default_scopes);

        let scope_refs: Vec<&str> = required_scopes.iter().map(|s| s.as_str()).collect();
        Ok(auth.has_all_scopes(&scope_refs))
    }

    async fn required_scopes_for_tool(&self, tool_name: &str) -> Result<Vec<String>> {
        Ok(self
            .tool_scopes
            .get(tool_name)
            .unwrap_or(&self.default_scopes)
            .clone())
    }
}

impl Default for ScopeBasedAuthorizer {
    fn default() -> Self {
        Self::new()
    }
}
