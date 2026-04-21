//! OAuth authentication support for CLI tools.
//!
//! This module implements multiple OAuth 2.0 flows for CLI authentication:
//! - Authorization Code Flow with PKCE (RFC 7636) - browser-based, most compatible
//! - Device Code Flow (RFC 8628) - fallback for servers that support it
//!
//! Supports automatic OAuth discovery via:
//! - OpenID Connect Discovery (/.well-known/openid-configuration)
//! - OAuth 2.0 Server Metadata (/.well-known/oauth-authorization-server)
//!
//! # Feature Gate
//!
//! This module is only available when the `oauth` feature is enabled:
//!
//! ```toml
//! pmcp = { version = "1.11", features = ["oauth"] }
//! ```

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::sleep;
use url::Url;

use crate::client::auth::{OidcDiscoveryClient, TokenExchangeClient};
use crate::client::http_middleware::HttpMiddlewareChain;
use crate::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
use crate::error::{Error, Result};
use crate::server::auth::oauth2::OidcDiscoveryMetadata;

// Re-export RFC 7591 DCR types from the authoritative server-side definitions
// so library users can construct DCR requests via `pmcp::client::oauth::DcrRequest`.
// Source of truth: src/server/auth/provider.rs:302-382.
pub use crate::server::auth::provider::{DcrRequest, DcrResponse};

/// OAuth configuration for CLI authentication flows.
///
/// # Migration note (pmcp 2.5.0, Phase 74)
///
/// The `client_id` field type changed `String` -> `Option<String>` to support
/// RFC 7591 Dynamic Client Registration. Existing callers that passed a
/// pre-registered id must now wrap it in `Some(...)`:
///
/// ```rust,ignore
/// // Before (pmcp < 2.5.0):
/// OAuthConfig { client_id: "my-client".to_string(), /* ... */ }
/// // After (pmcp 2.5.0+):
/// OAuthConfig {
///     client_id: Some("my-client".to_string()),
///     client_name: None,
///     dcr_enabled: false,
///     /* ... */
/// }
/// ```
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// OAuth issuer URL (e.g., `https://auth.example.com`).
    /// If `None`, will auto-discover from MCP server URL.
    pub issuer: Option<String>,
    /// MCP server URL for auto-discovery (required if issuer is `None`).
    pub mcp_server_url: Option<String>,
    /// OAuth client ID. When `None` and `dcr_enabled` is `true` and the
    /// discovery metadata advertises a `registration_endpoint`, the SDK
    /// auto-performs RFC 7591 Dynamic Client Registration to obtain one.
    pub client_id: Option<String>,
    /// Client name advertised to the authorization server during DCR
    /// (RFC 7591 §2). Only consulted when DCR fires. Falls back to the
    /// literal `"pmcp-sdk"` if `None` at DCR time.
    pub client_name: Option<String>,
    /// Enable RFC 7591 Dynamic Client Registration when `client_id` is
    /// `None` and the server advertises a `registration_endpoint`.
    /// Defaults to `true` via `Default::default()`; set to `false` to
    /// opt out and require an explicit `client_id`.
    pub dcr_enabled: bool,
    /// OAuth scopes to request.
    pub scopes: Vec<String>,
    /// Cache file path for storing tokens.
    pub cache_file: Option<PathBuf>,
    /// Redirect port for localhost callback (default: 8080).
    pub redirect_port: u16,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            issuer: None,
            mcp_server_url: None,
            client_id: None,
            client_name: None,
            dcr_enabled: true,
            scopes: Vec::new(),
            cache_file: None,
            redirect_port: 8080,
        }
    }
}

/// Result of a successful OAuth authorization flow, carrying the full set of
/// artifacts a cache consumer needs to persist and later refresh.
///
/// Introduced in pmcp 2.5.0 (Phase 74 Blocker #6) to unblock the multi-server
/// cache's refresh semantics (D-15 near-expiry refresh, D-16 force-refresh):
/// the previous `OAuthHelper::get_access_token` API only returned the raw
/// access_token string, making it impossible for `cargo pmcp auth login` to
/// persist a usable `refresh_token` / `expires_at` in the cache entry.
///
/// # Field semantics
///
/// - `access_token`: The bearer token. Put in `Authorization: Bearer <...>` headers.
/// - `refresh_token`: Present when the IdP returned one (Okta, Auth0, Keycloak
///   with offline_access). `None` when the IdP does not issue refresh tokens
///   (some public-PKCE flows). Pitfall 5 tracks this case.
///
///   **Device-code flow (RFC 8628) note (review MED-3):** When `OAuthHelper`
///   falls back from authorization-code to device-code (e.g., the IdP does
///   not support localhost callbacks or the caller explicitly requested
///   device flow), `refresh_token` may be `None` because RFC 8628 §3.5 does
///   NOT require the token response to include a `refresh_token`. Users
///   will need to re-run `cargo pmcp auth login` when the access_token
///   expires on such IdPs. `issuer` is still populated from discovery;
///   `client_id` is whatever was passed in `OAuthConfig` (or DCR-issued if
///   DCR fired); `expires_at` captures whatever `expires_in` the token
///   response provided (or `None` if absent).
/// - `expires_at`: Absolute unix seconds (not `expires_in` relative). `None`
///   when the IdP omitted `expires_in` from the token response.
/// - `scopes`: The scopes the IdP actually granted. May differ from
///   `config.scopes` (the requested scopes) when the server downgrades or
///   expands them.
/// - `issuer`: The effective issuer — caller-provided if present, else the
///   value discovered from `.well-known/openid-configuration`. Always `Some`
///   for a successful flow.
/// - `client_id`: The effective client_id — the DCR-issued id when DCR fired,
///   or the caller-provided value otherwise. Always populated.
#[derive(Debug, Clone)]
pub struct AuthorizationResult {
    /// Bearer access token.
    pub access_token: String,
    /// Refresh token, if the IdP issued one.
    pub refresh_token: Option<String>,
    /// Absolute expiration time (unix seconds).
    pub expires_at: Option<u64>,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Effective issuer (caller-provided or discovered).
    pub issuer: Option<String>,
    /// Effective client_id (DCR-issued or caller-provided).
    pub client_id: String,
}

