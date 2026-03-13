//! Shared authentication middleware resolution for server-connecting commands.
//!
//! Converts an [`AuthMethod`] (produced by [`AuthFlags::resolve()`]) into
//! the HTTP middleware chain expected by `ServerTester`, `LoadTestEngine`,
//! and other consumers. This module centralizes the auth wiring so that
//! every command uses the same logic.

use std::sync::Arc;

use anyhow::Result;
use pmcp::client::http_middleware::HttpMiddlewareChain;

use super::flags::AuthMethod;

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
            use pmcp::client::oauth::{default_cache_path, OAuthConfig, OAuthHelper};

            let cache_file = if *no_cache {
                None
            } else {
                Some(default_cache_path())
            };

            let config = OAuthConfig {
                issuer: issuer.clone(),
                mcp_server_url: Some(mcp_server_url.to_string()),
                client_id: client_id.clone(),
                scopes: scopes.clone(),
                cache_file,
                redirect_port: *redirect_port,
            };

            let helper = OAuthHelper::new(config)
                .map_err(|e| anyhow::anyhow!("OAuth setup failed: {e}"))?;
            let chain = helper
                .create_middleware_chain()
                .await
                .map_err(|e| anyhow::anyhow!("OAuth authentication failed: {e}"))?;
            Ok(Some(chain))
        },
    }
}

/// Extract the API key string from an [`AuthMethod`], if present.
///
/// Returns `Some(key)` for the `ApiKey` variant, `None` otherwise.
/// Useful for consumers like `ServerTester::new()` that accept an
/// `api_key: Option<&str>` parameter alongside the middleware chain.
#[allow(dead_code)] // Used by Plan 03 when preview/schema/connect wire up auth
pub fn resolve_api_key(auth_method: &AuthMethod) -> Option<&str> {
    match auth_method {
        AuthMethod::ApiKey(key) => Some(key.as_str()),
        _ => None,
    }
}
