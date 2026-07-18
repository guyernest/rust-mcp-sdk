//! A redacting secret wrapper for API keys used by the HTTP completion sources.
//!
//! [`SecretString`] holds a secret value (an API key) and guarantees it is
//! never rendered by its [`Debug`](std::fmt::Debug) or
//! [`Display`](std::fmt::Display) implementations — both emit a fixed
//! redaction. The only way to read the underlying value is the explicit
//! [`SecretString::expose`] call, which marks the single call site that hands
//! the key to the transport. This satisfies the T-108-04-01 information-
//! disclosure mitigation (ASVS V7): keys never reach `tracing`, error
//! variants, or accidental `{:?}` formatting.

use std::fmt;

/// The fixed text emitted in place of a secret by [`Debug`] / [`Display`].
const REDACTION: &str = "SecretString(***)";

/// A secret string (e.g. an API key) whose `Debug`/`Display` never leak the
/// value.
///
/// `SecretString` is deliberately **not** `#[derive(Debug)]`: both formatter
/// impls are hand-written to emit [`REDACTION`]. Read the raw value only via
/// [`SecretString::expose`].
///
/// # Examples
///
/// ```
/// use pmcp_agent::sources::SecretString;
///
/// let key = SecretString::new("sk-super-secret");
/// // The value never appears in Debug/Display output:
/// assert_eq!(format!("{key:?}"), "SecretString(***)");
/// assert_eq!(format!("{key}"), "SecretString(***)");
/// assert!(!format!("{key:?}").contains("secret"));
/// // The one explicit accessor still returns the raw value:
/// assert_eq!(key.expose(), "sk-super-secret");
/// ```
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    /// Wrap a secret value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Expose the raw secret value.
    ///
    /// This is the single sanctioned read path — call it only where the value
    /// must cross into the transport (e.g. building an `Authorization` header).
    /// Never log or format the returned `&str`.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether the wrapped secret is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTION)
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTION)
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::SecretString;

    #[test]
    fn debug_redacts_the_value() {
        let s = SecretString::new("k-1234-secret");
        let dbg = format!("{s:?}");
        assert_eq!(dbg, "SecretString(***)");
        assert!(!dbg.contains("k-1234-secret"));
        assert!(!dbg.contains("secret"));
    }

    #[test]
    fn display_redacts_the_value() {
        let s = SecretString::new("another-secret");
        let disp = format!("{s}");
        assert_eq!(disp, "SecretString(***)");
        assert!(!disp.contains("another-secret"));
    }

    #[test]
    fn expose_returns_the_raw_value() {
        let s = SecretString::new("raw-value");
        assert_eq!(s.expose(), "raw-value");
    }

    #[test]
    fn from_str_and_string() {
        let a: SecretString = "x".into();
        let b: SecretString = String::from("y").into();
        assert_eq!(a.expose(), "x");
        assert_eq!(b.expose(), "y");
    }

    #[test]
    fn is_empty_reports_emptiness() {
        assert!(SecretString::new("").is_empty());
        assert!(!SecretString::new("x").is_empty());
    }
}
