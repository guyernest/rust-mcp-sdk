//! Server-specific transport implementations.

#[cfg(feature = "websocket")]
pub mod websocket;

#[cfg(feature = "websocket")]
pub mod websocket_enhanced;

#[cfg(feature = "websocket")]
pub use websocket::{WebSocketServerBuilder, WebSocketServerConfig, WebSocketServerTransport};

#[cfg(feature = "websocket")]
pub use websocket_enhanced::{ClientId, EnhancedWebSocketConfig, EnhancedWebSocketServer};
