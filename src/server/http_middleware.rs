//! Server-side HTTP middleware for request/response interception.
//!
//! This module provides HTTP-level middleware for MCP servers, enabling:
//! - Request/response logging with sensitive data redaction
//! - Authentication and authorization
//! - Rate limiting and throttling
//! - Custom request/response transformation
//!
//! # Architecture
//!
//! Server HTTP middleware operates at the HTTP transport layer (before JSON-RPC
//! processing), providing symmetry with client-side HTTP middleware.
//!
//! Two-layer model:
//! - **HTTP Layer**: `ServerHttpMiddleware` for transport-level concerns
//! - **Protocol Layer**: `EnhancedMiddleware` for JSON-RPC message processing
//!
//! # Examples
//!
//! ```rust
//! use pmcp::server::http_middleware::{ServerHttpMiddleware, ServerHttpLoggingMiddleware};
//! use pmcp::server::http_middleware::ServerHttpMiddlewareChain;
//! use std::sync::Arc;
//!
//! // Create logging middleware with redaction
//! let logging = ServerHttpLoggingMiddleware::new()
//!     .with_redact_query(true)
//!     .with_max_body_bytes(1024);
//!
//! // Build middleware chain
//! let mut chain = ServerHttpMiddlewareChain::new();
//! chain.add(Arc::new(logging));
//! ```

use crate::error::{Error, Result};
use async_trait::async_trait;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

/// HTTP request for server middleware processing.
///
/// Represents an incoming HTTP request with method, URI, headers, and body.
/// Used by server HTTP middleware to inspect and transform requests.
#[derive(Debug, Clone)]
pub struct ServerHttpRequest {
    /// HTTP method (GET, POST, etc.)
    pub method: Method,

    /// Request URI
    pub uri: Uri,

    /// HTTP headers
    pub headers: HeaderMap<HeaderValue>,

    /// Request body (raw bytes)
    pub body: Vec<u8>,
}

impl ServerHttpRequest {
    /// Create a new server HTTP request.
    pub fn new(method: Method, uri: Uri, headers: HeaderMap<HeaderValue>, body: Vec<u8>) -> Self {
        Self {
            method,
            uri,
            headers,
            body,
        }
    }

    /// Get a header value by name.
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers.get(name)?.to_str().ok()
    }

    /// Add or update a header.
    pub fn add_header(&mut self, name: &str, value: &str) {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            self.headers.insert(name, value);
        }
    }
}

/// HTTP response for server middleware processing.
///
/// Represents an outgoing HTTP response with status, headers, and body.
/// Used by server HTTP middleware to inspect and transform responses.
#[derive(Debug, Clone)]
pub struct ServerHttpResponse {
    /// HTTP status code
    pub status: StatusCode,

    /// HTTP headers
    pub headers: HeaderMap<HeaderValue>,

    /// Response body (raw bytes)
    pub body: Vec<u8>,
}

impl ServerHttpResponse {
    /// Create a new server HTTP response.
    pub fn new(status: StatusCode, headers: HeaderMap<HeaderValue>, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Get a header value by name.
    pub fn get_header(&self, name: &str) -> Option<&str> {
        self.headers.get(name)?.to_str().ok()
    }

    /// Add or update a header.
    pub fn add_header(&mut self, name: &str, value: &str) {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(value),
        ) {
            self.headers.insert(name, value);
        }
    }
}

/// Context for server HTTP middleware processing.
///
/// Provides metadata and state for middleware execution, including:
/// - Request identifier for tracing
/// - Start time for performance measurement
/// - Session identifier for connection tracking
#[derive(Debug, Clone)]
pub struct ServerHttpContext {
    /// Unique request identifier
    pub request_id: String,

    /// Request start time
    pub start_time: Instant,

    /// Optional session identifier
    pub session_id: Option<String>,
}

impl ServerHttpContext {
    /// Create a new server HTTP context.
    pub fn new(request_id: String) -> Self {
        Self {
            request_id,
            start_time: Instant::now(),
            session_id: None,
        }
    }

    /// Create context with session ID.
    pub fn with_session(request_id: String, session_id: String) -> Self {
        Self {
            request_id,
            start_time: Instant::now(),
            session_id: Some(session_id),
        }
    }

    /// Get elapsed time since request start.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }
}

