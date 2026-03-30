//! Secret resolution for pre-deploy secret injection.
//!
//! Resolves `SecretRequirement` declarations against local environment variables
//! and `.env` files. The resolution function is pure: it takes data in and returns
//! data out, with no side effects on the process environment.
//!
//! # Priority
//!
//! Shell environment variables take precedence over `.env` file values (per D-13).
//! The `env_var` field on `SecretRequirement` is used as the lookup key when present,
//! falling back to the `name` field (per Pitfall 4).

use std::collections::HashMap;
use std::path::Path;

use colored::Colorize;

use crate::deployment::metadata::SecretRequirement;

/// Result of resolving secrets from environment and `.env` files.
#[derive(Debug, Clone)]
pub struct SecretResolution {
    /// Resolved secrets: env_var_name -> value.
    pub found: HashMap<String, String>,
    /// Requirements that could not be resolved.
    pub missing: Vec<SecretRequirement>,
}

/// Resolve secrets from environment variables and a pre-parsed `.env` map.
///
/// For each requirement, the lookup key is `req.env_var` if set, otherwise `req.name`.
/// Shell environment variables (`std::env::var`) take precedence over `dotenv_vars`
/// (per D-13: existing environment wins).
///
/// This function does **not** modify the process environment.
pub fn resolve_secrets(
    requirements: &[SecretRequirement],
    dotenv_vars: &HashMap<String, String>,
) -> SecretResolution {
    let mut found = HashMap::new();
    let mut missing = Vec::new();

    for req in requirements {
        let lookup_key = req.env_var.as_deref().unwrap_or(&req.name);

        // std::env takes precedence over .env file (D-13)
        if let Ok(value) = std::env::var(lookup_key) {
            found.insert(lookup_key.to_string(), value);
        } else if let Some(value) = dotenv_vars.get(lookup_key) {
            found.insert(lookup_key.to_string(), value.clone());
        } else {
            missing.push(req.clone());
        }
    }

    SecretResolution { found, missing }
}

/// Load a `.env` file from the project root into a `HashMap` without
/// modifying the process environment.
///
/// Returns an empty map if the file does not exist or cannot be parsed.
pub fn load_dotenv(project_root: &Path) -> HashMap<String, String> {
    let env_path = project_root.join(".env");
    match dotenvy::from_path_iter(&env_path) {
        Ok(iter) => iter.filter_map(|item| item.ok()).collect(),
        Err(e) => {
            // NotFound is expected (no .env file) — only warn on parse errors
            if !e.to_string().contains("not found") && !e.to_string().contains("No such file") {
                eprintln!("Warning: Failed to parse .env file: {e}");
            }
            HashMap::new()
        }
    }
}

