//! Shared authentication middleware resolution for server-connecting commands.
//!
//! Converts an [`AuthMethod`] (produced by [`AuthFlags::resolve()`]) into
//! the HTTP middleware chain expected by `ServerTester`, `LoadTestEngine`,
//! and other consumers. This module centralizes the auth wiring so that
//! every command uses the same logic.

use std::sync::Arc;

use anyhow::Result;
use pmcp::client::http_middleware::HttpMiddlewareChain;
use pmcp::client::oauth::{default_cache_path, OAuthConfig, OAuthHelper};

use super::flags::AuthMethod;

/// Build an [`OAuthHelper`] from the fields of an [`AuthMethod::OAuth`] variant.
///
/// Centralizes `OAuthConfig` construction so that `resolve_auth_middleware`
/// and `resolve_auth_header` share a single code path.
fn build_oauth_helper(
    mcp_server_url: &str,
    client_id: &str,
    issuer: &Option<String>,
    scopes: &[String],
    no_cache: bool,
    redirect_port: u16,
) -> Result<OAuthHelper> {
    let cache_file = if no_cache {
        None
    } else {
        Some(default_cache_path())
    };
    let config = OAuthConfig {
        issuer: issuer.clone(),
        mcp_server_url: Some(mcp_server_url.to_string()),
        client_id: client_id.to_string(),
        scopes: scopes.to_vec(),
        cache_file,
        redirect_port,
    };
    OAuthHelper::new(config).map_err(|e| anyhow::anyhow!("OAuth setup failed: {e}"))
}

/// Convert an [`AuthMethod`] into an optional HTTP middleware chain.
///
/// Returns `Ok(None)` when no auth is configured. For API key auth, wraps
/// the key in a `BearerToken` middleware. For OAuth, runs the full PKCE
/// flow (or loads a cached token) via `OAuthHelper`.
///
/// Call this once at command startup -- the returned chain is `Arc`-wrapped
/// and safe to share across concurrent requests.
pub async fn resolve_auth_middleware(
    mcp_server_url: &str,
    auth_method: &AuthMethod,
) -> Result<Option<Arc<HttpMiddlewareChain>>> {
    match auth_method {
        AuthMethod::None => Ok(None),

        AuthMethod::ApiKey(key) => {
            use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};

            let bearer_token = BearerToken::new(key.clone());
            let middleware = OAuthClientMiddleware::new(bearer_token);
            let mut chain = HttpMiddlewareChain::new();
            chain.add(Arc::new(middleware));
            Ok(Some(Arc::new(chain)))
        },

        AuthMethod::OAuth {
            client_id,
            issuer,
            scopes,
            no_cache,
            redirect_port,
        } => {
            let helper = build_oauth_helper(
                mcp_server_url,
                client_id,
                issuer,
                scopes,
                *no_cache,
                *redirect_port,
            )?;
            let chain = helper
                .create_middleware_chain()
                .await
                .map_err(|e| anyhow::anyhow!("OAuth authentication failed: {e}"))?;
            Ok(Some(chain))
        },
    }
}

/// Resolve auth into an `Authorization` header value (e.g. `"Bearer sk-..."`).
///
/// For API key auth, returns the key as a bearer token. For OAuth, runs the
/// full PKCE flow (or loads a cached token) and returns the access token.
/// Returns `Ok(None)` when no auth is configured.
///
/// Use this for consumers that need a plain header string (preview, schema)
/// rather than an [`HttpMiddlewareChain`] (test check, loadtest).
pub async fn resolve_auth_header(
    mcp_server_url: &str,
    auth_method: &AuthMethod,
) -> Result<Option<String>> {
    match auth_method {
        AuthMethod::None => Ok(None),
        AuthMethod::ApiKey(key) => Ok(Some(format!("Bearer {}", key))),
        AuthMethod::OAuth {
            client_id,
            issuer,
            scopes,
            no_cache,
            redirect_port,
        } => {
            let helper = build_oauth_helper(
                mcp_server_url,
                client_id,
                issuer,
                scopes,
                *no_cache,
                *redirect_port,
            )?;
            let token = helper
                .get_access_token()
                .await
                .map_err(|e| anyhow::anyhow!("OAuth token acquisition failed: {e}"))?;
            Ok(Some(format!("Bearer {}", token)))
        },
    }
}
