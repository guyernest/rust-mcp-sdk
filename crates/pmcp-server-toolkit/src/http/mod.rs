// Net-new code for Phase 90 OAPI-01 (HttpConnector trait + HttpClient) +
// OAPI-03 (HttpAuthProvider + AuthConfig six modes). SHAPE lifted from the SQL
// connector analog (`crate::sql`); BODY lifted from the pmcp-run OpenAPI
// reference (`mcp-openapi-server-core`): the reference HTTP client + the shared
// `mcp-server-common` auth providers. The lift replaces the `mcp_server_common`
// path-dependency with toolkit-owned types.

//! HTTP backend primitives for config-driven OpenAPI MCP servers.
//!
//! This module is the backend seam the single-call synthesizer (Plan 03), the
//! code-mode executor (Plan 04), and the binary dispatch (Plan 06) build on. It
//! mirrors [`crate::sql`] in shape:
//!
//! - [`HttpConnector`] — the `#[async_trait] Send + Sync + 'static` trait that
//!   executes a REST [`Operation`] and returns JSON (analog of `SqlConnector`).
//! - [`HttpConnectorError`] — the `#[non_exhaustive]` error enum whose `Display`
//!   reaches MCP clients and therefore MUST NOT echo credentials or URLs (analog
//!   of `ConnectorError`, mirrors its Connection Security doc-comment).
//! - [`Operation`] / [`Parameter`] / [`ParameterLocation`] — the request model
//!   the trait signature needs. Defined here in Wave 1; Plan 03 (OAPI-02) extends
//!   them from the `openapiv3` parse in [`schema`].
//! - [`join_url`] — the ONE shared `base_url` + `path` concatenation helper. Both
//!   [`client::HttpClient`] (this plan) and Plan 04's `HttpCodeExecutor` call it
//!   instead of re-inlining the trim logic — it preserves an API-Gateway stage
//!   prefix (`/v1`) where `Url::join` would silently drop it (Pitfall 2).
//!
//! The whole module is gated behind the opt-in `http` feature so the curated /
//! no-`http` toolkit build stays light (RESEARCH Pitfall 4).

// Why: HTTP method names ("GET", "POST") and product nouns ("OpenAPI") are
// proper nouns / acronyms clippy::doc_markdown otherwise flags for back-ticks.
#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Authentication providers for OUTGOING HTTP requests (OAPI-03 / D-05).
pub mod auth;
/// reqwest-backed [`HttpConnector`] implementation (OAPI-01).
pub mod client;
/// OpenAPI schema parsing seam — forward stub filled by Plan 03 (OAPI-02).
pub mod schema;

#[doc(inline)]
pub use auth::{
    create_auth_provider, create_passthrough_auth_provider, AuthConfig, HttpAuthProvider,
};
#[doc(inline)]
pub use client::{HttpClient, HttpConfig};

/// Concatenate a base URL and a request path with exactly one separating slash,
/// PRESERVING any non-root path already on the base.
///
/// This is the ONE shared URL-join helper for the `http` module (de-dup: both
/// [`client::HttpClient`] and Plan 04's `HttpCodeExecutor` call it). It is
/// deliberately NOT `Url::join`, which follows RFC 3986 and treats an absolute
/// request path (e.g. `/users`) as REPLACING the base path — that silently drops
/// an API-Gateway stage prefix like `/v1` (Pitfall 2 / T-90-01-05).
///
/// # Examples
///
/// ```
/// # // join_url is pub(crate); the behaviour is asserted in the module tests.
/// // join_url("https://x/v1", "/users") == "https://x/v1/users"
/// ```
#[must_use]
pub(crate) fn join_url(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// An extracted REST operation backed by an OpenAPI definition.
///
/// Defined here in Plan 01 so the [`HttpConnector::execute`] signature can name
/// it; Plan 03 (OAPI-02) populates these values from an `openapiv3` parse in
/// [`schema`] and MAY extend the struct additively. The shape mirrors the
/// pmcp-run reference `mcp-openapi-server-core::schema::Operation`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// HTTP method (`GET`, `POST`, ...).
    pub method: String,

    /// Path template, e.g. `"/users/{id}"`.
    pub path: String,

    /// Input parameters (path / query / header).
    #[serde(default)]
    pub parameters: Vec<Parameter>,

    /// Whether this operation expects a request body.
    #[serde(default)]
    pub has_request_body: bool,
}

