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
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::sleep;
use url::Url;

use crate::client::auth::{AuthorizationServerExtras, OidcDiscoveryClient, TokenExchangeClient};
use crate::client::http_middleware::HttpMiddlewareChain;
use crate::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
use crate::error::{Error, Result};
use crate::server::auth::oauth2::OidcDiscoveryMetadata;
use crate::shared::http_body_cap::DEFAULT_AUTH_RESPONSE_BYTES;
use crate::shared::oauth_validation::{
    derive_application_type, iss_presence_from, parse_iss_env_value,
    validate_authorization_response, AuthorizationRequestRecord, IssPresence,
};
use crate::shared::pkce::generate_state;

/// The environment variable an operator sets to override RFC 9207 `iss`
/// strictness without a redeploy or a code change.
///
/// Accepted values are `strict` and `lenient`, parsed case-insensitively after
/// trimming by
/// [`parse_iss_env_value`](crate::shared::oauth_validation::parse_iss_env_value).
/// A value the SDK does not recognise is **announced** with a `tracing::warn!`
/// naming this variable and its two accepted values, then ignored — it never
/// silently leaves validation lenient.
const ISS_VALIDATION_ENV_VAR: &str = "PMCP_OAUTH_ISS_VALIDATION";

/// The largest HTTP request line the loopback callback listener will read.
///
/// Any process on the local machine can connect to the callback port and send
/// an unbounded request line. The authorization response is a query string —
/// a `code`, a `state` and possibly an `iss`, each well under 100 bytes — never
/// a payload, so refusing at 16 `KiB` costs nothing legitimate. The read is
/// bounded at the socket, so an oversized line is refused without the whole
/// line ever being allocated.
///
/// This is the transport-level twin of
/// [`MAX_CALLBACK_QUERY_BYTES`](crate::shared::oauth_validation::MAX_CALLBACK_QUERY_BYTES),
/// which bounds the query the pure validator will parse.
pub const MAX_CALLBACK_REQUEST_LINE_BYTES: usize = 16_384;

/// The response served when the callback validated. Byte-identical to the page
/// this module has always served on success.
const CALLBACK_SUCCESS_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
     <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
     <h1 style='color: green;'>Authentication Successful!</h1>\
     <p>You can close this window and return to the terminal.</p>\
     </body></html>";

/// The response served when the callback did NOT validate. Byte-identical to
/// the page this module has always served on failure, and deliberately so: it
/// carries no authorization-server-supplied text. RFC 9207 forbids acting on or
/// displaying an `error_description` that arrived behind a failing `iss`, so the
/// failure page must stay fixed bytes.
const CALLBACK_FAILURE_RESPONSE: &str =
    "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html\r\n\r\n\
     <html><body style='font-family: sans-serif; text-align: center; padding: 50px;'>\
     <h1 style='color: red;'>Authentication Failed</h1>\
     <p>No authorization code received. Please try again.</p>\
     </body></html>";

/// The scope an OAuth client asks for when it wants a refresh token (SEP-2207).
///
/// It appears at exactly two protocol stages and means a DIFFERENT thing at
/// each, which is why this module writes it in two places and deliberately not
/// in a third:
///
/// | Stage | What writing it there means | Written here? |
/// |---|---|---|
/// | the Dynamic Client Registration request's `scope` | CLIENT METADATA: what this client is permitted to ask for | yes, when advertised |
/// | the authorization request's `scope` | the ASK itself — the only stage at which requesting it does anything | yes, when advertised |
/// | a refresh request's `scope` | a scope CHANGE on an existing grant | **no, never** |
///
/// The third row is the one that matters. RFC 6749 §6 permits a refresh request
/// to narrow the granted scope and never to widen it, so introducing a scope at
/// refresh that was never granted can have the authorization server refuse a
/// refresh that would otherwise have succeeded. Refresh sends only what was
/// GRANTED, or no `scope` at all.
///
/// Both writes are conditioned on the authorization server ADVERTISING the scope
/// in its `scopes_supported`, because SEP-2207 states the client MAY request it
/// when the server advertises support — the condition is the whole rule.
const OFFLINE_ACCESS_SCOPE: &str = "offline_access";

/// The largest Dynamic Client Registration response body this client will read,
/// on the success path and on the rejection path alike.
///
/// Defined as the shared authorization-surface cap rather than as a second
/// literal, so a hostile registration endpoint faces ONE number and the refusal
/// message that names it cannot drift from the number actually enforced.
const MAX_DCR_RESPONSE_BYTES: usize = DEFAULT_AUTH_RESPONSE_BYTES;

/// Compose a `scope` value from the configured scopes plus
/// [`OFFLINE_ACCESS_SCOPE`], and only when the authorization server advertises
/// that scope in its `scopes_supported`.
///
/// The result is **order-stable** (configured scopes keep their order,
/// `offline_access` is appended last) and **deduplicated** (a caller who already
/// listed `offline_access` gets one entry, not two — a duplicated scope token is
/// legal but is the kind of thing a strict authorization server rejects).
///
/// It takes a slice and returns a fresh `Vec`, so `OAuthConfig::scopes` is never
/// mutated. That is not a stylistic preference: `scopes` is a public field on a
/// public struct, and a caller who reuses one [`OAuthConfig`] across two flows
/// against an advertising server must not watch it grow an entry per flow.
fn compose_scopes_with_offline_access(
    configured: &[String],
    scopes_supported: &[String],
) -> Vec<String> {
    let mut composed: Vec<String> = Vec::with_capacity(configured.len() + 1);
    for scope in configured {
        if !composed.iter().any(|held| held == scope) {
            composed.push(scope.clone());
        }
    }

    let advertised = scopes_supported
        .iter()
        .any(|scope| scope == OFFLINE_ACCESS_SCOPE);
    if advertised && !composed.iter().any(|held| held == OFFLINE_ACCESS_SCOPE) {
        composed.push(OFFLINE_ACCESS_SCOPE.to_string());
    }

    composed
}

