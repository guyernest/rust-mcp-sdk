//! MCP App metadata validation subcommand for cargo-pmcp
//!
//! Validates App-capable tools on an MCP server for correct `_meta` structure,
//! resource cross-references, and host-specific keys.

use anyhow::{Context, Result};
use colored::Colorize;
use mcp_tester::{AppValidationMode, AppValidator, TestReport, TestStatus};
use std::time::Duration;

use crate::commands::GlobalFlags;

/// Execute the `cargo pmcp test apps` command.
#[allow(clippy::too_many_arguments)]
pub async fn execute(
    url: String,
    mode: Option<String>,
    tool: Option<String>,
    strict: bool,
    transport: Option<String>,
    verbose: bool,
    timeout: u64,
    global_flags: &GlobalFlags,
) -> Result<()> {
    // Parse validation mode
    let validation_mode: AppValidationMode = mode
        .as_deref()
        .unwrap_or("standard")
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;

    if global_flags.should_output() {
        println!();
        println!("{}", "MCP App Validation".bright_cyan().bold());
        println!(
            "{}",
            "────────────────────────────────────────".bright_cyan()
        );
        println!("  URL:  {}", url.bright_white());
        println!("  Mode: {}", validation_mode.to_string().bright_white());
        if strict {
            println!("  Strict: {}", "yes".bright_yellow());
        }
        if let Some(ref t) = tool {
            println!("  Tool filter: {}", t.bright_white());
        }
        println!();
    }

    // Create server tester and verify connectivity
    let mut tester = mcp_tester::ServerTester::new(
        &url,
        Duration::from_secs(timeout),
        false, // insecure
        None,  // api_key
        transport.as_deref(),
        None, // http_middleware_chain
    )
    .context("Failed to create server tester")?;

    if global_flags.should_output() {
        println!("{}", "1. Testing connectivity...".bright_white());
    }

    let init_report = tester
        .run_quick_test()
        .await
        .context("Failed to run connectivity test")?;

    if init_report.has_failures() {
        for result in &init_report.tests {
            let icon = match result.status {
                TestStatus::Passed => "✓".green(),
                TestStatus::Failed => "✗".red(),
                TestStatus::Warning => "⚠".yellow(),
                TestStatus::Skipped => "○".yellow(),
            };
            println!("   {} {}", icon, result.name);
            if let Some(ref error) = result.error {
                println!("      {} {}", "Error:".red(), error);
            }
        }
        anyhow::bail!("Server connectivity check failed - cannot validate App metadata");
    }

    if global_flags.should_output() {
        println!("   {} Connected", "✓".green());
        println!();
        println!("{}", "2. Discovering tools and resources...".bright_white());
    }

    // List tools and resources
    let tools = match tester.list_tools().await {
        Ok(result) => result.tools,
        Err(e) => {
            if verbose {
                eprintln!("   {} Tools listing failed: {}", "✗".red(), e);
            }
            anyhow::bail!("Failed to list tools: {e}");
        },
    };

    let resources = match tester.list_resources().await {
        Ok(result) => result.resources,
        Err(e) => {
            if verbose {
                eprintln!(
                    "   {} Resources listing failed (continuing): {}",
                    "⚠".yellow(),
                    e
                );
            }
            Vec::new()
        },
    };

    if global_flags.should_output() {
        println!(
            "   {} {} tools, {} resources discovered",
            "✓".green(),
            tools.len(),
            resources.len()
        );
    }

    // Check for App-capable tools
    let app_count = tools
        .iter()
        .filter(|t| AppValidator::is_app_capable(t))
        .count();

    if app_count == 0 && tool.is_none() {
        if global_flags.should_output() {
            println!();
            println!(
                "   {} No App-capable tools found on this server ({} tools total)",
                "i".bright_cyan(),
                tools.len()
            );
            println!();
        }
        return Ok(());
    }

    if global_flags.should_output() {
        println!(
            "   {} {} App-capable tool{}",
            "i".bright_cyan(),
            app_count,
            if app_count == 1 { "" } else { "s" }
        );
        println!();
        println!("{}", "3. Validating App metadata...".bright_white());
    }

    // Run validation
    let validator = AppValidator::new(validation_mode, tool);
    let results = validator.validate_tools(&tools, &resources);

    if results.is_empty() {
        if global_flags.should_output() {
            println!("   {} No validation results", "i".bright_cyan());
            println!();
        }
        return Ok(());
    }

    // Build report
    let mut report = TestReport::new();
    for result in results {
        report.add_test(result);
    }

    if strict {
        report.apply_strict_mode();
    }

    // Print report
    report.print(mcp_tester::OutputFormat::Pretty);

    if report.has_failures() {
        anyhow::bail!("App validation failed - see errors above");
    }

    if global_flags.should_output() {
        println!(
            "{} {}",
            "✓".green().bold(),
            "App validation passed".green().bold()
        );
        println!();
    }

    Ok(())
}
