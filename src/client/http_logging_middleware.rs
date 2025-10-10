//! HTTP logging middleware with sensitive header redaction.
//!
//! This middleware provides safe logging at the HTTP transport layer with default-on
//! redaction for sensitive headers like Authorization, Cookie, and API keys.
//!
//! # Security
//!
//! **Default-on redaction** prevents accidental leakage of sensitive information:
//! - `authorization`: Redacted as "Bearer [REDACTED]" (scheme visible by default)
//! - `cookie` / `set-cookie`: Redacted as "[REDACTED]"
//! - `x-api-key`, `proxy-authorization`, `x-auth-token`: Redacted as "[REDACTED]"
//!
//! # Examples
//!
//! ```rust
//! use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;
//! use pmcp::client::http_middleware::HttpMiddlewareChain;
//! use std::sync::Arc;
//!
//! # fn example() {
//! let mut http_chain = HttpMiddlewareChain::new();
//!
//! // Use default configuration (INFO level, default redactions)
//! http_chain.add(Arc::new(HttpLoggingMiddleware::default()));
//!
//! // Or customize
//! let logging = HttpLoggingMiddleware::new()
//!     .with_level(tracing::Level::DEBUG)
//!     .with_max_body_bytes(1024); // Log up to 1KB of body
//!
//! http_chain.add(Arc::new(logging));
//! # }
//! ```

use crate::client::http_middleware::{
    HttpMiddleware, HttpMiddlewareContext, HttpRequest, HttpResponse,
};
use crate::error::Result;
use async_trait::async_trait;
use http::header::HeaderName;
use std::collections::HashSet;

/// HTTP logging middleware with sensitive header redaction.
///
/// Logs HTTP requests and responses at the transport layer with automatic redaction
/// of sensitive headers to prevent accidental exposure of credentials and secrets.
///
/// # Customization
///
/// All defaults can be customized:
/// - Add custom sensitive headers via `.redact_header()`
/// - Remove headers from redaction via `.allow_header()` (use with caution)
/// - Enable query parameter redaction via `.with_redact_query()`
/// - Control body logging via `.with_max_body_bytes()` (respects content-type)
///
/// See examples in module documentation for common customization patterns.
#[derive(Debug, Clone)]
pub struct HttpLoggingMiddleware {
    level: tracing::Level,
    redact_headers: HashSet<HeaderName>,
    show_auth_scheme: bool,
    max_header_value_len: Option<usize>,
    max_body_bytes: Option<usize>,
    redact_query: bool,
    log_body_content_types: HashSet<String>,
}

impl Default for HttpLoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpLoggingMiddleware {
    /// Create a new HTTP logging middleware with secure defaults.
    ///
    /// # Defaults
    ///
    /// - **Log level**: INFO
    /// - **Redacted headers** (customize via `.redact_header()` or `.allow_header()`):
    ///   - authorization, proxy-authorization
    ///   - cookie, set-cookie
    ///   - x-api-key, x-auth-token
    ///   - x-amz-security-token (AWS)
    ///   - x-goog-api-key (Google Cloud)
    /// - **Show auth scheme**: true (logs "Bearer [REDACTED]" instead of "[REDACTED]")
    /// - **Max header value length**: None (no truncation)
    /// - **Max body bytes**: None (don't log bodies by default)
    /// - **Redact query params**: false (customize via `.with_redact_query()`)
    /// - **Body content types**: application/json, text/* when body logging enabled
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;
    /// use http::header::HeaderName;
    ///
    /// // Use defaults
    /// let middleware = HttpLoggingMiddleware::new();
    ///
    /// // Customize: add custom header and enable query redaction
    /// let custom = HttpLoggingMiddleware::new()
    ///     .redact_header(HeaderName::from_static("x-custom-secret"))
    ///     .with_redact_query(true)
    ///     .with_max_body_bytes(512); // Log first 512 bytes of JSON bodies
    /// ```
    pub fn new() -> Self {
        let mut redact_headers = HashSet::new();
        // Standard auth headers
        redact_headers.insert(HeaderName::from_static("authorization"));
        redact_headers.insert(HeaderName::from_static("proxy-authorization"));
        // Cookie headers
        redact_headers.insert(HeaderName::from_static("cookie"));
        redact_headers.insert(HeaderName::from_static("set-cookie"));
        // API key headers
        redact_headers.insert(HeaderName::from_static("x-api-key"));
        redact_headers.insert(HeaderName::from_static("x-auth-token"));
        // Cloud provider security tokens
        redact_headers.insert(HeaderName::from_static("x-amz-security-token"));
        redact_headers.insert(HeaderName::from_static("x-goog-api-key"));

        let mut log_body_content_types = HashSet::new();
        log_body_content_types.insert("application/json".to_string());
        log_body_content_types.insert("text/plain".to_string());
        log_body_content_types.insert("text/html".to_string());
        log_body_content_types.insert("text/xml".to_string());

        Self {
            level: tracing::Level::INFO,
            redact_headers,
            show_auth_scheme: true,
            max_header_value_len: None,
            max_body_bytes: None,
            redact_query: false,
            log_body_content_types,
        }
    }

