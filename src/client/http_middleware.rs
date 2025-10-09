//! HTTP transport middleware for request/response transformation.
//!
//! Provides middleware that operates at the HTTP layer, before MCP protocol processing.
//! This allows for header injection, compression, authentication, and other HTTP-specific concerns.

use crate::error::Result;
use async_trait::async_trait;
use hyper::http::header::{HeaderName, HeaderValue};
use hyper::http::HeaderMap;
use std::collections::HashMap;
use std::sync::Arc;

/// Context for HTTP middleware execution.
#[derive(Debug, Clone)]
pub struct HttpMiddlewareContext {
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// URL being requested
    pub url: String,
    /// HTTP method
    pub method: String,
    /// Attempt number (for retry middleware)
    pub attempt: u32,
    /// Custom metadata
    pub metadata: Arc<parking_lot::RwLock<HashMap<String, String>>>,
}

impl HttpMiddlewareContext {
    /// Create a new HTTP middleware context
    pub fn new(url: String, method: String) -> Self {
        Self {
            request_id: None,
            url,
            method,
            attempt: 0,
            metadata: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    /// Set metadata value
    pub fn set_metadata(&self, key: String, value: String) {
        self.metadata.write().insert(key, value);
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.read().get(key).cloned()
    }
}

/// Simple HTTP request/response representation for middleware
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Request URL
    pub url: String,
    /// Request headers (case-insensitive)
    pub headers: HeaderMap,
    /// Request body
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Create a new HTTP request
    pub fn new(method: String, url: String, body: Vec<u8>) -> Self {
        Self {
            method,
            url,
            headers: HeaderMap::new(),
            body,
        }
    }

    /// Add a header (case-insensitive)
    ///
    /// # Panics
    /// Panics if the header name or value is invalid
    pub fn add_header(&mut self, name: &str, value: &str) {
        let header_name = HeaderName::from_bytes(name.as_bytes()).expect("Invalid header name");
        let header_value = HeaderValue::from_str(value).expect("Invalid header value");
        self.headers.insert(header_name, header_value);
    }

    /// Get a header value (case-insensitive lookup)
    pub fn get_header(&self, name: &str) -> Option<&str> {
        let header_name = HeaderName::from_bytes(name.as_bytes()).ok()?;
        self.headers.get(header_name)?.to_str().ok()
    }

    /// Check if a header exists (case-insensitive)
    pub fn has_header(&self, name: &str) -> bool {
        HeaderName::from_bytes(name.as_bytes())
            .ok()
            .and_then(|n| self.headers.get(n))
            .is_some()
    }

    /// Remove a header (case-insensitive)
    pub fn remove_header(&mut self, name: &str) -> Option<String> {
        let header_name = HeaderName::from_bytes(name.as_bytes()).ok()?;
        self.headers
            .remove(header_name)
            .and_then(|v| v.to_str().ok().map(|s| s.to_string()))
    }
}

/// HTTP response representation for middleware
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers (case-insensitive)
    pub headers: HeaderMap,
    /// Response body
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Create a new HTTP response
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body,
        }
    }

    /// Create a response with headers
    pub fn with_headers(status: u16, headers: HeaderMap, body: Vec<u8>) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// Add a header (case-insensitive)
    ///
    /// # Panics
    /// Panics if the header name or value is invalid
    pub fn add_header(&mut self, name: &str, value: &str) {
        let header_name = HeaderName::from_bytes(name.as_bytes()).expect("Invalid header name");
        let header_value = HeaderValue::from_str(value).expect("Invalid header value");
        self.headers.insert(header_name, header_value);
    }

    /// Get a header value (case-insensitive lookup)
    pub fn get_header(&self, name: &str) -> Option<&str> {
        let header_name = HeaderName::from_bytes(name.as_bytes()).ok()?;
        self.headers.get(header_name)?.to_str().ok()
    }

    /// Check if a header exists (case-insensitive)
    pub fn has_header(&self, name: &str) -> bool {
        HeaderName::from_bytes(name.as_bytes())
            .ok()
            .and_then(|n| self.headers.get(n))
            .is_some()
    }

    /// Check if response is success (2xx)
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// Check if response is client error (4xx)
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status)
    }

    /// Check if response is server error (5xx)
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status)
    }
}

