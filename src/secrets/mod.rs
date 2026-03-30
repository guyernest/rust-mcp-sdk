//! Runtime secret access for MCP servers.
//!
//! Secrets are injected as environment variables during deployment. This module
//! provides thin wrappers around [`std::env::var`] with developer-friendly error
//! messages that guide server authors toward the correct CLI commands.
//!
//! # Setting Secrets
//!
//! **Local development** (stored in OS keychain):
//!
//! ```bash
//! cargo pmcp secret set my-server/ANTHROPIC_API_KEY --prompt
//! ```
//!
//! **pmcp.run deployment** (stored in the platform secret store):
//!
//! ```bash
//! cargo pmcp secret set my-server/ANTHROPIC_API_KEY --prompt --remote
//! ```
//!
//! # Usage
//!
//! ```rust,ignore
//! use pmcp::secrets;
//!
//! // Optional secret -- returns None if not set
//! if let Some(key) = secrets::get("ANALYTICS_KEY") {
//!     configure_analytics(&key);
//! }
//!
//! // Required secret -- returns actionable error if missing
//! let api_key = secrets::require("ANTHROPIC_API_KEY")?;
//! ```
//!
//! # Design Notes
//!
//! This module is intentionally minimal: no compile-time validation, no global
//! state, no caching. Each call reads directly from the process environment.
//! This keeps the API simple and predictable for server authors.

/// Errors returned by secret access functions.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    /// A required secret was not found in the environment.
    ///
    /// The error message includes the secret name and an actionable CLI command
    /// to set it.
    #[error("Missing required secret '{name}'. Set with: cargo pmcp secret set <server>/{name} --prompt")]
    Missing {
        /// The name of the missing secret (environment variable name).
        name: String,
    },
}

/// Get an optional secret from environment variables.
///
/// Returns `None` if the environment variable is not set.
/// Use [`require`] for secrets that must be present.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::secrets;
///
/// if let Some(key) = secrets::get("ANALYTICS_KEY") {
///     configure_analytics(&key);
/// }
/// ```
pub fn get(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Get a required secret from environment variables.
///
/// Returns an actionable error message if the secret is not set,
/// including the exact CLI command to set it.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp::secrets;
///
/// let api_key = secrets::require("ANTHROPIC_API_KEY")?;
/// ```
///
/// # Errors
///
/// Returns [`SecretError::Missing`] if the environment variable is not set.
pub fn require(name: &str) -> Result<String, SecretError> {
    std::env::var(name).map_err(|_| SecretError::Missing {
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_some_when_env_var_is_set() {
        let key = "__PMCP_TEST_SECRET_GET_EXISTS";
        std::env::set_var(key, "test_value_42");
        let result = get(key);
        std::env::remove_var(key);
        assert_eq!(result, Some("test_value_42".to_string()));
    }

    #[test]
    fn get_returns_none_when_env_var_is_not_set() {
        let key = "__PMCP_TEST_SECRET_GET_MISSING";
        std::env::remove_var(key);
        assert_eq!(get(key), None);
    }

    #[test]
    fn require_returns_ok_when_env_var_is_set() {
        let key = "__PMCP_TEST_SECRET_REQUIRE_EXISTS";
        std::env::set_var(key, "required_value_99");
        let result = require(key);
        std::env::remove_var(key);
        assert_eq!(result.unwrap(), "required_value_99");
    }

    #[test]
    fn require_returns_err_when_env_var_is_not_set() {
        let key = "__PMCP_TEST_SECRET_REQUIRE_MISSING";
        std::env::remove_var(key);
        let result = require(key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecretError::Missing { .. }));
    }

    #[test]
    fn secret_error_display_contains_secret_name() {
        let err = SecretError::Missing {
            name: "MY_API_KEY".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("MY_API_KEY"),
            "Error message should contain the secret name, got: {msg}"
        );
    }

    #[test]
    fn secret_error_display_contains_cli_command() {
        let err = SecretError::Missing {
            name: "MY_API_KEY".to_string(),
        };
        let msg = err.to_string();
        assert!(
            msg.contains("cargo pmcp secret set"),
            "Error message should contain CLI command, got: {msg}"
        );
    }
}
