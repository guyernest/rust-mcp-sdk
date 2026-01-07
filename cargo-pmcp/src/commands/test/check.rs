//! Quick sanity check of MCP server connectivity and compliance
//!
//! This module provides a fast way to verify that an MCP server is:
//! - Reachable at the specified URL
//! - Responds correctly to the JSON-RPC initialize handshake
//! - Advertises its capabilities (tools, resources, prompts)
//!
//! Use `--verbose` to see raw JSON-RPC messages for debugging.

use anyhow::{Context, Result};
use colored::Colorize;
use mcp_tester::{ServerTester, TestStatus};
use std::time::Duration;

/// Execute the check command
pub async fn execute(
    url: String,
    transport: Option<String>,
    verbose: bool,
    timeout: u64,
) -> Result<()> {
    println!();
    println!("{}", "MCP Server Check".bright_cyan().bold());
    println!(
        "{}",
        "────────────────────────────────────────".bright_cyan()
    );
    println!("  URL: {}", url.bright_white());

    if let Some(ref t) = transport {
        println!("  Transport: {}", t.bright_white());
    } else {
        println!("  Transport: {} (auto-detect)", "default".bright_white());
    }
    println!();

    // Create the server tester
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(timeout),
        false, // insecure
        None,  // api_key
        transport.as_deref(),
        None, // http_middleware_chain
    )
    .context("Failed to create server tester")?;

    // Step 1: Connection and Initialize
    println!(
        "{}",
        "1. Testing connectivity and initialization...".bright_white()
    );

    let report = tester
        .run_quick_test()
        .await
        .context("Failed to run connectivity test")?;

    // Check results
    let mut has_failures = false;
    for result in &report.tests {
        let status_icon = match result.status {
            TestStatus::Passed => "✓".green(),
            TestStatus::Failed => {
                has_failures = true;
                "✗".red()
            },
            TestStatus::Warning => "⚠".yellow(),
            TestStatus::Skipped => "○".yellow(),
        };

        println!(
            "   {} {} ({}ms)",
            status_icon,
            result.name,
            result.duration.as_millis()
        );

        if verbose {
            if let Some(ref details) = result.details {
                println!("      {}", details.as_str().bright_black());
            }
        }

        if result.status == TestStatus::Failed {
            if let Some(ref error) = result.error {
                println!("      {} {}", "Error:".red(), error);
            }
        }
    }

    if has_failures {
        println!();
        println!(
            "{} {}",
            "✗".red().bold(),
            "Server check failed".red().bold()
        );
        println!();

        // Analyze errors to provide specific hints
        let error_messages: Vec<&str> = report
            .tests
            .iter()
            .filter_map(|r| r.error.as_deref())
            .collect();

        let has_transport_error = error_messages.iter().any(|e| {
            e.contains("untagged enum RequestId")
                || e.contains("Invalid message format")
                || e.contains("missing field `id`")
                || e.contains("Invalid response")
        });

        let transport_was_auto = transport.is_none();

        // Provide specific hint for transport mismatch
        if has_transport_error && transport_was_auto {
            println!(
                "{} {}",
                "💡".bright_yellow(),
                "Transport mismatch detected!".bright_yellow().bold()
            );
            println!();
            println!("   The server response format doesn't match the auto-detected transport.");
            println!("   This commonly happens with serverless deployments (Lambda, API Gateway).");
            println!();
            println!("   {} Try using JSON-RPC transport:", "→".bright_cyan());
            println!(
                "     cargo pmcp test check --url {} {}",
                url,
                "--transport jsonrpc".bright_green()
            );
            println!();
            println!("   {} Or try SSE streaming transport:", "→".bright_cyan());
            println!(
                "     cargo pmcp test check --url {} {}",
                url,
                "--transport http".bright_green()
            );
            println!();
        } else {
            println!("{}", "Troubleshooting tips:".bright_white().bold());
            println!("  1. Verify the server is running at the URL");
            println!("  2. Check if the URL requires authentication");
            println!("  3. Try a different transport: --transport jsonrpc or --transport http");
            println!("  4. Use --verbose to see detailed error messages");
            println!();
        }

        // Show raw error details in verbose mode
        if verbose {
            println!("{}", "Detailed error information:".bright_white().bold());
            for result in &report.tests {
                if result.status == TestStatus::Failed {
                    println!("  Test: {}", result.name);
                    if let Some(ref error) = result.error {
                        println!("  Error: {}", error);
                    }
                    if let Some(ref details) = result.details {
                        println!("  Details: {}", details);
                    }
                    println!();
                }
            }
        }

        anyhow::bail!("Server check failed - see errors above");
    }

    println!();

    // Step 2: Discover capabilities
    println!("{}", "2. Discovering server capabilities...".bright_white());

    // Try to list tools
    match tester.list_tools().await {
        Ok(tools_result) => {
            let count = tools_result.tools.len();
            if count > 0 {
                println!("   {} {} tools available", "✓".green(), count);
                if verbose {
                    for tool in &tools_result.tools {
                        println!(
                            "      • {} - {}",
                            tool.name.bright_white(),
                            tool.description.as_deref().unwrap_or("No description")
                        );
                    }
                }
            } else {
                println!("   {} No tools advertised", "○".yellow());
            }
        },
        Err(e) => {
            if verbose {
                println!("   {} Tools: {}", "○".yellow(), e);
            } else {
                println!("   {} Tools not available", "○".yellow());
            }
        },
    }

    // Try to list resources
    match tester.list_resources().await {
        Ok(resources_result) => {
            let count = resources_result.resources.len();
            if count > 0 {
                println!("   {} {} resources available", "✓".green(), count);
                if verbose {
                    for resource in &resources_result.resources {
                        println!(
                            "      • {} - {}",
                            resource.uri.bright_white(),
                            resource.name
                        );
                    }
                }
            } else {
                println!("   {} No resources advertised", "○".yellow());
            }
        },
        Err(e) => {
            if verbose {
                println!("   {} Resources: {}", "○".yellow(), e);
            } else {
                println!("   {} Resources not available", "○".yellow());
            }
        },
    }

    // Try to list prompts
    match tester.list_prompts().await {
        Ok(prompts_result) => {
            let count = prompts_result.prompts.len();
            if count > 0 {
                println!("   {} {} prompts available", "✓".green(), count);
                if verbose {
                    for prompt in &prompts_result.prompts {
                        println!(
                            "      • {} - {}",
                            prompt.name.bright_white(),
                            prompt.description.as_deref().unwrap_or("No description")
                        );
                    }
                }
            } else {
                println!("   {} No prompts advertised", "○".yellow());
            }
        },
        Err(e) => {
            if verbose {
                println!("   {} Prompts: {}", "○".yellow(), e);
            } else {
                println!("   {} Prompts not available", "○".yellow());
            }
        },
    }

    println!();
    println!(
        "{} {}",
        "✓".green().bold(),
        "Server check passed".green().bold()
    );
    println!();

    // Next steps
    println!("{}", "Next steps:".bright_white().bold());
    println!(
        "  • Generate test scenarios: cargo pmcp test generate --url {}",
        url
    );
    println!(
        "  • Run full test suite:    cargo pmcp test run --url {} --scenarios <path>",
        url
    );
    println!();

    Ok(())
}
