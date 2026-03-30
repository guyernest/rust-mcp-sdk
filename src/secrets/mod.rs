//! Runtime secret access for MCP servers.
//!
//! Thin wrappers around `std::env::var` with actionable error messages
//! that guide developers to the correct `cargo pmcp secret set` command.
//!
//! # Local Development
//!
//! Create a `.env` file in your project root:
//! ```text
//! ANTHROPIC_API_KEY=sk-ant-...
//! DATABASE_URL=postgresql://localhost/mydb
//! ```
//!
//! Run with `cargo pmcp dev <server>` to auto-load `.env` into the process.
//!
//! # Remote (pmcp.run)
//!
//! ```bash
//! cargo pmcp secret set --server chess ANTHROPIC_API_KEY --target pmcp --prompt
//! ```
//!
//! # Usage
//!
//! ```rust
//! use pmcp::secrets;
//!
//! // Optional secret
//! if let Some(key) = secrets::get("ANALYTICS_KEY") {
//!     // configure analytics
//! }
//!
//! // Required secret (returns actionable error if missing)
//! let api_key = secrets::require("ANTHROPIC_API_KEY")
//!     .expect("secret should be set");
//! ```

use std::fmt;

/// Error returned when a required secret is missing.
#[derive(Debug, Clone)]
pub struct SecretError {
    /// The name of the missing secret.
    pub name: String,
}

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Missing secret: {}. Set with: cargo pmcp secret set <server>/{} --prompt",
            self.name, self.name
        )
    }
}

impl std::error::Error for SecretError {}

/// Get a secret value from the environment.
///
/// Returns `None` if the environment variable is not set.
///
/// # Examples
///
/// ```rust
/// use pmcp::secrets;
///
/// if let Some(key) = secrets::get("OPTIONAL_API_KEY") {
///     // use key
/// }
/// ```
pub fn get(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Require a secret value from the environment.
///
/// Returns an error with an actionable message including the
/// `cargo pmcp secret set` command if the secret is not set.
///
/// # Examples
///
/// ```rust
/// use pmcp::secrets;
///
/// let result = secrets::require("SOME_KEY");
/// // If SOME_KEY is not set, error message will include:
/// // "Missing secret: SOME_KEY. Set with: cargo pmcp secret set <server>/SOME_KEY --prompt"
/// ```
pub fn require(name: &str) -> Result<String, SecretError> {
    std::env::var(name).map_err(|_| SecretError {
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_for_missing() {
        assert!(get("__PMCP_TEST_NONEXISTENT_SECRET__").is_none());
    }

    #[test]
    fn get_returns_value_when_set() {
        let key = "__PMCP_TEST_SECRET_GET__";
        unsafe { std::env::set_var(key, "test_value") };
        assert_eq!(get(key), Some("test_value".to_string()));
        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn require_returns_error_for_missing() {
        let result = require("__PMCP_TEST_NONEXISTENT_REQUIRE__");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.name, "__PMCP_TEST_NONEXISTENT_REQUIRE__");
    }

    #[test]
    fn require_returns_value_when_set() {
        let key = "__PMCP_TEST_SECRET_REQUIRE__";
        unsafe { std::env::set_var(key, "required_value") };
        assert_eq!(require(key).unwrap(), "required_value");
        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn error_message_includes_command() {
        let err = SecretError {
            name: "MY_KEY".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Missing secret: MY_KEY"));
        assert!(msg.contains("cargo pmcp secret set"));
    }

    #[test]
    fn error_message_includes_secret_name() {
        let err = SecretError {
            name: "ANTHROPIC_API_KEY".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("ANTHROPIC_API_KEY"));
    }
}