/// Token cache stored on disk.
#[derive(Debug, Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    scopes: Vec<String>,
}

/// Device code authorization response.
#[derive(Debug, Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

/// Token response from the OAuth token endpoint.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    token_type: String,
}

/// OAuth helper for CLI authentication flows.
///
/// Supports both Authorization Code Flow with PKCE and Device Code Flow,
/// with automatic discovery of OAuth endpoints from the MCP server URL.
#[derive(Debug)]
pub struct OAuthHelper {
    config: OAuthConfig,
    client: reqwest::Client,
}

impl OAuthHelper {
    /// Create a new OAuth helper with the given configuration.
    pub fn new(config: OAuthConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self { config, client })
    }

    /// Perform RFC 7591 Dynamic Client Registration against `registration_endpoint`.
    ///
    /// D-04: `client_name` falls back to `"pmcp-sdk"` when `self.config.client_name` is `None`.
    /// D-05: body is a public PKCE shape — `token_endpoint_auth_method: "none"`, no secret requested.
    /// T-74-A: rejects non-`https://` endpoints except localhost/127.0.0.1/::1 to prevent
    /// discovery-spoofing.
    ///
    /// Review HIGH-1: `response_types: ["code"]` is REQUIRED per RFC 7591 §3.1 — pmcp.run's
    /// `ClientRegistrationRequest` parser requires this field; with `#[serde(skip_serializing_if =
    /// "Vec::is_empty")]` on the struct, an empty Vec would silently drop the field from the
    /// wire body.
    ///
    /// Review LOW-11: response body size is capped at 1 MiB to defend against a hostile
    /// registration_endpoint that streams a huge response.
    async fn do_dynamic_client_registration(
        &self,
        registration_endpoint: &str,
    ) -> Result<crate::server::auth::provider::DcrResponse> {
        // T-74-A scheme guard
        let parsed = Url::parse(registration_endpoint)
            .map_err(|e| Error::internal(format!("Invalid registration_endpoint URL: {e}")))?;
        // Review LOW-7 — IPv6 loopback ("::1") added to the allowlist alongside
        // "localhost" and "127.0.0.1". Note: `url::Url::host_str()` returns
        // IPv6 literals WITH brackets (e.g., `http://[::1]/register` ->
        // host_str() == Some("[::1]")), so we match both bracketed and raw forms.
        let scheme_ok = parsed.scheme() == "https"
            || (parsed.scheme() == "http"
                && matches!(
                    parsed.host_str(),
                    Some("localhost") | Some("127.0.0.1") | Some("::1") | Some("[::1]")
                ));
        if !scheme_ok {
            return Err(Error::internal(format!(
                "registration_endpoint must be https:// (or http://localhost, \
                 http://127.0.0.1, http://[::1]) — got {}",
                registration_endpoint
            )));
        }

        let client_name = self
            .config
            .client_name
            .clone()
            .unwrap_or_else(|| "pmcp-sdk".to_string());
        let redirect_uri = format!("http://localhost:{}/callback", self.config.redirect_port);

        let request = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec![redirect_uri],
            client_name: Some(client_name),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".to_string()),
            grant_types: vec!["authorization_code".to_string()],
            // Review HIGH-1 fix — RFC 7591 §3.1 requires response_types in the DCR body
            // (pmcp.run's ClientRegistrationRequest parser requires this field). Previously
            // vec![] which, combined with `#[serde(skip_serializing_if = "Vec::is_empty")]`,
            // DROPPED the field from the wire body and caused DCR to fail. Must be
            // vec!["code".to_string()] for the authorization-code public-PKCE flow.
            response_types: vec!["code".to_string()],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };

        let response = self
            .client
            .post(registration_endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::internal(format!("DCR request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::internal(format!(
                "DCR failed ({}): {}\n\n\
                 The server rejected dynamic client registration. Pass a \
                 pre-registered client_id to skip DCR.",
                status, body
            )));
        }

        // Review LOW-11 (Gemini) — defense-in-depth: cap DCR response body at 1 MiB
        // to mitigate DoS from a hostile registration_endpoint that streams a huge
        // response. reqwest does not have a direct bytes-limit API, so we read
        // the body as bytes, enforce the cap, then parse from slice.
        const MAX_DCR_RESPONSE_BYTES: usize = 1_048_576; // 1 MiB
        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::internal(format!("Failed to read DCR response body: {e}")))?;
        if bytes.len() > MAX_DCR_RESPONSE_BYTES {
            return Err(Error::internal(format!(
                "DCR response exceeds {} byte cap (got {} bytes) — refusing to parse",
                MAX_DCR_RESPONSE_BYTES,
                bytes.len()
            )));
        }
        serde_json::from_slice::<crate::server::auth::provider::DcrResponse>(&bytes)
            .map_err(|e| Error::internal(format!("Failed to parse DCR response: {e}")))
    }

    /// Resolve the `client_id` for the current OAuth flow, performing DCR
    /// lazily when eligible per D-03:
    ///   1. `self.config.dcr_enabled == true`
    ///   2. `self.config.client_id.is_none()`
    ///   3. `metadata.registration_endpoint.is_some()`
    ///
    /// Returns `Err` with an actionable message when DCR is needed but the
    /// server does not advertise a `registration_endpoint` (D-03 last clause).
    async fn resolve_client_id_for_flow(
        &self,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<String> {
        // Fast path: caller provided a client_id — use it verbatim, skip DCR entirely
        // (D-20 escape hatch).
        if let Some(ref id) = self.config.client_id {
            return Ok(id.clone());
        }

        if !self.config.dcr_enabled {
            return Err(Error::internal(
                "no client_id configured and dcr_enabled is false — \
                 provide OAuthConfig::client_id or enable dcr_enabled"
                    .to_string(),
            ));
        }

        match metadata.registration_endpoint.as_ref() {
            Some(endpoint) => {
                tracing::info!("Performing Dynamic Client Registration at {}", endpoint);
                let response = self.do_dynamic_client_registration(endpoint).await?;
                tracing::info!("DCR succeeded — issued client_id");
                Ok(response.client_id)
            },
            None => Err(Error::internal(
                "server does not support DCR — pass a pre-registered client_id"
                    .to_string(),
            )),
        }
    }

    /// Test-only hook: drive the discovery + DCR resolver path without invoking
    /// the browser PKCE flow. Used by `tests/oauth_dcr_integration.rs`.
    ///
    /// Review LOW-6 — narrowed from `cfg(any(test, feature = "oauth"))` to
    /// `cfg(test)` only. The previous gate exposed this test-only hook in
    /// release builds that enable the `oauth` feature, broadening the public
    /// API. Integration tests under `tests/` are compiled with the test cfg
    /// AND link against the `oauth` feature, so `#[cfg(test)]` alone is
    /// sufficient for their access.
    #[doc(hidden)]
    #[cfg(test)]
    pub async fn test_resolve_client_id_from_discovery(&self) -> Result<String> {
        let metadata = self.get_metadata().await?;
        self.resolve_client_id_for_flow(&metadata).await
    }

    /// Extract base URL from MCP server URL.
    ///
    /// For example, `https://api.example.com/mcp` becomes `https://api.example.com`.
    fn extract_base_url(mcp_url: &str) -> Result<String> {
        let parsed = Url::parse(mcp_url)
            .map_err(|e| Error::internal(format!("Invalid MCP server URL: {e}")))?;

        // Build base URL with scheme, host, and port
        let mut base = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""));
        if let Some(port) = parsed.port() {
            // Only add port if it's not the default for the scheme
            let is_default_port = (parsed.scheme() == "https" && port == 443)
                || (parsed.scheme() == "http" && port == 80);
            if !is_default_port {
                base.push_str(&format!(":{}", port));
            }
        }

        Ok(base)
    }

    /// Discover OAuth metadata from MCP server URL using OIDC discovery.
    async fn discover_metadata(&self, mcp_url: &str) -> Result<OidcDiscoveryMetadata> {
        let base_url = Self::extract_base_url(mcp_url)?;

        tracing::info!("Discovering OAuth configuration from {}...", base_url);

        let discovery_client = OidcDiscoveryClient::new();

        match discovery_client.discover(&base_url).await {
            Ok(metadata) => {
                tracing::info!("OAuth discovery successful");
                tracing::debug!("Issuer: {}", metadata.issuer);
                if let Some(ref device_endpoint) = metadata.device_authorization_endpoint {
                    tracing::debug!("Device endpoint: {}", device_endpoint);
                }
                Ok(metadata)
            },
            Err(e) => Err(Error::internal(format!(
                "Failed to discover OAuth configuration at {}: {}\n\
                 \n\
                 Please provide --oauth-issuer explicitly, or ensure the server\n\
                 exposes OAuth metadata at {}/.well-known/openid-configuration",
                base_url, e, base_url
            ))),
        }
    }

    /// Get OAuth metadata (either by discovering or constructing from issuer).
    async fn get_metadata(&self) -> Result<OidcDiscoveryMetadata> {
        if let Some(ref mcp_url) = self.config.mcp_server_url {
            // Discover from MCP server URL
            self.discover_metadata(mcp_url).await
        } else if let Some(ref issuer) = self.config.issuer {
            // Manually provided issuer - try to discover from it
            tracing::info!("Discovering OAuth configuration from {}...", issuer);

            let discovery_client = OidcDiscoveryClient::new();
            match discovery_client.discover(issuer).await {
                Ok(metadata) => {
                    tracing::info!("OAuth discovery successful");
                    Ok(metadata)
                },
                Err(e) => Err(Error::internal(format!(
                    "Failed to discover OAuth configuration from issuer {}: {}\n\
                     \n\
                     Please ensure the issuer URL exposes OAuth metadata at\n\
                     {}/.well-known/openid-configuration",
                    issuer, e, issuer
                ))),
            }
        } else {
            Err(Error::internal(
                "Either oauth_issuer or mcp_server_url must be provided for OAuth authentication"
                    .to_string(),
            ))
        }
    }

    /// Get or refresh access token, performing OAuth flow if needed.
    ///
    /// For callers that only need a bearer-header value. Cache consumers that
    /// need to persist `refresh_token` / `expires_at` / `issuer` across runs
    /// should use `authorize_with_details()` instead (Phase 74 Blocker #6).
    pub async fn get_access_token(&self) -> Result<String> {
        // Try to load cached token first
        if let Some(ref cache_file) = self.config.cache_file {
            if let Ok(cached) = self.load_cached_token(cache_file).await {
                // Check if token is still valid
                if let Some(expires_at) = cached.expires_at {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if now < expires_at {
                        tracing::info!("Using cached OAuth token");
                        return Ok(cached.access_token);
                    }
                }

                // Try to refresh if we have a refresh token
                if let Some(refresh_token) = cached.refresh_token {
                    tracing::warn!("OAuth token expired, refreshing...");
                    if let Ok(new_token) = self.refresh_token(&refresh_token).await {
                        self.cache_token(&new_token, cache_file).await?;
                        return Ok(new_token.access_token);
                    }
                }
            }
        }

        // No valid cached token, try authorization code flow first
        tracing::info!("No cached token found, starting OAuth flow...");

        // Get metadata to see what flows are supported
        let metadata = self.get_metadata().await?;

        // Try authorization code flow first (more common, works with MCP Inspector-like servers)
        match self.authorization_code_flow(&metadata).await {
            Ok(token) => Ok(token),
            Err(e) => {
                tracing::warn!("Authorization code flow failed: {}", e);

                // Fall back to device code flow if available
                if metadata.device_authorization_endpoint.is_some() {
                    tracing::info!("Trying device code flow...");
                    return self.device_code_flow_with_metadata(&metadata).await;
                }
                Err(Error::internal(
                    "No supported OAuth flow available.\n\
                     \n\
                     The server must support either:\n\
                     - Authorization code flow (authorization_endpoint), or\n\
                     - Device code flow (device_authorization_endpoint)"
                        .to_string(),
                ))
            },
        }
    }

    /// Like `get_access_token` but returns the full authorization result for
    /// cache persistence.
    ///
    /// Cache callers (e.g., `cargo pmcp auth login`) should prefer this method;
    /// simple callers that just need a bearer header can keep using
    /// `get_access_token`.
    ///
    /// Drives DCR lazily per D-03; runs PKCE via the authorization_code flow;
    /// captures `refresh_token`, `expires_at`, `scopes`, and the effective
    /// issuer + client_id.
    ///
    /// # Device-code fallback note (review MED-3)
    ///
    /// If the authorization-code flow fails and the server advertises a
    /// device_authorization_endpoint, this method falls back to device code
    /// flow (RFC 8628). In that case, `refresh_token` may be `None` since
    /// RFC 8628 §3.5 does not require it, and `scopes` falls back to the
    /// requested scopes when the token response does not echo them.
    pub async fn authorize_with_details(&self) -> Result<AuthorizationResult> {
        let metadata = self.get_metadata().await?;

        // Effective issuer: prefer the caller-provided config.issuer; fall back
        // to discovery metadata.issuer. metadata.issuer is always populated by
        // OIDC-compliant servers.
        let effective_issuer = self
            .config
            .issuer
            .clone()
            .or_else(|| Some(metadata.issuer.clone()));

        // Try authorization code flow first (returns the full TokenResponse).
        match self.authorization_code_flow_inner(&metadata).await {
            Ok((token_response, resolved_client_id)) => {
                Ok(Self::build_auth_result(
                    token_response,
                    resolved_client_id,
                    effective_issuer,
                    &self.config.scopes,
                ))
            },
            Err(e) => {
                tracing::warn!("Authorization code flow failed: {}", e);

                // Fall back to device code flow if available — but device flow
                // only returns an access_token String via the legacy path, not
                // a full TokenResponse. For now, device-flow callers that need
                // full artifacts should use authorization-code flow instead.
                // See MED-3 rustdoc note above.
                if metadata.device_authorization_endpoint.is_some() {
                    tracing::info!(
                        "Trying device code flow (refresh_token may be None per RFC 8628)..."
                    );
                    // Resolve client_id the same way authorization_code would.
                    let resolved_client_id =
                        self.resolve_client_id_for_flow(&metadata).await?;
                    let access_token =
                        self.device_code_flow_with_metadata(&metadata).await?;
                    // Device flow returns only the access_token — populate what
                    // we know, leave refresh_token/expires_at/scopes at defaults.
                    return Ok(AuthorizationResult {
                        access_token,
                        refresh_token: None,
                        expires_at: None,
                        scopes: self.config.scopes.clone(),
                        issuer: effective_issuer,
                        client_id: resolved_client_id,
                    });
                }
                Err(Error::internal(
                    "No supported OAuth flow available.\n\
                     \n\
                     The server must support either:\n\
                     - Authorization code flow (authorization_endpoint), or\n\
                     - Device code flow (device_authorization_endpoint)"
                        .to_string(),
                ))
            },
        }
    }

    /// Helper to construct an `AuthorizationResult` from a TokenResponse.
    fn build_auth_result(
        token_response: crate::client::auth::TokenResponse,
        client_id: String,
        effective_issuer: Option<String>,
        requested_scopes: &[String],
    ) -> AuthorizationResult {
        // Convert `expires_in` (relative seconds) to `expires_at` (absolute unix seconds).
        let expires_at = token_response.expires_in.map(|ttl| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now + ttl
        });

        // If the server returned a `scope` string, split it; else fall back to
        // the requested scopes.
        let granted_scopes = token_response
            .scope
            .as_deref()
            .map(|s| s.split_whitespace().map(String::from).collect::<Vec<_>>())
            .unwrap_or_else(|| requested_scopes.to_vec());

        AuthorizationResult {
            access_token: token_response.access_token,
            refresh_token: token_response.refresh_token,
            expires_at,
            scopes: granted_scopes,
            issuer: effective_issuer,
            client_id,
        }
    }

    /// Generate PKCE code verifier (RFC 7636).
    fn generate_code_verifier() -> String {
        let random_bytes: [u8; 32] = rand::rng().random();
        URL_SAFE_NO_PAD.encode(random_bytes)
    }

    /// Generate PKCE code challenge from verifier (RFC 7636).
    fn generate_code_challenge(verifier: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        URL_SAFE_NO_PAD.encode(hash)
    }

    /// Perform OAuth authorization code flow with PKCE (public wrapper).
    ///
    /// Returns just the access token for the simple `get_access_token` caller.
    /// Full artifacts (refresh_token, expires_at, scopes, issuer, client_id) are
    /// available through `authorize_with_details()` via `authorization_code_flow_inner`.
    async fn authorization_code_flow(&self, metadata: &OidcDiscoveryMetadata) -> Result<String> {
        let (token_response, _client_id) = self.authorization_code_flow_inner(metadata).await?;
        Ok(token_response.access_token)
    }

    /// Inner PKCE authorization code flow returning the full token response.
    ///
    /// Returns (TokenResponse, resolved_client_id) so `authorize_with_details`
    /// can populate `AuthorizationResult` fields including refresh_token,
    /// expires_at, scopes, and the effective client_id.
    async fn authorization_code_flow_inner(
        &self,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<(crate::client::auth::TokenResponse, String)> {
        tracing::info!("Starting OAuth authorization code flow...");

        // Resolve client_id via DCR-aware resolver (D-03). Fires DCR lazily when
        // config.client_id is None, dcr_enabled is true, and the server advertises
        // a registration_endpoint.
        let resolved_client_id = self.resolve_client_id_for_flow(metadata).await?;

        // Generate PKCE challenge
        let code_verifier = Self::generate_code_verifier();
        let code_challenge = Self::generate_code_challenge(&code_verifier);

        // Start local callback server on configured port
        let redirect_port = self.config.redirect_port;
        let redirect_uri = format!("http://localhost:{}/callback", redirect_port);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", redirect_port))
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to bind to localhost:{}.\n\
                     \n\
                     This port may already be in use. Try a different port with:\n\
                     --oauth-redirect-port PORT\n\
                     \n\
                     Error: {e}",
                    redirect_port
                ))
            })?;

        tracing::debug!("Local callback server listening on port {}", redirect_port);
        tracing::warn!(
            "Ensure the redirect URI is registered in your OAuth provider: {}",
            redirect_uri
        );

        // Build authorization URL
        let mut auth_url = Url::parse(&metadata.authorization_endpoint)
            .map_err(|e| Error::internal(format!("Invalid authorization endpoint: {e}")))?;

        auth_url
            .query_pairs_mut()
            .append_pair("client_id", &resolved_client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("scope", &self.config.scopes.join(" "))
            .append_pair("code_challenge", &code_challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &Self::generate_code_verifier()); // Random state for CSRF protection

        tracing::info!("OAuth Authentication Required");
        tracing::info!("Opening browser for authentication...");
        tracing::info!("If the browser doesn't open, visit: {}", auth_url.as_str());

        // Open browser
        if let Err(e) = webbrowser::open(auth_url.as_str()) {
            tracing::warn!(
                "Failed to open browser: {}. Please open the URL manually.",
                e
            );
        }

        // Wait for OAuth callback
        let (tx, rx) = oneshot::channel();
        let callback_task = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut reader = BufReader::new(&mut stream);
                let mut request_line = String::new();

                if reader.read_line(&mut request_line).await.is_ok() {
                    // Parse the request line to extract the authorization code
                    if let Some(path) = request_line.split_whitespace().nth(1) {
                        if let Ok(callback_url) = Url::parse(&format!("http://localhost{}", path)) {
                            let code = callback_url
                                .query_pairs()
                                .find(|(key, _)| key == "code")
                                .map(|(_, value)| value.to_string());

                            // Send success response to browser
                            let response = if code.is_some() {
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                 <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
                                 <h1 style='color: green;'>Authentication Successful!</h1>\
                                 <p>You can close this window and return to the terminal.</p>\
                                 </body></html>"
                            } else {
                                "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
                                 <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
                                 <h1 style='color: red;'>Authentication Failed</h1>\
                                 <p>No authorization code received. Please try again.</p>\
                                 </body></html>"
                            };

                            let _ = stream.write_all(response.as_bytes()).await;
                            let _ = stream.flush().await;

                            if let Some(code) = code {
                                let _ = tx.send(code);
                            }
                        }
                    }
                }
            }
        });

        tracing::info!("Waiting for authorization...");

        // Wait for callback with timeout
        let authorization_code = tokio::time::timeout(Duration::from_secs(300), rx)
            .await
            .map_err(|_| {
                Error::internal("Timeout waiting for OAuth callback (5 minutes)".to_string())
            })?
            .map_err(|e| Error::internal(format!("OAuth callback channel error: {e}")))?;

        callback_task.abort();

        tracing::info!("Authorization code received");

        // Exchange authorization code for access token
        tracing::debug!("Exchanging authorization code for access token...");

        let token_exchange = TokenExchangeClient::new();
        let token_response = token_exchange
            .exchange_code(
                &metadata.token_endpoint,
                &authorization_code,
                &resolved_client_id,
                None, // No client secret for public clients
                &redirect_uri,
                Some(&code_verifier), // PKCE verifier
            )
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to exchange authorization code for token: {e}"
                ))
            })?;

        tracing::info!("Authentication successful");

        // Cache the token
        if let Some(ref cache_file) = self.config.cache_file {
            self.cache_token_from_response(&token_response, cache_file)
                .await?;
        }

        Ok((token_response, resolved_client_id))
    }

    /// Perform OAuth device code flow (with pre-fetched metadata).
    async fn device_code_flow_with_metadata(
        &self,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<String> {
        tracing::info!("Starting OAuth device code flow...");

        // Check if device flow is supported
        let device_auth_endpoint =
            metadata
                .device_authorization_endpoint
                .as_ref()
                .ok_or_else(|| {
                    Error::internal(
                        "Device authorization endpoint not found in OAuth metadata.\n\
                         \n\
                         The OAuth server does not support device code flow (RFC 8628)."
                            .to_string(),
                    )
                })?;

        // Rest of device code flow implementation...
        self.device_code_flow_internal(metadata, device_auth_endpoint)
            .await
    }

    /// Internal implementation of device code flow.
    async fn device_code_flow_internal(
        &self,
        metadata: &OidcDiscoveryMetadata,
        device_auth_endpoint: &str,
    ) -> Result<String> {
        // Resolve client_id via DCR-aware resolver (D-03).
        let resolved_client_id = self.resolve_client_id_for_flow(metadata).await?;

        // Step 1: Request device code
        let scope = self.config.scopes.join(" ");

        let response = self
            .client
            .post(device_auth_endpoint)
            .form(&[
                ("client_id", resolved_client_id.as_str()),
                ("scope", &scope),
            ])
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to request device code: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Device authorization failed: {}",
                response.text().await.unwrap_or_default()
            )));
        }

        let device_auth: DeviceAuthResponse = response.json().await.map_err(|e| {
            Error::internal(format!(
                "Failed to parse device authorization response: {e}"
            ))
        })?;

        // Step 2: Display user code and verification URL
        tracing::info!("OAuth device code flow");
        tracing::info!("1. Visit: {}", device_auth.verification_uri);
        tracing::info!("2. Enter code: {}", device_auth.user_code);

        if let Some(complete_uri) = &device_auth.verification_uri_complete {
            tracing::info!("Or visit directly: {}", complete_uri);
        }

        // Step 3: Poll for token
        let poll_interval = Duration::from_secs(device_auth.interval.unwrap_or(5));
        let token_endpoint = &metadata.token_endpoint;
        let expires_at = SystemTime::now() + Duration::from_secs(device_auth.expires_in);

        loop {
            if SystemTime::now() > expires_at {
                return Err(Error::internal(
                    "Device code expired. Please try again.".to_string(),
                ));
            }

            sleep(poll_interval).await;
            tracing::debug!("Polling for authorization...");

            let response = self
                .client
                .post(token_endpoint)
                .form(&[
                    ("client_id", resolved_client_id.as_str()),
                    ("device_code", &device_auth.device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .map_err(|e| Error::internal(format!("Failed to poll for token: {e}")))?;

            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|e| Error::internal(format!("Failed to read token response body: {e}")))?;

            if status.is_success() {
                let token_response: TokenResponse = serde_json::from_str(&body)
                    .map_err(|e| Error::internal(format!("Failed to parse token response: {e}")))?;

                tracing::info!("Authentication successful");

                // Cache the token
                if let Some(ref cache_file) = self.config.cache_file {
                    self.cache_token(&token_response, cache_file).await?;
                }

                return Ok(token_response.access_token);
            }

            // Check error response
            if let Ok(error) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(error_code) = error.get("error").and_then(|e| e.as_str()) {
                    match error_code {
                        "authorization_pending" => continue,
                        "slow_down" => {
                            sleep(poll_interval).await;
                            continue;
                        },
                        "access_denied" => {
                            return Err(Error::internal("User denied authorization".to_string()));
                        },
                        "expired_token" => {
                            return Err(Error::internal("Device code expired".to_string()));
                        },
                        _ => {
                            return Err(Error::internal(format!("OAuth error: {}", error_code)));
                        },
                    }
                }
            }
        }
    }

    /// Refresh an existing token.
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        let metadata = self.get_metadata().await?;
        let token_endpoint = &metadata.token_endpoint;

        // Refresh requires a previously-established client_id — DCR is not
        // re-run on refresh (cached entry implies we already have one).
        let client_id = self.config.client_id.as_deref().ok_or_else(|| {
            Error::internal("cannot refresh token without a cached client_id".to_string())
        })?;

        let response = self
            .client
            .post(token_endpoint)
            .form(&[
                ("client_id", client_id),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to refresh token: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Token refresh failed: {}",
                response.text().await.unwrap_or_default()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| Error::internal(format!("Failed to parse token response: {e}")))
    }

    /// Load cached token from disk.
    async fn load_cached_token(&self, cache_file: &PathBuf) -> Result<TokenCache> {
        let content = tokio::fs::read_to_string(cache_file)
            .await
            .map_err(|e| Error::internal(format!("Failed to read token cache: {e}")))?;
        serde_json::from_str(&content)
            .map_err(|e| Error::internal(format!("Failed to parse token cache: {e}")))
    }

    /// Cache token to disk.
    async fn cache_token(&self, token: &TokenResponse, cache_file: &PathBuf) -> Result<()> {
        let expires_at = token.expires_in.map(|secs| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + secs
        });

        let cache = TokenCache {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_at,
            scopes: self.config.scopes.clone(),
        };

        // Ensure directory exists
        if let Some(parent) = cache_file.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| Error::internal(format!("Failed to create cache directory: {e}")))?;
        }

        let json = serde_json::to_string_pretty(&cache)
            .map_err(|e| Error::internal(format!("Failed to serialize cache: {e}")))?;
        tokio::fs::write(cache_file, json)
            .await
            .map_err(|e| Error::internal(format!("Failed to write token cache: {e}")))?;

        tracing::debug!("Token cached to: {}", cache_file.display());

        Ok(())
    }

    /// Cache token from the SDK's auth `TokenResponse` type.
    async fn cache_token_from_response(
        &self,
        token: &crate::client::auth::TokenResponse,
        cache_file: &PathBuf,
    ) -> Result<()> {
        // Convert to internal TokenResponse
        let internal_token = TokenResponse {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_in: token.expires_in,
            token_type: token.token_type.clone(),
        };
        self.cache_token(&internal_token, cache_file).await
    }

    /// Create HTTP middleware chain with OAuth bearer token.
    ///
    /// Obtains an access token (from cache, refresh, or interactive flow)
    /// and wraps it in a middleware chain suitable for HTTP transports.
    pub async fn create_middleware_chain(&self) -> Result<Arc<HttpMiddlewareChain>> {
        let access_token = self.get_access_token().await?;

        tracing::debug!(
            "Creating OAuth middleware with token: {}...",
            &access_token[..access_token.len().min(20)]
        );

        let bearer_token = BearerToken::new(access_token);
        let oauth_middleware = OAuthClientMiddleware::new(bearer_token);

        let mut chain = HttpMiddlewareChain::new();
        chain.add(Arc::new(oauth_middleware));

        tracing::info!("OAuth middleware added to chain");

        Ok(Arc::new(chain))
    }
}

