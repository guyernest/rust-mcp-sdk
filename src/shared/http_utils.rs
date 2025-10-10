//! Shared HTTP utilities for client and server middleware.
//!
//! This module provides common functionality used by both client-side and
//! server-side HTTP middleware, ensuring consistent behavior and avoiding
//! code duplication.

use hyper::http::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashSet;

/// Redact sensitive header values.
///
/// Returns either the redacted value or the original value if the header
/// is not in the redaction list.
///
/// # Arguments
///
/// * `name` - Header name to check
/// * `value` - Original header value
/// * `redact_headers` - Set of headers to redact
/// * `show_auth_scheme` - Whether to keep auth scheme (e.g., "Bearer")
/// * `max_len` - Optional maximum length for non-redacted values
pub fn redact_header_value(
    name: &HeaderName,
    value: &str,
    redact_headers: &HashSet<HeaderName>,
    show_auth_scheme: bool,
    max_len: Option<usize>,
) -> String {
    // Check if this header should be redacted
    if !redact_headers.contains(name) {
        // Not in redaction list - return with optional truncation
        return truncate_value(value, max_len);
    }

    // Redact based on header type
    if name == "authorization" && show_auth_scheme {
        // Keep auth scheme, redact token
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

/// Truncate a value to a maximum length.
fn truncate_value(value: &str, max_len: Option<usize>) -> String {
    match max_len {
        Some(max) if value.len() > max => {
            format!("{}...", &value[..max.min(value.len())])
        },
        _ => value.to_string(),
    }
}

/// Check if a content-type should allow body logging.
///
/// Returns true if the content type is text-based and suitable for logging.
///
/// # Arguments
///
/// * `content_type` - Content-Type header value (may include charset)
/// * `allowed_types` - Set of allowed base content types
pub fn should_log_body_for_content_type(
    content_type: Option<&str>,
    allowed_types: &HashSet<String>,
) -> bool {
    let Some(ct) = content_type else {
        return false;
    };

    // Extract base content type (ignore charset, etc.)
    let base_ct = ct.split(';').next().unwrap_or(ct).trim();

    // Check exact match or text/* wildcard
    allowed_types.contains(base_ct)
        || (base_ct.starts_with("text/") && allowed_types.iter().any(|t| t == "text/plain"))
}

/// Redact query parameters from a URL.
///
/// Returns the URL with query parameters replaced by "[REDACTED]".
///
/// # Arguments
///
/// * `url` - Original URL
/// * `redact_query` - Whether to redact query parameters
pub fn redact_url_query(url: &str, redact_query: bool) -> String {
    if !redact_query {
        return url.to_string();
    }

    if let Some(query_start) = url.find('?') {
        format!("{}?[REDACTED]", &url[..query_start])
    } else {
        url.to_string()
    }
}

/// Format headers for logging with redaction.
///
/// Returns a formatted string with headers redacted as appropriate.
pub fn format_headers_for_logging(
    headers: &HeaderMap<HeaderValue>,
    redact_headers: &HashSet<HeaderName>,
    show_auth_scheme: bool,
    max_value_len: Option<usize>,
) -> String {
    let mut header_strs = Vec::new();

    for (name, _value) in headers {
        // Get all values for this header (handles multi-value headers)
        let values: Vec<&HeaderValue> = headers.get_all(name).iter().collect();

        for value in values {
            if let Ok(value_str) = value.to_str() {
                let redacted = redact_header_value(
                    name,
                    value_str,
                    redact_headers,
                    show_auth_scheme,
                    max_value_len,
                );
                header_strs.push(format!("{}={}", name.as_str(), redacted));
            }
        }
    }

    header_strs.join(", ")
}

/// Get default sensitive headers to redact.
///
/// Returns a set of commonly sensitive header names that should be
/// redacted by default in logging middleware.
pub fn default_sensitive_headers() -> HashSet<HeaderName> {
    let mut headers = HashSet::new();

    // Standard auth headers
    headers.insert(HeaderName::from_static("authorization"));
    headers.insert(HeaderName::from_static("proxy-authorization"));

    // Cookie headers
    headers.insert(HeaderName::from_static("cookie"));
    headers.insert(HeaderName::from_static("set-cookie"));

    // API key headers
    headers.insert(HeaderName::from_static("x-api-key"));
    headers.insert(HeaderName::from_static("x-auth-token"));

    // Cloud provider security tokens
    headers.insert(HeaderName::from_static("x-amz-security-token"));
    headers.insert(HeaderName::from_static("x-goog-api-key"));

    headers
}

/// Get default content types that are safe to log.
///
/// Returns a set of content types that contain text-based data
/// suitable for logging (not binary).
pub fn default_loggable_content_types() -> HashSet<String> {
    let mut types = HashSet::new();

    types.insert("application/json".to_string());
    types.insert("text/plain".to_string());
    types.insert("text/html".to_string());
    types.insert("text/xml".to_string());

    types
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_header_value_authorization_with_scheme() {
        let mut redact_headers = HashSet::new();
        redact_headers.insert(HeaderName::from_static("authorization"));

        let name = HeaderName::from_static("authorization");
        let value = "Bearer secret-token-12345";

        let redacted = redact_header_value(&name, value, &redact_headers, true, None);
        assert_eq!(redacted, "Bearer [REDACTED]");
    }

    #[test]
    fn test_redact_header_value_authorization_no_scheme() {
        let mut redact_headers = HashSet::new();
        redact_headers.insert(HeaderName::from_static("authorization"));

        let name = HeaderName::from_static("authorization");
        let value = "Bearer secret-token-12345";

        let redacted = redact_header_value(&name, value, &redact_headers, false, None);
        assert_eq!(redacted, "[REDACTED]");
    }

    #[test]
    fn test_redact_header_value_cookie() {
        let mut redact_headers = HashSet::new();
        redact_headers.insert(HeaderName::from_static("cookie"));

        let name = HeaderName::from_static("cookie");
        let value = "session=abc123; user=john";

        let redacted = redact_header_value(&name, value, &redact_headers, true, None);
        assert_eq!(redacted, "[REDACTED]");
    }

    #[test]
    fn test_redact_header_value_not_sensitive() {
        let redact_headers = HashSet::new();

        let name = HeaderName::from_static("content-type");
        let value = "application/json";

        let redacted = redact_header_value(&name, value, &redact_headers, true, None);
        assert_eq!(redacted, "application/json");
    }

    #[test]
    fn test_redact_header_value_truncation() {
        let redact_headers = HashSet::new();

        let name = HeaderName::from_static("x-custom-header");
        let value = "very-long-value-that-should-be-truncated";

        let redacted = redact_header_value(&name, value, &redact_headers, true, Some(10));
        assert_eq!(redacted, "very-long-...");
    }

    #[test]
    fn test_should_log_body_for_content_type() {
        let allowed = default_loggable_content_types();

        // JSON should be allowed
        assert!(should_log_body_for_content_type(
            Some("application/json"),
            &allowed
        ));
        assert!(should_log_body_for_content_type(
            Some("application/json; charset=utf-8"),
            &allowed
        ));

        // Text types should be allowed
        assert!(should_log_body_for_content_type(
            Some("text/plain"),
            &allowed
        ));
        assert!(should_log_body_for_content_type(
            Some("text/html"),
            &allowed
        ));

        // Other text/* should be allowed via wildcard
        assert!(should_log_body_for_content_type(Some("text/csv"), &allowed));

        // Binary types should not be allowed
        assert!(!should_log_body_for_content_type(
            Some("application/octet-stream"),
            &allowed
        ));
        assert!(!should_log_body_for_content_type(
            Some("image/png"),
            &allowed
        ));

        // No content type should not be allowed
        assert!(!should_log_body_for_content_type(None, &allowed));
    }

    #[test]
    fn test_redact_url_query() {
        assert_eq!(
            redact_url_query("http://example.com/api?token=secret", true),
            "http://example.com/api?[REDACTED]"
        );

        assert_eq!(
            redact_url_query("http://example.com/api?token=secret", false),
            "http://example.com/api?token=secret"
        );

        assert_eq!(
            redact_url_query("http://example.com/api", true),
            "http://example.com/api"
        );
    }

    #[test]
    fn test_default_sensitive_headers() {
        let headers = default_sensitive_headers();

        assert!(headers.contains(&HeaderName::from_static("authorization")));
        assert!(headers.contains(&HeaderName::from_static("cookie")));
        assert!(headers.contains(&HeaderName::from_static("x-api-key")));
        assert!(headers.contains(&HeaderName::from_static("x-amz-security-token")));
        assert!(headers.contains(&HeaderName::from_static("x-goog-api-key")));
    }
}
