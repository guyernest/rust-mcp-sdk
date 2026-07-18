//! Shared HTTP-source plumbing for [`OpenAiCompatSource`] and [`AnthropicSource`].
//!
//! Endpoint-scheme policy (T-108-04-03/05), reqwest client construction with a
//! request timeout (T-108-04-04), bounded response-body reads (T-108-04-04),
//! and HTTP-status → [`CompletionError`] classification live here so both
//! sources share one hardened implementation.

use std::time::Duration;

use crate::seams::CompletionError;

/// Default request timeout for the HTTP sources.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Default maximum response-body size (8 MiB) — guards against unbounded memory
/// growth from a hostile/oversized endpoint response (T-108-04-04).
pub(crate) const DEFAULT_MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Tunable options shared by the HTTP completion sources.
///
/// Defaults: 60 s request timeout, 8 MiB body cap, and `allow_insecure_http =
/// false` (plain HTTP allowed only for loopback/localhost hosts).
#[derive(Debug, Clone)]
pub struct HttpSourceOptions {
    /// Per-request timeout applied to the reqwest client.
    pub timeout: Duration,
    /// Maximum response-body size read into memory.
    pub max_body_bytes: usize,
    /// Permit plain `http://` for non-loopback hosts (explicit opt-in).
    pub allow_insecure_http: bool,
}

impl Default for HttpSourceOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            allow_insecure_http: false,
        }
    }
}

impl HttpSourceOptions {
    /// Set the per-request timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set the maximum response-body size (bytes).
    #[must_use]
    pub fn with_max_body_bytes(mut self, max: usize) -> Self {
        self.max_body_bytes = max;
        self
    }

    /// Permit plain HTTP for non-loopback hosts.
    #[must_use]
    pub fn with_allow_insecure_http(mut self, allow: bool) -> Self {
        self.allow_insecure_http = allow;
        self
    }
}

/// A minimal scheme+host split — avoids a `url` crate dependency just to enforce
/// the loopback policy.
struct ParsedUrl {
    scheme: String,
    host: String,
}

/// Parse `scheme://host[:port]/...` far enough to extract scheme and host.
fn url_parse(input: &str) -> Option<ParsedUrl> {
    let (scheme, rest) = input.split_once("://")?;
    if scheme.is_empty() || rest.is_empty() {
        return None;
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, hp)| hp);
    let host = host_port
        .rsplit_once(':')
        .map_or(host_port, |(h, _)| h)
        .trim_matches(|c| c == '[' || c == ']');
    if host.is_empty() {
        return None;
    }
    Some(ParsedUrl {
        scheme: scheme.to_ascii_lowercase(),
        host: host.to_ascii_lowercase(),
    })
}

/// Whether `host` is a loopback/localhost address that may use plain HTTP.
fn is_loopback_host(host: &str) -> bool {
    if host == "localhost" || host == "::1" {
        return true;
    }
    host.parse::<std::net::Ipv4Addr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// Enforce the endpoint-scheme policy (T-108-04-03/05).
///
/// `https://` is always allowed; plain `http://` is allowed only for
/// loopback/localhost hosts or when `allow_insecure_http` is set. Any other
/// scheme, or a malformed URL, is a `Fatal` [`CompletionError::Decode`].
pub(crate) fn validate_endpoint(
    base_url: &str,
    allow_insecure_http: bool,
) -> Result<(), CompletionError> {
    let url = url_parse(base_url)
        .ok_or_else(|| CompletionError::Decode(format!("invalid base URL: {base_url}")))?;
    match url.scheme.as_str() {
        "https" => Ok(()),
        "http" if allow_insecure_http || is_loopback_host(&url.host) => Ok(()),
        "http" => Err(CompletionError::Decode(format!(
            "plain http:// is only allowed for loopback/localhost or with allow_insecure_http (host: {})",
            url.host
        ))),
        other => Err(CompletionError::Decode(format!(
            "unsupported URL scheme: {other}"
        ))),
    }
}

/// Build a reqwest client with the configured request timeout.
pub(crate) fn build_client(timeout: Duration) -> Result<reqwest::Client, CompletionError> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| CompletionError::Transport(format!("client build failed: {e}")))
}

