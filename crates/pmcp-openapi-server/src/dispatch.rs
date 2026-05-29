//! Backend dispatch: `[backend]` → `(Arc<dyn HttpConnector>, HttpCodeExecutor)`.
//!
//! This is the OpenAPI analog of the SQL binary's `[database] type` →
//! `Arc<dyn SqlConnector>` seam, with one structural difference: the OpenAPI
//! binary serves BOTH a single-call connector surface (`HttpConnector`, Plan 03)
//! AND a Code-Mode / script-tool execution surface (`HttpCodeExecutor`, Plan 04)
//! over the SAME backend. So [`dispatch`] returns the PAIR — a shared
//! `reqwest::Client` + auth provider are built ONCE and threaded into both.
//!
//! # Lazy startup (CF-2)
//!
//! Construction is offline-safe: the `reqwest::Client` is built without
//! contacting the backend, the auth provider is constructed statically, and the
//! `HttpClient` only parses the `base_url`. No spec read, no backend request,
//! and no network call is made at dispatch time — the backend is contacted only
//! on the first tool invocation.
//!
//! # `oauth_passthrough` runtime forwarding (Plan 90-10 / OAPI-03 / OAPI-05)
//!
//! For an `oauth_passthrough` backend, dispatch installs an
//! `OAuthPassthroughAuth` provider via
//! [`create_passthrough_auth_provider`](pmcp_server_toolkit::http::auth::create_passthrough_auth_provider)
//! holding NO construction-time token — the per-request inbound MCP token is
//! threaded in via `apply`'s `inbound_token` from the toolkit handler seam
//! (`request_executor_from_extra`). This is what makes the captured token
//! actually reach `target_header` at runtime; the previous `create_auth_provider`
//! installed a `MissingTokenAuth`/`NoAuth` provider that never forwarded the
//! token. Non-passthrough configs are unaffected:
//! `create_passthrough_auth_provider` delegates to `create_auth_provider` for
//! every other `AuthConfig` variant.
//!
//! # Credential safety (V7 / Pitfall 5 / T-90-06-01)
//!
//! [`DispatchError`]'s `Display` NEVER echoes the backend `base_url`, connection
//! URLs, or any credential substring from the config — it names the backend /
//! field only. The wrapped [`HttpConnectorError`] is already credential-redacted
//! at its source (the toolkit's auth/client constructors strip secrets before
//! constructing it).

use std::sync::Arc;

use pmcp_server_toolkit::code_mode::HttpCodeExecutor;
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::http::auth::create_passthrough_auth_provider;
use pmcp_server_toolkit::http::{HttpClient, HttpConnector, HttpConnectorError};

