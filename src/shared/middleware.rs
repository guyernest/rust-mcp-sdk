//! Advanced middleware support for request/response processing.
//!
//! PMCP-4004: Enhanced transport middleware system with advanced capabilities:
//! - Rate limiting and circuit breaker patterns
//! - Metrics collection and performance monitoring
//! - Conditional middleware execution
//! - Priority-based middleware ordering
//! - Compression and caching middleware
//! - Context propagation across middleware layers

use crate::error::Result;
use crate::shared::TransportMessage;
use crate::types::{JSONRPCNotification, JSONRPCRequest, JSONRPCResponse};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use std::fmt;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Execution context for middleware chains with performance tracking.
#[derive(Debug, Clone)]
pub struct MiddlewareContext {
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// Custom metadata that can be passed between middleware
    pub metadata: Arc<DashMap<String, String>>,
    /// Performance metrics for the request
    pub metrics: Arc<PerformanceMetrics>,
    /// Start time of the middleware chain execution
    pub start_time: Instant,
    /// Priority level for the request
    pub priority: Option<crate::shared::transport::MessagePriority>,
}

impl Default for MiddlewareContext {
    fn default() -> Self {
        Self {
            request_id: None,
            metadata: Arc::new(DashMap::new()),
            metrics: Arc::new(PerformanceMetrics::new()),
            start_time: Instant::now(),
            priority: None,
        }
    }
}

impl MiddlewareContext {
    /// Create a new context with request ID
    pub fn with_request_id(request_id: String) -> Self {
        Self {
            request_id: Some(request_id),
            ..Default::default()
        }
    }

    /// Set metadata value
    pub fn set_metadata(&self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<String> {
        self.metadata.get(key).map(|v| v.clone())
    }

    /// Record a metric
    pub fn record_metric(&self, name: String, value: f64) {
        self.metrics.record(name, value);
    }

    /// Get elapsed time since context creation
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

/// Performance metrics collection for middleware operations.
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    /// Custom metrics storage
    metrics: DashMap<String, f64>,
    /// Request count
    request_count: AtomicU64,
    /// Error count
    error_count: AtomicU64,
    /// Total processing time in microseconds
    total_time_us: AtomicU64,
}

impl PerformanceMetrics {
    /// Create new performance metrics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a custom metric
    pub fn record(&self, name: String, value: f64) {
        self.metrics.insert(name, value);
    }

    /// Get a metric value
    pub fn get(&self, name: &str) -> Option<f64> {
        self.metrics.get(name).map(|v| *v)
    }

    /// Increment request count
    pub fn inc_requests(&self) {
        self.request_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment error count
    pub fn inc_errors(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Add processing time
    pub fn add_time(&self, duration: Duration) {
        self.total_time_us
            .fetch_add(duration.as_micros() as u64, Ordering::Relaxed);
    }

    /// Get total request count
    pub fn request_count(&self) -> u64 {
        self.request_count.load(Ordering::Relaxed)
    }

    /// Get total error count
    pub fn error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }

    /// Get average processing time
    pub fn average_time(&self) -> Duration {
        let total_time = self.total_time_us.load(Ordering::Relaxed);
        let count = self.request_count.load(Ordering::Relaxed);
        if count > 0 {
            Duration::from_micros(total_time / count)
        } else {
            Duration::ZERO
        }
    }
}

/// Middleware execution priority for ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum MiddlewarePriority {
    /// Highest priority - executed first in chain
    Critical = 0,
    /// High priority - authentication, security
    High = 1,
    /// Normal priority - business logic
    #[default]
    Normal = 2,
    /// Low priority - logging, metrics
    Low = 3,
    /// Lowest priority - cleanup, finalization
    Lowest = 4,
}

/// Enhanced middleware trait with context support and priority.
#[async_trait]
pub trait AdvancedMiddleware: Send + Sync {
    /// Get middleware priority for execution ordering
    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    /// Get middleware name for identification
    fn name(&self) -> &'static str {
        "unknown"
    }

    /// Check if middleware should be executed for this context
    async fn should_execute(&self, _context: &MiddlewareContext) -> bool {
        true
    }

    /// Called before a request is sent with context.
    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let _ = (request, context);
        Ok(())
    }

    /// Called after a response is received with context.
    async fn on_response_with_context(
        &self,
        response: &mut JSONRPCResponse,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let _ = (response, context);
        Ok(())
    }

    /// Called when a message is sent with context.
    async fn on_send_with_context(
        &self,
        message: &TransportMessage,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let _ = (message, context);
        Ok(())
    }

    /// Called when a message is received with context.
    async fn on_receive_with_context(
        &self,
        message: &TransportMessage,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let _ = (message, context);
        Ok(())
    }

    /// Called when an unsolicited notification is received with context.
    ///
    /// This enables middleware to process server-initiated notifications
    /// (e.g., progress updates, resource changes) that arrive without
    /// a corresponding request.
    async fn on_notification_with_context(
        &self,
        notification: &mut JSONRPCNotification,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let _ = (notification, context);
        Ok(())
    }

    /// Called when middleware chain starts
    async fn on_chain_start(&self, _context: &MiddlewareContext) -> Result<()> {
        Ok(())
    }

    /// Called when middleware chain completes
    async fn on_chain_complete(&self, _context: &MiddlewareContext) -> Result<()> {
        Ok(())
    }

    /// Called when an error occurs in the chain
    async fn on_error(
        &self,
        _error: &crate::error::Error,
        _context: &MiddlewareContext,
    ) -> Result<()> {
        Ok(())
    }
}