/// Server-side HTTP middleware trait.
///
/// Enables interception and transformation of HTTP requests and responses
/// at the transport layer (before JSON-RPC processing).
///
/// # Symmetric with Client
///
/// This trait mirrors the client-side `HttpMiddleware` trait, providing
/// the same mental model and API surface for server implementations.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait ServerHttpMiddleware: Send + Sync {
    /// Called before processing an HTTP request.
    ///
    /// Middleware can inspect, log, or modify the request before it reaches
    /// the protocol layer.
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        context: &ServerHttpContext,
    ) -> Result<()> {
        let _ = (request, context);
        Ok(())
    }

    /// Called after generating an HTTP response.
    ///
    /// Middleware can inspect, log, or modify the response before it's sent
    /// to the client.
    async fn on_response(
        &self,
        response: &mut ServerHttpResponse,
        context: &ServerHttpContext,
    ) -> Result<()> {
        let _ = (response, context);
        Ok(())
    }

    /// Called when an error occurs during request processing.
    ///
    /// Enables error logging, metrics, and custom error handling.
    async fn on_error(&self, error: &Error, context: &ServerHttpContext) -> Result<()> {
        let _ = (error, context);
        Ok(())
    }

    /// Get middleware priority (lower runs first).
    ///
    /// Default priority is 50. Use lower values (e.g., 10) for authentication,
    /// higher values (e.g., 90) for logging.
    fn priority(&self) -> i32 {
        50
    }

    /// Check if this middleware should execute for the given context.
    ///
    /// Allows conditional execution based on request properties.
    async fn should_execute(&self, _context: &ServerHttpContext) -> bool {
        true
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait ServerHttpMiddleware {
    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        context: &ServerHttpContext,
    ) -> Result<()> {
        let _ = (request, context);
        Ok(())
    }

    async fn on_response(
        &self,
        response: &mut ServerHttpResponse,
        context: &ServerHttpContext,
    ) -> Result<()> {
        let _ = (response, context);
        Ok(())
    }

    async fn on_error(&self, error: &Error, context: &ServerHttpContext) -> Result<()> {
        let _ = (error, context);
        Ok(())
    }

    fn priority(&self) -> i32 {
        50
    }

    async fn should_execute(&self, _context: &ServerHttpContext) -> bool {
        true
    }
}

/// Chain of server HTTP middleware handlers.
///
/// Executes middleware in priority order (lowest priority first).
/// Provides symmetry with client-side `HttpMiddlewareChain`.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::http_middleware::{ServerHttpMiddlewareChain, ServerHttpLoggingMiddleware};
/// use std::sync::Arc;
///
/// let mut chain = ServerHttpMiddlewareChain::new();
///
/// // Add logging middleware
/// chain.add(Arc::new(ServerHttpLoggingMiddleware::new()));
///
/// // Middleware executes in priority order
/// ```
#[derive(Default)]
pub struct ServerHttpMiddlewareChain {
    middlewares: Vec<Arc<dyn ServerHttpMiddleware>>,
}

impl std::fmt::Debug for ServerHttpMiddlewareChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerHttpMiddlewareChain")
            .field("middleware_count", &self.middlewares.len())
            .finish()
    }
}

impl ServerHttpMiddlewareChain {
    /// Create a new empty middleware chain.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add middleware to the chain.
    ///
    /// Middleware will be sorted by priority (lower priority executes first).
    pub fn add(&mut self, middleware: Arc<dyn ServerHttpMiddleware>) {
        self.middlewares.push(middleware);
        // Sort by priority (lower first)
        self.middlewares.sort_by_key(|m| m.priority());
    }

    /// Process an incoming HTTP request through the middleware chain.
    ///
    /// Executes all middleware `on_request` hooks in priority order.
    pub async fn process_request(
        &self,
        request: &mut ServerHttpRequest,
        context: &ServerHttpContext,
    ) -> Result<()> {
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                middleware.on_request(request, context).await?;
            }
        }
        Ok(())
    }

    /// Process an outgoing HTTP response through the middleware chain.
    ///
    /// Executes all middleware `on_response` hooks in priority order.
    pub async fn process_response(
        &self,
        response: &mut ServerHttpResponse,
        context: &ServerHttpContext,
    ) -> Result<()> {
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                middleware.on_response(response, context).await?;
            }
        }
        Ok(())
    }

    /// Handle an error through the middleware chain.
    ///
    /// Notifies all middleware of the error for logging/metrics.
    pub async fn handle_error(&self, error: &Error, context: &ServerHttpContext) -> Result<()> {
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                let _ = middleware.on_error(error, context).await;
            }
        }
        Ok(())
    }
}

