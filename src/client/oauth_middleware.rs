//! OAuth client middleware for automatic token injection and refresh.
//!
//! Provides HTTP middleware that handles OAuth token management:
//! - Automatic token injection into Authorization header
//! - Token expiry tracking
//! - 401/403 detection and token refresh
//!
//! This is a simplified implementation focused on bearer token injection.
//! For production use, consider the full OAuth implementation proposed in Issue #83.

use crate::client::http_middleware::{
    HttpMiddleware, HttpMiddlewareContext, HttpRequest, HttpResponse,
};
use crate::error::{Error, Result};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// OAuth bearer token
#[derive(Debug, Clone)]
pub struct BearerToken {
    /// The access token
    pub token: String,
    /// Token type (usually "Bearer")
    pub token_type: String,
    /// Optional expiration time
    pub expires_at: Option<SystemTime>,
}

impl BearerToken {
    /// Create a new bearer token
    pub fn new(token: String) -> Self {
        Self {
            token,
            token_type: "Bearer".to_string(),
            expires_at: None,
        }
    }

    /// Create a bearer token with expiration
    pub fn with_expiry(token: String, expires_in: Duration) -> Self {
        Self {
            token,
            token_type: "Bearer".to_string(),
            expires_at: Some(SystemTime::now() + expires_in),
        }
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            SystemTime::now() >= expires_at
        } else {
            false
        }
    }

    /// Check if token will expire soon (within threshold)
    pub fn expires_soon(&self, threshold: Duration) -> bool {
        if let Some(expires_at) = self.expires_at {
            if let Ok(remaining) = expires_at.duration_since(SystemTime::now()) {
                remaining < threshold
            } else {
                true // Already expired
            }
        } else {
            false
        }
    }

    /// Get the authorization header value
    pub fn to_header_value(&self) -> String {
        format!("{} {}", self.token_type, self.token)
    }
}

/// Simple OAuth client middleware for bearer token injection
///
/// This middleware:
/// - Automatically injects bearer tokens into the Authorization header
/// - Tracks token expiry (basic version)
/// - Detects 401/403 responses (for future refresh logic)
///
/// # Examples
///
/// ```rust
/// use pmcp::client::oauth_middleware::{OAuthClientMiddleware, BearerToken};
/// use pmcp::client::http_middleware::HttpMiddlewareChain;
/// use std::sync::Arc;
///
/// let token = BearerToken::new("my-api-token".to_string());
/// let oauth_middleware = OAuthClientMiddleware::new(token);
///
/// let mut chain = HttpMiddlewareChain::new();
/// chain.add(Arc::new(oauth_middleware));
/// ```
pub struct OAuthClientMiddleware {
    /// Current bearer token
    token: Arc<RwLock<BearerToken>>,
    /// Whether to check for token expiry before requests
    check_expiry: bool,
    /// Threshold for proactive token refresh
    refresh_threshold: Duration,
}

impl OAuthClientMiddleware {
    /// Create a new OAuth client middleware with a bearer token
    pub fn new(token: BearerToken) -> Self {
        Self {
            token: Arc::new(RwLock::new(token)),
            check_expiry: true,
            refresh_threshold: Duration::from_secs(60), // Refresh if <60s remaining
        }
    }

    /// Create OAuth middleware without expiry checking
    pub fn without_expiry_check(token: BearerToken) -> Self {
        Self {
            token: Arc::new(RwLock::new(token)),
            check_expiry: false,
            refresh_threshold: Duration::from_secs(60),
        }
    }

    /// Set the refresh threshold
    pub fn with_refresh_threshold(mut self, threshold: Duration) -> Self {
        self.refresh_threshold = threshold;
        self
    }

    /// Update the bearer token
    ///
    /// This can be called externally when a new token is obtained.
    pub fn update_token(&self, token: BearerToken) {
        *self.token.write() = token;
    }

    /// Get the current token
    pub fn get_token(&self) -> BearerToken {
        self.token.read().clone()
    }

    /// Check if the current token needs refresh
    fn needs_refresh(&self) -> bool {
        if !self.check_expiry {
            return false;
        }

        let token = self.token.read();
        token.is_expired() || token.expires_soon(self.refresh_threshold)
    }
}

impl std::fmt::Debug for OAuthClientMiddleware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthClientMiddleware")
            .field("check_expiry", &self.check_expiry)
            .field("refresh_threshold", &self.refresh_threshold)
            .field("token_expired", &self.token.read().is_expired())
            .finish()
    }
}

