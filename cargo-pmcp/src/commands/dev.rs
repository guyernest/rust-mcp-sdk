//! Development server command

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;
use std::process::Command;

use crate::secrets::resolve::load_dotenv;
use crate::utils::config::WorkspaceConfig;

/// Binary targets that are Lambda deployment wrappers and cannot run locally.
const LAMBDA_BINARIES: &[&str] = &["bootstrap"];

/// Resolve the binary target for a server name.
///
/// Tries in order:
/// 1. `{server}-server` (standard convention from `cargo pmcp new`)
/// 2. `{server}` (direct name match)
///
/// Excludes Lambda-only binaries (e.g. `bootstrap`) that cannot run locally.
/// Uses `cargo metadata` to discover available binary targets.
fn resolve_server_binary(server: &str) -> Result<String> {
    let candidates = [format!("{}-server", server), server.to_string()];

    // Use cargo metadata to find available binary targets
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("Failed to run cargo metadata")?;

    if !output.status.success() {
        // Fallback: just try the conventional name
        return Ok(format!("{}-server", server));
    }

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse cargo metadata")?;

    // Collect all binary target names across all workspace packages
    let mut available_bins: Vec<String> = Vec::new();
    if let Some(packages) = metadata["packages"].as_array() {
        for package in packages {
            if let Some(targets) = package["targets"].as_array() {
                for target in targets {
                    let kinds = target["kind"].as_array();
                    let is_bin = kinds
                        .map(|k| k.iter().any(|v| v.as_str() == Some("bin")))
                        .unwrap_or(false);
                    if is_bin {
                        if let Some(name) = target["name"].as_str() {
                            available_bins.push(name.to_string());
                        }
                    }
                }
            }
        }
    }

    // Try each candidate in priority order
    for candidate in &candidates {
        if available_bins.contains(candidate) {
            return Ok(candidate.clone());
        }
    }

    // Filter out Lambda-only binaries for the error message
    let local_bins: Vec<&String> = available_bins
        .iter()
        .filter(|b| !LAMBDA_BINARIES.contains(&b.as_str()))
        .collect();
    let lambda_bins: Vec<&String> = available_bins
        .iter()
        .filter(|b| LAMBDA_BINARIES.contains(&b.as_str()))
        .collect();

    let mut msg = format!(
        "No binary target found for server '{}'\n\
         Tried: {}",
        server,
        candidates.join(", "),
    );

    if local_bins.is_empty() && !lambda_bins.is_empty() {
        msg.push_str(&format!(
            "\n\nThis project only has Lambda binaries ({}), which cannot run locally.\n\
             To add a standalone server binary, create a '{}-server' crate:\n\
             \n  cargo pmcp new {} --template standalone\n\
             \nOr add a [[bin]] target to your Cargo.toml with name = \"{}-server\"",
            lambda_bins
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            server,
            server,
            server,
        ));
    } else {
        msg.push_str(&format!(
            "\nAvailable binary targets: {}",
            if available_bins.is_empty() {
                "(none)".to_string()
            } else {
                available_bins.join(", ")
            }
        ));
    }

    anyhow::bail!(msg);
}

/// Start development server
pub fn execute(
    server: String,
    mut port: u16,
    connect_client: Option<String>,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Starting development server".bright_cyan().bold());
        println!("{}", "────────────────────────────────────".bright_cyan());
    }

    // Verify we're in a workspace
    if !PathBuf::from("Cargo.toml").exists() {
        anyhow::bail!("Not in a workspace directory. Run 'cargo pmcp new <name>' first.");
    }

    // Load workspace config and use configured port if available
    let config = WorkspaceConfig::load()?;
    if let Some(server_config) = config.get_server(&server) {
        // Use configured port (overrides CLI --port unless explicitly set)
        if port == 3000 {
            // Default port, use configured one
            port = server_config.port;
            if global_flags.should_output() {
                println!(
                    "  {} Using configured port {}",
                    "→".blue(),
                    port.to_string().bright_yellow()
                );
            }
        }
    }

    // Resolve the actual binary target name
    let server_binary = resolve_server_binary(&server)?;

    if global_flags.should_output() {
        println!("\n{}", "Step 1: Building server".bright_white().bold());
    }
    let build_status = Command::new("cargo")
        .args(["build", "--bin", &server_binary])
        .status()
        .context("Failed to build server")?;

    if !build_status.success() {
        anyhow::bail!("Server build failed");
    }
    if global_flags.should_output() {
        println!("  {} Server built successfully", "✓".green());
    }

    // Load .env file for local development (D-12)
    let project_root = PathBuf::from(".");
    let dotenv_vars = load_dotenv(&project_root);
    if !dotenv_vars.is_empty() && global_flags.should_output() {
        println!(
            "  {} Loaded {} variable(s) from .env",
            "✓".green(),
            dotenv_vars.len()
        );
    }

    if global_flags.should_output() {
        println!("\n{}", "Step 2: Starting server".bright_white().bold());
    }
    let url = format!("http://0.0.0.0:{}", port);
    if global_flags.should_output() {
        println!("  {} Server URL: {}", "→".blue(), url.bright_yellow());
    }

    // If connect_client is specified, run connect command first
    if let Some(client) = connect_client {
        if global_flags.should_output() {
            println!(
                "\n{}",
                "Step 3: Connecting to MCP client".bright_white().bold()
            );
        }
        // Dev connect doesn't use auth -- pass default empty flags
        let default_auth = super::flags::AuthFlags {
            api_key: None,
            oauth_client_id: None,
            oauth_issuer: None,
            oauth_scopes: None,
            oauth_no_cache: false,
            oauth_redirect_port: 8080,
        };
        super::connect::execute(
            server.clone(),
            client,
            url.clone(),
            &default_auth,
            global_flags,
        )?;
        if global_flags.should_output() {
            println!();
        }
    }

    if global_flags.should_output() {
        println!("{}", "─────────────────────────────────────".bright_cyan());
        println!("{}", "Server is starting...".bright_white().bold());
        println!("Press Ctrl+C to stop");
        println!("{}", "─────────────────────────────────────".bright_cyan());
        println!();
    }

    // Start server in foreground (user sees logs)
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--bin", &server_binary])
        .env("MCP_HTTP_PORT", port.to_string())
        .env("RUST_LOG", "info");

    // Inject .env vars for local dev (D-12)
    // Only set if not already in shell environment (D-13: shell env wins)
    for (key, value) in &dotenv_vars {
        if std::env::var(key).is_err() {
            cmd.env(key, value);
        }
    }

    let status = cmd.status().context("Failed to start server")?;

    if !status.success() {
        anyhow::bail!("Server exited with error");
    }

    Ok(())
}
