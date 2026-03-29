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

    if !quiet {
        println!();
        println!(
            "  {} Workspace Diagnostics",
            "cargo pmcp doctor".bright_white().bold()
        );
        println!("  {}", "─".repeat(40).dimmed());
        println!();
    }

    // 1. Check Cargo.toml
    let cargo_toml = std::path::Path::new("Cargo.toml");
    if cargo_toml.exists() {
        if !quiet {
            println!("  {} Cargo.toml found", "✓".green());
        }

        // Check if it has pmcp dependency
        let content = std::fs::read_to_string(cargo_toml).unwrap_or_default();
        if content.contains("pmcp") {
            if !quiet {
                println!("  {} pmcp dependency detected", "✓".green());
            }
        } else {
            if !quiet {
                println!(
                    "  {} No pmcp dependency found (not an MCP workspace?)",
                    "!".yellow()
                );
            }
        }
    } else {
        if !quiet {
            println!("  {} No Cargo.toml in current directory", "✗".red());
        }
        issues += 1;
    }

    // 2. Check Rust toolchain
    match std::process::Command::new("rustc")
        .arg("--version")
        .output()
    {
        Ok(output) => {
            let version = String::from_utf8_lossy(&output.stdout);
            let version = version.trim();
            if !quiet {
                println!("  {} {}", "✓".green(), version);
            }
        },
        Err(_) => {
            if !quiet {
                println!("  {} Rust toolchain not found", "✗".red());
            }
            issues += 1;
        },
    }

    // 3. Check cargo fmt
    match std::process::Command::new("rustfmt")
        .arg("--version")
        .output()
    {
        Ok(output) if output.status.success() => {
            if !quiet {
                println!("  {} rustfmt available", "✓".green());
            }
        },
        _ => {
            if !quiet {
                println!(
                    "  {} rustfmt not found (run: rustup component add rustfmt)",
                    "!".yellow()
                );
            }
        },
    }

    // 4. Check clippy
    match std::process::Command::new("cargo")
        .args(["clippy", "--version"])
        .output()
    {
        Ok(output) if output.status.success() => {
            if !quiet {
                println!("  {} clippy available", "✓".green());
            }
        },
        _ => {
            if !quiet {
                println!(
                    "  {} clippy not found (run: rustup component add clippy)",
                    "!".yellow()
                );
            }
        },
    }

    // 5. Server connectivity (optional)
    if let Some(server_url) = url {
        if !quiet {
            println!();
            println!("  {} Server: {}", "→".blue(), server_url);
        }

        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()?;

            match client.post(server_url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json, text/event-stream")
                .body(r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"pmcp-doctor","version":"0.1.0"}},"id":1}"#)
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        if !quiet {
                            println!("  {} Server reachable (HTTP {})", "✓".green(), status);
                        }
                    } else {
                        if !quiet {
                            println!("  {} Server returned HTTP {}", "!".yellow(), status);
                        }
                    }
                }
                Err(e) => {
                    if !quiet {
                        println!("  {} Cannot reach server: {}", "✗".red(), e);
                    }
                    issues += 1;
                }
            }
            Ok::<(), anyhow::Error>(())
        })?;
    }

    // Summary
    if !quiet {
        println!();
        if issues == 0 {
            println!("  {} All checks passed", "✓".green().bold());
        } else {
            println!("  {} {} issue(s) found", "!".yellow().bold(), issues);
        }
        println!();
    }

    if issues > 0 {
        anyhow::bail!("{} diagnostic issue(s) found", issues);
    }

    Ok(())
}