    /// Set the log level for this middleware.
    pub fn with_level(mut self, level: tracing::Level) -> Self {
        self.level = level;
        self
    }

    /// Add a header to the redaction list.
    pub fn redact_header(mut self, name: HeaderName) -> Self {
        self.redact_headers.insert(name);
        self
    }

    /// Remove a header from the redaction list (use with caution).
    pub fn allow_header(mut self, name: &HeaderName) -> Self {
        self.redact_headers.remove(name);
        self
    }

    /// Set whether to show the authentication scheme (e.g., "Bearer") in redacted Authorization headers.
    ///
    /// If true: "Bearer [REDACTED]"
    /// If false: "[REDACTED]"
    pub fn with_show_auth_scheme(mut self, show: bool) -> Self {
        self.show_auth_scheme = show;
        self
    }

    /// Set maximum header value length. Values longer than this will be truncated.
    pub fn with_max_header_value_len(mut self, max_len: usize) -> Self {
        self.max_header_value_len = Some(max_len);
        self
    }

    /// Set maximum body bytes to log. If None, bodies are not logged.
    ///
    /// Body logging respects content-type - only logs text-based content
    /// (application/json, text/*) to avoid logging binary data.
    ///
    /// # Security Note
    ///
    /// Even with body logging enabled, keep the byte limit small (e.g., 512-1024 bytes)
    /// to avoid logging sensitive data in request/response bodies.
    pub fn with_max_body_bytes(mut self, max_bytes: usize) -> Self {
        self.max_body_bytes = Some(max_bytes);
        self
    }

    /// Enable or disable query parameter redaction in URLs.
    ///
    /// When true, URLs are logged without query parameters to prevent
    /// leaking sensitive data in query strings (e.g., tokens, API keys).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::client::http_logging_middleware::HttpLoggingMiddleware;
    ///
    /// let middleware = HttpLoggingMiddleware::new()
    ///     .with_redact_query(true);
    ///
    /// // Will log: http://example.com/api/users?[REDACTED]
    /// // Instead of: http://example.com/api/users?token=secret&id=123
    /// ```
    pub fn with_redact_query(mut self, redact: bool) -> Self {
        self.redact_query = redact;
        self
    }