/// Error returned when [`dispatch`] cannot produce the connector/executor pair.
///
/// # Security
///
/// `Display` names the backend / field ONLY. It MUST NOT echo the backend
/// `base_url`, any connection URL, or any credential substring from the config
/// (V7 / Pitfall 5 / T-90-06-01). The wrapped [`HttpConnectorError`] is redacted
/// at the toolkit source.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DispatchError {
    /// The config declares no `[backend]` section, so there is no REST API to
    /// dispatch to. Names no value.
    #[error("[backend] section is required (declare base_url + optional [backend.auth])")]
    MissingBackend,

    /// Constructing the outgoing auth provider from `[backend.auth]` failed.
    /// The wrapped error is credential-redacted at the toolkit source.
    #[error("backend auth provider construction failed: {0}")]
    Auth(#[source] HttpConnectorError),

    /// Constructing the single-call [`HttpConnector`] failed (e.g. an
    /// unparseable `base_url`). The wrapped error does NOT echo the URL.
    #[error("backend connector construction failed: {0}")]
    Connector(#[source] HttpConnectorError),
}

/// Select and construct the `(HttpConnector, HttpCodeExecutor)` pair for the
/// configured `[backend]`.
///
/// Reads `cfg.backend` (error [`DispatchError::MissingBackend`] when absent),
/// builds the outgoing auth provider via
/// [`create_passthrough_auth_provider`](pmcp_server_toolkit::http::auth::create_passthrough_auth_provider)
/// (an `oauth_passthrough` backend installs an `OAuthPassthroughAuth` provider
/// holding NO construction-time token — its per-request token is threaded by the
/// toolkit handler seam, Plan 90-10; every other config delegates to
/// `create_auth_provider`), builds a shared `reqwest::Client` (lazy — no
/// network, CF-2), and constructs
/// both an [`HttpClient`] (single-call connector, Plan 03) and an
/// [`HttpCodeExecutor`] (Code-Mode / script-tool execution surface, Plan 04)
/// over the SAME client + base_url + auth. Returns the pair.
///
/// Construction is offline-safe: no spec read, no backend request, and no
/// network call is made here — the backend is contacted only on the first tool
/// invocation (CF-2).
///
/// # Errors
///
/// - [`DispatchError::MissingBackend`] when `[backend]` is absent.
/// - [`DispatchError::Auth`] when the auth provider cannot be built.
/// - [`DispatchError::Connector`] when the single-call connector cannot be built
///   (e.g. an unparseable `base_url`).
pub async fn dispatch(
    cfg: &ServerConfig,
) -> Result<(Arc<dyn HttpConnector>, HttpCodeExecutor), DispatchError> {
    let backend = cfg.backend.as_ref().ok_or(DispatchError::MissingBackend)?;

    // Auth construction (Plan 90-10 / H1): for an `oauth_passthrough` backend
    // this installs an `OAuthPassthroughAuth` provider holding NO token here; the
    // per-request inbound MCP token is threaded in by the toolkit handler seam
    // (`request_executor_from_extra` → `HttpCodeExecutor::with_inbound_token` →
    // `apply`'s `inbound_token`) so it actually reaches `target_header`. Every
    // non-passthrough config delegates to `create_auth_provider` (unchanged).
    let auth = create_passthrough_auth_provider(&backend.auth, None).map_err(DispatchError::Auth)?;

    // Lazy (CF-2): the reqwest client is built without contacting the backend.
    // Shared by BOTH the single-call connector and the Code-Mode executor so a
    // single connection pool serves the whole binary.
    let client = reqwest::Client::new();

    // Single-call connector (Plan 03). LAZY: parses base_url, no network.
    let connector = HttpClient::new(client.clone(), backend.base_url.clone(), auth.clone())
        .map_err(DispatchError::Connector)?;
    let connector: Arc<dyn HttpConnector> = Arc::new(connector);

    // Code-Mode / script-tool execution surface (Plan 04). The SAME client +
    // base_url + auth — D-02 (one engine feeds tools + code-mode).
    let http_exec = HttpCodeExecutor::new(client, backend.base_url.clone(), auth);

    Ok((connector, http_exec))
}

#[cfg(test)]
mod tests {
    use super::{dispatch, DispatchError};
    use pmcp_server_toolkit::config::ServerConfig;

    /// A london-tube-shaped config with a `[backend]` block (base_url + no auth).
    fn cfg_with_backend() -> ServerConfig {
        let toml = r#"
[server]
name = "tube"
version = "0.1.0"

[backend]
base_url = "https://api.tfl.gov.uk"

[[tools]]
name = "get_line_status"
description = "Status for a tube line"
path = "/Line/{id}/Status"
method = "GET"

[[tools.parameters]]
name = "id"
type = "string"
required = true
"#;
        ServerConfig::from_toml_strict_validated(toml).expect("parse")
    }

    #[tokio::test]
    async fn dispatch_builds_pair_offline_without_network() {
        // CF-2: dispatch must build the connector+executor pair with NO network
        // call (no spec read, no backend request). The wiremock-free, fast
        // resolution here is the offline proof — a real backend contact would
        // hang/fail.
        let cfg = cfg_with_backend();
        let result = dispatch(&cfg).await;
        assert!(
            result.is_ok(),
            "dispatch must build the pair offline (CF-2): {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn dispatch_missing_backend_is_an_error() {
        let toml = r#"
[server]
name = "t"
version = "0.1.0"
"#;
        let cfg = ServerConfig::from_toml_strict_validated(toml).expect("parse");
        let err = dispatch(&cfg)
            .await
            .err()
            .expect("missing backend must error");
        assert!(
            matches!(err, DispatchError::MissingBackend),
            "absent [backend] yields MissingBackend, got {err:?}"
        );
    }

    #[test]
    fn dispatch_error_display_redacts_backend_and_secrets() {
        // Pitfall 5 / T-90-06-01: no DispatchError Display may echo the backend
        // base_url, a connection URL, or any credential substring. We assert the
        // backend base_url itself is absent (Codex LOW).
        let base_url = "https://api.tfl.gov.uk";
        let secret = "super-secret-token";
        let errors = [
            DispatchError::MissingBackend,
            DispatchError::Auth(pmcp_server_toolkit::http::HttpConnectorError::Backend(
                "invalid base URL".to_string(),
            )),
            DispatchError::Connector(pmcp_server_toolkit::http::HttpConnectorError::Backend(
                "invalid base URL".to_string(),
            )),
        ];
        for err in &errors {
            let rendered = format!("{err}");
            assert!(
                !rendered.contains(base_url),
                "DispatchError Display leaked the backend base_url: {rendered}"
            );
            assert!(
                !rendered.contains(secret),
                "DispatchError Display leaked a credential: {rendered}"
            );
        }
    }
}
