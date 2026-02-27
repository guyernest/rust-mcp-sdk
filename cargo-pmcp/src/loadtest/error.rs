//! Error types for the load testing engine.
//!
//! Defines [`LoadTestError`] for configuration errors and [`McpError`] for
//! MCP protocol and transport errors encountered during load test execution.

/// Errors that occur during load test configuration parsing, validation, or file I/O.
#[derive(Debug, thiserror::Error)]
pub enum LoadTestError {
    /// TOML parse failure -- the config file contains invalid TOML syntax
    /// or does not match the expected schema.
    #[error("Failed to parse config TOML: {source}")]
    ConfigParse {
        #[from]
        source: toml::de::Error,
    },

    /// Semantic validation failure -- the config parsed successfully but
    /// contains invalid values (e.g., empty scenario, zero total weight).
    #[error("Config validation error: {message}")]
    ConfigValidation { message: String },

    /// File I/O failure -- the config file could not be read from disk.
    #[error("Failed to read config file '{path}': {source}")]
    ConfigIo {
        source: std::io::Error,
        path: String,
    },

    /// CLI-level error (config not found, file I/O for reports).
    #[error("{message}")]
    Cli { message: String },
}

/// MCP protocol and transport errors encountered during load test requests.
///
/// Each variant represents a distinct error category that the metrics pipeline
/// can count and report separately.
#[derive(Debug, thiserror::Error, Clone)]
pub enum McpError {
    /// JSON-RPC protocol error returned by the MCP server in the response body.
    #[error("JSON-RPC error {code}: {message}")]
    JsonRpc { code: i32, message: String },

    /// HTTP transport error (4xx or 5xx status code).
    #[error("HTTP {status}: {body}")]
    Http { status: u16, body: String },

    /// The request exceeded the configured per-request timeout.
    #[error("Request timed out")]
    Timeout,

    /// Connection-level failure (DNS resolution, TCP connect, TLS handshake).
    #[error("Connection error: {message}")]
    Connection { message: String },
}

impl McpError {
    /// Returns `true` if this is a JSON-RPC "Method not found" error (code -32601).
    pub fn is_method_not_found(&self) -> bool {
        matches!(self, Self::JsonRpc { code: -32601, .. })
    }

    /// Returns `true` if this is a JSON-RPC "Invalid params" error (code -32602).
    pub fn is_invalid_params(&self) -> bool {
        matches!(self, Self::JsonRpc { code: -32602, .. })
    }

    /// Returns `true` if this is a JSON-RPC "Internal error" (code -32603).
    pub fn is_internal_error(&self) -> bool {
        matches!(self, Self::JsonRpc { code: -32603, .. })
    }

    /// Returns the error category as a static string for metrics classification.
    ///
    /// Categories: `"jsonrpc"`, `"http"`, `"timeout"`, `"connection"`.
    pub fn error_category(&self) -> &'static str {
        match self {
            Self::JsonRpc { .. } => "jsonrpc",
            Self::Http { .. } => "http",
            Self::Timeout => "timeout",
            Self::Connection { .. } => "connection",
        }
    }

    /// Classify a [`reqwest::Error`] into the appropriate [`McpError`] variant.
    pub fn classify_reqwest(err: &reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout
        } else if err.is_connect() {
            Self::Connection {
                message: err.to_string(),
            }
        } else if let Some(status) = err.status() {
            Self::Http {
                status: status.as_u16(),
                body: err.to_string(),
            }
        } else {
            Self::Connection {
                message: err.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_method_not_found() {
        let err = McpError::JsonRpc {
            code: -32601,
            message: "Method not found".to_string(),
        };
        assert!(err.is_method_not_found());

        let other = McpError::JsonRpc {
            code: -32602,
            message: "Invalid params".to_string(),
        };
        assert!(!other.is_method_not_found());
    }

    #[test]
    fn test_is_invalid_params() {
        let err = McpError::JsonRpc {
            code: -32602,
            message: "Invalid params".to_string(),
        };
        assert!(err.is_invalid_params());

        let other = McpError::JsonRpc {
            code: -32601,
            message: "Method not found".to_string(),
        };
        assert!(!other.is_invalid_params());
    }

    #[test]
    fn test_is_internal_error() {
        let err = McpError::JsonRpc {
            code: -32603,
            message: "Internal error".to_string(),
        };
        assert!(err.is_internal_error());

        let other = McpError::JsonRpc {
            code: -32601,
            message: "Method not found".to_string(),
        };
        assert!(!other.is_internal_error());
    }

    #[test]
    fn test_error_category_jsonrpc() {
        let err = McpError::JsonRpc {
            code: -32600,
            message: "Invalid request".to_string(),
        };
        assert_eq!(err.error_category(), "jsonrpc");
    }

    #[test]
    fn test_error_category_http() {
        let err = McpError::Http {
            status: 500,
            body: "Internal Server Error".to_string(),
        };
        assert_eq!(err.error_category(), "http");
    }

    #[test]
    fn test_error_category_timeout() {
        let err = McpError::Timeout;
        assert_eq!(err.error_category(), "timeout");
    }

    #[test]
    fn test_error_category_connection() {
        let err = McpError::Connection {
            message: "DNS resolution failed".to_string(),
        };
        assert_eq!(err.error_category(), "connection");
    }
}