#[async_trait]
impl HttpMiddleware for OAuthClientMiddleware {
    async fn on_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        // OAuth Precedence Policy: Skip if auth already set by transport
        // Priority: transport auth_provider > HttpMiddleware OAuth > extra headers
        if context.get_metadata("auth_already_set").is_some() {
            tracing::debug!(
                "Skipping OAuth middleware - auth already set by transport auth_provider"
            );
            return Ok(());
        }

        // Skip if Authorization header already present (from higher-priority middleware or config)
        if request.has_header("Authorization") {
            tracing::warn!(
                "Authorization header already present - skipping OAuth middleware injection. \
                Check for duplicate auth configuration."
            );
            return Ok(());
        }

        // Check if token needs refresh
        if self.needs_refresh() {
            return Err(Error::authentication(
                "OAuth token expired or expiring soon - refresh required",
            ));
        }

        // Inject bearer token into Authorization header
        let token = self.token.read();
        request.add_header("Authorization", &token.to_header_value());

        tracing::trace!("OAuth token injected into Authorization header");

        Ok(())
    }

    async fn on_response(
        &self,
        response: &mut HttpResponse,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        // Detect authentication failures
        if response.status == 401 || response.status == 403 {
            // Record in context for potential retry logic
            context.set_metadata("auth_failure".to_string(), "true".to_string());
            context.set_metadata("status_code".to_string(), response.status.to_string());

            // Check if this is a retry scenario
            if context.get_metadata("oauth.retry_used").is_some() {
                tracing::warn!("Authentication failed after OAuth retry - token may be invalid");
            }

            return Err(Error::authentication(format!(
                "Authentication failed with status {}",
                response.status
            )));
        }

        Ok(())
    }

    async fn on_error(&self, error: &Error, context: &HttpMiddlewareContext) -> Result<()> {
        // Log authentication errors with context
        if matches!(error, Error::Authentication(_)) {
            tracing::error!(
                "OAuth authentication error for {} {}: {}",
                context.method,
                context.url,
                error
            );

            // If token was expired, log for monitoring
            if self.token.read().is_expired() {
                tracing::error!("OAuth token was expired at time of error");
            }
        }

        Ok(())
    }

    fn priority(&self) -> i32 {
        10 // High priority - run early
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bearer_token_creation() {
        let token = BearerToken::new("test-token-123".to_string());
        assert_eq!(token.token, "test-token-123");
        assert_eq!(token.token_type, "Bearer");
        assert!(token.expires_at.is_none());
        assert!(!token.is_expired());
    }

    #[test]
    fn test_bearer_token_with_expiry() {
        let token = BearerToken::with_expiry(
            "test-token".to_string(),
            Duration::from_secs(3600), // 1 hour
        );
        assert!(!token.is_expired());
        assert!(!token.expires_soon(Duration::from_secs(120))); // Not expiring in 2 min
    }

    #[test]
    fn test_bearer_token_header_value() {
        let token = BearerToken::new("abc123".to_string());
        assert_eq!(token.to_header_value(), "Bearer abc123");
    }

    #[test]
    fn test_oauth_middleware_creation() {
        let token = BearerToken::new("test-token".to_string());
        let middleware = OAuthClientMiddleware::new(token);
        assert!(middleware.check_expiry);
    }

    #[test]
    fn test_oauth_middleware_token_update() {
        let token1 = BearerToken::new("token1".to_string());
        let middleware = OAuthClientMiddleware::new(token1);

        let token2 = BearerToken::new("token2".to_string());
        middleware.update_token(token2);

        let current = middleware.get_token();
        assert_eq!(current.token, "token2");
    }

    #[tokio::test]
    async fn test_oauth_middleware_injects_header() {
        let token = BearerToken::new("my-secret-token".to_string());
        let middleware = OAuthClientMiddleware::new(token);

        let mut request =
            HttpRequest::new("POST".to_string(), "http://example.com".to_string(), vec![]);
        let context =
            HttpMiddlewareContext::new("http://example.com".to_string(), "POST".to_string());

        middleware.on_request(&mut request, &context).await.unwrap();

        assert_eq!(
            request.get_header("Authorization"),
            Some(&"Bearer my-secret-token".to_string())
        );
    }

    #[tokio::test]
    async fn test_oauth_middleware_detects_401() {
        let token = BearerToken::new("token".to_string());
        let middleware = OAuthClientMiddleware::new(token);

        let mut response = HttpResponse::new(401, vec![]);
        let context =
            HttpMiddlewareContext::new("http://example.com".to_string(), "GET".to_string());

        let result = middleware.on_response(&mut response, &context).await;
        assert!(result.is_err());
        assert_eq!(
            context.get_metadata("auth_failure"),
            Some("true".to_string())
        );
    }
}