/// Write SEP-837's `application_type` onto a registration request, DERIVING it
/// from the `redirect_uris` the request is actually registering.
///
/// Returns the value that will go on the wire, so the caller can compare it
/// against whatever the authorization server echoes back.
///
/// # Why derive from `redirect_uris` rather than from `config.redirect_port`
///
/// The redirect URIs are the values the authorization server will enforce its
/// per-type constraints against. Deriving from anything else lets the declared
/// type and the registered URIs drift apart the moment the URI shape changes,
/// which is precisely the mismatch SEP-837 warns registration will be rejected
/// for.
///
/// # The override path
///
/// An `application_type` that is ALREADY set on the request is kept, never
/// overwritten. `DcrRequest::set_application_type` performs no validation
/// exactly so that it can serve as the authoritative override for this
/// derivation (116-03 / D-09), and silently clobbering it here would delete that
/// path.
///
/// # Errors
///
/// Propagates [`derive_application_type`]'s refusal for an empty, unparseable,
/// cleartext-remote or MIXED `redirect_uris` vector. A mixed vector is an error
/// and never a pick: choosing one classification for a vector containing both
/// registers a client whose declared type contradicts some of its own redirect
/// URIs, which is an open-redirect primitive (D-10).
fn apply_application_type(request: &mut DcrRequest) -> Result<String> {
    if let Some(explicit) = request.application_type() {
        return Ok(explicit.to_string());
    }

    let derived = derive_application_type(&request.redirect_uris)?;
    request.set_application_type(derived.as_str());
    Ok(derived.as_str().to_string())
}

/// How the interactive authorization URL reaches a human.
///
/// **This is a platform seam, not test scaffolding.** The interactive CLI flow
/// is one caller and not the only caller: a headless CI runner, a container
/// without a display, an SSH or remote-desktop session, or a hosting platform
/// that relays the URL through its own UI all want to PRINT or forward the URL
/// rather than open a window on whatever machine the process happens to be on.
/// Implement this trait to do that. It must not be hidden behind
/// `#[doc(hidden)]`.
///
/// A useful consequence is that the flow becomes testable end to end: a test
/// launcher can read the `state` and `code_challenge` out of the URL it
/// receives and deliver a callback, with no browser window and no human.
///
/// # Errors
///
/// [`BrowserLauncher::open`] returns `Err` only when the URL could not be
/// delivered to a human **at all**. The flow then aborts rather than waiting
/// five minutes for a callback nobody will deliver. A launcher that has a
/// fallback — as [`SystemBrowserLauncher`] does, because the flow already
/// logged the URL for manual entry — should return `Ok(())`.
pub trait BrowserLauncher: Send + Sync + std::fmt::Debug {
    /// Deliver the authorization URL to the human completing the flow.
    fn open(&self, url: &str) -> Result<()>;
}

/// The default [`BrowserLauncher`]: opens the platform's browser.
///
/// Returns `Ok(())` even when the platform browser cannot be opened, matching
/// this module's long-standing behaviour — the flow has already logged the URL
/// with "If the browser doesn't open, visit: …", so a human can still complete
/// the flow by pasting it. Aborting there would remove a working manual path.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemBrowserLauncher;

impl BrowserLauncher for SystemBrowserLauncher {
    fn open(&self, url: &str) -> Result<()> {
        if let Err(e) = webbrowser::open(url) {
            tracing::warn!(
                "Failed to open browser: {}. Please open the URL manually.",
                e
            );
        }
        Ok(())
    }
}

// Re-export RFC 7591 DCR types from the authoritative server-side definitions
// so library users can construct DCR requests via `pmcp::client::oauth::DcrRequest`.
// Source of truth: src/server/auth/provider.rs:302-382.
pub use crate::server::auth::provider::{DcrRequest, DcrResponse};

/// OAuth configuration for CLI authentication flows.
///
/// # Migration note (pmcp 2.5.0)
///
/// `client_id` changed from `String` to `Option<String>` to support RFC 7591
/// Dynamic Client Registration. Existing callers that passed a pre-registered
/// id must now wrap it in `Some(...)`:
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
/// # Field semantics
///
/// - `access_token`: The bearer token. Put in `Authorization: Bearer <...>` headers.
/// - `refresh_token`: Present when the IdP returned one (Okta, Auth0, Keycloak
///   with offline_access). `None` when the IdP does not issue refresh tokens.
///
///   **Device-code flow (RFC 8628):** When `OAuthHelper` falls back from
///   authorization-code to device-code (e.g., the IdP does not support
///   localhost callbacks), `refresh_token` may be `None` because RFC 8628 §3.5
///   does NOT require the token response to include a `refresh_token`. Users
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
///
/// # RFC 9207 `iss` validation and the `PMCP_OAUTH_ISS_VALIDATION` override
///
/// Every authorization response is validated before its `code` can be
/// exchanged: the CSRF `state` must match the value this flow generated, and an
/// `iss` that is PRESENT is always compared — byte for byte, with no
/// normalisation — against the issuer the authorization server published in its
/// own discovery document. That floor is unconditional. The only configurable
/// question is whether an ABSENT `iss` is fatal, and it is resolved from three
/// tiers, highest first:
///
/// | Tier | Source | Wins over |
/// |---|---|---|
/// | 1 | the `PMCP_OAUTH_ISS_VALIDATION` environment variable | everything below |
/// | 2 | [`OAuthHelper::with_iss_validation`] | the discovery flag |
/// | 3 | the discovery document's `authorization_response_iss_parameter_supported` | — |
///
/// `PMCP_OAUTH_ISS_VALIDATION` accepts exactly two values, compared
/// case-insensitively after trimming:
///
/// - `strict` — an authorization response with no `iss` is REJECTED.
/// - `lenient` — an absent `iss` proceeds (a present one is still compared).
///
/// **An unrecognised value is announced, not swallowed.** `true`, `1` and `yes`
/// are all plausible things to type and none of them is accepted; setting one
/// emits a `tracing::warn!` naming the variable, the value observed and the two
/// accepted values, and the next tier decides. **With the variable unset and no
/// builder call the behaviour is exactly the discovery flag's**, so an existing
/// deployment sees no change.
///
/// The variable is read at the point the flow needs it, never in a constructor,
/// so a hosting platform can supply the policy as a parameter through
/// [`OAuthHelper::with_iss_validation`] instead of through a process
/// environment.
#[derive(Debug)]
pub struct OAuthHelper {
    config: OAuthConfig,
    client: reqwest::Client,
    /// Tier 2 of the `iss` precedence chain. `None` means "not set by the
    /// builder", which is distinct from `Some(IssPresence::Optional)`.
    ///
    /// A PRIVATE field rather than an [`OAuthConfig`] field on purpose:
    /// `OAuthConfig` is public, all-pub-field and not `#[non_exhaustive]`, so a
    /// new field there is `constructible_struct_adds_field` — a MAJOR break that
    /// would invalidate every struct literal, including three in this
    /// repository. `OAuthHelper`'s fields are all private, so adding one here is
    /// semver-free.
    iss_validation: Option<IssPresence>,
    /// How the interactive authorization URL reaches a human. Defaults to
    /// [`SystemBrowserLauncher`], i.e. unchanged behaviour.
    browser_launcher: Arc<dyn BrowserLauncher>,
}