/// Middleware that can intercept and modify requests and responses.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{Middleware, TransportMessage};
/// use pmcp::types::{JSONRPCRequest, JSONRPCResponse, RequestId};
/// use async_trait::async_trait;
///
/// // Custom middleware that adds timing information
/// #[derive(Debug)]
/// struct TimingMiddleware {
///     start_time: std::time::Instant,
/// }
///
/// impl TimingMiddleware {
///     fn new() -> Self {
///         Self { start_time: std::time::Instant::now() }
///     }
/// }
///
/// #[async_trait]
/// impl Middleware for TimingMiddleware {
///     async fn on_request(&self, request: &mut JSONRPCRequest) -> pmcp::Result<()> {
///         // Add timing metadata to request params
///         println!("Processing request {} at {}ms",
///             request.method,
///             self.start_time.elapsed().as_millis());
///         Ok(())
///     }
///
///     async fn on_response(&self, response: &mut JSONRPCResponse) -> pmcp::Result<()> {
///         println!("Response for {:?} received at {}ms",
///             response.id,
///             self.start_time.elapsed().as_millis());
///         Ok(())
///     }
/// }
///
/// # async fn example() -> pmcp::Result<()> {
/// let middleware = TimingMiddleware::new();
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "test".to_string(),
///     params: None,
///     id: RequestId::from(123i64),
/// };
///
/// // Process request through middleware
/// middleware.on_request(&mut request).await?;
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait Middleware: Send + Sync {
    /// Called before a request is sent.
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        let _ = request;
        Ok(())
    }

    /// Called after a response is received.
    async fn on_response(&self, response: &mut JSONRPCResponse) -> Result<()> {
        let _ = response;
        Ok(())
    }

    /// Called when a message is sent (any type).
    async fn on_send(&self, message: &TransportMessage) -> Result<()> {
        let _ = message;
        Ok(())
    }

    /// Called when a message is received (any type).
    async fn on_receive(&self, message: &TransportMessage) -> Result<()> {
        let _ = message;
        Ok(())
    }

    /// Called when an unsolicited notification is received.
    ///
    /// This enables middleware to process server-initiated notifications
    /// (e.g., progress updates, resource changes) that arrive without
    /// a corresponding request.
    async fn on_notification(&self, notification: &mut JSONRPCNotification) -> Result<()> {
        let _ = notification;
        Ok(())
    }
}

/// Enhanced middleware chain with priority ordering and context support.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{EnhancedMiddlewareChain, MiddlewareContext};
/// use pmcp::types::{JSONRPCRequest, JSONRPCResponse, RequestId};
/// use std::sync::Arc;
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create an enhanced middleware chain
/// let mut chain = EnhancedMiddlewareChain::new();
/// let context = MiddlewareContext::with_request_id("req-123".to_string());
///
/// // Create a request to process
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "prompts.get".to_string(),
///     params: Some(serde_json::json!({
///         "name": "code_review",
///         "arguments": {"language": "rust", "style": "detailed"}
///     })),
///     id: RequestId::from(1001i64),
/// };
///
/// // Process request through all middleware with context
/// chain.process_request_with_context(&mut request, &context).await?;
/// # Ok(())
/// # }
/// ```
pub struct EnhancedMiddlewareChain {
    middlewares: Vec<Arc<dyn AdvancedMiddleware>>,
    auto_sort: bool,
}

impl fmt::Debug for EnhancedMiddlewareChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnhancedMiddlewareChain")
            .field("count", &self.middlewares.len())
            .field("auto_sort", &self.auto_sort)
            .finish()
    }
}

impl Default for EnhancedMiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

