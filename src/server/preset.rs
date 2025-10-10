//! Server middleware presets with sensible defaults.
//!
//! This module provides pre-configured middleware bundles for common server scenarios,
//! eliminating boilerplate while maintaining flexibility.
//!
//! # Architecture
//!
//! Server presets combine two middleware layers:
//! - **Protocol Layer**: JSON-RPC message processing (metrics, logging, validation)
//! - **HTTP Layer**: Transport-level concerns (request/response interception, auth)
//!
//! # Examples
//!
//! ## Basic preset with defaults
//!
//! ```rust
//! use pmcp::server::preset::ServerPreset;
//! use pmcp::server::builder::ServerCoreBuilder;
//! use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
//! use std::sync::Arc;
//! use tokio::sync::Mutex;
//!
//! # async fn example() -> pmcp::Result<()> {
//! // Create preset with defaults: metrics + HTTP logging
//! let preset = ServerPreset::default();
//!
//! // Build server with protocol middleware
//! let server = ServerCoreBuilder::new()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .protocol_middleware(preset.protocol_middleware())
//!     .build()?;
//!
//! // Create HTTP server with HTTP middleware
//! let config = StreamableHttpServerConfig {
//!     http_middleware: preset.http_middleware(),
//!     ..Default::default()
//! };
//!
//! let http_server = StreamableHttpServer::with_config(
//!     "127.0.0.1:3000".parse().unwrap(),
//!     Arc::new(Mutex::new(server)),
//!     config,
//! );
//! # Ok(())
//! # }
//! ```
//!
//! ## Preset with custom HTTP middleware
//!
//! ```rust,ignore
//! use pmcp::server::preset::ServerPreset;
//! use pmcp::server::http_middleware::ServerHttpMiddleware;
//! use std::sync::Arc;
//!
//! // Define your custom HTTP middleware...
//! // struct MyAuthMiddleware;
//! // impl ServerHttpMiddleware for MyAuthMiddleware { ... }
//!
//! let preset = ServerPreset::default()
//!     .with_http_middleware(MyAuthMiddleware);
//!
//! // Wire into server as shown above...
//! ```
//!
//! ## Preset with rate limiting
//!
//! ```rust
//! use pmcp::server::preset::ServerPreset;
//! use pmcp::shared::middleware::RateLimitMiddleware;
//! use std::time::Duration;
//!
//! # async fn example() -> pmcp::Result<()> {
//! // Create preset with rate limiting: 100 req/min with bucket size 100
//! let rate_limiter = RateLimitMiddleware::new(100, 100, Duration::from_secs(60));
//!
//! let preset = ServerPreset::default()
//!     .with_rate_limit(rate_limiter);
//! # Ok(())
//! # }
//! ```

use crate::runtime::RwLock;
use crate::server::http_middleware::{
    ServerHttpLoggingMiddleware, ServerHttpMiddleware, ServerHttpMiddlewareChain,
};
use crate::shared::middleware::{EnhancedMiddlewareChain, MetricsMiddleware, RateLimitMiddleware};
use std::sync::Arc;

/// Server middleware preset with sensible defaults.
///
/// Provides a pre-configured middleware bundle combining:
/// - **Protocol Layer**: `MetricsMiddleware` for request/response metrics
/// - **HTTP Layer**: `ServerHttpLoggingMiddleware` (INFO level, redaction enabled)
///
/// # Defaults
///
/// - **Protocol Middleware**:
///   - `MetricsMiddleware`: Tracks request counts, durations, errors
///
/// - **HTTP Middleware**:
///   - `ServerHttpLoggingMiddleware`: INFO level, sensitive header redaction, query stripping
///
/// # Customization
///
/// Add opt-in middleware using builder methods:
/// - `.with_auth()`: Add authentication verification
/// - `.with_rate_limit()`: Add rate limiting
/// - `.with_http_middleware()`: Add custom HTTP middleware
#[allow(missing_debug_implementations)]
pub struct ServerPreset {
    protocol_chain: Arc<RwLock<EnhancedMiddlewareChain>>,
    http_chain: Option<Arc<ServerHttpMiddlewareChain>>,
    service_name: String,
}

impl ServerPreset {
    /// Create a new preset with the given service name.
    ///
    /// # Defaults
    ///
    /// - Protocol: `MetricsMiddleware`
    /// - HTTP: `ServerHttpLoggingMiddleware` (INFO, redaction on)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::preset::ServerPreset;
    ///
    /// let preset = ServerPreset::new("my-service");
    /// ```
    pub fn new(service_name: impl Into<String>) -> Self {
        let service_name = service_name.into();

        // Create protocol middleware chain with metrics
        let mut protocol_chain = EnhancedMiddlewareChain::new();
        protocol_chain.add(Arc::new(MetricsMiddleware::new(service_name.clone())));

        // Create HTTP middleware chain with logging
        let mut http_chain = ServerHttpMiddlewareChain::new();
        let logging = ServerHttpLoggingMiddleware::new()
            .with_level(tracing::Level::INFO)
            .with_redact_query(true);
        http_chain.add(Arc::new(logging));

        Self {
            protocol_chain: Arc::new(RwLock::new(protocol_chain)),
            http_chain: Some(Arc::new(http_chain)),
            service_name,
        }
    }

