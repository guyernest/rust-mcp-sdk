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
//! - **HTTP Layer**: ServerHttpMiddleware for transport-level concerns
//! - **Protocol Layer**: EnhancedMiddleware for JSON-RPC message processing
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
use hyper::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri};
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
///     .redact_header("x-custom-secret");
/// ```
#[derive(Debug)]
pub struct ServerHttpLoggingMiddleware {
    // Implementation will mirror client HttpLoggingMiddleware
    // Placeholder for now - full implementation next
}

impl ServerHttpLoggingMiddleware {
    /// Create new logging middleware with secure defaults.
    pub fn new() -> Self {
        Self {}
    }

    /// Enable query parameter redaction.
    pub fn with_redact_query(self, _redact: bool) -> Self {
        self
    }

    /// Set maximum body bytes to log.
    pub fn with_max_body_bytes(self, _bytes: usize) -> Self {
        self
    }

    /// Add a header to the redaction list.
    pub fn redact_header(self, _name: &str) -> Self {
        self
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
        90 // Log after auth/rate limiting
    }

    async fn on_request(
        &self,
        request: &mut ServerHttpRequest,
        context: &ServerHttpContext,
    ) -> Result<()> {
        // Basic logging placeholder - full implementation next
        tracing::info!(
            request_id = %context.request_id,
            method = %request.method,
            uri = %request.uri,
            "Incoming HTTP request"
        );
        Ok(())
    }

    async fn on_response(
        &self,
        response: &mut ServerHttpResponse,
        context: &ServerHttpContext,
    ) -> Result<()> {
        tracing::info!(
            request_id = %context.request_id,
            status = %response.status,
            elapsed_ms = context.elapsed().as_millis(),
            "Outgoing HTTP response"
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
}