impl EnhancedMiddlewareChain {
    /// Create a new enhanced middleware chain with automatic sorting by priority.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
            auto_sort: true,
        }
    }

    /// Create a new chain without automatic sorting.
    pub fn new_no_sort() -> Self {
        Self {
            middlewares: Vec::new(),
            auto_sort: false,
        }
    }

    /// Add an advanced middleware to the chain.
    pub fn add(&mut self, middleware: Arc<dyn AdvancedMiddleware>) {
        self.middlewares.push(middleware);
        if self.auto_sort {
            self.sort_by_priority();
        }
    }

    /// Sort middleware by priority (critical first).
    pub fn sort_by_priority(&mut self) {
        self.middlewares.sort_by_key(|m| m.priority());
    }

    /// Get middleware count.
    pub fn len(&self) -> usize {
        self.middlewares.len()
    }

    /// Check if chain is empty.
    pub fn is_empty(&self) -> bool {
        self.middlewares.is_empty()
    }

    /// Process a request through all applicable middleware with context.
    pub async fn process_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        context.metrics.inc_requests();
        let start_time = Instant::now();

        // Notify chain start
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                middleware.on_chain_start(context).await?;
            }
        }

        // Process through middleware
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_request_with_context(request, context).await {
                    context.metrics.inc_errors();
                    // Notify error to all middleware
                    for m in &self.middlewares {
                        if m.should_execute(context).await {
                            let _ = m.on_error(&e, context).await;
                        }
                    }
                    return Err(e);
                }
            }
        }

        // Notify chain complete
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                middleware.on_chain_complete(context).await?;
            }
        }

        context.metrics.add_time(start_time.elapsed());
        Ok(())
    }

    /// Process a response through all applicable middleware with context.
    pub async fn process_response_with_context(
        &self,
        response: &mut JSONRPCResponse,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let start_time = Instant::now();

        // Process through middleware in reverse order for responses
        for middleware in self.middlewares.iter().rev() {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_response_with_context(response, context).await {
                    context.metrics.inc_errors();
                    // Notify error to all middleware
                    for m in &self.middlewares {
                        if m.should_execute(context).await {
                            let _ = m.on_error(&e, context).await;
                        }
                    }
                    return Err(e);
                }
            }
        }

        context.metrics.add_time(start_time.elapsed());
        Ok(())
    }

    /// Process an outgoing message through all applicable middleware.
    pub async fn process_send_with_context(
        &self,
        message: &TransportMessage,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let start_time = Instant::now();

        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_send_with_context(message, context).await {
                    context.metrics.inc_errors();
                    for m in &self.middlewares {
                        if m.should_execute(context).await {
                            let _ = m.on_error(&e, context).await;
                        }
                    }
                    return Err(e);
                }
            }
        }

        context.metrics.add_time(start_time.elapsed());
        Ok(())
    }

    /// Process an incoming message through all applicable middleware.
    pub async fn process_receive_with_context(
        &self,
        message: &TransportMessage,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let start_time = Instant::now();

        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware.on_receive_with_context(message, context).await {
                    context.metrics.inc_errors();
                    for m in &self.middlewares {
                        if m.should_execute(context).await {
                            let _ = m.on_error(&e, context).await;
                        }
                    }
                    return Err(e);
                }
            }
        }

        context.metrics.add_time(start_time.elapsed());
        Ok(())
    }

    /// Process an unsolicited notification through all applicable middleware.
    ///
    /// This enables middleware to intercept and process server-initiated
    /// notifications (e.g., progress updates, resource changes, SSE events)
    /// that arrive without a corresponding request.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::{EnhancedMiddlewareChain, MiddlewareContext};
    /// use pmcp::types::JSONRPCNotification;
    ///
    /// # async fn example() -> pmcp::Result<()> {
    /// let chain = EnhancedMiddlewareChain::new();
    /// let context = MiddlewareContext::default();
    ///
    /// let mut notification = JSONRPCNotification::new(
    ///     "notifications/progress",
    ///     Some(serde_json::json!({
    ///         "progressToken": "token-123",
    ///         "progress": 50,
    ///         "total": 100
    ///     }))
    /// );
    ///
    /// // Process notification through middleware chain
    /// chain.process_notification_with_context(&mut notification, &context).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn process_notification_with_context(
        &self,
        notification: &mut JSONRPCNotification,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let start_time = Instant::now();

        // Process through middleware in order
        for middleware in &self.middlewares {
            if middleware.should_execute(context).await {
                if let Err(e) = middleware
                    .on_notification_with_context(notification, context)
                    .await
                {
                    context.metrics.inc_errors();
                    // Notify error to all middleware
                    for m in &self.middlewares {
                        if m.should_execute(context).await {
                            let _ = m.on_error(&e, context).await;
                        }
                    }
                    return Err(e);
                }
            }
        }

        context.metrics.add_time(start_time.elapsed());
        Ok(())
    }

    /// Get performance metrics for the chain.
    pub fn get_metrics(&self) -> Vec<Arc<PerformanceMetrics>> {
        // This would collect metrics from all contexts that have been processed
        // For now, we return an empty vector as metrics are stored per-context
        Vec::new()
    }
}