/// Server HTTP logging middleware with sensitive data redaction.
///
/// Mirrors client-side `HttpLoggingMiddleware` behavior:
/// - Logs HTTP method, URI, status, headers
/// - Redacts sensitive headers (authorization, cookie, API keys, cloud tokens)
/// - Optional query parameter redaction
/// - Content-type gating for body logging
///
/// # Priority and Ordering
///
/// Default priority: 90 (logs after auth/rate-limiting, before response mutations).
/// This ensures logging captures the "raw server response" state. For logging
/// the "final outbound state" after all transformations, use a lower priority (e.g., 5).
///
/// # Streaming Responses
///
/// Never buffers SSE or streaming response bodies. For streaming responses,
/// logs status and headers only. Body logging is restricted to non-streaming
/// responses with content-type gating and size limits.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::http_middleware::ServerHttpLoggingMiddleware;
///
/// // Default configuration (secure defaults)
/// let logging = ServerHttpLoggingMiddleware::new();
///
/// // Custom configuration
/// let custom = ServerHttpLoggingMiddleware::new()
///     .with_redact_query(true)
///     .with_max_body_bytes(512)
///     .redact_header("x-custom-secret")
///     .with_level(tracing::Level::DEBUG);
/// ```
#[derive(Debug)]
pub struct ServerHttpLoggingMiddleware {
    /// Logging level
    level: tracing::Level,

    /// Headers to redact
    redact_headers: HashSet<HeaderName>,

    /// Whether to show auth scheme (e.g., "Bearer")
    show_auth_scheme: bool,

    /// Maximum header value length before truncation
    max_header_value_len: Option<usize>,

    /// Maximum body bytes to log (None = don't log bodies)
    max_body_bytes: Option<usize>,

    /// Whether to redact query parameters
    redact_query: bool,

    /// Content types that are safe to log
    log_body_content_types: HashSet<String>,
}

impl ServerHttpLoggingMiddleware {
    /// Create new logging middleware with secure defaults.
    ///
    /// # Defaults
    ///
    /// - **Log level**: INFO
    /// - **Redacted headers**: authorization, cookie, x-api-key, x-amz-security-token, x-goog-api-key
    /// - **Show auth scheme**: true (logs "Bearer [REDACTED]")
    /// - **Max header value length**: None (no truncation)
    /// - **Max body bytes**: None (don't log bodies by default)
    /// - **Redact query params**: false
    /// - **Body content types**: application/json, text/*
    pub fn new() -> Self {
        use crate::shared::http_utils::{
            default_loggable_content_types, default_sensitive_headers,
        };

        Self {
            level: tracing::Level::INFO,
            redact_headers: default_sensitive_headers(),
            show_auth_scheme: true,
            max_header_value_len: None,
            max_body_bytes: None,
            redact_query: false,
            log_body_content_types: default_loggable_content_types(),
        }
    }

    /// Set logging level.
    pub fn with_level(mut self, level: tracing::Level) -> Self {
        self.level = level;
        self
    }

    /// Enable query parameter redaction.
    pub fn with_redact_query(mut self, redact: bool) -> Self {
        self.redact_query = redact;
        self
    }