/// Get default cache file path (`~/.pmcp/oauth-tokens.json`).
///
/// Uses the user's home directory to store cached OAuth tokens.
/// Falls back to the current directory if the home directory cannot be determined.
pub fn default_cache_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".pmcp");
    path.push("oauth-tokens.json");
    path
}

/// Create an OAuth middleware chain from configuration.
///
/// This is a one-liner convenience for tools that just need a middleware chain:
/// ```no_run
/// # use pmcp::client::oauth::{OAuthConfig, create_oauth_middleware};
/// # async fn example() -> pmcp::Result<()> {
/// let config = OAuthConfig {
///     issuer: Some("https://auth.example.com".to_string()),
///     mcp_server_url: None,
///     client_id: Some("my-client".to_string()),
///     client_name: None,
///     dcr_enabled: false,
///     scopes: vec!["openid".to_string()],
///     cache_file: None,
///     redirect_port: 8080,
/// };
/// let chain = create_oauth_middleware(config).await?;
/// // Pass chain to HttpClient or transport
/// # Ok(())
/// # }
/// ```
pub async fn create_oauth_middleware(config: OAuthConfig) -> Result<Arc<HttpMiddlewareChain>> {
    let helper = OAuthHelper::new(config)?;
    helper.create_middleware_chain().await
}