impl Operation {
    /// Path parameters (the `{...}` segments of [`Operation::path`]).
    #[must_use]
    pub fn path_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Path)
            .collect()
    }

    /// Query parameters.
    #[must_use]
    pub fn query_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Query)
            .collect()
    }

    /// Header parameters.
    #[must_use]
    pub fn header_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.location == ParameterLocation::Header)
            .collect()
    }
}

/// A single OpenAPI operation parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    /// Parameter name (matches the `{name}` placeholder for path params).
    pub name: String,

    /// Where the parameter is carried in the request.
    pub location: ParameterLocation,

    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,
}

impl Parameter {
    /// Construct a parameter (test/parser convenience).
    #[must_use]
    pub fn new(name: impl Into<String>, location: ParameterLocation, required: bool) -> Self {
        Self {
            name: name.into(),
            location,
            required,
        }
    }
}

/// Where an [`Operation`] parameter is carried in the outgoing request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterLocation {
    /// Substituted into the path template (`/users/{id}`).
    Path,
    /// Appended to the query string.
    Query,
    /// Sent as a request header.
    Header,
}

/// Errors an [`HttpConnector`] implementation may surface.
///
/// The enum is `#[non_exhaustive]` so later plans can add failure modes
/// additively without a semver break (mirrors [`crate::sql::ConnectorError`]).
///
/// # Security
///
/// The inner `String` of every variant reaches MCP clients via `Display`.
/// Implementors MUST NOT include the request URL, an `Authorization` header
/// value, a bearer token, or an `app_key` in any inner `String` — those are
/// credentials or capability-bearing locators. Construct error messages from
/// non-secret context only (status code, a static reason). This mirrors the
/// `ConnectorError::Connection` discipline in `sql/mod.rs` (T-90-01-01).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HttpConnectorError {
    /// The outgoing request failed at the transport layer (connect / timeout /
    /// body read). The reqwest error is deliberately NOT forwarded verbatim —
    /// its `Display` can echo the URL — so this carries a redacted reason only.
    #[error("http request failed: {0}")]
    Request(String),

    /// The backend returned a non-2xx HTTP status.
    #[error("http backend returned status {status}")]
    Status {
        /// The HTTP status code (e.g. `401`, `503`).
        status: u16,
    },

    /// Authentication could not be applied to the outgoing request (e.g. a
    /// required passthrough token was absent). The reason MUST NOT echo the
    /// token or header value.
    #[error("authentication failed: {0}")]
    Auth(String),

    /// A header name or value could not be constructed from the configured /
    /// supplied value. The reason MUST NOT echo a credential header's value.
    #[error("invalid header: {0}")]
    InvalidHeader(String),

    /// A backend / configuration problem not covered by the variants above
    /// (e.g. an unparseable base URL, an unknown HTTP method).
    #[error("http backend error: {0}")]
    Backend(String),
}