    /// Set maximum body bytes to log.
    ///
    /// Bodies will only be logged for non-streaming responses with
    /// allowed content types. Set to Some(bytes) to enable body logging.
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = Some(bytes);
        self
    }

    /// Add a header to the redaction list.
    pub fn redact_header(mut self, name: &str) -> Self {
        if let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) {
            self.redact_headers.insert(header_name);
        }
        self
    }

    /// Remove a header from the redaction list.
    ///
    /// **Warning**: Use with caution. Only remove headers from redaction
    /// if you're certain they don't contain sensitive data.
    pub fn allow_header(mut self, name: &str) -> Self {
        if let Ok(header_name) = HeaderName::from_bytes(name.as_bytes()) {
            self.redact_headers.remove(&header_name);
        }
        self
    }

    /// Allow a content type for body logging.
    pub fn allow_body_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.log_body_content_types.insert(content_type.into());
        self
    }

    /// Set whether to show auth scheme in redacted headers.
    pub fn with_show_auth_scheme(mut self, show: bool) -> Self {
        self.show_auth_scheme = show;
        self
    }

    /// Set maximum header value length before truncation.
    pub fn with_max_header_value_len(mut self, len: usize) -> Self {
        self.max_header_value_len = Some(len);
        self
    }

    /// Check if body should be logged for this response.
    fn should_log_body(&self, response: &ServerHttpResponse) -> bool {
        use crate::shared::http_utils::should_log_body_for_content_type;

        if self.max_body_bytes.is_none() {
            return false;
        }

        // Check for streaming response (SSE, chunked, etc.)
        // SSE responses typically have text/event-stream content-type
        if let Some(ct) = response.get_header("content-type") {
            if ct.contains("text/event-stream") || ct.contains("stream") {
                return false; // Never buffer streaming responses
            }
        }

        // Check content-type gating
        should_log_body_for_content_type(
            response.get_header("content-type"),
            &self.log_body_content_types,
        )
    }
}