    /// Add a content type that should be logged when body logging is enabled.
    ///
    /// By default, only text-based content types are logged:
    /// - application/json
    /// - text/plain, text/html, text/xml
    ///
    /// Use this to add custom content types (e.g., "application/xml").
    pub fn allow_body_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.log_body_content_types.insert(content_type.into());
        self
    }

    /// Get the maximum body bytes configuration.
    ///
    /// Returns None if body logging is disabled (the default).
    pub fn max_body_bytes(&self) -> Option<usize> {
        self.max_body_bytes
    }

    /// Redact a header value based on the header name.
    ///
    /// This is public for testing purposes.
    pub fn redact_header_value(&self, name: &HeaderName, value: &str) -> String {
        if !self.redact_headers.contains(name) {
            // Not a sensitive header - apply length limit if configured
            return self.truncate_value(value.to_string());
        }

        // Redact based on header type
        if name == "authorization" && self.show_auth_scheme {
            // Try to preserve the scheme (e.g., "Bearer", "Basic")
            if let Some(space_idx) = value.find(' ') {
                let scheme = &value[..space_idx];
                format!("{} [REDACTED]", scheme)
            } else {
                "[REDACTED]".to_string()
            }
        } else {
            "[REDACTED]".to_string()
        }
    }

    /// Truncate a value to the configured maximum length.
    fn truncate_value(&self, value: String) -> String {
        if let Some(max_len) = self.max_header_value_len {
            if value.len() > max_len {
                format!("{}...", &value[..max_len.min(value.len())])
            } else {
                value
            }
        } else {
            value
        }
    }

    /// Redact query parameters from a URL if configured.
    fn redact_url(&self, url: &str) -> String {
        if !self.redact_query {
            return url.to_string();
        }

        if let Some(query_start) = url.find('?') {
            format!("{}?[REDACTED]", &url[..query_start])
        } else {
            url.to_string()
        }
    }

    /// Check if a content-type should be logged.
    fn should_log_body(&self, content_type: Option<&str>) -> bool {
        if self.max_body_bytes.is_none() {
            return false;
        }

        let Some(ct) = content_type else {
            return false;
        };

        // Extract base content type (ignore charset, etc.)
        let base_ct = ct.split(';').next().unwrap_or(ct).trim();

        // Check exact match or text/* wildcard
        self.log_body_content_types.contains(base_ct)
            || (base_ct.starts_with("text/")
                && self
                    .log_body_content_types
                    .iter()
                    .any(|t| t == "text/plain"))
    }

    /// Format headers for logging with redaction.
    ///
    /// This is public for testing purposes.
    pub fn format_headers(&self, headers: &hyper::http::HeaderMap) -> String {
        let mut header_strs = Vec::new();

        for (name, _value) in headers {
            // Get all values for this header (multi-value support)
            let values: Vec<String> = headers
                .get_all(name)
                .iter()
                .map(|v| {
                    let value_str = v.to_str().unwrap_or("<invalid-utf8>");
                    self.redact_header_value(name, value_str)
                })
                .collect();

            if values.len() == 1 {
                header_strs.push(format!("{}: {}", name.as_str(), values[0]));
            } else {
                // Multiple values for same header
                for (idx, val) in values.iter().enumerate() {
                    header_strs.push(format!("{}[{}]: {}", name.as_str(), idx, val));
                }
            }
        }

        if header_strs.is_empty() {
            "(no headers)".to_string()
        } else {
            header_strs.join(", ")
        }
    }

    /// Log a request with redacted headers.
    #[allow(clippy::cognitive_complexity)]
    fn log_request(&self, request: &HttpRequest, context: &HttpMiddlewareContext) {
        let url = self.redact_url(&request.url);
        let headers_str = self.format_headers(&request.headers);

        // Check content-type to determine if body should be logged
        let content_type = request.get_header("content-type");
        let should_log = self.should_log_body(content_type);

        let body_info = if should_log && self.max_body_bytes.is_some() {
            let max_bytes = self.max_body_bytes.unwrap();
            let body_len = request.body.len();
            if body_len > 0 {
                let preview_len = max_bytes.min(body_len);
                let preview = String::from_utf8_lossy(&request.body[..preview_len]);
                if body_len > max_bytes {
                    format!(
                        " body={}B (showing {}B): {}...",
                        body_len, preview_len, preview
                    )
                } else {
                    format!(" body={}B: {}", body_len, preview)
                }
            } else {
                " body=0B".to_string()
            }
        } else {
            format!(" body={}B", request.body.len())
        };

        match self.level {
            tracing::Level::TRACE => tracing::trace!(
                request_id = ?context.request_id,
                "HTTP {} {} | headers: [{}]{}",
                request.method,
                url,
                headers_str,
                body_info
            ),
            tracing::Level::DEBUG => tracing::debug!(
                request_id = ?context.request_id,
                "HTTP {} {} | headers: [{}]{}",
                request.method,
                url,
                headers_str,
                body_info
            ),
            tracing::Level::INFO => tracing::info!(
                request_id = ?context.request_id,
                "HTTP {} {}{}",
                request.method,
                url,
                if should_log { body_info.as_str() } else { "" }
            ),
            tracing::Level::WARN => tracing::warn!(
                request_id = ?context.request_id,
                "HTTP {} {}",
                request.method,
                url
            ),
            tracing::Level::ERROR => tracing::error!(
                request_id = ?context.request_id,
                "HTTP {} {}",
                request.method,
                url
            ),
        }
    }

    /// Log a response with redacted headers.
    #[allow(clippy::cognitive_complexity)]
    fn log_response(&self, response: &HttpResponse, context: &HttpMiddlewareContext) {
        let headers_str = self.format_headers(&response.headers);

        // Check content-type to determine if body should be logged
        let content_type = response.get_header("content-type");
        let should_log = self.should_log_body(content_type);

        let body_info = if should_log && self.max_body_bytes.is_some() {
            let max_bytes = self.max_body_bytes.unwrap();
            let body_len = response.body.len();
            if body_len > 0 {
                let preview_len = max_bytes.min(body_len);
                let preview = String::from_utf8_lossy(&response.body[..preview_len]);
                if body_len > max_bytes {
                    format!(
                        " body={}B (showing {}B): {}...",
                        body_len, preview_len, preview
                    )
                } else {
                    format!(" body={}B: {}", body_len, preview)
                }
            } else {
                " body=0B".to_string()
            }
        } else {
            format!(" body={}B", response.body.len())
        };

        let status_emoji = if response.is_success() {
            "✓"
        } else if response.is_client_error() {
            "⚠"
        } else if response.is_server_error() {
            "✗"
        } else {
            "→"
        };

        match self.level {
            tracing::Level::TRACE => tracing::trace!(
                request_id = ?context.request_id,
                "{} HTTP {} | headers: [{}]{}",
                status_emoji,
                response.status,
                headers_str,
                body_info
            ),
            tracing::Level::DEBUG => tracing::debug!(
                request_id = ?context.request_id,
                "{} HTTP {} | headers: [{}]{}",
                status_emoji,
                response.status,
                headers_str,
                body_info
            ),
            tracing::Level::INFO => tracing::info!(
                request_id = ?context.request_id,
                "{} HTTP {}{}",
                status_emoji,
                response.status,
                if self.max_body_bytes.is_some() { body_info.as_str() } else { "" }
            ),
            tracing::Level::WARN => tracing::warn!(
                request_id = ?context.request_id,
                "{} HTTP {}",
                status_emoji,
                response.status
            ),
            tracing::Level::ERROR => tracing::error!(
                request_id = ?context.request_id,
                "{} HTTP {}",
                status_emoji,
                response.status
            ),
        }
    }
}