#[cfg(test)]
mod oauth_config_tests {
    use super::*;

    #[test]
    fn oauth_config_default_has_dcr_enabled_and_none_client_id() {
        let c = OAuthConfig::default();
        assert!(
            c.client_id.is_none(),
            "default client_id must be None for DCR auto-fire"
        );
        assert!(c.dcr_enabled, "default dcr_enabled must be true");
        assert!(c.client_name.is_none(), "default client_name is None");
    }

    #[test]
    fn oauth_config_struct_literal_with_some_client_id_compiles() {
        let _c = OAuthConfig {
            issuer: None,
            mcp_server_url: Some("https://x.example".into()),
            client_id: Some("my-client".into()),
            client_name: None,
            dcr_enabled: false,
            scopes: vec![],
            cache_file: None,
            redirect_port: 8080,
        };
    }

    #[test]
    fn dcr_types_are_reexported() {
        // Compile-only: verifies pub use lands `DcrRequest` / `DcrResponse`
        // at `pmcp::client::oauth::*` per D-01.
        let _r: super::DcrRequest = super::DcrRequest {
            redirect_uris: vec!["http://localhost:8080/callback".into()],
            client_name: Some("test".into()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".into()),
            grant_types: vec!["authorization_code".into()],
            response_types: vec![],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };
        let _rsp = super::DcrResponse {
            client_id: "x".into(),
            client_secret: None,
            client_secret_expires_at: None,
            registration_access_token: None,
            registration_client_uri: None,
            token_endpoint_auth_method: None,
            extra: Default::default(),
        };
    }
}

#[cfg(test)]
mod dcr_tests {
    use super::*;
    use crate::server::auth::oauth2::OidcDiscoveryMetadata;