/// Backend-agnostic HTTP connector trait (OAPI-01).
///
/// The analog of [`crate::sql::SqlConnector`] for REST backends: an
/// implementation executes an [`Operation`] against a configured base URL and
/// returns the response body as JSON. [`base_url`](HttpConnector::base_url) is
/// the analog of `SqlConnector::dialect()` — a cheap accessor used by the
/// synthesizer / prompt assembly.
///
/// # Example
///
/// A minimal connector. The example defines a LOCAL dummy struct so the doctest
/// does not depend on any downstream crate (mirrors the `SqlConnector` doctest).
///
/// ```no_run
/// use pmcp_server_toolkit::http::{HttpConnector, HttpConnectorError, Operation};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct Dummy;
///
/// #[async_trait]
/// impl HttpConnector for Dummy {
///     fn base_url(&self) -> &str { "https://api.example.com/v1" }
///     async fn execute(&self, _operation: &Operation, _args: &Value)
///         -> Result<Value, HttpConnectorError> {
///         Ok(Value::Null)
///     }
/// }
/// ```
#[async_trait]
pub trait HttpConnector: Send + Sync + 'static {
    /// Execute `operation` with the caller-supplied `args` (a JSON object whose
    /// keys map to path / query / header / body parameters) and return the
    /// response body as a [`serde_json::Value`].
    ///
    /// # Errors
    ///
    /// Returns [`HttpConnectorError`] when the request fails at the transport
    /// layer ([`HttpConnectorError::Request`]), the backend returns a non-2xx
    /// status ([`HttpConnectorError::Status`]), authentication cannot be applied
    /// ([`HttpConnectorError::Auth`]), or a header is invalid
    /// ([`HttpConnectorError::InvalidHeader`]). Per the type-level Security note,
    /// no error message echoes a URL or credential.
    async fn execute(
        &self,
        operation: &Operation,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, HttpConnectorError>;

    /// The configured base URL (analog of `SqlConnector::dialect()`).
    fn base_url(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join_url_preserves_prefix() {
        // The API-Gateway stage prefix `/v1` survives; exactly one slash joins.
        assert_eq!(join_url("https://x/v1", "/users"), "https://x/v1/users");
        // Trailing slash on base + leading slash on path collapse to one.
        assert_eq!(join_url("https://x/v1/", "/users"), "https://x/v1/users");
        // No leading slash on path still joins with exactly one slash.
        assert_eq!(join_url("https://x/v1", "users"), "https://x/v1/users");
        // Root base.
        assert_eq!(join_url("https://x", "/users"), "https://x/users");
    }

    #[test]
    fn test_operation_path_parameters() {
        let op = Operation {
            method: "GET".to_string(),
            path: "/users/{id}".to_string(),
            parameters: vec![
                Parameter::new("id", ParameterLocation::Path, true),
                Parameter::new("verbose", ParameterLocation::Query, false),
                Parameter::new("x-trace", ParameterLocation::Header, false),
            ],
            has_request_body: false,
        };
        let path_params: Vec<&str> = op
            .path_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(path_params, vec!["id"]);
        let query_params: Vec<&str> = op
            .query_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(query_params, vec!["verbose"]);
        let header_params: Vec<&str> = op
            .header_parameters()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(header_params, vec!["x-trace"]);
    }

    /// T-90-01-01: the rendered `Display` of every error variant MUST NOT echo a
    /// URL, an `Authorization`/`Bearer` value, or an `app_key`. Mirrors the SQL
    /// redaction test `test_connection_display_does_not_echo_password`.
    #[test]
    fn test_http_error_display_does_not_echo_secret() {
        let variants = [
            HttpConnectorError::Request("connect timed out".to_string()),
            HttpConnectorError::Status { status: 401 },
            HttpConnectorError::Auth("required token absent".to_string()),
            HttpConnectorError::InvalidHeader("name contains illegal byte".to_string()),
            HttpConnectorError::Backend("unknown method".to_string()),
        ];
        for err in &variants {
            let rendered = format!("{err}");
            for forbidden in ["Bearer", "Authorization", "app_key", "https://", "http://"] {
                assert!(
                    !rendered.contains(forbidden),
                    "HttpConnectorError Display must not echo {forbidden:?}; got {rendered:?}"
                );
            }
        }
    }

    /// display_no_secret: the named `verify` automated check — Status{401}
    /// renders the code but never a credential token.
    #[test]
    fn display_no_secret_status_shows_code() {
        let rendered = HttpConnectorError::Status { status: 401 }.to_string();
        assert!(
            rendered.contains("401"),
            "status code must be visible: {rendered:?}"
        );
        for forbidden in ["Bearer", "Authorization", "app_key", "https://"] {
            assert!(!rendered.contains(forbidden), "must not echo {forbidden:?}");
        }
    }

    #[test]
    fn connector_trait_object_is_send_sync_static() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<Box<dyn HttpConnector>>();
    }
}