#[async_trait]
impl HttpMiddleware for HttpLoggingMiddleware {
    async fn on_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        self.log_request(request, context);
        Ok(())
    }

    async fn on_response(
        &self,
        response: &mut HttpResponse,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        self.log_response(response, context);
        Ok(())
    }

    fn priority(&self) -> i32 {
        100 // Run after most other middleware (high priority = early in chain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderMap;

    #[test]
    fn test_default_redaction_list() {
        let middleware = HttpLoggingMiddleware::new();

        assert!(middleware
            .redact_headers
            .contains(&HeaderName::from_static("authorization")));
        assert!(middleware
            .redact_headers
            .contains(&HeaderName::from_static("cookie")));
        assert!(middleware
            .redact_headers
            .contains(&HeaderName::from_static("set-cookie")));
        assert!(middleware
            .redact_headers
            .contains(&HeaderName::from_static("x-api-key")));
    }

    #[test]
    fn test_authorization_redaction_with_scheme() {
        let middleware = HttpLoggingMiddleware::new();
        let name = HeaderName::from_static("authorization");

        let redacted = middleware.redact_header_value(&name, "Bearer my-secret-token");
        assert_eq!(redacted, "Bearer [REDACTED]");

        let redacted2 = middleware.redact_header_value(&name, "Basic dXNlcjpwYXNz");
        assert_eq!(redacted2, "Basic [REDACTED]");
    }

    #[test]
    fn test_authorization_redaction_without_scheme() {
        let middleware = HttpLoggingMiddleware::new().with_show_auth_scheme(false);
        let name = HeaderName::from_static("authorization");

        let redacted = middleware.redact_header_value(&name, "Bearer my-secret-token");
        assert_eq!(redacted, "[REDACTED]");
    }

    #[test]
    fn test_cookie_redaction() {
        let middleware = HttpLoggingMiddleware::new();
        let name = HeaderName::from_static("cookie");

        let redacted = middleware.redact_header_value(&name, "session=abc123; user=john");
        assert_eq!(redacted, "[REDACTED]");
    }

    #[test]
    fn test_non_sensitive_header() {
        let middleware = HttpLoggingMiddleware::new();
        let name = HeaderName::from_static("content-type");

        let redacted = middleware.redact_header_value(&name, "application/json");
        assert_eq!(redacted, "application/json");
    }

    #[test]
    fn test_header_truncation() {
        let middleware = HttpLoggingMiddleware::new().with_max_header_value_len(10);
        let name = HeaderName::from_static("content-type");

        let redacted = middleware.redact_header_value(&name, "application/json; charset=utf-8");
        assert_eq!(redacted, "applicatio...");
    }

    #[test]
    fn test_allow_header_override() {
        let middleware =
            HttpLoggingMiddleware::new().allow_header(&HeaderName::from_static("x-api-key"));

        assert!(!middleware
            .redact_headers
            .contains(&HeaderName::from_static("x-api-key")));

        // Should not be redacted now
        let name = HeaderName::from_static("x-api-key");
        let redacted = middleware.redact_header_value(&name, "my-api-key-12345");
        assert_eq!(redacted, "my-api-key-12345");
    }

    #[test]
    fn test_format_headers_multivalue() {
        let middleware = HttpLoggingMiddleware::new();
        let mut headers = HeaderMap::new();

        headers.append("set-cookie", "session1=abc".parse().unwrap());
        headers.append("set-cookie", "session2=def".parse().unwrap());
        headers.insert("content-type", "application/json".parse().unwrap());

        let formatted = middleware.format_headers(&headers);

        // Should have both set-cookie entries redacted
        assert!(formatted.contains("[REDACTED]"));
        assert!(formatted.contains("application/json"));
    }

    #[test]
    fn test_query_redaction_enabled() {
        let middleware = HttpLoggingMiddleware::new().with_redact_query(true);

        let url_with_query = "http://example.com/api/users?token=secret&id=123";
        let redacted = middleware.redact_url(url_with_query);
        assert_eq!(redacted, "http://example.com/api/users?[REDACTED]");

        let url_without_query = "http://example.com/api/users";
        let redacted2 = middleware.redact_url(url_without_query);
        assert_eq!(redacted2, "http://example.com/api/users");
    }

    #[test]
    fn test_query_redaction_disabled() {
        let middleware = HttpLoggingMiddleware::new(); // default: redact_query = false

        let url_with_query = "http://example.com/api/users?token=secret&id=123";
        let redacted = middleware.redact_url(url_with_query);
        assert_eq!(redacted, url_with_query);
    }

    #[test]
    fn test_cloud_provider_headers_redacted() {
        let middleware = HttpLoggingMiddleware::new();

        // AWS
        let aws_header = HeaderName::from_static("x-amz-security-token");
        assert!(middleware.redact_headers.contains(&aws_header));
        let redacted = middleware.redact_header_value(&aws_header, "aws-session-token-12345");
        assert_eq!(redacted, "[REDACTED]");

        // GCP
        let gcp_header = HeaderName::from_static("x-goog-api-key");
        assert!(middleware.redact_headers.contains(&gcp_header));
        let redacted2 = middleware.redact_header_value(&gcp_header, "gcp-api-key-67890");
        assert_eq!(redacted2, "[REDACTED]");
    }

    #[test]
    fn test_body_logging_content_type_gating() {
        let middleware = HttpLoggingMiddleware::new().with_max_body_bytes(1024);

        // JSON should be logged
        assert!(middleware.should_log_body(Some("application/json")));
        assert!(middleware.should_log_body(Some("application/json; charset=utf-8")));

        // Text types should be logged
        assert!(middleware.should_log_body(Some("text/plain")));
        assert!(middleware.should_log_body(Some("text/html")));
        assert!(middleware.should_log_body(Some("text/xml")));

        // Other text/* types should be logged (via text/plain wildcard)
        assert!(middleware.should_log_body(Some("text/csv")));

        // Binary types should NOT be logged
        assert!(!middleware.should_log_body(Some("application/octet-stream")));
        assert!(!middleware.should_log_body(Some("image/png")));
        assert!(!middleware.should_log_body(Some("video/mp4")));

        // No content-type should NOT be logged
        assert!(!middleware.should_log_body(None));
    }

    #[test]
    fn test_body_logging_disabled_by_default() {
        let middleware = HttpLoggingMiddleware::new(); // max_body_bytes = None

        // Even with valid content-type, should not log if max_body_bytes is None
        assert!(!middleware.should_log_body(Some("application/json")));
        assert!(!middleware.should_log_body(Some("text/plain")));
    }

    #[test]
    fn test_allow_custom_body_content_type() {
        let middleware = HttpLoggingMiddleware::new()
            .with_max_body_bytes(1024)
            .allow_body_content_type("application/xml");

        // Custom content type should be allowed
        assert!(middleware.should_log_body(Some("application/xml")));

        // Default types still work
        assert!(middleware.should_log_body(Some("application/json")));
    }
}