    /// Construct an OidcDiscoveryMetadata with only the fields we care about
    /// for DCR tests. OidcDiscoveryMetadata does NOT implement Default, so we
    /// provide all required fields explicitly.
    fn metadata(reg: Option<&str>) -> OidcDiscoveryMetadata {
        OidcDiscoveryMetadata {
            issuer: "https://issuer.example".into(),
            authorization_endpoint: "https://issuer.example/auth".into(),
            token_endpoint: "https://issuer.example/token".into(),
            jwks_uri: None,
            userinfo_endpoint: None,
            registration_endpoint: reg.map(String::from),
            revocation_endpoint: None,
            introspection_endpoint: None,
            device_authorization_endpoint: None,
            response_types_supported: vec![],
            grant_types_supported: vec![],
            scopes_supported: vec![],
            token_endpoint_auth_methods_supported: vec![],
            code_challenge_methods_supported: vec![],
        }
    }

    #[tokio::test]
    async fn dcr_skipped_when_client_id_provided() {
        let cfg = OAuthConfig {
            client_id: Some("preset".into()),
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let resolved = helper
            .resolve_client_id_for_flow(&metadata(Some("https://x/register")))
            .await
            .unwrap();
        assert_eq!(resolved, "preset");
    }

    #[tokio::test]
    async fn dcr_skipped_when_dcr_disabled_with_client_id() {
        let cfg = OAuthConfig {
            client_id: Some("preset".into()),
            dcr_enabled: false,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let resolved = helper
            .resolve_client_id_for_flow(&metadata(None))
            .await
            .unwrap();
        assert_eq!(resolved, "preset");
    }

    #[tokio::test]
    async fn dcr_needed_but_unsupported_errors_with_actionable_message() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .resolve_client_id_for_flow(&metadata(None))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("server does not support DCR"),
            "expected actionable DCR-missing message, got: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_needed_but_disabled_errors_when_client_id_none() {
        let cfg = OAuthConfig {
            client_id: None,
            dcr_enabled: false,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .resolve_client_id_for_flow(&metadata(Some("https://x/register")))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("dcr_enabled is false"),
            "expected dcr_enabled=false error, got: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_rejects_http_non_localhost_endpoint() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://attacker.example/register")
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("must be https"), "got: {msg}");
    }