impl Default for ServerHttpLoggingMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServerHttpMiddleware for ServerHttpLoggingMiddleware {
    fn priority(&self) -> i32 {
        90 // Log after auth/rate limiting, before response mutations
    }

    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        context: &ServerHttpContext,
    ) -> Result<()> {
        use crate::shared::http_utils::{format_headers_for_logging, redact_url_query};

        // Redact URL query if configured
        let uri = redact_url_query(&request.uri.to_string(), self.redact_query);

        // Format headers with redaction
        let headers_str = format_headers_for_logging(
            &request.headers,
            &self.redact_headers,
            self.show_auth_scheme,
            self.max_header_value_len,
        );

        // Log request (bodies not logged by default for security)
        match self.level {
            tracing::Level::TRACE => tracing::trace!(
                request_id = %context.request_id,
                method = %request.method,
                uri = %uri,
                headers = %headers_str,
                body_len = request.body.len(),
                "Incoming HTTP request"
            ),
            tracing::Level::DEBUG => tracing::debug!(
                request_id = %context.request_id,
                method = %request.method,
                uri = %uri,
                headers = %headers_str,
                body_len = request.body.len(),
                "Incoming HTTP request"
            ),
            tracing::Level::INFO => tracing::info!(
                request_id = %context.request_id,
                method = %request.method,
                uri = %uri,
                "Incoming HTTP request"
            ),
            tracing::Level::WARN => tracing::warn!(
                request_id = %context.request_id,
                method = %request.method,
                uri = %uri,
                "Incoming HTTP request"
            ),
            tracing::Level::ERROR => tracing::error!(
                request_id = %context.request_id,
                method = %request.method,
                uri = %uri,
                "Incoming HTTP request"
            ),
        }

        Ok(())
    }

    async fn on_response(
        &self,
        response: &mut ServerHttpResponse,
        context: &ServerHttpContext,
    ) -> Result<()> {
        use crate::shared::http_utils::format_headers_for_logging;

        let elapsed_ms = context.elapsed().as_millis();

        // Format headers with redaction
        let headers_str = format_headers_for_logging(
            &response.headers,
            &self.redact_headers,
            self.show_auth_scheme,
            self.max_header_value_len,
        );

        // Check if we should log body
        let should_log_body = self.should_log_body(response);

        if should_log_body {
            // Preview body (capped at max_body_bytes)
            let max_bytes = self.max_body_bytes.unwrap_or(0);
            let body_preview_len = response.body.len().min(max_bytes);
            let body_preview = if body_preview_len > 0 {
                String::from_utf8_lossy(&response.body[..body_preview_len])
            } else {
                std::borrow::Cow::Borrowed("")
            };

            match self.level {
                tracing::Level::TRACE => tracing::trace!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    headers = %headers_str,
                    body_len = response.body.len(),
                    body_preview = %body_preview,
                    "Outgoing HTTP response"
                ),
                tracing::Level::DEBUG => tracing::debug!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    headers = %headers_str,
                    body_len = response.body.len(),
                    body_preview = %body_preview,
                    "Outgoing HTTP response"
                ),
                tracing::Level::INFO => tracing::info!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    body_len = response.body.len(),
                    "Outgoing HTTP response"
                ),
                tracing::Level::WARN => tracing::warn!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    body_len = response.body.len(),
                    "Outgoing HTTP response"
                ),
                tracing::Level::ERROR => tracing::error!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    body_len = response.body.len(),
                    "Outgoing HTTP response"
                ),
            }
        } else {
            // No body logging (streaming or not configured)
            match self.level {
                tracing::Level::TRACE => tracing::trace!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    headers = %headers_str,
                    body_len = response.body.len(),
                    "Outgoing HTTP response"
                ),
                tracing::Level::DEBUG => tracing::debug!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    headers = %headers_str,
                    body_len = response.body.len(),
                    "Outgoing HTTP response"
                ),
                tracing::Level::INFO => tracing::info!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    "Outgoing HTTP response"
                ),
                tracing::Level::WARN => tracing::warn!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    "Outgoing HTTP response"
                ),
                tracing::Level::ERROR => tracing::error!(
                    request_id = %context.request_id,
                    status = %response.status,
                    elapsed_ms = elapsed_ms,
                    "Outgoing HTTP response"
                ),
            }
        }

        Ok(())
    }

    async fn on_error(&self, error: &Error, context: &ServerHttpContext) -> Result<()> {
        tracing::error!(
            request_id = %context.request_id,
            elapsed_ms = context.elapsed().as_millis(),
            error = %error,
            "HTTP request error"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_http_middleware_chain() {
        let mut chain = ServerHttpMiddlewareChain::new();
        chain.add(Arc::new(ServerHttpLoggingMiddleware::new()));

        let context = ServerHttpContext::new("test-request-001".to_string());
        let mut request = ServerHttpRequest::new(
            Method::POST,
            "/mcp".parse().unwrap(),
            HeaderMap::new(),
            vec![],
        );

        // Should process without error
        assert!(chain.process_request(&mut request, &context).await.is_ok());
    }

    #[tokio::test]
    async fn test_server_http_context() {
        let context = ServerHttpContext::new("req-123".to_string());
        assert_eq!(context.request_id, "req-123");
        assert!(context.session_id.is_none());

        let with_session =
            ServerHttpContext::with_session("req-456".to_string(), "sess-789".to_string());
        assert_eq!(with_session.session_id, Some("sess-789".to_string()));
    }

    #[tokio::test]
    async fn test_server_logging_middleware_sensitive_headers() {
        let logging = ServerHttpLoggingMiddleware::new();
        let context = ServerHttpContext::new("req-001".to_string());

        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            "Bearer secret-token-12345".parse().unwrap(),
        );
        headers.insert("cookie", "session=abc123".parse().unwrap());
        headers.insert("x-api-key", "api-key-secret".parse().unwrap());
        headers.insert("x-amz-security-token", "aws-token-secret".parse().unwrap());
        headers.insert("x-goog-api-key", "gcp-api-key-secret".parse().unwrap());

        let mut request =
            ServerHttpRequest::new(Method::POST, "/mcp".parse().unwrap(), headers, vec![]);

        // Should process without error (redaction happens during logging)
        assert!(logging.on_request(&mut request, &context).await.is_ok());
    }

    #[tokio::test]
    async fn test_server_logging_middleware_query_redaction() {
        let logging = ServerHttpLoggingMiddleware::new().with_redact_query(true);
        let context = ServerHttpContext::new("req-002".to_string());

        let mut request = ServerHttpRequest::new(
            Method::GET,
            "/api/users?token=secret&id=123".parse().unwrap(),
            HeaderMap::new(),
            vec![],
        );

        // Should process without error (query redaction happens during logging)
        assert!(logging.on_request(&mut request, &context).await.is_ok());
    }

    #[tokio::test]
    async fn test_server_logging_middleware_multivalue_headers() {
        let logging = ServerHttpLoggingMiddleware::new();
        let context = ServerHttpContext::new("req-003".to_string());

        let mut headers = HeaderMap::new();
        headers.append("set-cookie", "session1=abc123".parse().unwrap());
        headers.append("set-cookie", "session2=def456".parse().unwrap());
        headers.append("set-cookie", "user=john".parse().unwrap());

        let mut response = ServerHttpResponse::new(StatusCode::OK, headers, vec![]);

        // Should process without error (multi-value headers handled)
        assert!(logging.on_response(&mut response, &context).await.is_ok());
    }

    #[tokio::test]
    async fn test_server_logging_middleware_sse_detection() {
        let logging = ServerHttpLoggingMiddleware::new().with_max_body_bytes(1024);

        let mut headers = HeaderMap::new();
        headers.insert("content-type", "text/event-stream".parse().unwrap());

        let response =
            ServerHttpResponse::new(StatusCode::OK, headers, b"data: event data\n\n".to_vec());

        // Should NOT log body for SSE responses
        assert!(!logging.should_log_body(&response));
    }

    #[tokio::test]
    async fn test_server_logging_middleware_content_type_gating() {
        let logging = ServerHttpLoggingMiddleware::new().with_max_body_bytes(512);

        // JSON should be allowed
        let mut headers_json = HeaderMap::new();
        headers_json.insert("content-type", "application/json".parse().unwrap());
        let response_json = ServerHttpResponse::new(
            StatusCode::OK,
            headers_json,
            b"{\"result\":\"success\"}".to_vec(),
        );
        assert!(logging.should_log_body(&response_json));

        // Binary should NOT be allowed
        let mut headers_binary = HeaderMap::new();
        headers_binary.insert("content-type", "application/octet-stream".parse().unwrap());
        let response_binary =
            ServerHttpResponse::new(StatusCode::OK, headers_binary, vec![0x00, 0x01, 0x02]);
        assert!(!logging.should_log_body(&response_binary));
    }

    #[tokio::test]
    async fn test_server_logging_middleware_error_handling() {
        let logging = ServerHttpLoggingMiddleware::new();
        let context = ServerHttpContext::new("req-004".to_string());

        let error = crate::Error::internal("Test error");

        // Should process error without panic
        assert!(logging.on_error(&error, &context).await.is_ok());
    }

    #[tokio::test]
    async fn test_server_logging_middleware_custom_config() {
        let logging = ServerHttpLoggingMiddleware::new()
            .with_level(tracing::Level::DEBUG)
            .with_redact_query(true)
            .with_max_body_bytes(256)
            .redact_header("x-custom-secret")
            .allow_body_content_type("application/xml");

        let context = ServerHttpContext::new("req-005".to_string());

        let mut headers = HeaderMap::new();
        headers.insert("x-custom-secret", "my-secret-value".parse().unwrap());

        let mut request =
            ServerHttpRequest::new(Method::POST, "/api/data".parse().unwrap(), headers, vec![]);

        // Should process with custom configuration
        assert!(logging.on_request(&mut request, &context).await.is_ok());
    }
}