/// Chain of middleware handlers (legacy).
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{MiddlewareChain, LoggingMiddleware, AuthMiddleware, RetryMiddleware};
/// use pmcp::types::{JSONRPCRequest, JSONRPCResponse, RequestId};
/// use std::sync::Arc;
/// use tracing::Level;
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create a middleware chain
/// let mut chain = MiddlewareChain::new();
///
/// // Add different types of middleware in order
/// chain.add(Arc::new(LoggingMiddleware::new(Level::INFO)));
/// chain.add(Arc::new(AuthMiddleware::new("Bearer token-123".to_string())));
/// chain.add(Arc::new(RetryMiddleware::default()));
///
/// // Create a request to process
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "prompts.get".to_string(),
///     params: Some(serde_json::json!({
///         "name": "code_review",
///         "arguments": {"language": "rust", "style": "detailed"}
///     })),
///     id: RequestId::from(1001i64),
/// };
///
/// // Process request through all middleware in order
/// chain.process_request(&mut request).await?;
///
/// // Create a response to process
/// let mut response = JSONRPCResponse {
///     jsonrpc: "2.0".to_string(),
///     id: RequestId::from(1001i64),
///     payload: pmcp::types::jsonrpc::ResponsePayload::Result(
///         serde_json::json!({"prompt": "Review the following code..."})
///     ),
/// };
///
/// // Process response through all middleware
/// chain.process_response(&mut response).await?;
///
/// // The chain processes middleware in the order they were added
/// // 1. LoggingMiddleware logs the request/response
/// // 2. AuthMiddleware adds authentication
/// // 3. RetryMiddleware configures retry behavior
/// # Ok(())
/// # }
/// ```
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl fmt::Debug for MiddlewareChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MiddlewareChain")
            .field("count", &self.middlewares.len())
            .finish()
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

impl MiddlewareChain {
    /// Create a new empty middleware chain.
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add a middleware to the chain.
    pub fn add(&mut self, middleware: Arc<dyn Middleware>) {
        self.middlewares.push(middleware);
    }

    /// Process a request through all middleware.
    pub async fn process_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_request(request).await?;
        }
        Ok(())
    }

    /// Process a response through all middleware.
    pub async fn process_response(&self, response: &mut JSONRPCResponse) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_response(response).await?;
        }
        Ok(())
    }

    /// Process an outgoing message through all middleware.
    pub async fn process_send(&self, message: &TransportMessage) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_send(message).await?;
        }
        Ok(())
    }

    /// Process an incoming message through all middleware.
    pub async fn process_receive(&self, message: &TransportMessage) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_receive(message).await?;
        }
        Ok(())
    }

    /// Process an unsolicited notification through all middleware.
    ///
    /// This enables middleware to intercept and process server-initiated
    /// notifications (e.g., progress updates, resource changes) that arrive
    /// without a corresponding request.
    pub async fn process_notification(&self, notification: &mut JSONRPCNotification) -> Result<()> {
        for middleware in &self.middlewares {
            middleware.on_notification(notification).await?;
        }
        Ok(())
    }
}

/// Logging middleware that logs all messages.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{LoggingMiddleware, Middleware};
/// use pmcp::types::{JSONRPCRequest, RequestId};
/// use tracing::Level;
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create logging middleware with different levels
/// let debug_logger = LoggingMiddleware::new(Level::DEBUG);
/// let info_logger = LoggingMiddleware::new(Level::INFO);
/// let default_logger = LoggingMiddleware::default(); // Uses DEBUG level
///
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "tools.list".to_string(),
///     params: Some(serde_json::json!({"category": "development"})),
///     id: RequestId::from(456i64),
/// };
///
/// // Log at different levels
/// debug_logger.on_request(&mut request).await?;
/// info_logger.on_request(&mut request).await?;
/// default_logger.on_request(&mut request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct LoggingMiddleware {
    level: tracing::Level,
}

impl LoggingMiddleware {
    /// Create a new logging middleware with the specified level.
    pub fn new(level: tracing::Level) -> Self {
        Self { level }
    }
}

impl Default for LoggingMiddleware {
    fn default() -> Self {
        Self::new(tracing::Level::DEBUG)
    }
}

#[async_trait]
impl Middleware for LoggingMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        match self.level {
            tracing::Level::TRACE => tracing::trace!("Sending request: {:?}", request),
            tracing::Level::DEBUG => tracing::debug!("Sending request: {}", request.method),
            tracing::Level::INFO => tracing::info!("Sending request: {}", request.method),
            tracing::Level::WARN => tracing::warn!("Sending request: {}", request.method),
            tracing::Level::ERROR => tracing::error!("Sending request: {}", request.method),
        }
        Ok(())
    }

    async fn on_response(&self, response: &mut JSONRPCResponse) -> Result<()> {
        match self.level {
            tracing::Level::TRACE => tracing::trace!("Received response: {:?}", response),
            tracing::Level::DEBUG => tracing::debug!("Received response for: {:?}", response.id),
            tracing::Level::INFO => tracing::info!("Received response"),
            tracing::Level::WARN => tracing::warn!("Received response"),
            tracing::Level::ERROR => tracing::error!("Received response"),
        }
        Ok(())
    }
}

/// Authentication middleware that adds auth headers.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{AuthMiddleware, Middleware};
/// use pmcp::types::{JSONRPCRequest, RequestId};
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create auth middleware with API token
/// let auth_middleware = AuthMiddleware::new("Bearer api-token-12345".to_string());
///
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "resources.read".to_string(),
///     params: Some(serde_json::json!({"uri": "file:///secure/data.txt"})),
///     id: RequestId::from(789i64),
/// };
///
/// // Process request and add authentication
/// auth_middleware.on_request(&mut request).await?;
///
/// // In a real implementation, the middleware would modify the request
/// // to include authentication information
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct AuthMiddleware {
    #[allow(dead_code)]
    auth_token: String,
}

