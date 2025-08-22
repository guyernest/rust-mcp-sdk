//! Error types for the MCP SDK.
//!
//! This module provides a comprehensive error type that covers all possible
//! failure modes in the MCP protocol.

pub mod recovery;

use std::fmt;
use thiserror::Error;

/// Result type alias for MCP operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for MCP operations.
#[derive(Error, Debug)]
pub enum Error {
    /// JSON-RPC protocol errors
    #[error("Protocol error: {code} - {message}")]
    Protocol {
        /// Error code as defined in JSON-RPC spec
        code: ErrorCode,
        /// Human-readable error message
        message: String,
        /// Optional additional error data
        data: Option<serde_json::Value>,
    },

    /// Transport-level errors
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Validation errors
    #[error("Validation error: {0}")]
    Validation(String),

    /// Authentication errors
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Timeout errors
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Capability errors
    #[error("Capability not supported: {0}")]
    UnsupportedCapability(String),

    /// Internal errors
    #[error("Internal error: {0}")]
    Internal(String),

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Invalid state
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Cancelled operation
    #[error("Operation cancelled")]
    Cancelled,

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimited,

    /// Circuit breaker is open
    #[error("Circuit breaker is open")]
    CircuitBreakerOpen,

    /// Other errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// JSON-RPC error code for custom errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ErrorCode(pub i32);

impl ErrorCode {
    /// Parse error (-32700)
    pub const PARSE_ERROR: Self = Self(-32700);
    /// Invalid request (-32600)
    pub const INVALID_REQUEST: Self = Self(-32600);
    /// Method not found (-32601)
    pub const METHOD_NOT_FOUND: Self = Self(-32601);
    /// Invalid params (-32602)
    pub const INVALID_PARAMS: Self = Self(-32602);
    /// Internal error (-32603)
    pub const INTERNAL_ERROR: Self = Self(-32603);
    /// Request timeout (-32001)
    pub const REQUEST_TIMEOUT: Self = Self(-32001);
    /// Unsupported capability (-32002)
    pub const UNSUPPORTED_CAPABILITY: Self = Self(-32002);
    /// Authentication required (-32003)
    pub const AUTHENTICATION_REQUIRED: Self = Self(-32003);
    /// Permission denied (-32004)
    pub const PERMISSION_DENIED: Self = Self(-32004);
    /// Rate limit exceeded (-32005)
    pub const RATE_LIMITED: Self = Self(-32005);
    /// Circuit breaker open (-32006)
    pub const CIRCUIT_BREAKER_OPEN: Self = Self(-32006);

    /// Create a custom error code.
    pub const fn other(code: i32) -> Self {
        Self(code)
    }

    /// Convert error code to i32 value.
    pub fn as_i32(&self) -> i32 {
        self.0
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Implement Hash for `ErrorCode` to use in `HashMap`
impl std::hash::Hash for ErrorCode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Transport-specific errors.
#[derive(Error, Debug)]
pub enum TransportError {
    /// IO error
    #[error("IO error: {0}")]
    Io(String),

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Invalid message format
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Request error
    #[error("Request error: {0}")]
    Request(String),

    /// Send error
    #[error("Send error: {0}")]
    Send(String),

    /// WebSocket error (when feature enabled)
    #[cfg(feature = "websocket")]
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    /// HTTP error (when feature enabled)
    #[cfg(feature = "http")]
    #[error("HTTP error: {0}")]
    Http(String),
}

impl From<std::io::Error> for TransportError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Transport(TransportError::Io(err.to_string()))
    }
}

impl Error {
    /// Create a new internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Create a new protocol error.
    pub fn protocol(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Protocol {
            code,
            message: message.into(),
            data: None,
        }
    }

    /// Get the error code for this error.
    pub fn error_code(&self) -> Option<ErrorCode> {
        match self {
            Self::Protocol { code, .. } => Some(*code),
            Self::Timeout(_) => Some(ErrorCode::REQUEST_TIMEOUT),
            Self::Authentication(_) => Some(ErrorCode::AUTHENTICATION_REQUIRED),
            Self::RateLimited => Some(ErrorCode::RATE_LIMITED),
            Self::CircuitBreakerOpen => Some(ErrorCode::CIRCUIT_BREAKER_OPEN),
            _ => None,
        }
    }

    /// Create a validation error.
    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    /// Create a parse error.
    pub fn parse(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::PARSE_ERROR,
            message: message.into(),
            data: None,
        }
    }

    /// Create an authentication error.
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    /// Create a timeout error.
    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout(duration_ms)
    }

    /// Create a not found error.
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    /// Create an unsupported capability error.
    pub fn unsupported_capability(capability: impl Into<String>) -> Self {
        Self::UnsupportedCapability(capability.into())
    }

    /// Create from JSON-RPC error.
    pub fn from_jsonrpc_error(error: crate::types::jsonrpc::JSONRPCError) -> Self {
        Self::Protocol {
            code: ErrorCode(error.code),
            message: error.message,
            data: error.data,
        }
    }

    /// Create a protocol error with just a message.
    pub fn protocol_msg(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INTERNAL_ERROR,
            message: message.into(),
            data: None,
        }
    }

    /// Check if this error matches a specific error code.
    pub fn is_error_code(&self, code: ErrorCode) -> bool {
        matches!(self.error_code(), Some(c) if c == code)
    }

    /// Create a capability error.
    pub fn capability(message: impl Into<String>) -> Self {
        Self::UnsupportedCapability(message.into())
    }

    /// Create an invalid state error.
    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState(message.into())
    }

    /// Create a cancelled error.
    pub fn cancelled() -> Self {
        Self::Cancelled
    }

    /// Create an invalid params error.
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::INVALID_PARAMS,
            message: message.into(),
            data: None,
        }
    }

    /// Create a method not found error.
    pub fn method_not_found(method: impl Into<String>) -> Self {
        Self::Protocol {
            code: ErrorCode::METHOD_NOT_FOUND,
            message: format!("Method not found: {}", method.into()),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = Error::internal("test error");
        assert!(matches!(err, Error::Internal(_)));

        let err = Error::protocol(ErrorCode::INVALID_REQUEST, "bad request");
        assert!(matches!(err, Error::Protocol { .. }));
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(ErrorCode::PARSE_ERROR.as_i32(), -32700);
        assert_eq!(ErrorCode::RATE_LIMITED.as_i32(), -32005);
        assert_eq!(ErrorCode::CIRCUIT_BREAKER_OPEN.as_i32(), -32006);
    }
}