/// Framework adapters for converting between middleware types and framework types.
///
/// These adapters provide zero-cost conversions between the framework-agnostic
/// middleware types and framework-specific types (axum, hyper, etc.).
#[cfg(feature = "streamable-http")]
pub mod adapters {
    use super::{Result, ServerHttpRequest, ServerHttpResponse};
    use axum::body::Body;
    use axum::response::Response;
    use hyper::body::Buf;

    /// Convert an axum Request into a `ServerHttpRequest`.
    ///
    /// This adapter extracts the method, URI, headers, and body from an axum
    /// request, converting the streaming body into bytes.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use axum::http::Request;
    /// use pmcp::server::http_middleware::adapters::from_axum;
    ///
    /// async fn handler(req: Request<Body>) -> Response {
    ///     let (parts, body) = req.into_parts();
    ///     let server_req = from_axum(parts, body).await.unwrap();
    ///
    ///     // Process through middleware...
    ///
    ///     Response::new(Body::empty())
    /// }
    /// ```
    pub async fn from_axum(
        parts: hyper::http::request::Parts,
        body: Body,
    ) -> Result<ServerHttpRequest> {
        use axum::body::to_bytes;

        // Extract body bytes
        let body_bytes = to_bytes(body, usize::MAX)
            .await
            .map_err(|e| crate::Error::internal(format!("Failed to read request body: {}", e)))?;

        Ok(ServerHttpRequest {
            method: parts.method,
            uri: parts.uri,
            headers: parts.headers,
            body: body_bytes.chunk().to_vec(),
        })
    }

