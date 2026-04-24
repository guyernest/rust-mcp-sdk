//! MCP App metadata validation subcommand for cargo-pmcp
//!
//! Validates App-capable tools on an MCP server for correct `_meta` structure,
//! resource cross-references, and host-specific keys.

use anyhow::{Context, Result};
use colored::Colorize;
use mcp_tester::{AppValidationMode, AppValidator, TestReport, TestStatus};
use std::time::Duration;

use crate::commands::auth;
use crate::commands::flags::AuthFlags;
use crate::commands::GlobalFlags;

/// Execute the `cargo pmcp test apps` command.
pub async fn execute(
    url: String,
    mode: Option<String>,
    tool: Option<String>,
    strict: bool,
    transport: Option<String>,
    timeout: u64,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let verbose = global_flags.verbose;
    let validation_mode: AppValidationMode = mode
        .as_deref()
        .unwrap_or("standard")
        .parse()
        .map_err(|e: String| anyhow::anyhow!(e))?;

    print_apps_header(&url, &validation_mode, strict, tool.as_deref(), global_flags);

    let auth_method = auth_flags.resolve();
    let middleware = auth::resolve_auth_middleware(&url, &auth_method).await?;

    let mut tester = mcp_tester::ServerTester::new(
        &url,
        Duration::from_secs(timeout),
        false,
        None,
        transport.as_deref(),
        middleware,
    )
    .context("Failed to create server tester")?;

    run_apps_connectivity(&mut tester, global_flags).await?;

    if global_flags.should_output() {
        println!();
        println!("{}", "2. Discovering tools and resources...".bright_white());
    }

    let tools = list_tools_for_apps(&mut tester, verbose).await?;
    let resources = list_resources_for_apps(&mut tester, verbose).await;

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

/// Print the command header: URL, mode, strict flag, tool filter.
fn print_apps_header(
    url: &str,
    validation_mode: &AppValidationMode,
    strict: bool,
    tool: Option<&str>,
    global_flags: &GlobalFlags,
) {
    if !global_flags.should_output() {
        return;
    }
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
    if let Some(t) = tool {
        println!("  Tool filter: {}", t.bright_white());
    }
    println!();
}

/// Run the connectivity quick-test and print per-test status. Returns an
/// error when any test fails.
async fn run_apps_connectivity(
    tester: &mut mcp_tester::ServerTester,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!("{}", "1. Testing connectivity...".bright_white());
    }

    let init_report = tester
        .run_quick_test()
        .await
        .context("Failed to run connectivity test")?;

    if init_report.has_failures() {
        print_connectivity_failures(&init_report.tests);
        anyhow::bail!("Server connectivity check failed - cannot validate App metadata");
    }

    if global_flags.should_output() {
        println!("   {} Connected", "✓".green());
    }
    Ok(())
}

/// Print each connectivity test result with icon + optional error line.
fn print_connectivity_failures(tests: &[mcp_tester::TestResult]) {
    for result in tests {
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
}

/// List tools with verbose-aware error logging; propagates failure.
async fn list_tools_for_apps(
    tester: &mut mcp_tester::ServerTester,
    verbose: bool,
) -> Result<Vec<pmcp::types::ToolInfo>> {
    match tester.list_tools().await {
        Ok(result) => Ok(result.tools),
        Err(e) => {
            if verbose {
                eprintln!("   {} Tools listing failed: {}", "✗".red(), e);
            }
            anyhow::bail!("Failed to list tools: {e}");
        },
    }
}

/// List resources with verbose-aware warning; best-effort (empty on failure).
async fn list_resources_for_apps(
    tester: &mut mcp_tester::ServerTester,
    verbose: bool,
) -> Vec<pmcp::types::ResourceInfo> {
    match tester.list_resources().await {
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
    }
}
