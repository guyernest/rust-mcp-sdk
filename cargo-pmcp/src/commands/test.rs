//! Test MCP servers using mcp-tester library

use anyhow::{Context, Result};
use colored::Colorize;
use mcp_tester::{generate_scenarios, run_scenario, GenerateOptions};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

pub fn execute(
    server: String,
    port: u16,
    do_generate_scenarios: bool,
    detailed: bool,
) -> Result<()> {
    println!("\n{}", "Testing MCP server".bright_cyan().bold());
    println!("{}", "─────────────────────────────────────".bright_cyan());

    // Verify we're in a workspace
    if !PathBuf::from("Cargo.toml").exists() {
        anyhow::bail!("Not in a workspace directory. Run 'cargo-pmcp new <name>' first.");
    }

    // Verify server exists
    let server_binary = format!("{}-server", server);
    let scenarios_dir = PathBuf::from("scenarios").join(&server);

    println!("\n{}", "Step 1: Building server".bright_white().bold());
    let build_status = Command::new("cargo")
        .args(["build", "--bin", &server_binary])
        .status()
        .context("Failed to build server")?;

    if !build_status.success() {
        anyhow::bail!("Server build failed");
    }
    println!("  {} Server built successfully", "✓".green());

    // Generate scenarios if requested
    if do_generate_scenarios {
        println!(
            "\n{}",
            "Step 2: Generating test scenarios".bright_white().bold()
        );

        // Start server in background
        println!("  {} Starting server on port {}...", "→".blue(), port);
        let mut server_process = Command::new("cargo")
            .args(["run", "--bin", &server_binary])
            .env("MCP_HTTP_PORT", port.to_string())
            .env("RUST_LOG", "error") // Quiet logs during scenario generation
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Failed to start server")?;

        // Wait for server to be ready
        thread::sleep(Duration::from_secs(5));

        // Generate scenarios
        let url = format!("http://0.0.0.0:{}", port);
        let output_path = scenarios_dir.join("generated.yaml");

        println!("  {} Generating scenarios...", "→".blue());

        let options = GenerateOptions {
            all_tools: true,
            with_resources: true,
            with_prompts: true,
        };

        // Use tokio runtime for async generation
        let generation_result = tokio::runtime::Runtime::new()?.block_on(async {
            generate_scenarios(&url, output_path.to_str().unwrap(), options).await
        });

        // Stop the server
        server_process.kill().ok();
        server_process.wait().ok();

        match generation_result {
            Ok(_) => {
                println!(
                    "  {} Scenarios generated at {}",
                    "✓".green(),
                    output_path.display()
                );
            },
            Err(e) => {
                println!("  {} Failed to generate scenarios: {}", "⚠".yellow(), e);
                println!("    Continuing with existing scenarios...");
            },
        }
    }

    // Run tests
    println!("\n{}", "Step 3: Running tests".bright_white().bold());

    // Start server in background
    println!("  {} Starting server on port {}...", "→".blue(), port);
    let mut server_process = Command::new("cargo")
        .args(["run", "--bin", &server_binary])
        .env("MCP_HTTP_PORT", port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to start server")?;

    // Wait for server to be ready
    thread::sleep(Duration::from_secs(5));

    let url = format!("http://0.0.0.0:{}", port);

    // Run mcp-tester
    println!("  {} Running mcp-tester...\n", "→".blue());

    // Try scenario-based testing first if scenarios exist
    let test_result = if scenarios_dir.exists() && scenarios_dir.read_dir()?.next().is_some() {
        // Find YAML scenarios
        let scenarios: Vec<_> = std::fs::read_dir(&scenarios_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s == "yaml" || s == "yml")
                    .unwrap_or(false)
            })
            .collect();

        if !scenarios.is_empty() {
            println!(
                "  {} Running {} scenario file(s)",
                "→".blue(),
                scenarios.len()
            );

            // Run each scenario using library
            let mut all_passed = true;
            for scenario in scenarios {
                let scenario_path = scenario.path();
                println!("\n  Testing: {}", scenario_path.display());

                let result = tokio::runtime::Runtime::new()?.block_on(async {
                    run_scenario(scenario_path.to_str().unwrap(), &url, detailed).await
                });

                match result {
                    Ok(_) => {
                        println!("  {} Passed", "✓".green());
                    },
                    Err(e) => {
                        println!("  {} Failed: {}", "✗".red(), e);
                        all_passed = false;
                    },
                }
            }

            Ok(all_passed)
        } else {
            // No scenarios found
            println!("  {} No scenarios found", "⚠".yellow());
            println!("    Run with --generate-scenarios to create test scenarios");
            Ok(true)
        }
    } else {
        // No scenarios directory
        println!("  {} No scenarios directory found", "⚠".yellow());
        println!("    Run with --generate-scenarios to create test scenarios");
        Ok(true)
    };

    // Stop the server
    server_process.kill().ok();
    server_process.wait().ok();

    println!();
    println!("{}", "═════════════════════════════════════".bright_cyan());

    match test_result {
        Ok(true) => {
            println!("{} All tests passed!", "✓".green().bold());
            println!("{}", "═════════════════════════════════════".bright_cyan());
            Ok(())
        },
        Ok(false) => {
            println!("{} Some tests failed", "✗".red().bold());
            println!("{}", "═════════════════════════════════════".bright_cyan());
            println!("\n{}", "Troubleshooting:".bright_white().bold());
            println!("  • Review scenario files in scenarios/{}/", server);
            println!("  • Check server logs for errors");
            println!(
                "  • Run with detailed output: {}",
                format!("cargo pmcp test --server {} --detailed", server).bright_cyan()
            );
            anyhow::bail!("Tests failed");
        },
        Err(e) => Err(e),
    }
}
