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

    let available_bins = match collect_workspace_binaries() {
        Ok(bins) => bins,
        Err(_) => return Ok(format!("{}-server", server)),
    };

    for candidate in &candidates {
        if available_bins.contains(candidate) {
            return Ok(candidate.clone());
        }
    }

    anyhow::bail!(build_no_binary_error(server, &candidates, &available_bins));
}

/// Shell out to `cargo metadata` and collect every workspace binary target
/// name. Returns Err when metadata call fails.
fn collect_workspace_binaries() -> Result<Vec<String>> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .context("Failed to run cargo metadata")?;

    if !output.status.success() {
        anyhow::bail!("cargo metadata returned non-zero exit status");
    }

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse cargo metadata")?;

    let mut available_bins: Vec<String> = Vec::new();
    let Some(packages) = metadata["packages"].as_array() else {
        return Ok(available_bins);
    };
    for package in packages {
        collect_package_binaries(package, &mut available_bins);
    }
    Ok(available_bins)
}

/// Append every `[[bin]]` target name from a package metadata node.
fn collect_package_binaries(package: &serde_json::Value, out: &mut Vec<String>) {
    let Some(targets) = package["targets"].as_array() else {
        return;
    };
    for target in targets {
        if target_is_bin(target) {
            if let Some(name) = target["name"].as_str() {
                out.push(name.to_string());
            }
        }
    }
}

/// True if a cargo-metadata target node has `kind` containing "bin".
fn target_is_bin(target: &serde_json::Value) -> bool {
    target["kind"]
        .as_array()
        .map(|k| k.iter().any(|v| v.as_str() == Some("bin")))
        .unwrap_or(false)
}

/// Build the helpful "no binary found" error message, distinguishing
/// Lambda-only setups from genuinely-missing binaries.
fn build_no_binary_error(server: &str, candidates: &[String], available_bins: &[String]) -> String {
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
    msg
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

    if !PathBuf::from("Cargo.toml").exists() {
        anyhow::bail!("Not in a workspace directory. Run 'cargo pmcp new <name>' first.");
    }

    port = resolve_dev_port(&server, port, global_flags)?;

    let server_binary = resolve_server_binary(&server)?;

    if global_flags.should_output() {
        println!("\n{}", "Step 1: Building server".bright_white().bold());
    }
    build_dev_server(&server_binary, global_flags)?;

    let dotenv_vars = load_dotenv_with_log(global_flags);

    print_server_starting(port, global_flags);
    let url = format!("http://0.0.0.0:{}", port);

    if let Some(client) = connect_client {
        run_dev_connect(&server, &url, client, global_flags)?;
    }

    print_dev_banner(global_flags);

    run_dev_server(&server_binary, port, &dotenv_vars)
}

/// Apply the configured-port override from WorkspaceConfig when CLI port is
/// the default (3000).
fn resolve_dev_port(
    server: &str,
    port: u16,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<u16> {
    let config = WorkspaceConfig::load()?;
    let Some(server_config) = config.get_server(server) else {
        return Ok(port);
    };
    if port != 3000 {
        return Ok(port);
    }
    let new_port = server_config.port;
    if global_flags.should_output() {
        println!(
            "  {} Using configured port {}",
            "→".blue(),
            new_port.to_string().bright_yellow()
        );
    }
    Ok(new_port)
}

/// Run `cargo build --bin <server_binary>` and bail on failure.
fn build_dev_server(
    server_binary: &str,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    let build_status = Command::new("cargo")
        .args(["build", "--bin", server_binary])
        .status()
        .context("Failed to build server")?;

    if !build_status.success() {
        anyhow::bail!("Server build failed");
    }
    if global_flags.should_output() {
        println!("  {} Server built successfully", "✓".green());
    }
    Ok(())
}

/// Load .env variables and log the count when output is enabled.
fn load_dotenv_with_log(
    global_flags: &crate::commands::GlobalFlags,
) -> std::collections::HashMap<String, String> {
    let project_root = PathBuf::from(".");
    let dotenv_vars = load_dotenv(&project_root);
    if !dotenv_vars.is_empty() && global_flags.should_output() {
        println!(
            "  {} Loaded {} variable(s) from .env",
            "✓".green(),
            dotenv_vars.len()
        );
    }
    dotenv_vars
}

/// Print the "Step 2: Starting server" banner + URL.
fn print_server_starting(port: u16, global_flags: &crate::commands::GlobalFlags) {
    if !global_flags.should_output() {
        return;
    }
    println!("\n{}", "Step 2: Starting server".bright_white().bold());
    let url = format!("http://0.0.0.0:{}", port);
    println!("  {} Server URL: {}", "→".blue(), url.bright_yellow());
}

/// Run the `connect` subcommand to attach the MCP client if requested.
fn run_dev_connect(
    server: &str,
    url: &str,
    client: String,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!(
            "\n{}",
            "Step 3: Connecting to MCP client".bright_white().bold()
        );
    }
    let default_auth = super::flags::AuthFlags {
        api_key: None,
        oauth_client_id: None,
        oauth_issuer: None,
        oauth_scopes: None,
        oauth_no_cache: false,
        oauth_redirect_port: 8080,
    };
    super::connect::execute(
        server.to_string(),
        client,
        url.to_string(),
        &default_auth,
        global_flags,
    )?;
    if global_flags.should_output() {
        println!();
    }
    Ok(())
}

/// Print the "Server is starting..." banner.
fn print_dev_banner(global_flags: &crate::commands::GlobalFlags) {
    if !global_flags.should_output() {
        return;
    }
    println!("{}", "─────────────────────────────────────".bright_cyan());
    println!("{}", "Server is starting...".bright_white().bold());
    println!("Press Ctrl+C to stop");
    println!("{}", "─────────────────────────────────────".bright_cyan());
    println!();
}

/// Run the MCP server as a foreground child process with injected env vars.
fn run_dev_server(
    server_binary: &str,
    port: u16,
    dotenv_vars: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.args(["run", "--bin", server_binary])
        .env("MCP_HTTP_PORT", port.to_string())
        .env("RUST_LOG", "info");

    // Inject .env vars for local dev (D-12)
    // Only set if not already in shell environment (D-13: shell env wins)
    for (key, value) in dotenv_vars {
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
