// Originated from pmcp-run/built-in/shared/mcp-server-common (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace as a public SDK crate for Phase 83.

//! Toolkit error type and crate-level `Result` alias.
//!
//! [`ToolkitError`] is `#[non_exhaustive]`: downstream crates must match with a
//! catch-all arm so the toolkit can add variants without a breaking change.
//! Phase 83 Plan 04 extends this enum with a `Validation` variant wrapping a
//! `ConfigValidationError`; Plan 04 Task 2 lands `ConfigValidationError::InlineSecretRejected`
//! per review R9.

/// Crate-level result alias used by every public API in `pmcp-server-toolkit`.
pub type Result<T> = std::result::Result<T, ToolkitError>;

/// Errors surfaced by the `pmcp-server-toolkit` runtime.
///
/// The enum is `#[non_exhaustive]` — match callers must include a wildcard arm.
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::ToolkitError;
/// use std::error::Error;
///
/// // ToolkitError is a real `std::error::Error`, with a usable `Display` impl.
/// let err: ToolkitError = ToolkitError::MissingField("database.dsn".into());
/// assert_eq!(err.to_string(), "missing required config field: database.dsn");
/// // Implements `std::error::Error`, so it composes with `?` and `Box<dyn Error>`.
/// let boxed: Box<dyn Error + Send + Sync> = Box::new(err);
/// assert!(boxed.source().is_none());
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolkitError {
    /// TOML parse failure while loading a `ServerConfig`.
    #[error("failed to parse config TOML: {0}")]
    Parse(#[from] toml::de::Error),

    /// A required config field was absent during tool synthesis.
    #[error("missing required config field: {0}")]
    MissingField(String),

    /// `[[tools]]` synthesis failed (covers Phase 83 TKIT-07 failure modes).
    #[error("tool synthesis failed: {0}")]
    Synth(String),

    /// Code-mode wiring failed (covers Phase 83 TKIT-09 failure modes).
    #[error("code-mode wiring failed: {0}")]
    CodeMode(String),

    /// Filesystem failure while reading a config or fixture.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Secret resolution failed (env var missing, AWS API error, etc.).
    ///
    /// Carries the secret name and a descriptive cause string; the underlying
    /// raw value is NEVER carried in this variant — only the lookup-key
    /// metadata and the error context. This preserves the `SecretValue`
    /// negative-trait invariants at the error path (review R5 + T-83-02-02).
    #[error("secret '{name}' not resolvable: {cause}")]
    Secret {
        /// The secret name that could not be resolved.
        name: String,
        /// Human-readable cause (provider name + underlying error).
        cause: String,
    },
}