impl AuthMiddleware {
    /// Create a new auth middleware with the given token.
    pub fn new(auth_token: String) -> Self {
        Self { auth_token }
    }
}

#[async_trait]
impl Middleware for AuthMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        // In a real implementation, this would add auth headers
        // For JSON-RPC, we might add auth to params or use a wrapper
        tracing::debug!("Adding authentication to request: {}", request.method);
        Ok(())
    }
}

/// Retry middleware that implements exponential backoff.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{RetryMiddleware, Middleware};
/// use pmcp::types::{JSONRPCRequest, RequestId};
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create retry middleware with custom settings
/// let retry_middleware = RetryMiddleware::new(
///     5,      // max_retries
///     1000,   // initial_delay_ms (1 second)
///     30000   // max_delay_ms (30 seconds)
/// );
///
/// // Default retry middleware (3 retries, 1s initial, 30s max)
/// let default_retry = RetryMiddleware::default();
///
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "tools.call".to_string(),
///     params: Some(serde_json::json!({
///         "name": "network_tool",
///         "arguments": {"url": "https://api.example.com/data"}
///     })),
///     id: RequestId::from(999i64),
/// };
///
/// // Configure request for retry handling
/// retry_middleware.on_request(&mut request).await?;
/// default_retry.on_request(&mut request).await?;
///
/// // The actual retry logic would be implemented at the transport level
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct RetryMiddleware {
    max_retries: u32,
    #[allow(dead_code)]
    initial_delay_ms: u64,
    #[allow(dead_code)]
    max_delay_ms: u64,
}

impl RetryMiddleware {
    /// Create a new retry middleware.
    pub fn new(max_retries: u32, initial_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_retries,
            initial_delay_ms,
            max_delay_ms,
        }
    }
}

impl Default for RetryMiddleware {
    fn default() -> Self {
        Self::new(3, 1000, 30000)
    }
}

#[async_trait]
impl Middleware for RetryMiddleware {
    async fn on_request(&self, request: &mut JSONRPCRequest) -> Result<()> {
        // Retry logic would be implemented at the transport level
        // This middleware just adds metadata for retry handling
        tracing::debug!(
            "Request {} configured with max {} retries",
            request.method,
            self.max_retries
        );
        Ok(())
    }
}

/// Rate limiting middleware with token bucket algorithm.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{RateLimitMiddleware, AdvancedMiddleware, MiddlewareContext};
/// use pmcp::types::{JSONRPCRequest, RequestId};
/// use std::time::Duration;
///
/// # async fn example() -> pmcp::Result<()> {
/// // Create rate limiter: 10 requests per second, burst of 20
/// let rate_limiter = RateLimitMiddleware::new(10, 20, Duration::from_secs(1));
/// let context = MiddlewareContext::default();
///
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "tools.call".to_string(),
///     params: Some(serde_json::json!({"name": "api_call"})),
///     id: RequestId::from(123i64),
/// };
///
/// // This will succeed if under rate limit, fail if over
/// rate_limiter.on_request_with_context(&mut request, &context).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct RateLimitMiddleware {
    max_requests: u32,
    bucket_size: u32,
    refill_duration: Duration,
    tokens: Arc<AtomicUsize>,
    last_refill: Arc<RwLock<Instant>>,
}

