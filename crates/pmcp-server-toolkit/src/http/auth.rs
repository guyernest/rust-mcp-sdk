//! Authentication providers for OUTGOING HTTP requests (OAPI-03 / D-05 / H1).
//!
//! This module is the OUTBOUND counterpart to the inbound
//! [`crate::auth::AuthProvider`] (`pmcp::server::auth::AuthProvider`, which
//! authenticates an INCOMING MCP request). The two are kept deliberately
//! distinct (Pitfall 1): the trait here is [`HttpAuthProvider`] and its method
//! is [`apply`](HttpAuthProvider::apply) — it MUTATES the headers / query of a
//! request the toolkit is about to SEND to a REST backend. This module does NOT
//! re-implement the inbound request-validation surface.
//!
//! # The six auth modes (D-05) split into two construction strategies
//!
//! [`AuthConfig`] has SIX variants — `None` + five authenticated ones. They
//! split by HOW the credential is obtained:
//!
//! - **Static** (`None`/`ApiKey`/`Bearer`/`Basic`/`OAuth2ClientCredentials`):
//!   fully determined by `config.toml` (operator credentials / `${ENV}` secrets).
//!   Built ONCE at startup via [`create_auth_provider`] and shared as
//!   `Arc<dyn HttpAuthProvider>`. They IGNORE any inbound MCP client token.
//! - **Per-request passthrough** (`OAuthPassthrough`): needs the INCOMING MCP
//!   client token for EACH request, so it cannot be fully built at startup.
//!   [`apply`](HttpAuthProvider::apply) accepts an OPTIONAL `inbound_token` so a
//!   SINGLE trait serves both strategies — static providers ignore it,
//!   [`OAuthPassthroughAuth`] forwards it. Plan 04 carries the per-request token
//!   to `apply`; Plan 06 wires the inbound `TokenCaptureAuthProvider` so the
//!   captured token lands in `AuthContext` and is threaded into this `apply`.
//!
//! # Ownership
//!
//! [`AuthConfig`] and the provider types are OWNED HERE so Plan 01 and Plan 02
//! changes stay confined — Plan 02 RE-EXPORTS
//! `pmcp_server_toolkit::http::auth::AuthConfig` rather than redefining it.

use super::HttpConnectorError;
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Default `required` flag (true) for authenticated [`AuthConfig`] variants.
fn default_true() -> bool {
    true
}

/// Default outgoing header for [`AuthConfig::OAuthPassthrough`].
fn default_auth_header() -> String {
    "Authorization".to_string()
}