impl OAuthHelper {
    /// Create a new OAuth helper with the given configuration.
    pub fn new(config: OAuthConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| Error::internal(format!("Failed to create HTTP client: {e}")))?;

        Ok(Self {
            config,
            client,
            iss_validation: None,
            browser_launcher: Arc::new(SystemBrowserLauncher),
        })
    }

    /// Set tier 2 of the RFC 9207 `iss` precedence chain.
    ///
    /// See the [type-level documentation](Self) for the full chain. In short:
    /// `PMCP_OAUTH_ISS_VALIDATION` still wins over whatever is set here, and
    /// whatever is set here wins over the authorization server's own
    /// `authorization_response_iss_parameter_supported` flag. Passing
    /// [`IssPresence::Required`] makes an authorization response with no `iss`
    /// fatal even against a server that advertises nothing.
    ///
    /// This is an inherent builder method rather than an [`OAuthConfig`] field
    /// because that struct is exhaustively constructible downstream and gaining
    /// a field would be a MAJOR semver break.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::client::oauth::{OAuthConfig, OAuthHelper};
    /// use pmcp::IssPresence;
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let helper = OAuthHelper::new(OAuthConfig::default())?
    ///     .with_iss_validation(IssPresence::Required);
    /// # let _ = helper;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_iss_validation(mut self, presence: IssPresence) -> Self {
        self.iss_validation = Some(presence);
        self
    }

    /// Route the interactive authorization URL through a custom
    /// [`BrowserLauncher`] instead of the platform browser.
    ///
    /// Use this on a headless runner, in a container without a display, or from
    /// a hosting platform that relays the URL through its own UI. With no call
    /// to this method the flow uses [`SystemBrowserLauncher`], i.e. today's
    /// behaviour, unchanged.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::sync::Arc;
    /// use pmcp::client::oauth::{BrowserLauncher, OAuthConfig, OAuthHelper};
    ///
    /// #[derive(Debug)]
    /// struct PrintTheUrl;
    /// impl BrowserLauncher for PrintTheUrl {
    ///     fn open(&self, url: &str) -> pmcp::Result<()> {
    ///         println!("Open this URL to continue: {url}");
    ///         Ok(())
    ///     }
    /// }
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let helper = OAuthHelper::new(OAuthConfig::default())?
    ///     .with_browser_launcher(Arc::new(PrintTheUrl));
    /// # let _ = helper;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_browser_launcher(mut self, launcher: Arc<dyn BrowserLauncher>) -> Self {
        self.browser_launcher = launcher;
        self
    }

    /// Resolve the effective [`IssPresence`] for one flow, reading the
    /// `PMCP_OAUTH_ISS_VALIDATION` override at the CALL SITE.
    ///
    /// The read lives here rather than in [`OAuthHelper::new`] so construction
    /// stays I/O-free and a platform can supply the policy as a parameter. An
    /// unrecognised value warns and falls through; the precedence arithmetic
    /// itself is [`iss_presence_from`]'s, never re-implemented here.
    fn resolve_iss_presence(&self, discovery_flag: Option<bool>) -> IssPresence {
        let env_override = match std::env::var("PMCP_OAUTH_ISS_VALIDATION") {
            Ok(raw) => {
                let parsed = parse_iss_env_value(&raw);
                if parsed.is_none() {
                    tracing::warn!(
                        "{} is set to `{}`, which is not one of its two accepted values \
                         `strict` or `lenient`. The variable is IGNORED; the builder setting \
                         or the authorization server's discovery flag decides instead.",
                        ISS_VALIDATION_ENV_VAR,
                        raw
                    );
                }
                parsed
            },
            Err(std::env::VarError::NotUnicode(_)) => {
                tracing::warn!(
                    "{} is set to a value that is not valid Unicode, so it cannot be one of \
                     the two accepted values `strict` or `lenient`. The variable is IGNORED.",
                    ISS_VALIDATION_ENV_VAR
                );
                None
            },
            Err(std::env::VarError::NotPresent) => None,
        };

        iss_presence_from(env_override, self.iss_validation, discovery_flag)
    }

    /// Perform RFC 7591 Dynamic Client Registration against `registration_endpoint`.
    ///
    /// Body is a public PKCE shape (`token_endpoint_auth_method: "none"`, no secret
    /// requested). `client_name` falls back to `"pmcp-sdk"` when the config value is
    /// `None`. Non-`https://` endpoints are rejected except for localhost loopback
    /// variants, guarding against discovery-spoofing. Response body size is capped
    /// at [`MAX_DCR_RESPONSE_BYTES`] on both the success and the rejection path.
    ///
    /// # The two SEPs this body carries
    ///
    /// - **SEP-837** — an `application_type` derived from the `redirect_uris`
    ///   being registered, sent UNCONDITIONALLY. Omitting it defaults to `web`
    ///   under OIDC, which contradicts the loopback redirect this very request
    ///   registers; a non-OIDC authorization server safely ignores it.
    /// - **SEP-2207** — `refresh_token` declared in `grant_types`, plus
    ///   `offline_access` in the registered `scope` when the authorization server
    ///   advertises it. See [`OFFLINE_ACCESS_SCOPE`] for why this is client
    ///   METADATA and not the request itself.
    ///
    /// `metadata` is taken as a parameter (rather than re-discovered) so
    /// `scopes_supported` is read from the same document the rest of the flow
    /// used. This is a private method, so widening its signature costs nothing.
    async fn do_dynamic_client_registration(
        &self,
        registration_endpoint: &str,
        metadata: &OidcDiscoveryMetadata,
    ) -> Result<crate::server::auth::provider::DcrResponse> {
        let parsed = Url::parse(registration_endpoint)
            .map_err(|e| Error::internal(format!("Invalid registration_endpoint URL: {e}")))?;
        // `url::Url::host_str()` returns IPv6 literals WITH brackets (e.g.
        // `http://[::1]/register` -> `Some("[::1]")`); match both forms.
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
        // Literal `127.0.0.1` rather than `localhost` — per RFC 8252 §7.3, avoids
        // browsers resolving `localhost` to `::1` when the listener binds IPv4-only.
        let redirect_uri = format!("http://127.0.0.1:{}/callback", self.config.redirect_port);

        // SEP-2207 client metadata: declaring `offline_access` here says what this
        // client is PERMITTED to ask for. The ask itself happens at the
        // authorization request — see `build_authorization_url`.
        let registered_scopes =
            compose_scopes_with_offline_access(&self.config.scopes, &metadata.scopes_supported);

        let mut request = crate::server::auth::provider::DcrRequest {
            redirect_uris: vec![redirect_uri],
            client_name: Some(client_name),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".to_string()),
            // SEP-2207: an authorization server that was never told this client
            // wants a refresh grant has every reason not to issue a refresh
            // token. This is the missing prerequisite for refresh working at all.
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            // `DcrRequest` has `#[serde(skip_serializing_if = "Vec::is_empty")]`;
            // RFC 7591 §3.1 requires `response_types` in the body, so it must be
            // non-empty. `"code"` is the authorization-code public-PKCE flow.
            response_types: vec!["code".to_string()],
            // `scope` is `skip_serializing_if = "Option::is_none"`, so an empty
            // composition leaves the key off the wire entirely rather than
            // registering an empty string.
            scope: (!registered_scopes.is_empty()).then(|| registered_scopes.join(" ")),
            software_id: None,
            software_version: None,
            extra: Default::default(),
        };

        // SEP-837, sent on every era and under every configuration: there is no
        // gate, no feature flag and no era check. `application_type` has been a
        // standard OIDC Dynamic Registration parameter since 2014, a non-OIDC
        // server ignores it, and era-gating would require plumbing a protocol
        // era into DCR that does not exist before an MCP connection is
        // established.
        apply_application_type(&mut request)?;

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

        // reqwest has no direct bytes-limit API — read the body as bytes, enforce
        // the cap, then parse from slice.
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
        let registration =
            serde_json::from_slice::<crate::server::auth::provider::DcrResponse>(&bytes)
                .map_err(|e| Error::internal(format!("Failed to parse DCR response: {e}")))?;

        Ok(registration)
    }

    /// Resolve the `client_id` for the current OAuth flow, performing DCR
    /// lazily when all three conditions hold:
    ///   1. `self.config.dcr_enabled == true`
    ///   2. `self.config.client_id.is_none()`
    ///   3. `metadata.registration_endpoint.is_some()`
    ///
    /// Returns `Err` with an actionable message when DCR is needed but the
    /// server does not advertise a `registration_endpoint`.
    async fn resolve_client_id_for_flow(&self, metadata: &OidcDiscoveryMetadata) -> Result<String> {
        // Caller-provided client_id skips DCR entirely.
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
                let response = self
                    .do_dynamic_client_registration(endpoint, metadata)
                    .await?;
                tracing::info!("DCR succeeded — issued client_id");
                Ok(response.client_id)
            },
            None => Err(Error::internal(
                "server does not support DCR — pass a pre-registered client_id".to_string(),
            )),
        }
    }

    /// Test-only hook: drive the discovery + DCR resolver path without invoking
    /// the browser PKCE flow. Used by `tests/oauth_dcr_integration.rs`.
    ///
    /// Integration tests under `tests/` compile as a separate crate, so the
    /// library's `#[cfg(test)]` does not apply. The `oauth` feature gate plus
    /// `#[doc(hidden)]` and the `test_` prefix discourage external use.
    #[doc(hidden)]
    #[cfg(any(test, feature = "oauth"))]
    pub async fn test_resolve_client_id_from_discovery(&self) -> Result<String> {
        let metadata = self.get_metadata().await?;
        self.resolve_client_id_for_flow(&metadata).await
    }

    /// Whether an authorization-code-flow failure is a validation REFUSAL that
    /// must be surfaced verbatim rather than downgraded.
    ///
    /// Both callers of the authorization-code flow fall back to device code (or
    /// to a generic "no supported OAuth flow available" message) when it fails.
    /// That is right for "this flow was not available" and WRONG for "this flow
    /// detected an attack", for two reasons:
    ///
    /// - It destroys the stable programmatic identity. A caller that branches on
    ///   `err.is_iss_mismatch()` — the whole reason those markers exist — would
    ///   see a generic internal error instead, and message-substring matching is
    ///   exactly what the markers replaced.
    /// - It re-attempts authentication against an authorization server whose
    ///   response just failed a mix-up or CSRF check, handing the same attacker
    ///   a second attempt through a different grant.
    ///
    /// Every other failure keeps its existing fallback behaviour untouched.
    fn is_terminal_authorization_refusal(error: &Error) -> bool {
        error.is_iss_mismatch() || error.is_state_mismatch()
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
    async fn discover_metadata_with_extras(
        &self,
        mcp_url: &str,
    ) -> Result<(OidcDiscoveryMetadata, AuthorizationServerExtras)> {
        let base_url = Self::extract_base_url(mcp_url)?;

        tracing::info!("Discovering OAuth configuration from {}...", base_url);

        let discovery_client = OidcDiscoveryClient::new();

        match discovery_client.discover_with_extras(&base_url).await {
            Ok((metadata, extras)) => {
                tracing::info!("OAuth discovery successful");
                tracing::debug!("Issuer: {}", metadata.issuer);
                if let Some(ref device_endpoint) = metadata.device_authorization_endpoint {
                    tracing::debug!("Device endpoint: {}", device_endpoint);
                }
                Ok((metadata, extras))
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
    ///
    /// Delegates to [`Self::get_metadata_with_extras`] and discards the RFC 9207
    /// flag, so callers that do not resolve `iss` policy are untouched.
    async fn get_metadata(&self) -> Result<OidcDiscoveryMetadata> {
        self.get_metadata_with_extras()
            .await
            .map(|(metadata, _)| metadata)
    }

    /// Get OAuth metadata together with the discovery-only values that do not
    /// fit on [`OidcDiscoveryMetadata`] — in particular RFC 9207's
    /// `authorization_response_iss_parameter_supported`, which is tier 3 of the
    /// `iss` precedence chain.
    async fn get_metadata_with_extras(
        &self,
    ) -> Result<(OidcDiscoveryMetadata, AuthorizationServerExtras)> {
        if let Some(ref mcp_url) = self.config.mcp_server_url {
            // Discover from MCP server URL
            self.discover_metadata_with_extras(mcp_url).await
        } else if let Some(ref issuer) = self.config.issuer {
            // Manually provided issuer - try to discover from it
            tracing::info!("Discovering OAuth configuration from {}...", issuer);

            let discovery_client = OidcDiscoveryClient::new();
            match discovery_client.discover_with_extras(issuer).await {
                Ok(found) => {
                    tracing::info!("OAuth discovery successful");
                    Ok(found)
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
    /// should use [`authorize_with_details`](Self::authorize_with_details) instead.
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

        // Get metadata to see what flows are supported. The extras carry the
        // RFC 9207 flag, which is tier 3 of the `iss` precedence chain.
        let (metadata, extras) = self.get_metadata_with_extras().await?;
        let iss_presence = self.resolve_iss_presence(extras.iss_parameter_supported());

        // Try authorization code flow first (more common, works with MCP Inspector-like servers)
        match self.authorization_code_flow(&metadata, iss_presence).await {
            Ok(token) => Ok(token),
            Err(e) if Self::is_terminal_authorization_refusal(&e) => Err(e),
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
    /// Drives DCR lazily when eligible; runs PKCE via the authorization_code
    /// flow; captures `refresh_token`, `expires_at`, `scopes`, and the
    /// effective issuer + client_id.
    ///
    /// # Device-code fallback
    ///
    /// If the authorization-code flow fails and the server advertises a
    /// `device_authorization_endpoint`, this method falls back to device code
    /// flow (RFC 8628). In that case, `refresh_token` may be `None` since
    /// RFC 8628 §3.5 does not require it, and `scopes` falls back to the
    /// requested scopes when the token response does not echo them.
    pub async fn authorize_with_details(&self) -> Result<AuthorizationResult> {
        let (metadata, extras) = self.get_metadata_with_extras().await?;
        let iss_presence = self.resolve_iss_presence(extras.iss_parameter_supported());

        // Effective issuer: prefer the caller-provided config.issuer; fall back
        // to discovery metadata.issuer. metadata.issuer is always populated by
        // OIDC-compliant servers.
        //
        // NOTE: this is the value REPORTED to the caller for cache persistence,
        // and it is deliberately NOT the RFC 9207 comparison anchor. The anchor
        // is `metadata.issuer` alone — see `authorization_code_flow_inner`.
        let effective_issuer = self
            .config
            .issuer
            .clone()
            .or_else(|| Some(metadata.issuer.clone()));

        // The scope this flow ACTUALLY requested at the authorization request —
        // `config.scopes` plus `offline_access` when the server advertises it.
        // It is composed by the same function `build_authorization_url` uses, so
        // the value recorded as "requested" cannot drift from the value sent.
        // RFC 6749 §5.1 makes this the granted scope when the token response
        // omits `scope`, so getting it from `config.scopes` alone would silently
        // narrow every subsequent refresh.
        let requested_scopes =
            compose_scopes_with_offline_access(&self.config.scopes, &metadata.scopes_supported);

        // Try authorization code flow first (returns the full TokenResponse).
        match self
            .authorization_code_flow_inner(&metadata, iss_presence)
            .await
        {
            Ok((token_response, resolved_client_id)) => Ok(Self::build_auth_result(
                token_response,
                resolved_client_id,
                effective_issuer,
                &requested_scopes,
            )),
            Err(e) if Self::is_terminal_authorization_refusal(&e) => Err(e),
            Err(e) => {
                tracing::warn!("Authorization code flow failed: {}", e);

                // Device flow only returns an access_token string via the legacy
                // path — `refresh_token` / full `TokenResponse` are unavailable.
                // See the rustdoc on this function for the device-code caveat.
                if metadata.device_authorization_endpoint.is_some() {
                    tracing::info!(
                        "Trying device code flow (refresh_token may be None per RFC 8628)..."
                    );
                    // Resolve client_id the same way authorization_code would.
                    let resolved_client_id = self.resolve_client_id_for_flow(&metadata).await?;
                    let access_token = self.device_code_flow_with_metadata(&metadata).await?;
                    // Device flow returns only the access_token — populate what
                    // we know, leave refresh_token/expires_at/scopes at defaults.
                    // `scopes` stays `config.scopes`: the device grant never
                    // builds an authorization URL, so `offline_access` was never
                    // requested on this path and recording it would be a lie.
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

    /// Helper to construct an `AuthorizationResult` from a `TokenResponse`.
    ///
    /// `requested_scopes` must be the scope the flow ACTUALLY sent at the
    /// authorization request — including `offline_access` when it was added —
    /// because RFC 6749 §5.1 makes it the granted scope whenever the token
    /// response omits `scope`.
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

        // The GRANTED scope, per RFC 6749 §5.1. The two branches look
        // interchangeable and are NOT, which is why each names its own rule:
        //
        // - `scope` PRESENT: that string IS the granted scope. It is recorded
        //   verbatim even when it is NARROWER than what was asked for — the
        //   authorization server is entitled to downgrade, and assuming the
        //   request was honoured is exactly the tampering assumption T-116-38b
        //   names.
        // - `scope` ABSENT: RFC 6749 §5.1 says the parameter is OPTIONAL "if
        //   identical to the scope requested by the client", so an omission
        //   means the request was granted in full and the REQUESTED scope is
        //   what was granted.
        //
        // 116-12 refreshes with this value and nothing else, so a wrong branch
        // here silently re-widens or narrows every subsequent refresh.
        let granted_scopes = match token_response.scope.as_deref() {
            Some(granted) => granted
                .split_whitespace()
                .map(String::from)
                .collect::<Vec<_>>(),
            None => requested_scopes.to_vec(),
        };

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
    async fn authorization_code_flow(
        &self,
        metadata: &OidcDiscoveryMetadata,
        iss_presence: IssPresence,
    ) -> Result<String> {
        let (token_response, _client_id) = self
            .authorization_code_flow_inner(metadata, iss_presence)
            .await?;
        Ok(token_response.access_token)
    }

    /// Bind the loopback callback listener and return it with the redirect URI
    /// that must be advertised to the authorization server.
    ///
    /// The advertised URL and the bind address MUST be the literal `127.0.0.1`
    /// (not `localhost`), otherwise browsers can resolve `localhost` to `::1`
    /// (IPv6) and hit `ERR_CONNECTION_REFUSED` on our IPv4-only listener.
    async fn bind_callback_listener(redirect_port: u16) -> Result<(TcpListener, String)> {
        let redirect_uri = format!("http://127.0.0.1:{}/callback", redirect_port);

        let listener = TcpListener::bind(format!("127.0.0.1:{}", redirect_port))
            .await
            .map_err(|e| {
                Error::internal(format!(
                    "Failed to bind to 127.0.0.1:{}.\n\
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

        Ok((listener, redirect_uri))
    }

    /// Build the authorization URL from the per-request record.
    ///
    /// `state` and `code_challenge` both come from the record, so the value in
    /// the URL and the value that will be compared on the callback cannot
    /// diverge.
    fn build_authorization_url(
        &self,
        metadata: &OidcDiscoveryMetadata,
        client_id: &str,
        redirect_uri: &str,
        record: &AuthorizationRequestRecord,
    ) -> Result<Url> {
        let mut auth_url = Url::parse(&metadata.authorization_endpoint)
            .map_err(|e| Error::internal(format!("Invalid authorization endpoint: {e}")))?;

        // SEP-2207: THIS is the stage at which requesting `offline_access` means
        // anything. Declaring it in the registration says what the client may
        // ask for; this is the ask. Composed from `config.scopes` — never by
        // mutating it, because a caller who reuses one `OAuthConfig` across two
        // flows must not watch its public `scopes` field grow.
        let requested_scopes =
            compose_scopes_with_offline_access(&self.config.scopes, &metadata.scopes_supported);

        auth_url
            .query_pairs_mut()
            .append_pair("client_id", client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", &requested_scopes.join(" "))
            .append_pair(
                "code_challenge",
                &Self::generate_code_challenge(record.code_verifier()),
            )
            .append_pair("code_challenge_method", "S256")
            // The CSRF `state` is the record's — a BOUND value, comparable on
            // the callback. It is produced by `generate_state()`, never by the
            // PKCE verifier generator: RFC 7636's verifier and RFC 6749 §10.12's
            // `state` are distinct roles and conflating their generators hides
            // that one of them is never checked.
            .append_pair("state", record.state());

        Ok(auth_url)
    }

    /// Read one HTTP request line, refusing anything over
    /// [`MAX_CALLBACK_REQUEST_LINE_BYTES`].
    ///
    /// The cap is applied at the socket by a limited reader, so an oversized
    /// line is refused without ever being allocated in full. There is
    /// deliberately no unbounded read here: any local process can connect to
    /// this port.
    async fn read_request_line_within_cap(stream: &mut TcpStream) -> Result<String> {
        let mut limited = BufReader::new(stream).take(MAX_CALLBACK_REQUEST_LINE_BYTES as u64 + 1);
        let mut raw = Vec::with_capacity(256);

        limited
            .read_until(b'\n', &mut raw)
            .await
            .map_err(|e| Error::internal(format!("Failed to read OAuth callback request: {e}")))?;

        if raw.len() > MAX_CALLBACK_REQUEST_LINE_BYTES {
            return Err(Error::internal(format!(
                "OAuth callback request line exceeds the \
                 MAX_CALLBACK_REQUEST_LINE_BYTES limit of {MAX_CALLBACK_REQUEST_LINE_BYTES} \
                 bytes; refused at the socket, and none of it is reproduced here"
            )));
        }

        String::from_utf8(raw)
            .map_err(|_| Error::internal("OAuth callback request line is not UTF-8".to_string()))
    }

    /// Isolate the query component of a callback request line.
    ///
    /// The request line is `GET /callback?<query> HTTP/1.1`; parsing it against
    /// a dummy origin is a convenient way to split the path from the query. The
    /// raw query is handed on undecoded — the percent-decode belongs to the
    /// validator, which performs the `application/x-www-form-urlencoded` decode
    /// RFC 9207 §2.4 requires. Never hand-roll it here.
    fn callback_query_from_request_line(request_line: &str) -> Result<String> {
        let path = request_line.split_whitespace().nth(1).ok_or_else(|| {
            Error::internal("OAuth callback request line has no request target".to_string())
        })?;

        let callback_url = Url::parse(&format!("http://localhost{}", path)).map_err(|e| {
            Error::internal(format!("OAuth callback request target is unparseable: {e}"))
        })?;

        Ok(callback_url.query().unwrap_or_default().to_string())
    }

    /// Accept ONE loopback callback, validate it, and only then write a
    /// response.
    ///
    /// The ordering is the security property, not a style choice. Reading,
    /// isolating the query and validating all happen BEFORE any response byte is
    /// committed, so the page the human sees and the code the caller receives are
    /// consequences of the SAME `Result`. Serving a success page first and
    /// validating afterwards would teach a user to trust a login that was about
    /// to be rejected, and would make the failure page unselectable — the task
    /// cannot know the outcome at the moment it must choose which page to write.
    async fn serve_one_callback(
        listener: TcpListener,
        record: &AuthorizationRequestRecord,
    ) -> Result<String> {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| Error::internal(format!("Failed to accept OAuth callback: {e}")))?;

        // Steps 1-3: bounded read, isolate the query, validate through the pure
        // tier. Not one byte has been written to the socket yet.
        let outcome = match Self::read_request_line_within_cap(&mut stream).await {
            Ok(request_line) => Self::callback_query_from_request_line(&request_line)
                .and_then(|raw_query| validate_authorization_response(&raw_query, record)),
            Err(e) => Err(e),
        };

        // Step 4: the page is chosen by the outcome that already exists. Neither
        // page carries authorization-server-supplied text.
        let response = if outcome.is_ok() {
            CALLBACK_SUCCESS_RESPONSE
        } else {
            CALLBACK_FAILURE_RESPONSE
        };
        let _ = stream.write_all(response.as_bytes()).await;
        let _ = stream.flush().await;

        outcome
    }

    /// Wait for a validated authorization code, or the typed refusal that says
    /// why there will not be one.
    ///
    /// The channel carries a `Result`, so the caller's only route to the token
    /// exchange is the `Ok` branch. The record is CLONED into the listener task
    /// rather than shared behind an `Arc`: it is a small owned struct of two
    /// short strings, an issuer and a copy enum, and cloning keeps the task
    /// `'static` without adding a second ownership story.
    async fn await_validated_authorization_code(
        listener: TcpListener,
        record: AuthorizationRequestRecord,
    ) -> Result<String> {
        let (tx, rx) = oneshot::channel::<Result<String>>();
        let callback_task = tokio::spawn(async move {
            let _ = tx.send(Self::serve_one_callback(listener, &record).await);
        });

        tracing::info!("Waiting for authorization...");

        let received = tokio::time::timeout(Duration::from_mins(5), rx)
            .await
            .map_err(|_| {
                Error::internal("Timeout waiting for OAuth callback (5 minutes)".to_string())
            })?
            .map_err(|e| Error::internal(format!("OAuth callback channel error: {e}")))?;

        callback_task.abort();

        received
    }

    /// Inner PKCE authorization code flow returning the full token response.
    ///
    /// Returns (`TokenResponse`, `resolved_client_id`) so `authorize_with_details`
    /// can populate `AuthorizationResult` fields including `refresh_token`,
    /// `expires_at`, `scopes`, and the effective `client_id`.
    ///
    /// `iss_presence` is resolved by the caller through
    /// [`Self::resolve_iss_presence`] and passed in, so the environment is read
    /// once per flow rather than once per layer.
    async fn authorization_code_flow_inner(
        &self,
        metadata: &OidcDiscoveryMetadata,
        iss_presence: IssPresence,
    ) -> Result<(crate::client::auth::TokenResponse, String)> {
        tracing::info!("Starting OAuth authorization code flow...");

        let resolved_client_id = self.resolve_client_id_for_flow(metadata).await?;

        // The specification's mandated per-request record: the expected issuer,
        // the PKCE verifier, the CSRF `state` and the `iss` policy, bound into
        // ONE value. Three separate locals is exactly how `state` came to be an
        // unnamed temporary that nothing could compare.
        //
        // The anchor is `metadata.issuer` — the value the authorization server
        // published in its OWN discovery document, validated against the issuer
        // used to build the discovery URL at fetch time. It is deliberately NOT
        // `config.issuer` (a user-typed discovery seed) and not the effective
        // issuer reported to cache consumers: the attack being defended against
        // is "this response came from a different authorization server than the
        // one whose metadata I fetched".
        let record = AuthorizationRequestRecord::new(
            metadata.issuer.clone(),
            Self::generate_code_verifier(),
            generate_state()?,
            iss_presence,
        );

        let (listener, redirect_uri) =
            Self::bind_callback_listener(self.config.redirect_port).await?;

        let auth_url =
            self.build_authorization_url(metadata, &resolved_client_id, &redirect_uri, &record)?;

        tracing::info!("OAuth Authentication Required");
        tracing::info!("Opening browser for authentication...");
        tracing::info!("If the browser doesn't open, visit: {}", auth_url.as_str());

        // A launcher that cannot deliver the URL to a human at all aborts here,
        // rather than leaving the flow waiting five minutes for a callback
        // nobody will deliver.
        self.browser_launcher.open(auth_url.as_str())?;

        // Validation happens INSIDE the listener, before any response byte is
        // written. An `Err` here is unreachable-from-redemption by construction:
        // the token exchange below is only reachable on the `Ok` branch.
        let authorization_code =
            Self::await_validated_authorization_code(listener, record.clone()).await?;

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
                Some(record.code_verifier()), // PKCE verifier, from the record
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
        let resolved_client_id = self.resolve_client_id_for_flow(metadata).await?;

        // Step 1: Request device code. `offline_access` is deliberately NOT
        // composed in here: the device grant never builds an authorization URL,
        // and SEP-2207's request stage is that URL.
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
        // at `pmcp::client::oauth::*`.
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
            .do_dynamic_client_registration("http://attacker.example/register", &metadata(None))
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
        // Serde-level guard: `DcrRequest` has
        // `#[serde(skip_serializing_if = "Vec::is_empty")]` on `response_types`,
        // so an accidental empty-Vec default would silently drop the field.
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
    async fn dcr_advertises_127_0_0_1_redirect_not_localhost() {
        // Regression guard: advertising `http://localhost:<port>` causes browsers
        // to resolve to `::1` (IPv6) and miss the IPv4-only callback listener.
        // The mock only matches when the wire body pins `127.0.0.1`; a regression
        // back to `localhost` makes this mock return 501 and the DCR call errors.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/register")
            .match_body(mockito::Matcher::PartialJsonString(
                serde_json::json!({
                    "redirect_uris": ["http://127.0.0.1:8080/callback"]
                })
                .to_string(),
            ))
            .with_status(201)
            .with_body(r#"{"client_id":"ok"}"#)
            .create_async()
            .await;
        let helper = OAuthHelper::new(OAuthConfig {
            dcr_enabled: true,
            redirect_port: 8080,
            ..OAuthConfig::default()
        })
        .unwrap();
        let result = helper
            .do_dynamic_client_registration(&format!("{}/register", server.url()), &metadata(None))
            .await;
        assert!(
            result.is_ok(),
            "DCR body did not pin 127.0.0.1 redirect_uri"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn dcr_accepts_ipv6_loopback_registration_endpoint() {
        // The guard must accept `[::1]` alongside `localhost` and `127.0.0.1`.
        // It rejects BEFORE the HTTP call, so a connection failure on port 9
        // is the expected non-scheme-guard error here.
        let cfg = OAuthConfig {
            dcr_enabled: true,
            ..OAuthConfig::default()
        };
        let helper = OAuthHelper::new(cfg).unwrap();
        let err = helper
            .do_dynamic_client_registration("http://[::1]:9/register", &metadata(None))
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
            .do_dynamic_client_registration("http://localhost:9/register", &metadata(None))
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
            .do_dynamic_client_registration("http://127.0.0.1:9/register", &metadata(None))
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

/// SEP-837 and SEP-2207 composition rules, pinned at the level they are
/// DECIDED at rather than at the level they are observed at.
///
/// Each of these three helpers is a private pure function precisely so its rule
/// can be asserted without a network, a log subscriber or a new public field.
/// The wire-level consequences are asserted separately in
/// `tests/oauth_dcr_integration.rs`; a rule that only had wire coverage would be
/// the shape 116-09 measured and named — a suite that is green over a defect it
/// structurally cannot see.
#[cfg(test)]
mod sep837_sep2207_composition_tests {
    use super::*;

    fn dcr_request_registering(redirect_uris: &[&str]) -> DcrRequest {
        DcrRequest {
            redirect_uris: redirect_uris.iter().map(|u| (*u).to_string()).collect(),
            client_name: Some("pmcp-sdk".to_string()),
            client_uri: None,
            logo_uri: None,
            contacts: vec![],
            token_endpoint_auth_method: Some("none".to_string()),
            grant_types: vec![
                "authorization_code".to_string(),
                "refresh_token".to_string(),
            ],
            response_types: vec!["code".to_string()],
            scope: None,
            software_id: None,
            software_version: None,
            extra: Default::default(),
        }
    }

    fn owned(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| (*v).to_string()).collect()
    }

    // -----------------------------------------------------------------------
    // apply_application_type — derivation, and the override that outranks it
    // -----------------------------------------------------------------------

    #[test]
    fn a_loopback_redirect_derives_native() {
        let mut request = dcr_request_registering(&["http://127.0.0.1:8080/callback"]);
        let sent = apply_application_type(&mut request).expect("loopback derives");
        assert_eq!(sent, "native");
        assert_eq!(request.application_type(), Some("native"));
    }

    #[test]
    fn an_https_non_loopback_redirect_derives_web() {
        // pmcp's own flow only ever registers a loopback URI, so this row is
        // unreachable end to end and is exercised directly — a platform
        // `oauth-proxy` registering an https redirect is the real caller.
        let mut request = dcr_request_registering(&["https://proxy.example.com/callback"]);
        let sent = apply_application_type(&mut request).expect("https non-loopback derives");
        assert_eq!(sent, "web");
        assert_eq!(request.application_type(), Some("web"));
    }

    #[test]
    fn an_explicit_application_type_is_never_clobbered_by_the_derivation() {
        // D-09's documented override path. The redirect URI here would derive
        // `native`, so a clobbering implementation produces a DIFFERENT value
        // and this test is a real detector rather than a tautology.
        let mut request = dcr_request_registering(&["http://127.0.0.1:8080/callback"]);
        request.set_application_type("web");
        let sent = apply_application_type(&mut request).expect("an override is not re-derived");
        assert_eq!(sent, "web");
        assert_eq!(request.application_type(), Some("web"));
    }

    #[test]
    fn a_mixed_redirect_vector_is_an_error_and_never_a_pick() {
        let mut request = dcr_request_registering(&[
            "http://127.0.0.1:8080/callback",
            "https://proxy.example.com/callback",
        ]);
        let err = apply_application_type(&mut request)
            .expect_err("D-10: a mixed vector is an ERROR, never a silent choice");
        let message = err.to_string();
        assert!(
            message.contains("127.0.0.1"),
            "names the native URI: {message}"
        );
        assert!(
            message.contains("proxy.example.com"),
            "names the web URI: {message}"
        );
        assert_eq!(
            request.application_type(),
            None,
            "a refused derivation must not leave a half-written value on the request"
        );
    }

    // -----------------------------------------------------------------------
    // compose_scopes_with_offline_access — SEP-2207's advertise-conditioned add
    // -----------------------------------------------------------------------

    #[test]
    fn offline_access_is_added_only_when_the_server_advertises_it() {
        let configured = owned(&["openid", "profile"]);
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &owned(&["openid", "offline_access"])),
            owned(&["openid", "profile", "offline_access"]),
            "advertised: appended last, configured order preserved"
        );
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &owned(&["openid", "profile"])),
            owned(&["openid", "profile"]),
            "NOT advertised: absent, because SEP-2207 conditions the request on support"
        );
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &[]),
            owned(&["openid", "profile"]),
            "an empty scopes_supported advertises nothing"
        );
    }

    #[test]
    fn an_already_configured_offline_access_is_not_duplicated() {
        let configured = owned(&["openid", "offline_access"]);
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &owned(&["offline_access"])),
            owned(&["openid", "offline_access"]),
            "a duplicated scope token is legal but is what a strict server rejects"
        );
    }

    #[test]
    fn the_configured_scopes_are_never_mutated_and_never_accumulate() {
        let configured = owned(&["openid"]);
        let advertised = owned(&["offline_access"]);

        let first = compose_scopes_with_offline_access(&configured, &advertised);
        let second = compose_scopes_with_offline_access(&configured, &advertised);

        assert_eq!(
            configured,
            owned(&["openid"]),
            "`OAuthConfig::scopes` is a public field; a caller reusing one config \
             across two flows must not watch it grow"
        );
        assert_eq!(
            first, second,
            "two flows compose the same value, not a longer one"
        );
        assert_eq!(first, owned(&["openid", "offline_access"]));
    }

    #[test]
    fn duplicate_configured_scopes_collapse_to_one_entry() {
        let configured = owned(&["openid", "openid", "profile"]);
        assert_eq!(
            compose_scopes_with_offline_access(&configured, &[]),
            owned(&["openid", "profile"])
        );
    }
}

#[cfg(test)]
mod dcr_proptest {
    use super::*;
    use proptest::prelude::*;

    fn arb_dcr_request() -> impl Strategy<Value = crate::server::auth::provider::DcrRequest> {
        (
            prop::collection::vec("[a-z][a-z0-9-]{2,30}", 1..3),
            prop::option::of("[a-zA-Z][a-zA-Z0-9 _-]{1,40}"),
            prop::option::of(
                prop::string::string_regex("(none|client_secret_basic|client_secret_post)")
                    .unwrap(),
            ),
        )
            .prop_map(|(uris, name, auth_method)| {
                let redirect_uris = uris
                    .into_iter()
                    .map(|u| format!("http://localhost:8080/{u}"))
                    .collect();
                crate::server::auth::provider::DcrRequest {
                    redirect_uris,
                    client_name: name,
                    client_uri: None,
                    logo_uri: None,
                    contacts: vec![],
                    token_endpoint_auth_method: auth_method,
                    grant_types: vec!["authorization_code".into()],
                    response_types: vec!["code".into()],
                    scope: None,
                    software_id: None,
                    software_version: None,
                    extra: Default::default(),
                }
            })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn dcr_request_serde_roundtrip(req in arb_dcr_request()) {
            let v = serde_json::to_value(&req).unwrap();
            let back: crate::server::auth::provider::DcrRequest =
                serde_json::from_value(v).unwrap();
            prop_assert_eq!(req.redirect_uris, back.redirect_uris);
            prop_assert_eq!(req.client_name, back.client_name);
            prop_assert_eq!(req.token_endpoint_auth_method, back.token_endpoint_auth_method);
        }

        #[test]
        fn oauth_config_builder_allows_all_combinations(
            has_id in any::<bool>(),
            has_name in any::<bool>(),
            dcr in any::<bool>(),
        ) {
            let cfg = OAuthConfig {
                client_id: has_id.then(|| "id".into()),
                client_name: has_name.then(|| "name".into()),
                dcr_enabled: dcr,
                mcp_server_url: Some("https://x.example".into()),
                ..OAuthConfig::default()
            };
            OAuthHelper::new(cfg).unwrap();
        }
    }
}

// In-tree proptest smoke check for DCR response parsing. The authoritative
// robustness gate is the cargo-fuzz target at `fuzz/fuzz_targets/dcr_response_parser.rs`
// (CLAUDE.md ALWAYS / FUZZ Testing). This module exists for fast per-PR
// regression coverage that runs as part of `cargo test`.
#[cfg(test)]
mod dcr_parser_fuzz {
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]
        #[test]
        fn parser_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
            // Must return Result, never panic. Error paths are acceptable.
            let _ = serde_json::from_slice::<
                crate::server::auth::provider::DcrResponse
            >(&bytes);
        }

        #[test]
        fn parser_accepts_minimal_valid_response(
            id in "[a-zA-Z0-9-]{8,40}",
            has_secret in any::<bool>(),
        ) {
            let mut v = serde_json::json!({"client_id": id});
            if has_secret {
                v["client_secret"] = serde_json::json!("s3cret");
            }
            let parsed: crate::server::auth::provider::DcrResponse =
                serde_json::from_value(v).unwrap();
            prop_assert_eq!(parsed.client_id, id);
            prop_assert_eq!(parsed.client_secret.is_some(), has_secret);
        }
    }
}