    #[test]
    fn dcr_request_body_matches_rfc7591_public_pkce_shape() {
        let req = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec!["http://localhost:8080/callback".into()],
            client_name: Some("pmcp-sdk".into()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".into()),
            grant_types: vec!["authorization_code".into()],
            response_types: vec![],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["client_name"], "pmcp-sdk");
        assert_eq!(
            v["redirect_uris"],
            serde_json::json!(["http://localhost:8080/callback"])
        );
        assert_eq!(v["grant_types"], serde_json::json!(["authorization_code"]));
        assert_eq!(v["token_endpoint_auth_method"], "none");
    }

    #[test]
    fn dcr_request_body_contains_response_types_code() {
        // Review HIGH-1 regression guard — RFC 7591 §3.1 + pmcp.run's
        // ClientRegistrationRequest parser require response_types.
        // This is a serde-level guard that fires if the DcrRequest struct
        // or its serde attributes ever change in a way that drops the field.
        let req = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec!["http://localhost:8080/callback".into()],
            client_name: Some("pmcp-sdk".into()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".into()),
            grant_types: vec!["authorization_code".into()],
            response_types: vec!["code".into()],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert!(
            s.contains(r#""response_types":["code"]"#),
            "RFC 7591 §3.1 response_types missing from wire body: {s}"
        );
    }

    #[tokio::test]
    async fn dcr_accepts_ipv6_loopback_registration_endpoint() {
        // Review LOW-7 — [::1] IPv6 loopback must be accepted alongside
        // localhost and 127.0.0.1. The guard rejects BEFORE the HTTP call,
        // so we expect a non-scheme-guard error (connection failure is fine).
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://[::1]:9/register")
            .await
            .unwrap_err();
        let msg = format!("{err}");
        // Must NOT be the scheme-guard error — the guard passed, we only
        // failed on the downstream HTTP call (port 9 is unreachable).
        assert!(
            !msg.contains("must be https"),
            "scheme guard should accept http://[::1] but rejected: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_accepts_http_localhost_registration_endpoint() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://localhost:9/register")
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            !msg.contains("must be https"),
            "scheme guard should accept http://localhost but rejected: {msg}"
        );
    }

    #[tokio::test]
    async fn dcr_accepts_http_ipv4_loopback_registration_endpoint() {
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://127.0.0.1:9/register")
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            !msg.contains("must be https"),
            "scheme guard should accept http://127.0.0.1 but rejected: {msg}"
        );
    }

