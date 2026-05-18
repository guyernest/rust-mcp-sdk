// Originated from pmcp-run/built-in/shared/mcp-server-common/src/auth.rs
// (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace for Phase 83 toolkit lift (P83-02).
//
// Note on shape divergence (documented as Plan 02 deviation):
// The mcp-server-common::auth module models OUTBOUND HTTP backend auth (apply
// auth to outgoing reqwest::HeaderMap), whereas `pmcp::server::auth::AuthProvider`
// models INBOUND request validation (`validate_request(authorization_header)`).
// These are different concerns and cannot be lifted byte-for-byte. Per
// 83-PATTERNS.md §3 (the authoritative shape spec for this lift target), the
// natural toolkit impl is `StaticAuthProvider { expected_token }` impling
// `pmcp::server::auth::AuthProvider`. The token-comparison body is structured
// after the mcp-server-common::auth::BearerAuth provider (the closest source
// analog) but reshaped for the inbound trait.

//! `AuthProvider` impls for the toolkit — bearer-token-based static auth suitable
//! for dev/test environments. Production callers should use pmcp's OAuth/JWT
//! providers instead.
//!
//! The headline type is [`StaticAuthProvider`], which validates inbound
//! `Authorization: Bearer <token>` headers against a single expected token.
//! Use it for tests, smoke deployments, and `cargo pmcp pentest`-style local
//! servers. **Never put a static bearer token in a production server.**

use async_trait::async_trait;
use pmcp::error::ErrorCode;
use pmcp::server::auth::{AuthContext, AuthProvider};
use pmcp::Result;

/// Static bearer-token auth provider, suitable for dev and tests.
///
/// Validates that incoming `Authorization` headers match exactly one configured
/// bearer token. Returns `Some(AuthContext)` with `subject = "static-bearer"`
/// on match, an `Err(ErrorCode::INVALID_REQUEST)` on token mismatch, and an
/// `Err(ErrorCode::INVALID_REQUEST)` on missing header (because
/// [`AuthProvider::is_required`] defaults to `true`).
///
/// # Example
/// ```no_run
/// use pmcp_server_toolkit::auth::StaticAuthProvider;
/// let provider = StaticAuthProvider::new("dev-token-do-not-use-in-prod");
/// # let _ = provider;
/// ```
///
/// # Security note
/// Token comparison uses [`constant_time_eq`] semantics via byte-wise XOR
/// accumulation to avoid timing-side-channel leaks during dev/test use.
/// Production callers should use pmcp's OAuth2 + JWT validator pipeline
/// instead.
pub struct StaticAuthProvider {
    /// The single expected bearer token. Compared in constant time.
    expected_token: String,
}

impl StaticAuthProvider {
    /// Create a new `StaticAuthProvider` that accepts exactly one bearer token.
    ///
    /// # Example
    /// ```no_run
    /// use pmcp_server_toolkit::auth::StaticAuthProvider;
    /// let provider = StaticAuthProvider::new("dev-token");
    /// # let _ = provider;
    /// ```
    pub fn new(expected_token: impl Into<String>) -> Self {
        Self {
            expected_token: expected_token.into(),
        }
    }
}

/// Constant-time byte comparison.
///
/// Returns `true` iff `a` and `b` have the same length AND every byte matches.
/// The function runs in time proportional to `max(a.len(), b.len())` and does
/// NOT short-circuit on the first mismatch. This blocks timing-side-channel
/// attacks against the bearer-token check.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[async_trait]
impl AuthProvider for StaticAuthProvider {
    async fn validate_request(
        &self,
        authorization_header: Option<&str>,
    ) -> Result<Option<AuthContext>> {
        // Missing header → unauthenticated. is_required() defaults true, so
        // the caller treats this as a 401.
        let header = match authorization_header {
            Some(h) => h,
            None => {
                return Err(pmcp::Error::protocol(
                    ErrorCode::INVALID_REQUEST,
                    "Missing Authorization header",
                ));
            }
        };

        // Strip the "Bearer " prefix (case-insensitive scheme name per RFC 6750).
        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .ok_or_else(|| {
                pmcp::Error::protocol(
                    ErrorCode::INVALID_REQUEST,
                    "Authorization scheme must be Bearer",
                )
            })?;

        if !constant_time_eq(token.as_bytes(), self.expected_token.as_bytes()) {
            return Err(pmcp::Error::protocol(
                ErrorCode::INVALID_REQUEST,
                "Invalid bearer token",
            ));
        }

        let mut ctx = AuthContext::new("static-bearer");
        ctx.token = Some(token.to_string());
        ctx.client_id = Some("static-bearer".to_string());
        Ok(Some(ctx))
    }

    fn auth_scheme(&self) -> &'static str {
        "Bearer"
    }

    fn is_required(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn valid_bearer_token_returns_some_auth_context() {
        let provider = StaticAuthProvider::new("secret-token");
        let result = provider
            .validate_request(Some("Bearer secret-token"))
            .await
            .expect("expected Ok");
        let ctx = result.expect("expected Some(AuthContext)");
        assert_eq!(ctx.user_id(), "static-bearer");
        assert!(ctx.authenticated);
    }

    #[tokio::test]
    async fn invalid_bearer_token_returns_err() {
        let provider = StaticAuthProvider::new("secret-token");
        let result = provider
            .validate_request(Some("Bearer wrong-token"))
            .await;
        assert!(result.is_err(), "expected Err for mismatched token");
    }

    #[tokio::test]
    async fn missing_authorization_header_returns_err() {
        let provider = StaticAuthProvider::new("secret-token");
        let result = provider.validate_request(None).await;
        assert!(result.is_err(), "expected Err for missing header");
    }

    #[tokio::test]
    async fn non_bearer_scheme_returns_err() {
        let provider = StaticAuthProvider::new("secret-token");
        let result = provider
            .validate_request(Some("Basic dXNlcjpwYXNz"))
            .await;
        assert!(result.is_err(), "expected Err for non-Bearer scheme");
    }

    #[tokio::test]
    async fn case_insensitive_bearer_prefix() {
        let provider = StaticAuthProvider::new("secret-token");
        let result = provider
            .validate_request(Some("bearer secret-token"))
            .await
            .expect("expected Ok");
        assert!(result.is_some());
    }

    #[test]
    fn constant_time_eq_handles_mismatched_lengths() {
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"x"));
    }

    #[test]
    fn constant_time_eq_handles_equal_inputs() {
        assert!(constant_time_eq(b"hunter2", b"hunter2"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn constant_time_eq_detects_mismatch() {
        assert!(!constant_time_eq(b"hunter2", b"hunter3"));
    }
}