/// Outgoing-HTTP authentication configuration (OAPI-03 / D-05).
///
/// Lifted near-verbatim from the pmcp-run reference `AuthConfig`. The
/// `#[serde(tag = "type", rename_all = "snake_case")]` shape means a
/// `config.toml` `[backend.auth]` block selects the variant via `type = "..."`
/// (`none`, `api_key`, `bearer`, `basic`, `oauth2_client_credentials`,
/// `oauth_passthrough`). [`Default`] is [`AuthConfig::None`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthConfig {
    /// No authentication.
    #[default]
    None,

    /// API key passed as query parameters and/or headers.
    ApiKey {
        /// API key values carried as query parameters.
        #[serde(default)]
        query_params: HashMap<String, String>,
        /// API key values carried as headers.
        #[serde(default)]
        headers: HashMap<String, String>,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// Bearer token (`Authorization: Bearer <token>`).
    Bearer {
        /// Token value (operator-configured; may be a `${VAR}` reference resolved upstream).
        token: String,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// HTTP Basic auth (`Authorization: Basic <base64(user:pass)>`).
    Basic {
        /// Username.
        username: String,
        /// Password.
        password: String,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// OAuth2 client-credentials grant.
    OAuth2ClientCredentials {
        /// Token endpoint URL.
        token_url: String,
        /// Client ID.
        client_id: String,
        /// Client secret.
        client_secret: String,
        /// Requested scopes.
        #[serde(default)]
        scopes: Vec<String>,
        /// Whether authentication is required.
        #[serde(default = "default_true")]
        required: bool,
    },

    /// Forward the INCOMING MCP client token to the backend (SSO passthrough, H1).
    OAuthPassthrough {
        /// Outgoing header to set (default `Authorization`).
        #[serde(default = "default_auth_header")]
        target_header: String,
        /// Whether to fail when no inbound token is present.
        #[serde(default = "default_true")]
        required: bool,
    },
}

impl AuthConfig {
    /// Whether this configuration requires authentication to succeed.
    #[must_use]
    pub fn is_required(&self) -> bool {
        match self {
            Self::None => false,
            Self::ApiKey { required, .. }
            | Self::Bearer { required, .. }
            | Self::Basic { required, .. }
            | Self::OAuth2ClientCredentials { required, .. }
            | Self::OAuthPassthrough { required, .. } => *required,
        }
    }
}

/// Outbound HTTP authentication provider (OAPI-03).
///
/// DISTINCT from the inbound [`crate::auth::AuthProvider`] (Pitfall 1): this
/// MUTATES the outgoing request. [`apply`](HttpAuthProvider::apply) accepts an
/// OPTIONAL `inbound_token` — the per-request MCP client token captured via the
/// `AuthContext` bridge (H1). Static providers ignore it; the passthrough
/// provider forwards it.
#[async_trait]
pub trait HttpAuthProvider: Send + Sync + 'static {
    /// Apply credentials to the outgoing request's `headers` and `query`.
    ///
    /// `inbound_token` is the per-request MCP client token (when present). Static
    /// providers MUST ignore it; [`OAuthPassthroughAuth`] forwards it.
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError::Auth`] when a required credential is absent,
    /// or [`HttpConnectorError::InvalidHeader`] when a header name/value cannot be
    /// constructed. No error message echoes the token or credential value.
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        query: &mut HashMap<String, String>,
        inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError>;
}

/// No authentication — a no-op provider.
pub struct NoAuth;

#[async_trait]
impl HttpAuthProvider for NoAuth {
    async fn apply(
        &self,
        _headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        Ok(())
    }
}

/// Provider that always fails — used when a required passthrough token is absent.
pub struct MissingTokenAuth;

#[async_trait]
impl HttpAuthProvider for MissingTokenAuth {
    async fn apply(
        &self,
        _headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        // Honour a late-arriving per-request token if the static constructor was
        // built without one (the passthrough construction-time fallback).
        if inbound_token.map(str::is_empty) == Some(false) {
            return Ok(());
        }
        Err(HttpConnectorError::Auth(
            "authentication required but no inbound token was provided".to_string(),
        ))
    }
}

/// API key authentication (query params and/or headers). STATIC: ignores `inbound_token`.
pub struct ApiKeyAuth {
    query_params: HashMap<String, String>,
    headers: HashMap<String, String>,
}

#[async_trait]
impl HttpAuthProvider for ApiKeyAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        for (key, value) in &self.query_params {
            query.insert(key.clone(), value.clone());
        }
        for (key, value) in &self.headers {
            let name = HeaderName::try_from(key.as_str()).map_err(|_| {
                HttpConnectorError::InvalidHeader("invalid header name".to_string())
            })?;
            let val = HeaderValue::try_from(value.as_str()).map_err(|_| {
                HttpConnectorError::InvalidHeader("invalid header value".to_string())
            })?;
            headers.insert(name, val);
        }
        Ok(())
    }
}

/// Bearer token authentication. STATIC: ignores `inbound_token`.
pub struct BearerAuth {
    token: String,
}

#[async_trait]
impl HttpAuthProvider for BearerAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        let value = format!("Bearer {}", self.token);
        let header_value = HeaderValue::try_from(value)
            .map_err(|_| HttpConnectorError::InvalidHeader("invalid bearer token".to_string()))?;
        headers.insert(reqwest::header::AUTHORIZATION, header_value);
        Ok(())
    }
}

/// HTTP Basic authentication. STATIC: ignores `inbound_token`.
pub struct BasicAuth {
    username: String,
    password: String,
}

#[async_trait]
impl HttpAuthProvider for BasicAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        use base64::Engine;
        let credentials = format!("{}:{}", self.username, self.password);
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        let value = format!("Basic {encoded}");
        let header_value = HeaderValue::try_from(value).map_err(|_| {
            HttpConnectorError::InvalidHeader("invalid basic credentials".to_string())
        })?;
        headers.insert(reqwest::header::AUTHORIZATION, header_value);
        Ok(())
    }
}

/// OAuth2 client-credentials authentication. STATIC config; ignores `inbound_token`.
///
/// The token is fetched lazily from `token_url` on first `apply` and cached. The
/// fetch uses a fresh `reqwest::Client` (mirrors the reference). The cached token
/// is stored under a `tokio::sync::RwLock`.
pub struct OAuth2ClientCredentialsAuth {
    token_url: String,
    client_id: String,
    client_secret: String,
    scopes: Vec<String>,
    cached: tokio::sync::RwLock<Option<String>>,
}

impl OAuth2ClientCredentialsAuth {
    /// Construct a client-credentials provider (no network until first `apply`).
    #[must_use]
    pub fn new(
        token_url: String,
        client_id: String,
        client_secret: String,
        scopes: Vec<String>,
    ) -> Self {
        Self {
            token_url,
            client_id,
            client_secret,
            scopes,
            cached: tokio::sync::RwLock::new(None),
        }
    }

    async fn fetch_token(&self) -> Result<String, HttpConnectorError> {
        let client = reqwest::Client::new();
        let mut params = vec![
            ("grant_type", "client_credentials".to_string()),
            ("client_id", self.client_id.clone()),
            ("client_secret", self.client_secret.clone()),
        ];
        if !self.scopes.is_empty() {
            params.push(("scope", self.scopes.join(" ")));
        }
        let response = client
            .post(&self.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|_| HttpConnectorError::Auth("oauth2 token request failed".to_string()))?;
        if !response.status().is_success() {
            return Err(HttpConnectorError::Auth(format!(
                "oauth2 token endpoint returned status {}",
                response.status().as_u16()
            )));
        }
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
        }
        let token: TokenResponse = response.json().await.map_err(|_| {
            HttpConnectorError::Auth("oauth2 token response unparseable".to_string())
        })?;
        Ok(token.access_token)
    }
}

#[async_trait]
impl HttpAuthProvider for OAuth2ClientCredentialsAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        _inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        {
            let cached = self.cached.read().await;
            if cached.is_none() {
                drop(cached);
                let fetched = self.fetch_token().await?;
                *self.cached.write().await = Some(fetched);
            }
        }
        let cached = self.cached.read().await;
        if let Some(access_token) = cached.as_ref() {
            let value = format!("Bearer {access_token}");
            let header_value = HeaderValue::try_from(value).map_err(|_| {
                HttpConnectorError::InvalidHeader("invalid oauth2 access token".to_string())
            })?;
            headers.insert(reqwest::header::AUTHORIZATION, header_value);
        }
        Ok(())
    }
}

/// OAuth passthrough — forwards the INCOMING MCP client token to the backend (H1).
///
/// PER-REQUEST: prefers the per-request `inbound_token` arg to `apply`; falls back
/// to the construction-time captured `incoming_token` (via
/// [`create_passthrough_auth_provider`]). When neither is present and the config
/// is `required`, `apply` returns [`HttpConnectorError::Auth`].
pub struct OAuthPassthroughAuth {
    target_header: String,
    incoming_token: Option<String>,
    required: bool,
}

#[async_trait]
impl HttpAuthProvider for OAuthPassthroughAuth {
    async fn apply(
        &self,
        headers: &mut HeaderMap,
        _query: &mut HashMap<String, String>,
        inbound_token: Option<&str>,
    ) -> Result<(), HttpConnectorError> {
        // Prefer the per-request token; fall back to the construction-time capture.
        let token: Option<&str> = inbound_token
            .filter(|t| !t.is_empty())
            .or_else(|| self.incoming_token.as_deref().filter(|t| !t.is_empty()));

        match token {
            Some(tok) => {
                let header_name =
                    HeaderName::try_from(self.target_header.as_str()).map_err(|_| {
                        HttpConnectorError::InvalidHeader(
                            "invalid passthrough target header".to_string(),
                        )
                    })?;
                // Forward the token verbatim if it already carries a scheme,
                // otherwise prefix with "Bearer ".
                let value = if tok.starts_with("Bearer ") || tok.starts_with("Basic ") {
                    tok.to_string()
                } else {
                    format!("Bearer {tok}")
                };
                let header_value = HeaderValue::try_from(value).map_err(|_| {
                    HttpConnectorError::InvalidHeader("invalid passthrough token value".to_string())
                })?;
                headers.insert(header_name, header_value);
                Ok(())
            },
            None if self.required => Err(HttpConnectorError::Auth(
                "passthrough authentication required but no inbound token was provided".to_string(),
            )),
            None => Ok(()),
        }
    }
}

/// Build a STATIC auth provider from `cfg`, shared as `Arc<dyn HttpAuthProvider>`.
///
/// For [`AuthConfig::OAuthPassthrough`], use [`create_passthrough_auth_provider`]
/// instead — without a token this returns a [`MissingTokenAuth`] (if required) or
/// [`NoAuth`], since the per-request token is not yet known at startup.
///
/// # Errors
///
/// This constructor never fails today (returns `Ok`) — the fallible signature is
/// reserved so a future variant requiring construction-time validation can error
/// without a breaking change.
/// Resolve a single api_key value, expanding a `${VAR}` or `env:VAR` reference
/// from the process environment.
///
/// The outbound api_key (carried as a query param or header) frequently holds a
/// secret reference (`"${TFL_APP_KEY}"`) rather than a literal — mirroring the
/// `token_secret` convention in [`crate::code_mode`]. Without expansion the
/// LITERAL `${TFL_APP_KEY}` would be sent to the backend, so 100% of authenticated
/// calls would fail (this is a correctness requirement, not a convenience).
///
/// Resolution rules (matching the `token_secret` env-ref discipline):
/// - `"${VAR}"` / `"env:VAR"` → the value of `VAR` from the process env.
/// - An UNSET or set-but-empty/whitespace `VAR` resolves to an empty string, so
///   a `required = false` api_key is OMITTED rather than sent as a degenerate
///   empty/placeholder value.
/// - A plain literal (no `${...}` / `env:` prefix) is returned verbatim.
///
/// No error path: an unresolvable reference yields an empty string (omission),
/// never a panic and never the literal `${...}` reaching the wire.
fn resolve_api_key_value(raw: &str) -> String {
    let var = if let Some(v) = raw.strip_prefix("env:") {
        Some(v)
    } else if let Some(inner) = raw.strip_prefix("${").and_then(|s| s.strip_suffix('}')) {
        (!inner.is_empty()).then_some(inner)
    } else {
        // Plain literal — used verbatim.
        return raw.to_string();
    };
    match var {
        Some(name) => std::env::var(name)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_default(),
        // Malformed reference (e.g. `"${}"`) → empty (omitted).
        None => String::new(),
    }
}

/// Expand every value in an api_key map, dropping entries that resolve to empty
/// (an unset `required = false` reference is omitted, not sent empty).
fn expand_api_key_map(map: &HashMap<String, String>) -> HashMap<String, String> {
    map.iter()
        .filter_map(|(k, v)| {
            let resolved = resolve_api_key_value(v);
            (!resolved.is_empty()).then(|| (k.clone(), resolved))
        })
        .collect()
}

pub fn create_auth_provider(
    cfg: &AuthConfig,
) -> Result<Arc<dyn HttpAuthProvider>, HttpConnectorError> {
    let provider: Arc<dyn HttpAuthProvider> = match cfg {
        AuthConfig::None => Arc::new(NoAuth),
        AuthConfig::ApiKey {
            query_params,
            headers,
            ..
        } => {
            // Expand `${VAR}` / `env:VAR` references BEFORE building the provider
            // so the RESOLVED secret (not the literal placeholder) is applied to
            // outgoing requests. Unset references are dropped (omitted).
            let query_params = expand_api_key_map(query_params);
            let headers = expand_api_key_map(headers);
            let has_values = query_params.values().any(|v| !v.is_empty())
                || headers.values().any(|v| !v.is_empty());
            if has_values {
                Arc::new(ApiKeyAuth {
                    query_params,
                    headers,
                })
            } else {
                Arc::new(NoAuth)
            }
        },
        AuthConfig::Bearer { token, .. } => {
            if token.is_empty() {
                Arc::new(NoAuth)
            } else {
                Arc::new(BearerAuth {
                    token: token.clone(),
                })
            }
        },
        AuthConfig::Basic {
            username, password, ..
        } => {
            if username.is_empty() && password.is_empty() {
                Arc::new(NoAuth)
            } else {
                Arc::new(BasicAuth {
                    username: username.clone(),
                    password: password.clone(),
                })
            }
        },
        AuthConfig::OAuth2ClientCredentials {
            token_url,
            client_id,
            client_secret,
            scopes,
            ..
        } => {
            if client_id.is_empty() || client_secret.is_empty() {
                Arc::new(NoAuth)
            } else {
                Arc::new(OAuth2ClientCredentialsAuth::new(
                    token_url.clone(),
                    client_id.clone(),
                    client_secret.clone(),
                    scopes.clone(),
                ))
            }
        },
        AuthConfig::OAuthPassthrough { required, .. } => {
            if *required {
                Arc::new(MissingTokenAuth)
            } else {
                Arc::new(NoAuth)
            }
        },
    };
    Ok(provider)
}

/// Build an auth provider, capturing an `incoming_token` for the
/// [`AuthConfig::OAuthPassthrough`] per-request path (H1).
///
/// For passthrough configs the captured token is stored and forwarded by
/// [`OAuthPassthroughAuth::apply`] (preferring a per-request `inbound_token` when
/// one is also passed to `apply`). For all other configs this delegates to
/// [`create_auth_provider`].
///
/// # Errors
///
/// Propagates any error from [`create_auth_provider`] for non-passthrough configs.
pub fn create_passthrough_auth_provider(
    cfg: &AuthConfig,
    incoming_token: Option<String>,
) -> Result<Arc<dyn HttpAuthProvider>, HttpConnectorError> {
    match cfg {
        AuthConfig::OAuthPassthrough {
            target_header,
            required,
        } => Ok(Arc::new(OAuthPassthroughAuth {
            target_header: target_header.clone(),
            incoming_token: incoming_token.filter(|t| !t.is_empty()),
            required: *required,
        })),
        other => create_auth_provider(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_auth() {
        let auth = create_auth_provider(&AuthConfig::None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert!(headers.is_empty());
        assert!(query.is_empty());
    }

    #[tokio::test]
    async fn test_bearer_auth() {
        let cfg = AuthConfig::Bearer {
            token: "my_token".to_string(),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        // inbound_token is ignored by a static provider.
        auth.apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer my_token"
        );
        assert!(query.is_empty());
    }

    #[tokio::test]
    async fn test_basic_auth() {
        let cfg = AuthConfig::Basic {
            username: "user".to_string(),
            password: "pass".to_string(),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        // base64("user:pass") = "dXNlcjpwYXNz"
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Basic dXNlcjpwYXNz"
        );
    }

    #[tokio::test]
    async fn test_api_key_query_param() {
        // D-04 london-tube path: api key carried as a query param (app_key).
        let cfg = AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), "secret123".to_string())]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: true,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(query.get("app_key"), Some(&"secret123".to_string()));
        assert!(
            headers.is_empty(),
            "api-key-in-query must not touch headers"
        );
    }

    #[tokio::test]
    async fn test_api_key_query_param_expands_braced_env_ref() {
        // The RESOLVED ${VAR} value (not the literal `${...}`) reaches the wire.
        let var = "PMCP_TEST_TFL_APP_KEY_BRACED";
        std::env::set_var(var, "dummy");
        let cfg = AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), format!("${{{var}}}"))]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: false,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(
            query.get("app_key"),
            Some(&"dummy".to_string()),
            "resolved env value lands on the query, not the literal ${{...}}"
        );
        std::env::remove_var(var);
    }

    #[tokio::test]
    async fn test_api_key_query_param_unset_ref_is_omitted() {
        // required=false + an UNSET ${VAR} → the param is omitted (not sent
        // empty, not the literal placeholder).
        let var = "PMCP_TEST_TFL_APP_KEY_UNSET";
        std::env::remove_var(var);
        let cfg = AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), format!("${{{var}}}"))]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: false,
        };
        let auth = create_auth_provider(&cfg).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert!(
            query.get("app_key").is_none(),
            "an unset required=false api_key ref is omitted, not sent empty/literal"
        );
    }

    #[test]
    fn test_resolve_api_key_value_forms() {
        let var = "PMCP_TEST_RESOLVE_API_KEY_FORM";
        std::env::set_var(var, "resolved");
        assert_eq!(resolve_api_key_value(&format!("${{{var}}}")), "resolved");
        assert_eq!(resolve_api_key_value(&format!("env:{var}")), "resolved");
        assert_eq!(resolve_api_key_value("plain-literal"), "plain-literal");
        std::env::remove_var(var);
        assert_eq!(resolve_api_key_value(&format!("${{{var}}}")), "");
        assert_eq!(resolve_api_key_value("${}"), "");
    }

    #[tokio::test]
    async fn test_passthrough_forwards_inbound_token() {
        // H1 per-request path: passthrough forwards the inbound token.
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer client-tok"
        );
    }

    #[tokio::test]
    async fn test_passthrough_custom_target_header() {
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "X-Forwarded-Token".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(
            headers.get("X-Forwarded-Token").unwrap(),
            "Bearer client-tok"
        );
    }

    #[tokio::test]
    async fn test_passthrough_uses_construction_time_token() {
        // Construction-time capture path: inbound_token=None falls back to stored.
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth =
            create_passthrough_auth_provider(&cfg, Some("captured-tok".to_string())).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        auth.apply(&mut headers, &mut query, None).await.unwrap();
        assert_eq!(
            headers.get(reqwest::header::AUTHORIZATION).unwrap(),
            "Bearer captured-tok"
        );
    }

    #[tokio::test]
    async fn test_passthrough_required_missing_token_errors() {
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        let err = auth
            .apply(&mut headers, &mut query, None)
            .await
            .unwrap_err();
        assert!(matches!(err, HttpConnectorError::Auth(_)));
    }

    #[tokio::test]
    async fn test_static_provider_ignores_inbound_token() {
        // T-90-01-06: a static provider must NOT leak the inbound token into its
        // output — it applies ONLY its configured credential.
        let bearer = create_auth_provider(&AuthConfig::Bearer {
            token: "static-tok".to_string(),
            required: true,
        })
        .unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        bearer
            .apply(&mut headers, &mut query, Some("client-tok"))
            .await
            .unwrap();
        let rendered = headers
            .get(reqwest::header::AUTHORIZATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(rendered, "Bearer static-tok");
        assert!(
            !rendered.contains("client-tok"),
            "static provider must not forward the inbound token"
        );

        // Same for api-key-in-query.
        let apikey = create_auth_provider(&AuthConfig::ApiKey {
            query_params: [("app_key".to_string(), "kkk".to_string())]
                .into_iter()
                .collect(),
            headers: HashMap::new(),
            required: true,
        })
        .unwrap();
        let mut headers2 = HeaderMap::new();
        let mut query2 = HashMap::new();
        apikey
            .apply(&mut headers2, &mut query2, Some("client-tok"))
            .await
            .unwrap();
        assert_eq!(query2.get("app_key"), Some(&"kkk".to_string()));
        assert!(
            !query2.values().any(|v| v.contains("client-tok")),
            "static api-key provider must not forward the inbound token"
        );
        assert!(headers2.is_empty());
    }

    #[tokio::test]
    async fn test_auth_error_display_no_secret() {
        // The error surfaced when a required token is missing must not echo a token.
        let cfg = AuthConfig::OAuthPassthrough {
            target_header: "Authorization".to_string(),
            required: true,
        };
        let auth = create_passthrough_auth_provider(&cfg, None).unwrap();
        let mut headers = HeaderMap::new();
        let mut query = HashMap::new();
        let err = auth
            .apply(&mut headers, &mut query, None)
            .await
            .unwrap_err();
        let rendered = err.to_string();
        for forbidden in ["Bearer", "client-tok", "app_key", "https://"] {
            assert!(
                !rendered.contains(forbidden),
                "auth error Display must not echo {forbidden:?}; got {rendered:?}"
            );
        }
    }

    #[test]
    fn test_auth_config_deserializes_snake_case_tag() {
        let toml_src = r#"type = "bearer"
token = "abc"
"#;
        let cfg: AuthConfig = toml::from_str(toml_src).unwrap();
        assert!(matches!(cfg, AuthConfig::Bearer { .. }));
        assert!(cfg.is_required());
    }

    #[test]
    fn test_auth_config_default_is_none() {
        assert!(matches!(AuthConfig::default(), AuthConfig::None));
        assert!(!AuthConfig::None.is_required());
    }
}