/// HTTP-level middleware trait.
///
/// This trait operates at the HTTP transport layer, before MCP protocol processing.
/// It's useful for:
/// - Header injection (auth tokens, correlation IDs)
/// - Request/response logging with HTTP details
/// - Compression
/// - Status code-based error handling
///
/// # Error Handling
///
/// - If `on_request` returns `Err`, the chain short-circuits and `on_error` is called for all middleware
/// - If `on_response` returns `Err`, the chain short-circuits and `on_error` is called for all middleware
/// - `on_error` allows middleware to clean up resources or log errors
///
/// # Examples
///
/// ```rust
/// use pmcp::client::http_middleware::{HttpMiddleware, HttpRequest, HttpResponse, HttpMiddlewareContext};
/// use async_trait::async_trait;
///
/// struct CustomHeaderMiddleware {
///     api_key: String,
/// }
///
/// #[async_trait]
/// impl HttpMiddleware for CustomHeaderMiddleware {
///     async fn on_request(
///         &self,
///         request: &mut HttpRequest,
///         _context: &HttpMiddlewareContext,
///     ) -> pmcp::Result<()> {
///         request.add_header("X-API-Key", &self.api_key);
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait HttpMiddleware: Send + Sync {
    /// Called before HTTP request is sent
    async fn on_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        let _ = (request, context);
        Ok(())
    }

    /// Called after HTTP response is received
    async fn on_response(
        &self,
        response: &mut HttpResponse,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        let _ = (response, context);
        Ok(())
    }

    /// Called when an error occurs during middleware execution or transport operations.
    ///
    /// This hook allows middleware to:
    /// - Clean up resources (e.g., release locks, close connections)
    /// - Log errors with context
    /// - Record metrics for failures
    ///
    /// Note: Errors from `on_error` itself are logged but don't propagate to avoid cascading failures.
    async fn on_error(
        &self,
        error: &crate::error::Error,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        let _ = (error, context);
        Ok(())
    }

    /// Priority for ordering (lower runs first)
    fn priority(&self) -> i32 {
        50 // Default priority
    }

    /// Should this middleware execute for this context?
    async fn should_execute(&self, _context: &HttpMiddlewareContext) -> bool {
        true
    }
}

/// Chain of HTTP middleware
pub struct HttpMiddlewareChain {
    middlewares: Vec<Arc<dyn HttpMiddleware>>,
}

impl HttpMiddlewareChain {
    /// Create a new HTTP middleware chain
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add middleware to the chain
    pub fn add(&mut self, middleware: Arc<dyn HttpMiddleware>) {
        self.middlewares.push(middleware);
        // Sort by priority
        self.middlewares.sort_by_key(|m| m.priority());
    }

    /// Process request through all middleware.
    ///
    /// If any middleware returns an error:
    /// 1. Processing short-circuits immediately
    /// 2. `on_error` is called for all middleware (to allow cleanup)
    /// 3. The original error is returned
    pub async fn process_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_request(request, context).await {
                    // Short-circuit: call on_error for all middleware
                    self.handle_error(&e, context).await;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Process response through all middleware (in reverse order).
    ///
    /// If any middleware returns an error:
    /// 1. Processing short-circuits immediately
    /// 2. `on_error` is called for all middleware (to allow cleanup)
    /// 3. The original error is returned
    pub async fn process_response(
        &self,
        response: &mut HttpResponse,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        for middleware in self.middlewares.iter().rev() {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_response(response, context).await {
                    // Short-circuit: call on_error for all middleware
                    self.handle_error(&e, context).await;
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Handle error by calling `on_error` for all middleware.
    ///
    /// Errors from `on_error` itself are logged but don't propagate.
    async fn handle_error(&self, error: &crate::error::Error, context: &HttpMiddlewareContext) {
        for middleware in &self.middlewares {
            if let Err(e) = middleware.on_error(error, context).await {
                // Log but don't propagate to avoid cascading failures
                tracing::error!(
                    "Error in middleware on_error hook: {} (original error: {})",
                    e,
                    error
                );
            }
        }
    }

    /// Handle error from transport operations.
    ///
    /// This should be called when a transport error occurs (e.g., network failure)
    /// to allow middleware to clean up resources.
    pub async fn handle_transport_error(
        &self,
        error: &crate::error::Error,
        context: &HttpMiddlewareContext,
    ) {
        self.handle_error(error, context).await;
    }
}

impl Default for HttpMiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HttpMiddlewareChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpMiddlewareChain")
            .field("count", &self.middlewares.len())
            .finish()
    }
}