/// Classify a non-2xx HTTP status into a [`CompletionError`], or `None` for 2xx.
///
/// 5xx → transient; 429/529 → capacity; 401/403 → auth; other 4xx → decode
/// (fatal — a malformed/unacceptable request the loop should not blindly retry).
pub(crate) fn classify_status(status: u16) -> Option<CompletionError> {
    match status {
        200..=299 => None,
        401 | 403 => Some(CompletionError::Auth),
        429 | 529 => Some(CompletionError::Capacity(format!("http {status}"))),
        500..=599 => Some(CompletionError::Transport(format!("http {status}"))),
        _ => Some(CompletionError::Decode(format!("http {status}"))),
    }
}

/// Map a reqwest transport error into a [`CompletionError`].
///
/// A timeout is transient (retryable); everything else is transient transport as
/// well (connection reset, DNS, TLS) — never `Fatal`, and never echoing a key.
pub(crate) fn map_reqwest_error(err: &reqwest::Error) -> CompletionError {
    if err.is_timeout() {
        CompletionError::Transport("request timed out".to_string())
    } else {
        CompletionError::Transport(format!("request failed: {err}"))
    }
}

/// Read a response body with a hard size cap (T-108-04-04).
///
/// Streams chunks and errors as soon as the accumulated size would exceed
/// `max_body_bytes`, so a hostile endpoint cannot exhaust memory.
pub(crate) async fn read_bounded_body(
    mut resp: reqwest::Response,
    max_body_bytes: usize,
) -> Result<Vec<u8>, CompletionError> {
    let mut buf = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| map_reqwest_error(&e))? {
        if buf.len() + chunk.len() > max_body_bytes {
            return Err(CompletionError::Decode(format!(
                "response body exceeds {max_body_bytes} bytes"
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::{classify_status, validate_endpoint, HttpSourceOptions};
    use crate::seams::{CompletionError, RetryClass};

    #[test]
    fn https_always_allowed() {
        assert!(validate_endpoint("https://api.example.com/v1", false).is_ok());
    }

    #[test]
    fn localhost_http_allowed() {
        assert!(validate_endpoint("http://localhost:11434/v1", false).is_ok());
        assert!(validate_endpoint("http://127.0.0.1:8080", false).is_ok());
        assert!(validate_endpoint("http://127.5.6.7:8080", false).is_ok());
    }

    #[test]
    fn remote_http_rejected_without_optin() {
        let err = validate_endpoint("http://api.example.com/v1", false).unwrap_err();
        assert_eq!(err.retry_class(), RetryClass::Fatal);
    }

    #[test]
    fn remote_http_allowed_with_optin() {
        assert!(validate_endpoint("http://api.example.com/v1", true).is_ok());
    }

    #[test]
    fn malformed_or_unsupported_scheme_rejected() {
        assert!(validate_endpoint("not-a-url", false).is_err());
        assert!(validate_endpoint("ftp://example.com", false).is_err());
    }

    #[test]
    fn status_classification() {
        assert!(classify_status(200).is_none());
        assert!(matches!(
            classify_status(500),
            Some(CompletionError::Transport(_))
        ));
        assert!(matches!(
            classify_status(503),
            Some(CompletionError::Transport(_))
        ));
        assert!(matches!(
            classify_status(429),
            Some(CompletionError::Capacity(_))
        ));
        assert!(matches!(
            classify_status(529),
            Some(CompletionError::Capacity(_))
        ));
        assert!(matches!(classify_status(401), Some(CompletionError::Auth)));
        assert!(matches!(classify_status(403), Some(CompletionError::Auth)));
        assert!(matches!(
            classify_status(400),
            Some(CompletionError::Decode(_))
        ));
    }

    #[test]
    fn options_builder_defaults() {
        let opts = HttpSourceOptions::default();
        assert_eq!(opts.timeout.as_secs(), 60);
        assert_eq!(opts.max_body_bytes, 8 * 1024 * 1024);
        assert!(!opts.allow_insecure_http);

        let tuned = HttpSourceOptions::default()
            .with_timeout(std::time::Duration::from_secs(5))
            .with_max_body_bytes(1024)
            .with_allow_insecure_http(true);
        assert_eq!(tuned.timeout.as_secs(), 5);
        assert_eq!(tuned.max_body_bytes, 1024);
        assert!(tuned.allow_insecure_http);
    }
}