    /// Convert a `ServerHttpResponse` into an axum Response.
    ///
    /// This adapter applies the status code, headers (including multi-value headers),
    /// and body to an axum response. Content-Length is set automatically based on
    /// the body size.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::server::http_middleware::{ServerHttpResponse, adapters::into_axum};
    /// use axum::http::StatusCode;
    ///
    /// async fn handler() -> Response {
    ///     let server_res = ServerHttpResponse::new(
    ///         StatusCode::OK,
    ///         HeaderMap::new(),
    ///         b"Hello, world!".to_vec(),
    ///     );
    ///
    ///     into_axum(server_res)
    /// }
    /// ```
    pub fn into_axum(response: ServerHttpResponse) -> Response {
        use axum::http::header::CONTENT_LENGTH;

        let mut axum_response = Response::builder().status(response.status);

        // Apply all headers (including multi-value headers)
        for (name, value) in &response.headers {
            axum_response = axum_response.header(name, value);
        }

        // Set content-length if not already set
        if !response.headers.contains_key(CONTENT_LENGTH) && !response.body.is_empty() {
            axum_response = axum_response.header(CONTENT_LENGTH, response.body.len());
        }

        // Build response with body
        axum_response
            .body(Body::from(response.body))
            .unwrap_or_else(|e| {
                // Fallback to error response if construction fails
                Response::builder()
                    .status(hyper::http::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::from(format!("Failed to build response: {}", e)))
                    .unwrap()
            })
    }

    /// Convert a hyper Request into a `ServerHttpRequest` (thin wrapper).
    ///
    /// This is a thin wrapper around `from_axum` since axum uses hyper internally.
    pub async fn from_hyper(
        parts: hyper::http::request::Parts,
        body: Body,
    ) -> Result<ServerHttpRequest> {
        from_axum(parts, body).await
    }

    /// Convert a `ServerHttpResponse` into a hyper Response (thin wrapper).
    ///
    /// This is a thin wrapper around `into_axum` since axum uses hyper internally.
    pub fn into_hyper(response: ServerHttpResponse) -> Response {
        into_axum(response)
    }

    #[cfg(test)]
    mod tests {
        use super::super::{HeaderMap, Method, StatusCode};
        use super::*;
        use axum::body::Body;
        use http::Request;

        #[tokio::test]
        async fn test_from_axum_basic() {
            let req = Request::builder()
                .method("POST")
                .uri("/test")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"test":"data"}"#))
                .unwrap();

            let (parts, body) = req.into_parts();
            let server_req = from_axum(parts, body).await.unwrap();

            assert_eq!(server_req.method, Method::POST);
            assert_eq!(server_req.uri.path(), "/test");
            assert_eq!(
                server_req.get_header("content-type").unwrap(),
                "application/json"
            );
            assert_eq!(
                String::from_utf8_lossy(&server_req.body),
                r#"{"test":"data"}"#
            );
        }

        #[tokio::test]
        async fn test_from_axum_multi_value_headers() {
            let mut req = Request::builder()
                .method("GET")
                .uri("/cookies")
                .body(Body::empty())
                .unwrap();

            // Add multiple set-cookie headers
            req.headers_mut()
                .insert("set-cookie", "session=abc123".parse().unwrap());
            req.headers_mut()
                .append("set-cookie", "user=john".parse().unwrap());

            let (parts, body) = req.into_parts();
            let server_req = from_axum(parts, body).await.unwrap();

            let cookies: Vec<_> = server_req
                .headers
                .get_all("set-cookie")
                .iter()
                .map(|v| v.to_str().unwrap())
                .collect();
            assert_eq!(cookies.len(), 2);
            assert!(cookies.contains(&"session=abc123"));
            assert!(cookies.contains(&"user=john"));
        }

        #[tokio::test]
        async fn test_into_axum_basic() {
            let mut headers = HeaderMap::new();
            headers.insert("content-type", "application/json".parse().unwrap());

            let server_res = ServerHttpResponse::new(
                StatusCode::OK,
                headers,
                br#"{"result":"success"}"#.to_vec(),
            );

            let axum_res = into_axum(server_res);

            assert_eq!(axum_res.status(), StatusCode::OK);
            assert_eq!(
                axum_res
                    .headers()
                    .get("content-type")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "application/json"
            );
            assert_eq!(
                axum_res
                    .headers()
                    .get("content-length")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "20"
            );

            // Verify body
            let body_bytes = axum::body::to_bytes(axum_res.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(body_bytes.chunk()),
                r#"{"result":"success"}"#
            );
        }

