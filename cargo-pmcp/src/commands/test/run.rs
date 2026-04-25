//! Run tests locally against an MCP server

use anyhow::Result;
use colored::Colorize;
use mcp_tester::run_scenario_with_transport;
use std::path::PathBuf;

use crate::commands::flags::{AuthFlags, AuthMethod, ServerFlags};
use crate::commands::GlobalFlags;

/// Run test scenarios against a local or remote MCP server
pub fn execute(
    server_flags: ServerFlags,
    port: u16,
    scenarios: Option<PathBuf>,
    transport: Option<String>,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    // Warn if auth is configured: run_scenario_with_transport() does not yet support auth passthrough
    let auth_method = auth_flags.resolve();
    if !matches!(auth_method, AuthMethod::None) {
        eprintln!(
            "Warning: Auth flags are accepted but run_scenario_with_transport does not yet \
             support auth passthrough. For authenticated servers, use `cargo pmcp test check` \
             to verify connectivity."
        );
    }

    let detailed = global_flags.verbose;
    let (target_url, server) = server_flags.resolve_url(port)?;

    if global_flags.should_output() {
        println!("\n{}", "Running MCP server tests".bright_cyan().bold());
        println!("{}", "─────────────────────────────────────".bright_cyan());
    }

    // Determine scenarios directory
    let scenarios_dir = if let Some(dir) = scenarios {
        dir
    } else if let Some(server) = &server {
        PathBuf::from("scenarios").join(server)
    } else {
        // Try to find scenarios in current directory
        PathBuf::from("scenarios")
    };

    if global_flags.should_output() {
        if let Some(server) = &server {
            println!("\n{}", "Prerequisites:".bright_white().bold());
            println!("  {} Server must be running on port {}", "→".blue(), port);
            println!(
                "  {} Run in another terminal: {}",
                "→".blue(),
                format!("cargo pmcp dev --server {}", server).bright_cyan()
            );
        }

        // Run tests
        println!("\n{}", "Running tests".bright_white().bold());
        println!("  {} Target: {}", "→".blue(), target_url);
    }

    let test_result: Result<bool> = run_scenarios_if_present(
        &scenarios_dir,
        &target_url,
        transport.as_deref(),
        detailed,
        global_flags,
    );

    if global_flags.should_output() {
        println!();
        println!("{}", "═════════════════════════════════════".bright_cyan());
    }

    match test_result {
        Ok(true) => {
            if global_flags.should_output() {
                println!("{} All tests passed!", "✓".green().bold());
                println!("{}", "═════════════════════════════════════".bright_cyan());
            }
            Ok(())
        },
        Ok(false) => {
            println!("{} Some tests failed", "✗".red().bold());
            if global_flags.should_output() {
                println!("{}", "═════════════════════════════════════".bright_cyan());
                println!("\n{}", "Troubleshooting:".bright_white().bold());
                println!("  - Review scenario files in {}", scenarios_dir.display());
                println!("  - Check server logs for errors");
                println!("  - Run with --verbose for more output");
            }
            anyhow::bail!("Tests failed");
        },
        Err(e) => Err(e),
    }
}

/// Discover scenarios in `scenarios_dir` and run each via
/// `run_scenario_with_transport`, returning whether all passed. Handles the
/// "no scenarios" / "no directory" cases with user-facing hints.
fn run_scenarios_if_present(
    scenarios_dir: &PathBuf,
    target_url: &str,
    transport: Option<&str>,
    detailed: bool,
    global_flags: &GlobalFlags,
) -> Result<bool> {
    if !scenarios_dir.exists() || scenarios_dir.read_dir()?.next().is_none() {
        if global_flags.should_output() {
            println!(
                "  {} No scenarios directory found at {}",
                "⚠".yellow(),
                scenarios_dir.display()
            );
            println!("    Run 'cargo pmcp test generate' to create test scenarios");
        }
        return Ok(true);
    }

    let scenarios = discover_yaml_scenarios(scenarios_dir)?;

    if scenarios.is_empty() {
        if global_flags.should_output() {
            println!(
                "  {} No scenarios found in {}",
                "⚠".yellow(),
                scenarios_dir.display()
            );
            println!("    Run 'cargo pmcp test generate' to create test scenarios");
        }
        return Ok(true);
    }

    if global_flags.should_output() {
        println!(
            "  {} Running {} scenario file(s) from {}",
            "→".blue(),
            scenarios.len(),
            scenarios_dir.display()
        );
    }

    let mut all_passed = true;
    for scenario in scenarios {
        let scenario_path = scenario.path();
        if global_flags.should_output() {
            println!("\n  Testing: {}", scenario_path.display());
        }
        if !run_single_scenario(&scenario_path, target_url, transport, detailed)? {
            all_passed = false;
        }
    }
    Ok(all_passed)
}

/// Read `scenarios_dir` for files with .yaml/.yml extensions.
fn discover_yaml_scenarios(scenarios_dir: &PathBuf) -> Result<Vec<std::fs::DirEntry>> {
    Ok(std::fs::read_dir(scenarios_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s == "yaml" || s == "yml")
                .unwrap_or(false)
        })
        .collect())
}

/// Execute a single scenario file and log pass/fail; return whether it passed.
fn run_single_scenario(
    scenario_path: &std::path::Path,
    target_url: &str,
    transport: Option<&str>,
    detailed: bool,
) -> Result<bool> {
    let result = tokio::runtime::Runtime::new()?.block_on(async {
        run_scenario_with_transport(
            scenario_path.to_str().unwrap(),
            target_url,
            detailed,
            transport,
        )
        .await
    });

    match result {
        Ok(_) => {
            println!("  {} Passed", "✓".green());
            Ok(true)
        },
        Err(e) => {
            println!("  {} Failed: {}", "✗".red(), e);
            Ok(false)
        },
    }
}
