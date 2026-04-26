//! Shared target-name validation (T-77-03 path-traversal mitigation).
//!
//! Used by `configure add` and `configure use` — both must enforce the same rule
//! to keep `~/.pmcp/config.toml` keys and `.pmcp/active-target` content tractable.

use anyhow::{bail, Result};

/// Validates that `name` matches `[A-Za-z0-9_-]+` and does not start with `-`.
///
/// # Errors
///
/// - empty string
/// - leading `-` (would be confused with a clap flag)
/// - any character outside `[A-Za-z0-9_-]` (rejects `..`, `/`, `\`, spaces, unicode, etc.)
///
/// # Examples
///
/// ```ignore
/// // `commands::configure::*` is bin-only (not exposed via lib.rs per HIGH-1).
/// // Marked `ignore` because doctests compile against the lib target.
/// use cargo_pmcp::commands::configure::name_validation::validate_target_name;
/// assert!(validate_target_name("dev").is_ok());
/// assert!(validate_target_name("prod-east-1").is_ok());
/// assert!(validate_target_name("staging_v2").is_ok());
/// assert!(validate_target_name("../etc").is_err());
/// assert!(validate_target_name("-foo").is_err());
/// assert!(validate_target_name("").is_err());
/// ```
pub fn validate_target_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("target name must not be empty");
    }
    if name.starts_with('-') {
        bail!("target name must not start with '-'");
    }
    for ch in name.chars() {
        if !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-' {
            bail!(
                "target name '{}' contains invalid character '{}' — must match [A-Za-z0-9_-]+",
                name,
                ch
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty() {
        assert!(validate_target_name("").is_err());
    }

    #[test]
    fn rejects_leading_dash() {
        assert!(validate_target_name("-foo").is_err());
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_target_name("../etc").is_err());
    }

    #[test]
    fn rejects_slash() {
        assert!(validate_target_name("foo/bar").is_err());
    }

    #[test]
    fn rejects_backslash() {
        assert!(validate_target_name("foo\\bar").is_err());
    }

    #[test]
    fn rejects_space() {
        assert!(validate_target_name("foo bar").is_err());
    }

    #[test]
    fn rejects_unicode() {
        assert!(validate_target_name("dev—prod").is_err());
    }

    #[test]
    fn accepts_alphanumeric() {
        assert!(validate_target_name("dev").is_ok());
    }

    #[test]
    fn accepts_with_dash() {
        assert!(validate_target_name("prod-east-1").is_ok());
    }

    #[test]
    fn accepts_with_underscore() {
        assert!(validate_target_name("staging_v2").is_ok());
    }
}
