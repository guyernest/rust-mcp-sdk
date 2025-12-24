//! Error types for MCP composition.

use std::fmt;

/// Errors that can occur during composition operations.
#[derive(Debug)]
pub enum CompositionError {
    /// Foundation server not found in configuration.
    ServerNotFound(String),

    /// Failed to initialize connection to foundation server.
    ConnectionFailed(String),

    /// Tool call failed on the foundation server.
    ToolCallFailed {
        /// The server ID where the error occurred.
        server_id: String,
        /// The tool name that failed.
        tool_name: String,
        /// Error message from the server.
        message: String,
    },

    /// Resource read failed on the foundation server.
    ResourceReadFailed {
        /// The server ID where the error occurred.
        server_id: String,
        /// The resource URI that failed.
        uri: String,
        /// Error message from the server.
        message: String,
    },

    /// Prompt retrieval failed on the foundation server.
    PromptFailed {
        /// The server ID where the error occurred.
        server_id: String,
        /// The prompt name that failed.
        prompt_name: String,
        /// Error message from the server.
        message: String,
    },

    /// Failed to deserialize response from foundation server.
    Deserialization(String),

    /// Failed to serialize request to foundation server.
    Serialization(String),

    /// Invalid response from foundation server.
    InvalidResponse(String),

    /// Configuration error.
    Configuration(String),

    /// Transport-level error.
    Transport(String),

    /// Server returned an error response.
    ServerError {
        /// Error code from the server.
        code: i32,
        /// Error message from the server.
        message: String,
    },

    /// Operation timed out.
    Timeout(String),

    /// Server is not available.
    Unavailable(String),
}

impl fmt::Display for CompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServerNotFound(id) => {
                write!(f, "Foundation server not found: {}", id)
            },
            Self::ConnectionFailed(msg) => {
                write!(f, "Failed to connect to foundation server: {}", msg)
            },
            Self::ToolCallFailed {
                server_id,
                tool_name,
                message,
            } => {
                write!(
                    f,
                    "Tool call failed on {}/{}: {}",
                    server_id, tool_name, message
                )
            },
            Self::ResourceReadFailed {
                server_id,
                uri,
                message,
            } => {
                write!(
                    f,
                    "Resource read failed on {}/{}: {}",
                    server_id, uri, message
                )
            },
            Self::PromptFailed {
                server_id,
                prompt_name,
                message,
            } => {
                write!(
                    f,
                    "Prompt failed on {}/{}: {}",
                    server_id, prompt_name, message
                )
            },
            Self::Deserialization(msg) => {
                write!(f, "Deserialization error: {}", msg)
            },
            Self::Serialization(msg) => {
                write!(f, "Serialization error: {}", msg)
            },
            Self::InvalidResponse(msg) => {
                write!(f, "Invalid response: {}", msg)
            },
            Self::Configuration(msg) => {
                write!(f, "Configuration error: {}", msg)
            },
            Self::Transport(msg) => {
                write!(f, "Transport error: {}", msg)
            },
            Self::ServerError { code, message } => {
                write!(f, "Server error ({}): {}", code, message)
            },
            Self::Timeout(msg) => {
                write!(f, "Operation timed out: {}", msg)
            },
            Self::Unavailable(msg) => {
                write!(f, "Server unavailable: {}", msg)
            },
        }
    }
}

impl std::error::Error for CompositionError {}

impl From<serde_json::Error> for CompositionError {
    fn from(err: serde_json::Error) -> Self {
        Self::Deserialization(err.to_string())
    }
}

impl From<std::io::Error> for CompositionError {
    fn from(err: std::io::Error) -> Self {
        Self::Configuration(err.to_string())
    }
}

impl From<toml::de::Error> for CompositionError {
    fn from(err: toml::de::Error) -> Self {
        Self::Configuration(format!("TOML parse error: {}", err))
    }
}

impl From<crate::Error> for CompositionError {
    fn from(err: crate::Error) -> Self {
        Self::Transport(err.to_string())
    }
}