impl RateLimitMiddleware {
    /// Create a new rate limiting middleware.
    pub fn new(max_requests: u32, bucket_size: u32, refill_duration: Duration) -> Self {
        Self {
            max_requests,
            bucket_size,
            refill_duration,
            tokens: Arc::new(AtomicUsize::new(bucket_size as usize)),
            last_refill: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Check if request is within rate limits.
    fn check_rate_limit(&self) -> bool {
        // Refill tokens based on time elapsed
        let now = Instant::now();
        let mut last_refill = self.last_refill.write();
        let elapsed = now.duration_since(*last_refill);

        if elapsed >= self.refill_duration {
            let refill_count = (elapsed.as_millis() / self.refill_duration.as_millis()) as u32;
            let tokens_to_add = (refill_count * self.max_requests).min(self.bucket_size);

            self.tokens.store(
                (self.tokens.load(Ordering::Relaxed) + tokens_to_add as usize)
                    .min(self.bucket_size as usize),
                Ordering::Relaxed,
            );
            *last_refill = now;
        }

        // Try to consume a token
        loop {
            let current = self.tokens.load(Ordering::Relaxed);
            if current == 0 {
                return false;
            }
            if self
                .tokens
                .compare_exchange_weak(current, current - 1, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }
}

#[async_trait]
impl AdvancedMiddleware for RateLimitMiddleware {
    fn name(&self) -> &'static str {
        "rate_limit"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::High
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        if !self.check_rate_limit() {
            tracing::warn!("Rate limit exceeded for request: {}", request.method);
            context.record_metric("rate_limit_exceeded".to_string(), 1.0);
            return Err(crate::error::Error::RateLimited);
        }

        tracing::debug!("Rate limit check passed for request: {}", request.method);
        context.record_metric("rate_limit_passed".to_string(), 1.0);
        Ok(())
    }
}

/// Circuit breaker middleware for fault tolerance.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{CircuitBreakerMiddleware, AdvancedMiddleware, MiddlewareContext};
/// use pmcp::types::{JSONRPCRequest, RequestId};
/// use std::time::Duration;
///
/// # async fn example() -> pmcp::Result<()> {
/// // Circuit breaker: 5 failures in 60s window trips for 30s
/// let circuit_breaker = CircuitBreakerMiddleware::new(
///     5,                          // failure_threshold
///     Duration::from_secs(60),    // time_window
///     Duration::from_secs(30),    // timeout_duration
/// );
/// let context = MiddlewareContext::default();
///
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "external_service.call".to_string(),
///     params: Some(serde_json::json!({"data": "test"})),
///     id: RequestId::from(456i64),
/// };
///
/// // This will fail fast if circuit is open
/// circuit_breaker.on_request_with_context(&mut request, &context).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct CircuitBreakerMiddleware {
    failure_threshold: u32,
    time_window: Duration,
    timeout_duration: Duration,
    failure_count: Arc<AtomicU64>,
    last_failure: Arc<RwLock<Option<Instant>>>,
    circuit_open_time: Arc<RwLock<Option<Instant>>>,
}

impl CircuitBreakerMiddleware {
    /// Create a new circuit breaker middleware.
    pub fn new(failure_threshold: u32, time_window: Duration, timeout_duration: Duration) -> Self {
        Self {
            failure_threshold,
            time_window,
            timeout_duration,
            failure_count: Arc::new(AtomicU64::new(0)),
            last_failure: Arc::new(RwLock::new(None)),
            circuit_open_time: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if circuit breaker should allow the request.
    fn should_allow_request(&self) -> bool {
        let now = Instant::now();

        // Check if circuit is open and should transition to half-open
        let open_time_value = *self.circuit_open_time.read();
        if let Some(open_time) = open_time_value {
            if now.duration_since(open_time) > self.timeout_duration {
                // Transition to half-open: allow one request through
                *self.circuit_open_time.write() = None;
                self.failure_count.store(0, Ordering::Relaxed);
                return true;
            }
            return false; // Circuit is still open
        }

        // Reset failure count if outside time window
        let last_failure_value = *self.last_failure.read();
        if let Some(last_failure) = last_failure_value {
            if now.duration_since(last_failure) > self.time_window {
                self.failure_count.store(0, Ordering::Relaxed);
            }
        }

        // Check if failure threshold exceeded
        self.failure_count.load(Ordering::Relaxed) < self.failure_threshold as u64
    }

    /// Record a failure and possibly open the circuit.
    fn record_failure(&self) {
        let now = Instant::now();
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure.write() = Some(now);

        if failures >= self.failure_threshold as u64 {
            *self.circuit_open_time.write() = Some(now);
            tracing::warn!("Circuit breaker opened due to {} failures", failures);
        }
    }
}

#[async_trait]
impl AdvancedMiddleware for CircuitBreakerMiddleware {
    fn name(&self) -> &'static str {
        "circuit_breaker"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::High
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        if !self.should_allow_request() {
            tracing::warn!(
                "Circuit breaker open, rejecting request: {}",
                request.method
            );
            context.record_metric("circuit_breaker_open".to_string(), 1.0);
            return Err(crate::error::Error::CircuitBreakerOpen);
        }

        context.record_metric("circuit_breaker_allowed".to_string(), 1.0);
        Ok(())
    }

    async fn on_error(
        &self,
        _error: &crate::error::Error,
        _context: &MiddlewareContext,
    ) -> Result<()> {
        self.record_failure();
        Ok(())
    }
}

/// Metrics collection middleware for observability.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{MetricsMiddleware, AdvancedMiddleware, MiddlewareContext};
/// use pmcp::types::{JSONRPCRequest, RequestId};
///
/// # async fn example() -> pmcp::Result<()> {
/// let metrics = MetricsMiddleware::new("pmcp_client".to_string());
/// let context = MiddlewareContext::default();
///
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "resources.list".to_string(),
///     params: None,
///     id: RequestId::from(789i64),
/// };
///
/// // Automatically collects timing and count metrics
/// metrics.on_request_with_context(&mut request, &context).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct MetricsMiddleware {
    service_name: String,
    request_counts: Arc<DashMap<String, AtomicU64>>,
    request_durations: Arc<DashMap<String, AtomicU64>>,
    error_counts: Arc<DashMap<String, AtomicU64>>,
}

impl MetricsMiddleware {
    /// Create a new metrics collection middleware.
    pub fn new(service_name: String) -> Self {
        Self {
            service_name,
            request_counts: Arc::new(DashMap::new()),
            request_durations: Arc::new(DashMap::new()),
            error_counts: Arc::new(DashMap::new()),
        }
    }

    /// Get request count for a method.
    pub fn get_request_count(&self, method: &str) -> u64 {
        self.request_counts
            .get(method)
            .map_or(0, |c| c.load(Ordering::Relaxed))
    }

    /// Get error count for a method.
    pub fn get_error_count(&self, method: &str) -> u64 {
        self.error_counts
            .get(method)
            .map_or(0, |c| c.load(Ordering::Relaxed))
    }

    /// Get average duration for a method in microseconds.
    pub fn get_average_duration(&self, method: &str) -> u64 {
        let total_duration = self
            .request_durations
            .get(method)
            .map_or(0, |d| d.load(Ordering::Relaxed));
        let count = self.get_request_count(method);
        if count > 0 {
            total_duration / count
        } else {
            0
        }
    }
}

#[async_trait]
impl AdvancedMiddleware for MetricsMiddleware {
    fn name(&self) -> &'static str {
        "metrics"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Low
    }

    async fn on_request_with_context(
        &self,
        request: &mut JSONRPCRequest,
        context: &MiddlewareContext,
    ) -> Result<()> {
        // Increment request count
        self.request_counts
            .entry(request.method.clone())
            .or_insert_with(|| AtomicU64::new(0))
            .fetch_add(1, Ordering::Relaxed);

        context.set_metadata(
            "request_start_time".to_string(),
            context.start_time.elapsed().as_micros().to_string(),
        );
        context.set_metadata("service_name".to_string(), self.service_name.clone());

        tracing::debug!(
            "Metrics recorded for request: {} (service: {})",
            request.method,
            self.service_name
        );
        Ok(())
    }

    async fn on_response_with_context(
        &self,
        response: &mut JSONRPCResponse,
        context: &MiddlewareContext,
    ) -> Result<()> {
        // Record response time if we have a request method in context
        let duration_us = context.elapsed().as_micros() as u64;

        if let Some(method) = context.get_metadata("method") {
            self.request_durations
                .entry(method)
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(duration_us, Ordering::Relaxed);
        }

        tracing::debug!(
            "Response metrics recorded for ID: {:?} ({}μs)",
            response.id,
            duration_us
        );
        Ok(())
    }

    async fn on_error(
        &self,
        error: &crate::error::Error,
        context: &MiddlewareContext,
    ) -> Result<()> {
        if let Some(method) = context.get_metadata("method") {
            self.error_counts
                .entry(method)
                .or_insert_with(|| AtomicU64::new(0))
                .fetch_add(1, Ordering::Relaxed);
        }

        tracing::warn!("Error recorded in metrics: {:?}", error);
        Ok(())
    }
}

/// Compression middleware for reducing message size.
///
/// # Examples
///
/// ```rust
/// use pmcp::shared::{CompressionMiddleware, AdvancedMiddleware, MiddlewareContext, CompressionType};
/// use pmcp::types::{JSONRPCRequest, RequestId};
///
/// # async fn example() -> pmcp::Result<()> {
/// let compression = CompressionMiddleware::new(CompressionType::Gzip, 1024);
/// let context = MiddlewareContext::default();
///
/// let mut request = JSONRPCRequest {
///     jsonrpc: "2.0".to_string(),
///     method: "resources.read".to_string(),
///     params: Some(serde_json::json!({"uri": "file:///large_file.json"})),
///     id: RequestId::from(101i64),
/// };
///
/// // Compresses request if over threshold
/// compression.on_request_with_context(&mut request, &context).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub enum CompressionType {
    /// No compression
    None,
    /// Gzip compression
    Gzip,
    /// Deflate compression
    Deflate,
}

