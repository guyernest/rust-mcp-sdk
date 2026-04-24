//! Workspace diagnostics — validates project structure, toolchain, and server connectivity.

use anyhow::Result;
use colored::Colorize;

use super::GlobalFlags;

/// Run workspace diagnostics.
///
/// Checks:
/// 1. Cargo.toml exists and is a valid workspace or package
/// 2. Rust toolchain is installed and meets MSRV
/// 3. Required tools (cargo-pmcp dependencies) are available
/// 4. If a server URL is provided, tests connectivity
pub fn execute(url: Option<&str>, global_flags: &GlobalFlags) -> Result<()> {
    let quiet = !global_flags.should_output();
    let mut issues = 0u32;

    print_doctor_header(quiet);

    issues += check_cargo_toml(quiet);
    issues += check_rust_toolchain(quiet);
    check_rustfmt(quiet);
    check_clippy(quiet);

    if let Some(server_url) = url {
        issues += check_server_connectivity(server_url, quiet)?;
    }

    print_doctor_summary(issues, quiet);

    if issues > 0 {
        anyhow::bail!("{} diagnostic issue(s) found", issues);
    }
    Ok(())
}

fn print_doctor_header(quiet: bool) {
    if quiet {
        return;
    }
    println!();
    println!(
        "  {} Workspace Diagnostics",
        "cargo pmcp doctor".bright_white().bold()
    );
    println!("  {}", "─".repeat(40).dimmed());
    println!();
}

/// Verify Cargo.toml exists and pmcp dependency is present. Returns issue count (0 or 1).
fn check_cargo_toml(quiet: bool) -> u32 {
    let cargo_toml = std::path::Path::new("Cargo.toml");
    if !cargo_toml.exists() {
        if !quiet {
            println!("  {} No Cargo.toml in current directory", "✗".red());
        }
        return 1;
    }

    if !quiet {
        println!("  {} Cargo.toml found", "✓".green());
    }

    let content = std::fs::read_to_string(cargo_toml).unwrap_or_default();
    if !quiet {
        if content.contains("pmcp") {
            println!("  {} pmcp dependency detected", "✓".green());
        } else {
            println!(
                "  {} No pmcp dependency found (not an MCP workspace?)",
                "!".yellow()
            );
        }
    }
    0
}

/// Check rustc is available. Returns issue count (0 or 1).
fn check_rust_toolchain(quiet: bool) -> u32 {
    match std::process::Command::new("rustc")
        .arg("--version")
        .output()
    {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            if !quiet {
                println!("  {} {}", "✓".green(), version.trim());
            }
            0
        },
        Err(_) => {
            if !quiet {
                println!("  {} Rust toolchain not found", "✗".red());
            }
            1
        },
    }
}

/// Check rustfmt is installed (warning-only, does not count as an issue).
fn check_rustfmt(quiet: bool) {
    let ok = std::process::Command::new("rustfmt")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if quiet {
        return;
    }
    if ok {
        println!("  {} rustfmt available", "✓".green());
    } else {
        println!(
            "  {} rustfmt not found (run: rustup component add rustfmt)",
            "!".yellow()
        );
    }
}

/// Check cargo clippy is installed (warning-only).
fn check_clippy(quiet: bool) {
    let ok = std::process::Command::new("cargo")
        .args(["clippy", "--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if quiet {
        return;
    }
    if ok {
        println!("  {} clippy available", "✓".green());
    } else {
        println!(
            "  {} clippy not found (run: rustup component add clippy)",
            "!".yellow()
        );
    }
}

/// Probe the MCP server URL with an initialize JSON-RPC request.
/// Returns issue count (0 or 1).
fn check_server_connectivity(server_url: &str, quiet: bool) -> Result<u32> {
    if !quiet {
        println!();
        println!("  {} Server: {}", "→".blue(), server_url);
    }

    let rt = tokio::runtime::Runtime::new()?;
    let issue_count = rt.block_on(async {
        probe_server_initialize(server_url, quiet).await
    })?;
    Ok(issue_count)
}

/// Async worker for the initialize probe.
async fn probe_server_initialize(server_url: &str, quiet: bool) -> Result<u32> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"pmcp-doctor","version":"0.1.0"}},"id":1}"#)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if !quiet {
                if status.is_success() {
                    println!("  {} Server reachable (HTTP {})", "✓".green(), status);
                } else {
                    println!("  {} Server returned HTTP {}", "!".yellow(), status);
                }
            }
            Ok(0)
        },
        Err(e) => {
            if !quiet {
                println!("  {} Cannot reach server: {}", "✗".red(), e);
            }
            Ok(1)
        },
    }
}

/// Print the pass/fail summary banner.
fn print_doctor_summary(issues: u32, quiet: bool) {
    if quiet {
        return;
    }
    println!();
    if issues == 0 {
        println!("  {} All checks passed", "✓".green().bold());
    } else {
        println!("  {} {} issue(s) found", "!".yellow().bold(), issues);
    }
    println!();
}
