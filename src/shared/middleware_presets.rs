//! Pre-configured middleware stacks for common use cases.
//!
//! This module provides ready-to-use middleware configurations for typical scenarios,
//! allowing users to quickly set up production-quality middleware without needing to
//! understand all the implementation details.
//!
//! # Available Presets
//!
//! - **stdio**: For stdio transports (logging, validation, NO compression)
//! - **http**: For HTTP transports (OAuth, logging, retry, compression)
//! - **websocket**: For WebSocket transports (reconnection, heartbeat)
//!
//! # Examples
//!
//! ```rust
//! use pmcp::shared::middleware_presets::PresetConfig;
//! use pmcp::{ClientBuilder, StdioTransport};
//!
//! # async fn example() -> Result<(), pmcp::Error> {
//! // Use stdio preset for stdio transport
//! let transport = StdioTransport::new();
//! let client = ClientBuilder::new(transport)
//!     .middleware_chain(PresetConfig::stdio().build_protocol_chain())
//!     .build();
//! # Ok(())
//! # }
//! ```

use crate::shared::{EnhancedMiddlewareChain, MetricsMiddleware};
use std::sync::Arc;

/// Configuration for middleware presets.
///
/// Provides factory methods for creating common middleware configurations.
#[derive(Debug, Clone)]
pub struct PresetConfig {
    service_name: String,
}

impl PresetConfig {
    /// Create a new preset configuration with the given service name.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }

    /// Create a stdio transport preset (metrics, NO compression).
    ///
    /// **Critical**: This preset explicitly excludes compression middleware because
    /// compression would break stdio's line-delimited JSON framing.
    ///
    /// Includes:
    /// - `MetricsMiddleware`: Performance and error metrics
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::middleware_presets::PresetConfig;
    /// use pmcp::{ClientBuilder, StdioTransport};
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let transport = StdioTransport::new();
    /// let client = ClientBuilder::new(transport)
    ///     .middleware_chain(PresetConfig::stdio().build_protocol_chain())
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn stdio() -> Self {
        Self::new("pmcp-stdio-client")
    }

    /// Create an HTTP transport preset (metrics).
    ///
    /// Includes:
    /// - `MetricsMiddleware`: Performance and error metrics
    ///
    /// Note: OAuth and compression middleware should be added via
    /// `StreamableHttpTransportConfigBuilder::with_http_middleware()`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::middleware_presets::PresetConfig;
    /// use pmcp::shared::streamable_http::StreamableHttpTransportConfigBuilder;
    /// use pmcp::{ClientBuilder, StreamableHttpTransport};
    /// use url::Url;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// // Protocol middleware preset
    /// let protocol_chain = PresetConfig::http().build_protocol_chain();
    ///
    /// // HTTP middleware (OAuth, etc.) configured separately
    /// let config = StreamableHttpTransportConfigBuilder::new(
    ///         Url::parse("http://localhost:8080").unwrap()
    ///     )
    ///     // .with_http_middleware(...) // Add HTTP middleware here
    ///     .build();
    ///
    /// let transport = StreamableHttpTransport::new(config).await?;
    /// let client = ClientBuilder::new(transport)
    ///     .middleware_chain(protocol_chain)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn http() -> Self {
        Self::new("pmcp-http-client")
    }

    /// Create a WebSocket transport preset (metrics).
    ///
    /// Includes:
    /// - `MetricsMiddleware`: Performance and error metrics
    ///
    /// Note: WebSocket-specific features like reconnection and heartbeat are
    /// handled by the WebSocket transport itself, not middleware.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::shared::middleware_presets::PresetConfig;
    /// # use pmcp::ClientBuilder;
    ///
    /// # async fn example() -> Result<(), pmcp::Error> {
    /// let protocol_chain = PresetConfig::websocket().build_protocol_chain();
    /// # Ok(())
    /// # }
    /// ```
    pub fn websocket() -> Self {
        Self::new("pmcp-websocket-client")
    }

    /// Build a protocol-level middleware chain from this preset.
    ///
    /// Creates an `EnhancedMiddlewareChain` with the middleware configured
    /// for this preset's transport type.
    ///
    /// Currently includes:
    /// - `MetricsMiddleware`: Performance and error metrics tracking
    ///
    /// Users can add additional middleware (logging, retry, etc.) via
    /// `.with_protocol_middleware()` after using the preset.
    pub fn build_protocol_chain(&self) -> EnhancedMiddlewareChain {
        let mut chain = EnhancedMiddlewareChain::new();

        // Add metrics middleware for performance tracking
        chain.add(Arc::new(MetricsMiddleware::new(
            self.service_name.clone(),
        )));

        chain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stdio_preset() {
        let preset = PresetConfig::stdio();
        assert_eq!(preset.service_name, "pmcp-stdio-client");

        let _chain = preset.build_protocol_chain();
        // Chain should have metrics middleware
        // (Exact count checking would require exposing middleware list, which we don't do)
    }

    #[test]
    fn test_http_preset() {
        let preset = PresetConfig::http();
        assert_eq!(preset.service_name, "pmcp-http-client");

        let _chain = preset.build_protocol_chain();
        // Chain should have metrics middleware
    }

    #[test]
    fn test_websocket_preset() {
        let preset = PresetConfig::websocket();
        assert_eq!(preset.service_name, "pmcp-websocket-client");

        let _chain = preset.build_protocol_chain();
        // Chain should have metrics middleware
    }

    #[test]
    fn test_custom_service_name() {
        let preset = PresetConfig::new("my-custom-service");
        assert_eq!(preset.service_name, "my-custom-service");

        let _chain = preset.build_protocol_chain();
        // Chain should have metrics middleware with custom service name
    }
}