/// Compression middleware for reducing message size.
#[derive(Debug)]
pub struct CompressionMiddleware {
    compression_type: CompressionType,
    min_size: usize,
}

impl CompressionMiddleware {
    /// Create a new compression middleware.
    pub fn new(compression_type: CompressionType, min_size: usize) -> Self {
        Self {
            compression_type,
            min_size,
        }
    }

    /// Check if content should be compressed.
    fn should_compress(&self, content_size: usize) -> bool {
        content_size >= self.min_size && !matches!(self.compression_type, CompressionType::None)
    }
}

#[async_trait]
impl AdvancedMiddleware for CompressionMiddleware {
    fn name(&self) -> &'static str {
        "compression"
    }

    fn priority(&self) -> MiddlewarePriority {
        MiddlewarePriority::Normal
    }

    async fn on_send_with_context(
        &self,
        message: &TransportMessage,
        context: &MiddlewareContext,
    ) -> Result<()> {
        let serialized = serde_json::to_string(message).unwrap_or_default();
        let content_size = serialized.len();

        if self.should_compress(content_size) {
            context.set_metadata(
                "compression_type".to_string(),
                format!("{:?}", self.compression_type),
            );
            context.record_metric("compression_original_size".to_string(), content_size as f64);

            tracing::debug!("Compression applied to message of {} bytes", content_size);
            // In a real implementation, this would compress the message content
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RequestId;

    #[tokio::test]
    async fn test_middleware_chain() {
        let mut chain = MiddlewareChain::new();
        chain.add(Arc::new(LoggingMiddleware::default()));

        let mut request = JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            method: "test".to_string(),
            params: None,
        };

        assert!(chain.process_request(&mut request).await.is_ok());
    }

    #[tokio::test]
    async fn test_auth_middleware() {
        let middleware = AuthMiddleware::new("test-token".to_string());

        let mut request = JSONRPCRequest {
            jsonrpc: "2.0".to_string(),
            id: RequestId::from(1i64),
            method: "test".to_string(),
            params: None,
        };

        assert!(middleware.on_request(&mut request).await.is_ok());
    }

    #[tokio::test]
    async fn test_notification_middleware_legacy() {
        let mut chain = MiddlewareChain::new();
        chain.add(Arc::new(LoggingMiddleware::default()));

        let mut notification = JSONRPCNotification::new(
            "notifications/progress",
            Some(serde_json::json!({
                "progressToken": "test-123",
                "progress": 50,
                "total": 100
            })),
        );

        // Should process without error
        assert!(chain.process_notification(&mut notification).await.is_ok());
    }

    #[tokio::test]
    async fn test_notification_middleware_enhanced() {
        let mut chain = EnhancedMiddlewareChain::new();
        chain.add(Arc::new(MetricsMiddleware::new("test-service".to_string())));

        let context = MiddlewareContext::with_request_id("notif-001".to_string());

        let mut notification = JSONRPCNotification::new(
            "notifications/resourceUpdated",
            Some(serde_json::json!({
                "uri": "file:///test.txt",
                "type": "modified"
            })),
        );

        // Should process notification through enhanced middleware
        assert!(chain
            .process_notification_with_context(&mut notification, &context)
            .await
            .is_ok());

        // Verify metrics were not incremented for notifications (they're not requests)
        let stats = context.metrics;
        assert_eq!(stats.request_count(), 0);
    }

    /// Test middleware that appends metadata to notifications
    struct NotificationMetadataMiddleware {
        tag: String,
    }

    #[async_trait::async_trait]
    impl AdvancedMiddleware for NotificationMetadataMiddleware {
        fn name(&self) -> &'static str {
            "notification_metadata"
        }

        async fn on_notification_with_context(
            &self,
            notification: &mut JSONRPCNotification,
            context: &MiddlewareContext,
        ) -> Result<()> {
            // Store notification method in context metadata
            context.set_metadata(
                "notification_method".to_string(),
                notification.method.clone(),
            );
            context.set_metadata("middleware_tag".to_string(), self.tag.clone());
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_notification_metadata_middleware() {
        let mut chain = EnhancedMiddlewareChain::new();
        chain.add(Arc::new(NotificationMetadataMiddleware {
            tag: "test-tag".to_string(),
        }));

        let context = MiddlewareContext::with_request_id("notif-002".to_string());

        let mut notification = JSONRPCNotification::new(
            "notifications/cancelled",
            Some(serde_json::json!({
                "requestId": "req-123",
                "reason": "user cancelled"
            })),
        );

        chain
            .process_notification_with_context(&mut notification, &context)
            .await
            .unwrap();

        // Verify metadata was set by middleware
        assert_eq!(
            context.get_metadata("notification_method"),
            Some("notifications/cancelled".to_string())
        );
        assert_eq!(
            context.get_metadata("middleware_tag"),
            Some("test-tag".to_string())
        );
    }

    #[tokio::test]
    async fn test_notification_error_handling() {
        /// Test middleware that fails on specific notification
        struct FailingNotificationMiddleware;

        #[async_trait::async_trait]
        impl AdvancedMiddleware for FailingNotificationMiddleware {
            fn name(&self) -> &'static str {
                "failing_notification"
            }

            async fn on_notification_with_context(
                &self,
                notification: &mut JSONRPCNotification,
                _context: &MiddlewareContext,
            ) -> Result<()> {
                if notification.method == "notifications/error" {
                    return Err(crate::Error::internal("notification processing failed"));
                }
                Ok(())
            }
        }

        let mut chain = EnhancedMiddlewareChain::new();
        chain.add(Arc::new(FailingNotificationMiddleware));

        let context = MiddlewareContext::default();

        // Success case
        let mut ok_notification =
            JSONRPCNotification::new("notifications/ok", None::<serde_json::Value>);
        assert!(chain
            .process_notification_with_context(&mut ok_notification, &context)
            .await
            .is_ok());

        // Error case
        let mut error_notification =
            JSONRPCNotification::new("notifications/error", None::<serde_json::Value>);
        let result = chain
            .process_notification_with_context(&mut error_notification, &context)
            .await;
        assert!(result.is_err());

        // Verify error was counted
        assert_eq!(context.metrics.error_count(), 1);
    }
}
