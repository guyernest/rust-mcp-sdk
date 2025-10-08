//! HTTP transport middleware for request/response transformation.
//!
//! Provides middleware that operates at the HTTP layer, before MCP protocol processing.
//! This allows for header injection, compression, authentication, and other HTTP-specific concerns.

use crate::error::Result;
use async_trait::async_trait;
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
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Create a new HTTP request
    pub fn new(body: Vec<u8>) -> Self {
        Self {
            headers: HashMap::new(),
            body,
        }
    }

    /// Add a header
    pub fn add_header(&mut self, name: String, value: String) {
        self.headers.insert(name, value);
    }

    /// Get a header value
    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.get(name)
    }
}

/// HTTP response representation for middleware
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Create a new HTTP response
    pub fn new(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: HashMap::new(),
            body,
        }
    }

    /// Add a header
    pub fn add_header(&mut self, name: String, value: String) {
        self.headers.insert(name, value);
    }

    /// Get a header value
    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.get(name)
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
///         request.add_header("X-API-Key".to_string(), self.api_key.clone());
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

    /// Process request through all middleware
    pub async fn process_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                middleware.on_request(request, context).await?;
            }
        }
        Ok(())
    }

    /// Process response through all middleware (in reverse order)
    pub async fn process_response(
        &self,
        response: &mut HttpResponse,
        context: &HttpMiddlewareContext,
    ) -> Result<()> {
        for middleware in self.middlewares.iter().rev() {
            if middleware.should_execute(context).await {
                middleware.on_response(response, context).await?;
            }
        }
        Ok(())
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