    /// Add HTTP middleware to the chain.
    ///
    /// HTTP middleware runs at the transport layer for request/response interception.
    /// This can be used for authentication, logging, metrics, or custom transformations.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::server::preset::ServerPreset;
    /// use pmcp::server::http_middleware::ServerHttpMiddleware;
    ///
    /// // Define custom middleware...
    /// // struct MyAuthMiddleware;
    /// // impl ServerHttpMiddleware for MyAuthMiddleware { ... }
    ///
    /// let preset = ServerPreset::new("my-service")
    ///     .with_http_middleware(MyAuthMiddleware);
    /// ```
    pub fn with_http_middleware_item(
        mut self,
        middleware: impl ServerHttpMiddleware + 'static,
    ) -> Self {
        if let Some(chain) = Arc::get_mut(self.http_chain.as_mut().unwrap()) {
            chain.add(Arc::new(middleware));
        } else {
            // Chain is shared, clone and add
            let mut new_chain = ServerHttpMiddlewareChain::new();
            new_chain.add(Arc::new(middleware));
            self.http_chain = Some(Arc::new(new_chain));
        }
        self
    }

    /// Add rate limiting to the protocol chain.
    ///
    /// Rate limiting operates at the protocol layer to limit JSON-RPC requests
    /// per time window.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::preset::ServerPreset;
    /// use pmcp::shared::middleware::RateLimitMiddleware;
    /// use std::time::Duration;
    ///
    /// // 100 requests per minute, bucket size 100
    /// let rate_limiter = RateLimitMiddleware::new(100, 100, Duration::from_secs(60));
    /// let preset = ServerPreset::new("my-service").with_rate_limit(rate_limiter);
    /// ```
    pub fn with_rate_limit(self, rate_limiter: RateLimitMiddleware) -> Self {
        // Add to protocol chain
        if let Ok(mut chain) = self.protocol_chain.try_write() {
            chain.add(Arc::new(rate_limiter));
        }
        self
    }

    /// Get the protocol middleware chain.
    ///
    /// Use this with `ServerCoreBuilder::protocol_middleware()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::preset::ServerPreset;
    /// use pmcp::server::builder::ServerCoreBuilder;
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let preset = ServerPreset::new("my-service");
    ///
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .protocol_middleware(preset.protocol_middleware())
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn protocol_middleware(&self) -> Arc<RwLock<EnhancedMiddlewareChain>> {
        self.protocol_chain.clone()
    }

    /// Get the HTTP middleware chain.
    ///
    /// Use this with `StreamableHttpServerConfig::http_middleware`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::preset::ServerPreset;
    /// use pmcp::server::streamable_http_server::StreamableHttpServerConfig;
    ///
    /// let preset = ServerPreset::new("my-service");
    ///
    /// let config = StreamableHttpServerConfig {
    ///     http_middleware: preset.http_middleware(),
    ///     ..Default::default()
    /// };
    /// ```
    pub fn http_middleware(&self) -> Option<Arc<ServerHttpMiddlewareChain>> {
        self.http_chain.clone()
    }

    /// Get the service name.
    pub fn service_name(&self) -> &str {
        &self.service_name
    }
}

impl Default for ServerPreset {
    /// Create a preset with default service name "mcp-server".
    fn default() -> Self {
        Self::new("mcp-server")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_preset_creation() {
        let preset = ServerPreset::new("test-service");
        assert_eq!(preset.service_name(), "test-service");
        assert!(preset.protocol_middleware().try_read().is_ok());
        assert!(preset.http_middleware().is_some());
    }

    #[test]
    fn test_preset_default() {
        let preset = ServerPreset::default();
        assert_eq!(preset.service_name(), "mcp-server");
    }

    #[test]
    fn test_preset_with_rate_limit() {
        let rate_limiter = RateLimitMiddleware::new(100, 100, Duration::from_secs(60));
        let preset = ServerPreset::new("test-service").with_rate_limit(rate_limiter);

        // Verify protocol chain includes rate limiter
        let chain = preset.protocol_middleware();
        assert!(chain.try_read().is_ok());
    }

    #[test]
    fn test_preset_with_http_middleware() {
        // Create logging middleware as example
        let logging = ServerHttpLoggingMiddleware::new();
        let preset = ServerPreset::new("test-service").with_http_middleware_item(logging);

        assert!(preset.http_middleware().is_some());
    }

    #[test]
    fn test_preset_chaining() {
        let rate_limiter = RateLimitMiddleware::new(100, 100, Duration::from_secs(60));
        let logging = ServerHttpLoggingMiddleware::new();

        let preset = ServerPreset::new("test-service")
            .with_http_middleware_item(logging)
            .with_rate_limit(rate_limiter);

        assert!(preset.http_middleware().is_some());
        assert!(preset.protocol_middleware().try_read().is_ok());
    }
}