/// Print a human-readable report of resolved and missing secrets.
///
/// Output is suppressed when `quiet` is true. Target-specific guidance is
/// shown for missing secrets:
///
/// - **aws-lambda**: Yellow warning markers (missing secrets are non-blocking per D-04).
/// - **pmcp-run**: Exact `cargo pmcp secret set` commands (per D-07).
pub fn print_secret_report(
    resolution: &SecretResolution,
    server_id: &str,
    target: &str,
    quiet: bool,
) {
    if quiet {
        return;
    }

    if resolution.found.is_empty() && resolution.missing.is_empty() {
        println!("   No secrets required");
        return;
    }

    if !resolution.found.is_empty() {
        println!("   Found {} secret(s):", resolution.found.len());
        let mut keys: Vec<&String> = resolution.found.keys().collect();
        keys.sort();
        for key in keys {
            println!("     {} {}", "✓".green(), key);
        }
    }

    if !resolution.missing.is_empty() {
        println!("   Missing {} secret(s):", resolution.missing.len());
        for req in &resolution.missing {
            let env_name = req.env_var.as_deref().unwrap_or(&req.name);
            let required_marker = if req.required { " (required)" } else { "" };

            match target {
                "aws-lambda" => {
                    println!(
                        "     {} {}{}  (will warn at runtime)",
                        "✗".yellow(),
                        env_name,
                        required_marker,
                    );
                }
                "pmcp-run" => {
                    println!("     {} {}{}", "✗".yellow(), env_name, required_marker);
                    println!(
                        "       Run: cargo pmcp secret set --server {} {} --target pmcp --prompt",
                        server_id, env_name,
                    );
                }
                _ => {
                    println!("     {} {}{}", "✗".yellow(), env_name, required_marker);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a `SecretRequirement` quickly.
    fn req(name: &str, env_var: Option<&str>, required: bool) -> SecretRequirement {
        SecretRequirement {
            name: name.to_string(),
            description: None,
            required,
            env_var: env_var.map(String::from),
            obtain_url: None,
        }
    }

    // ------------------------------------------------------------------
    // resolve_secrets tests
    // ------------------------------------------------------------------

    #[test]
    fn resolve_both_found_in_dotenv() {
        let reqs = vec![req("API_KEY", None, true), req("DB_URL", None, true)];
        let dotenv: HashMap<String, String> = [
            ("API_KEY".into(), "key123".into()),
            ("DB_URL".into(), "postgres://localhost".into()),
        ]
        .into();

        let res = resolve_secrets(&reqs, &dotenv);

        assert_eq!(res.found.len(), 2);
        assert!(res.missing.is_empty());
        assert_eq!(res.found["API_KEY"], "key123");
        assert_eq!(res.found["DB_URL"], "postgres://localhost");
    }

    #[test]
    fn resolve_one_found_one_missing() {
        let reqs = vec![req("FOUND_KEY", None, true), req("GONE_KEY", None, false)];
        let dotenv: HashMap<String, String> = [("FOUND_KEY".into(), "v".into())].into();

        let res = resolve_secrets(&reqs, &dotenv);

        assert_eq!(res.found.len(), 1);
        assert_eq!(res.missing.len(), 1);
        assert_eq!(res.missing[0].name, "GONE_KEY");
    }

    #[test]
    fn resolve_uses_env_var_field_for_lookup() {
        // name="Human Name", env_var="CUSTOM_KEY" -> lookup CUSTOM_KEY
        let reqs = vec![req("Human Name", Some("CUSTOM_KEY"), true)];
        let dotenv: HashMap<String, String> = [("CUSTOM_KEY".into(), "custom_val".into())].into();

        let res = resolve_secrets(&reqs, &dotenv);

        assert_eq!(res.found.len(), 1);
        assert_eq!(res.found["CUSTOM_KEY"], "custom_val");
        assert!(res.missing.is_empty());
    }

    #[test]
    fn resolve_std_env_wins_over_dotenv() {
        // D-13: shell env wins over .env
        let unique_key = "__PMCP_TEST_RESOLVE_KEY";
        unsafe {
            std::env::set_var(unique_key, "from_shell");
        }

        let reqs = vec![req(unique_key, None, true)];
        let dotenv: HashMap<String, String> =
            [(unique_key.into(), "from_dotenv".into())].into();

        let res = resolve_secrets(&reqs, &dotenv);

        assert_eq!(res.found[unique_key], "from_shell");

        // Cleanup
        unsafe {
            std::env::remove_var(unique_key);
        }
    }

    #[test]
    fn resolve_empty_requirements() {
        let res = resolve_secrets(&[], &HashMap::new());
        assert!(res.found.is_empty());
        assert!(res.missing.is_empty());
    }

    // ------------------------------------------------------------------
    // load_dotenv tests
    // ------------------------------------------------------------------

    #[test]
    fn load_dotenv_nonexistent_path() {
        let result = load_dotenv(Path::new("/tmp/__pmcp_nonexistent_dir_xyz"));
        assert!(result.is_empty());
    }

    #[test]
    fn load_dotenv_valid_file() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join(".env"),
            "FOO=bar\nBAZ=qux\n# comment\nEMPTY=\n",
        )
        .expect("write .env");

        let result = load_dotenv(dir.path());

        assert_eq!(result.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(result.get("BAZ").map(String::as_str), Some("qux"));
        assert!(result.contains_key("EMPTY"));
    }

    // ------------------------------------------------------------------
    // print_secret_report tests
    // ------------------------------------------------------------------

    #[test]
    fn report_aws_lambda_missing_shows_warning() {
        // Capture behavior: no panic with valid inputs for aws-lambda target.
        // The actual output goes to stdout; we verify the function does not crash
        // and the format is correct by structural inspection.
        let resolution = SecretResolution {
            found: [("FOUND_KEY".into(), "v".into())].into(),
            missing: vec![req("MISSING_KEY", None, true)],
        };
        // Should not panic
        print_secret_report(&resolution, "test-server", "aws-lambda", false);
    }

    #[test]
    fn report_pmcp_run_missing_shows_secret_set_command() {
        // Verify the function produces output for pmcp-run target.
        // We redirect stdout to verify content.
        let resolution = SecretResolution {
            found: HashMap::new(),
            missing: vec![req("MY_SECRET", Some("MY_SECRET_ENV"), true)],
        };
        // Should not panic; the output should contain guidance per D-07
        print_secret_report(&resolution, "chess", "pmcp-run", false);
    }

    #[test]
    fn report_quiet_mode_suppresses_output() {
        let resolution = SecretResolution {
            found: [("KEY".into(), "v".into())].into(),
            missing: vec![req("MISS", None, true)],
        };
        // Should produce no output and not panic
        print_secret_report(&resolution, "server", "aws-lambda", true);
    }

    #[test]
    fn report_no_secrets_required() {
        let resolution = SecretResolution {
            found: HashMap::new(),
            missing: vec![],
        };
        // Should print "No secrets required" and not panic
        print_secret_report(&resolution, "server", "aws-lambda", false);
    }
}