    #[tokio::test]
    async fn authorize_with_details_fails_cleanly_without_server() {
        // Unit-test scope: verify the method signature compiles and returns
        // an error when no real server is reachable (not a behavior test —
        // full behavior is in the mockito integration test, Task 1.3).
        let cfg = OAuthConfig {
            mcp_server_url: Some("http://localhost:1/nonexistent".into()),
            client_id: Some("x".into()),
            dcr_enabled: false,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper.authorize_with_details().await.unwrap_err();
        // Any error path is acceptable here — the test ensures no panic.
        let _ = format!("{err}");
    }

    #[test]
    fn authorization_result_struct_has_expected_fields() {
        // Compile-time check: every required field is present and public.
        let _r = AuthorizationResult {
            access_token: "a".into(),
            refresh_token: Some("r".into()),
            expires_at: Some(1),
            scopes: vec!["openid".into()],
            issuer: Some("https://i.example".into()),
            client_id: "c".into(),
        };
    }

    #[test]
    fn build_auth_result_converts_expires_in_to_expires_at() {
        let token = crate::client::auth::TokenResponse {
            access_token: "a".into(),
            token_type: "Bearer".into(),
            expires_in: Some(3600),
            refresh_token: Some("r".into()),
            scope: Some("openid profile".into()),
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let r = OAuthHelper::build_auth_result(
            token,
            "c1".into(),
            Some("https://i.example".into()),
            &["openid".into()],
        );
        assert_eq!(r.client_id, "c1");
        assert_eq!(r.refresh_token.as_deref(), Some("r"));
        assert_eq!(r.issuer.as_deref(), Some("https://i.example"));
        assert_eq!(r.scopes, vec!["openid".to_string(), "profile".into()]);
        let expires_at = r.expires_at.expect("expires_at populated");
        assert!(
            expires_at >= now + 3599 && expires_at <= now + 3601,
            "expires_at ({}) should be approximately now+3600 ({})",
            expires_at,
            now + 3600
        );
    }

    #[test]
    fn build_auth_result_falls_back_to_requested_scopes_when_no_grant() {
        let token = crate::client::auth::TokenResponse {
            access_token: "a".into(),
            token_type: "Bearer".into(),
            expires_in: None,
            refresh_token: None,
            scope: None,
        };
        let requested = vec!["openid".to_string(), "email".to_string()];
        let r = OAuthHelper::build_auth_result(token, "c".into(), None, &requested);
        assert_eq!(r.scopes, requested);
        assert!(r.expires_at.is_none());
        assert!(r.refresh_token.is_none());
    }
}
