//! Secret resolution from environment variables and `.env` files.
//!
//! Provides pure functions for resolving secrets declared in server configuration
//! against the developer's local environment (shell env vars + `.env` file).
//! Shell environment variables take precedence over `.env` values (D-13).

use std::collections::HashMap;
use std::path::Path;

use colored::Colorize;

use crate::deployment::metadata::SecretRequirement;

/// Result of resolving secrets from environment and `.env` files.
#[derive(Debug, Clone)]
pub struct SecretResolution {
    /// Resolved secrets: env_var_name -> value
    pub found: HashMap<String, String>,
    /// Requirements that could not be resolved
    pub missing: Vec<SecretRequirement>,
}

/// Resolve secret requirements against shell env vars and dotenv values.
///
/// For each requirement, the lookup key is `req.env_var` (if set) or `req.name`.
/// Shell env vars take precedence over dotenv values per D-13.
pub fn resolve_secrets(
    requirements: &[SecretRequirement],
    dotenv_vars: &HashMap<String, String>,
) -> SecretResolution {
    let mut found = HashMap::new();
    let mut missing = Vec::new();

    for req in requirements {
        let lookup_key = req.env_var.as_deref().unwrap_or(&req.name);

        // Shell env wins over .env (D-13)
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

/// Load `.env` file from the project root without mutating process environment.
///
/// Returns an empty `HashMap` if the file does not exist or cannot be parsed.
pub fn load_dotenv(project_root: &Path) -> HashMap<String, String> {
    let env_path = project_root.join(".env");
    if !env_path.exists() {
        return HashMap::new();
    }

    match dotenvy::from_path_iter(&env_path) {
        Ok(iter) => iter.filter_map(|item| item.ok()).collect(),
        Err(e) => {
            eprintln!("Warning: failed to parse .env file: {e}");
            HashMap::new()
        }
    }
}

/// Print a human-readable report of secret resolution results.
///
/// For `aws-lambda`: missing secrets produce a warning (they will be absent at runtime).
/// For `pmcp-run`: missing secrets show `cargo pmcp secret set` commands per D-07.
pub fn print_secret_report(
    resolution: &SecretResolution,
    server_id: &str,
    target: &str,
    global_flags: &crate::commands::GlobalFlags,
) {
    if !global_flags.should_output() {
        return;
    }

    if resolution.found.is_empty() && resolution.missing.is_empty() {
        println!("  No secrets required");
        return;
    }

    if !resolution.found.is_empty() {
        println!(
            "  {} Found {} secret(s):",
            "✓".green(),
            resolution.found.len()
        );
        for key in resolution.found.keys() {
            println!("    {} {}", "✓".green(), key);
        }
    }

    if !resolution.missing.is_empty() {
        match target {
            "aws-lambda" => {
                println!(
                    "  {} Missing {} secret(s) (will warn at runtime):",
                    "✗".yellow(),
                    resolution.missing.len()
                );
                for req in &resolution.missing {
                    let env_name = req.env_var.as_deref().unwrap_or(&req.name);
                    let suffix = if req.required { " (required)" } else { "" };
                    println!("    {} {}{}", "✗".yellow(), env_name, suffix);
                }
            }
            "pmcp-run" | "pmcp" => {
                println!(
                    "  {} Missing {} secret(s) for pmcp.run:",
                    "✗".yellow(),
                    resolution.missing.len()
                );
                for req in &resolution.missing {
                    let env_name = req.env_var.as_deref().unwrap_or(&req.name);
                    let suffix = if req.required { " (required)" } else { "" };
                    println!("    {} {}{}", "✗".yellow(), env_name, suffix);
                    println!(
                        "     Run: cargo pmcp secret set --server {} {} --target pmcp --prompt",
                        server_id, env_name
                    );
                }
            }
            _ => {
                println!(
                    "  {} Missing {} secret(s):",
                    "✗".yellow(),
                    resolution.missing.len()
                );
                for req in &resolution.missing {
                    let env_name = req.env_var.as_deref().unwrap_or(&req.name);
                    let suffix = if req.required { " (required)" } else { "" };
                    println!("    {} {}{}", "✗".yellow(), env_name, suffix);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_req(name: &str, env_var: Option<&str>, required: bool) -> SecretRequirement {
        SecretRequirement {
            name: name.to_string(),
            description: None,
            required,
            env_var: env_var.map(|s| s.to_string()),
            obtain_url: None,
        }
    }

    #[test]
    fn resolve_both_found() {
        let reqs = vec![make_req("A", None, true), make_req("B", None, true)];
        let mut dotenv = HashMap::new();
        dotenv.insert("A".to_string(), "val_a".to_string());
        dotenv.insert("B".to_string(), "val_b".to_string());

        let result = resolve_secrets(&reqs, &dotenv);
        assert_eq!(result.found.len(), 2);
        assert!(result.missing.is_empty());
    }

    #[test]
    fn resolve_one_missing() {
        let reqs = vec![make_req("X", None, true), make_req("Y", None, false)];
        let mut dotenv = HashMap::new();
        dotenv.insert("X".to_string(), "val_x".to_string());

        let result = resolve_secrets(&reqs, &dotenv);
        assert_eq!(result.found.len(), 1);
        assert_eq!(result.missing.len(), 1);
        assert_eq!(result.missing[0].name, "Y");
    }

    #[test]
    fn resolve_uses_env_var_field() {
        let reqs = vec![make_req("Human Name", Some("CUSTOM_KEY"), true)];
        let mut dotenv = HashMap::new();
        dotenv.insert("CUSTOM_KEY".to_string(), "custom_val".to_string());

        let result = resolve_secrets(&reqs, &dotenv);
        assert_eq!(result.found.len(), 1);
        assert!(result.found.contains_key("CUSTOM_KEY"));
    }

    #[test]
    fn resolve_shell_env_wins_over_dotenv() {
        let unique_key = "__PMCP_TEST_RESOLVE_KEY_PRECEDENCE";
        // Set shell env
        unsafe { std::env::set_var(unique_key, "shell_value") };

        let reqs = vec![make_req(unique_key, None, true)];
        let mut dotenv = HashMap::new();
        dotenv.insert(unique_key.to_string(), "dotenv_value".to_string());

        let result = resolve_secrets(&reqs, &dotenv);
        assert_eq!(result.found[unique_key], "shell_value");

        // Cleanup
        unsafe { std::env::remove_var(unique_key) };
    }

    #[test]
    fn resolve_empty_requirements() {
        let result = resolve_secrets(&[], &HashMap::new());
        assert!(result.found.is_empty());
        assert!(result.missing.is_empty());
    }

    #[test]
    fn load_dotenv_nonexistent() {
        let result = load_dotenv(Path::new("/tmp/__pmcp_nonexistent_path_test"));
        assert!(result.is_empty());
    }

    #[test]
    fn load_dotenv_valid_file() {
        let dir = std::env::temp_dir().join("__pmcp_test_dotenv");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join(".env"), "FOO=bar\nBAZ=qux\n").unwrap();

        let result = load_dotenv(&dir);
        assert_eq!(result["FOO"], "bar");
        assert_eq!(result["BAZ"], "qux");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
