//! Error types for secret management operations.

use thiserror::Error;

/// Errors that can occur during secret operations.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum SecretError {
    /// Secret not found
    #[error("Secret '{name}' not found")]
    NotFound { name: String },

    /// Secret already exists (when no-overwrite is set)
    #[error("Secret '{name}' already exists")]
    AlreadyExists { name: String },

    /// Invalid secret name for the provider
    #[error("Invalid secret name '{name}': {reason}")]
    InvalidName { name: String, reason: String },

    /// Provider authentication failed
    #[error("Authentication failed for provider '{provider}': {message}")]
    AuthenticationFailed { provider: String, message: String },

    /// Provider not available
    #[error("Provider '{provider}' is not available: {reason}")]
    ProviderNotAvailable { provider: String, reason: String },

    /// Provider operation failed
    #[error("Provider '{provider}' operation failed: {message}")]
    ProviderError { provider: String, message: String },

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Value too large
    #[error("Secret value exceeds maximum size of {max_size} bytes")]
    ValueTooLarge { max_size: usize },

    /// User cancelled operation
    #[error("Operation cancelled by user")]
    Cancelled,

    /// Generic error
    #[error("{0}")]
    Other(String),
}

impl From<anyhow::Error> for SecretError {
    fn from(err: anyhow::Error) -> Self {
        SecretError::Other(err.to_string())
    }
}

/// Result type for secret operations.
pub type SecretResult<T> = Result<T, SecretError>;
