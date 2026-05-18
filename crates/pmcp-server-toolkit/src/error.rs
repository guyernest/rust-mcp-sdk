// Originated from pmcp-run/built-in/shared/mcp-server-common (https://github.com/guyernest/pmcp-run)
// Promoted to rust-mcp-sdk workspace as a public SDK crate for Phase 83.

//! Toolkit error type and crate-level `Result` alias.
//!
//! [`ToolkitError`] is `#[non_exhaustive]`: downstream crates must match with a
//! catch-all arm so the toolkit can add variants without a breaking change.
//! Phase 83 Plan 04 extends this enum with a `Validation` variant wrapping a
//! [`ConfigValidationError`] (per review R8) which catches missing-required-value
//! bugs the `Default` impls on sub-sections would otherwise silently hide.

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

    /// Semantic validation of a parsed [`crate::config::ServerConfig`] failed.
    ///
    /// Wraps a [`ConfigValidationError`] surfaced by
    /// [`crate::config::ServerConfig::validate`] /
    /// [`crate::config::ServerConfig::from_toml_strict_validated`]. Per Phase 83
    /// review R8 this catches the empty-required-value trap that the
    /// `Default` impls on sub-sections would otherwise hide behind silent
    /// successes (e.g. `server.name = ""` if the `[server]` header is typo'd).
    #[error("config validation failed: {0}")]
    Validation(#[from] ConfigValidationError),
}

/// Semantic-validation errors surfaced by
/// [`crate::config::ServerConfig::validate`].
///
/// Per Phase 83 review R8 — the `Default` impls on `ServerConfig` and its
/// sub-sections deliberately allow `from_toml` to succeed even when required
/// fields are missing (so partial configs can be merged programmatically). The
/// [`crate::config::ServerConfig::validate`] entry-point catches these gaps at
/// parse time and surfaces them as a typed enum variant per rule.
///
/// The enum is `#[non_exhaustive]` — match callers must include a wildcard arm
/// so additional rules can be added without a breaking change.
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::ConfigValidationError;
///
/// // Each variant has a precise `Display` describing the rule violated.
/// let err = ConfigValidationError::EmptyServerName;
/// assert_eq!(err.to_string(), "server.name must be non-empty");
/// let err = ConfigValidationError::EmptyToolName(3);
/// assert_eq!(err.to_string(), "[[tools]] entry at index 3 has empty name");
/// ```
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigValidationError {
    /// `[server] name` is missing or whitespace-only.
    #[error("server.name must be non-empty")]
    EmptyServerName,
    /// `[server] version` is missing or whitespace-only.
    #[error("server.version must be non-empty")]
    EmptyServerVersion,
    /// `[[tools]]` entry at `index` has an empty / whitespace-only `name`.
    #[error("[[tools]] entry at index {0} has empty name")]
    EmptyToolName(usize),
    /// `[[database.tables]]` entry at `index` has an empty / whitespace-only `name`.
    #[error("[[database.tables]] entry at index {0} has empty name")]
    EmptyTableName(usize),
    /// Per Phase 83 Plan 06 review R9: `[code_mode].token_secret` was given as
    /// an inline literal (e.g. `token_secret = "raw-string"`) instead of the
    /// `env:VAR_NAME` reference form, and the dev-only escape hatch
    /// `allow_inline_token_secret_for_dev` was not set. Inline literals in
    /// committed configs leak HMAC signing keys; the toolkit defaults to
    /// rejecting them.
    #[error(
        "[code_mode].token_secret is an inline literal; use 'env:VAR_NAME' \
         or set allow_inline_token_secret_for_dev=true (NEVER in production)"
    )]
    InlineSecretRejected,
}