        #[tokio::test]
        async fn test_into_axum_multi_value_headers() {
            let mut headers = HeaderMap::new();
            headers.insert("set-cookie", "session=xyz789".parse().unwrap());
            headers.append("set-cookie", "preferences=dark".parse().unwrap());

            let server_res = ServerHttpResponse::new(StatusCode::OK, headers, vec![]);

            let axum_res = into_axum(server_res);

            let cookies: Vec<_> = axum_res
                .headers()
                .get_all("set-cookie")
                .iter()
                .map(|v| v.to_str().unwrap())
                .collect();
            assert_eq!(cookies.len(), 2);
            assert!(cookies.contains(&"session=xyz789"));
            assert!(cookies.contains(&"preferences=dark"));
        }

        #[tokio::test]
        async fn test_round_trip_fidelity() {
            // Create an axum request
            let original_req = Request::builder()
                .method("POST")
                .uri("/api/test?param=value")
                .header("authorization", "Bearer token123")
                .header("content-type", "application/json")
                .header("x-custom", "custom-value")
                .body(Body::from(r#"{"data":"test"}"#))
                .unwrap();

            let original_method = original_req.method().clone();
            let original_uri = original_req.uri().clone();

            // Convert to ServerHttpRequest
            let (parts, body) = original_req.into_parts();
            let server_req = from_axum(parts, body).await.unwrap();

            // Verify conversion preserved everything
            assert_eq!(server_req.method, original_method);
            assert_eq!(server_req.uri, original_uri);
            assert_eq!(
                server_req.get_header("authorization").unwrap(),
                "Bearer token123"
            );
            assert_eq!(
                server_req.get_header("content-type").unwrap(),
                "application/json"
            );
            assert_eq!(server_req.get_header("x-custom").unwrap(), "custom-value");

            // Create a response
            let mut response_headers = HeaderMap::new();
            response_headers.insert("content-type", "application/json".parse().unwrap());
            response_headers.insert("x-response", "response-value".parse().unwrap());

            let response = ServerHttpResponse::new(
                StatusCode::CREATED,
                response_headers.clone(),
                br#"{"status":"created"}"#.to_vec(),
            );

            // Convert to axum Response
            let axum_res = into_axum(response);

            // Verify response conversion
            assert_eq!(axum_res.status(), StatusCode::CREATED);
            assert_eq!(
                axum_res
                    .headers()
                    .get("content-type")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "application/json"
            );
            assert_eq!(
                axum_res
                    .headers()
                    .get("x-response")
                    .unwrap()
                    .to_str()
                    .unwrap(),
                "response-value"
            );

            let body_bytes = axum::body::to_bytes(axum_res.into_body(), usize::MAX)
                .await
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(body_bytes.chunk()),
                r#"{"status":"created"}"#
            );
        }

        #[tokio::test]
        async fn test_hyper_adapters_are_thin_wrappers() {
            // Verify hyper adapters work identically to axum adapters
            let req = Request::builder()
                .method("GET")
                .uri("/test")
                .body(Body::from("test"))
                .unwrap();

            let (parts, body) = req.into_parts();

            // Test from_hyper
            let server_req = from_hyper(parts, body).await.unwrap();
            assert_eq!(server_req.method, Method::GET);
            assert_eq!(server_req.uri.path(), "/test");

            // Test into_hyper
            let response =
                ServerHttpResponse::new(StatusCode::OK, HeaderMap::new(), b"response".to_vec());
            let hyper_res = into_hyper(response);
            assert_eq!(hyper_res.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn test_empty_body_handling() {
            // Test empty request body
            let req = Request::builder()
                .method("GET")
                .uri("/empty")
                .body(Body::empty())
                .unwrap();

            let (parts, body) = req.into_parts();
            let server_req = from_axum(parts, body).await.unwrap();
            assert!(server_req.body.is_empty());

            // Test empty response body
            let response =
                ServerHttpResponse::new(StatusCode::NO_CONTENT, HeaderMap::new(), vec![]);
            let axum_res = into_axum(response);
            assert_eq!(axum_res.status(), StatusCode::NO_CONTENT);

            // Empty body should not have content-length
            let body_bytes = axum::body::to_bytes(axum_res.into_body(), usize::MAX)
                .await
                .unwrap();
            assert!(body_bytes.is_empty());
        }
    }
}
