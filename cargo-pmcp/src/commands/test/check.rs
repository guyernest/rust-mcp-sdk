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
use mcp_tester::post_deploy_report::{
    FailureDetail, PostDeployReport, TestCommand as PdrCommand, TestOutcome,
};
use mcp_tester::{AppValidator, ServerTester, TestStatus};
use std::time::Duration;

use super::TestFormatValue;
use crate::commands::auth;
use crate::commands::flags::AuthFlags;
use crate::commands::GlobalFlags;

/// Execute the check command. Branches on `format`:
/// - [`TestFormatValue::Pretty`] (default) — preserves the existing terminal UX byte-for-byte.
/// - [`TestFormatValue::Json`] — emits a single [`PostDeployReport`] JSON document on stdout
///   for Phase 79 post-deploy verifier consumption (Plan 79-03).
pub async fn execute(
    url: String,
    transport: Option<String>,
    timeout: u64,
    format: TestFormatValue,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    match format {
        TestFormatValue::Json => execute_json(url, transport, timeout, auth_flags).await,
        TestFormatValue::Pretty => {
            execute_pretty(url, transport, timeout, auth_flags, global_flags).await
        },
    }
}

/// Pretty (human-readable) execution path. Behavior is byte-identical to the
/// pre-Phase-79 implementation — no UX regression.
async fn execute_pretty(
    url: String,
    transport: Option<String>,
    timeout: u64,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    let verbose = global_flags.verbose;

    print_check_header(&url, transport.as_deref(), global_flags);

    let mut tester = build_tester(&url, transport.as_deref(), timeout, auth_flags).await?;

    print_step_connectivity(global_flags);
    let report = tester
        .run_quick_test()
        .await
        .context("Failed to run connectivity test")?;

    let has_failures = print_test_results(&report.tests, verbose);

    if has_failures {
        print_failure_diagnostics(
            &report.tests,
            &url,
            transport.is_none(),
            verbose,
            global_flags,
        );
        anyhow::bail!("Server check failed - see errors above");
    }

    print_step_capabilities(global_flags);
    probe_and_print_tools(&mut tester, &url, verbose, global_flags).await;
    probe_and_print_resources(&mut tester, verbose).await;
    probe_and_print_prompts(&mut tester, verbose).await;

    print_check_success(&url, global_flags);

    Ok(())
}

/// JSON execution path. Emits exactly one [`PostDeployReport`] document on
/// stdout (no ANSI / log lines / banners) and exits with the standard
/// 0=Passed / 1=TestFailed / 2=InfraError code so the Phase 79 verifier can
/// dispatch on the typed outcome.
async fn execute_json(
    url: String,
    transport: Option<String>,
    timeout: u64,
    auth_flags: &AuthFlags,
) -> Result<()> {
    let started = std::time::Instant::now();

    let mut tester = match build_tester(&url, transport.as_deref(), timeout, auth_flags).await {
        Ok(t) => t,
        Err(e) => {
            emit_infra_error_json(PdrCommand::Check, &url, e.to_string(), started.elapsed())?;
            std::process::exit(2);
        },
    };

    let report = match tester.run_quick_test().await {
        Ok(r) => r,
        Err(e) => {
            emit_infra_error_json(PdrCommand::Check, &url, e.to_string(), started.elapsed())?;
            std::process::exit(2);
        },
    };

    let dur_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let failures: Vec<FailureDetail> = report
        .tests
        .iter()
        .filter(|t| t.status == TestStatus::Failed)
        .map(|t| FailureDetail {
            tool: None,
            message: t.error.clone().unwrap_or_else(|| t.name.clone()),
            reproduce: format!("cargo pmcp test check {url}"),
        })
        .collect();
    let any_failed = !failures.is_empty();

    let outcome = if any_failed {
        TestOutcome::TestFailed
    } else {
        TestOutcome::Passed
    };
    let pdr = PostDeployReport {
        command: PdrCommand::Check,
        url: url.clone(),
        mode: None,
        outcome,
        summary: None,
        failures,
        duration_ms: dur_ms,
        schema_version: "1".to_string(),
    };

    println!("{}", serde_json::to_string_pretty(&pdr)?);
    if any_failed {
        std::process::exit(1);
    }
    Ok(())
}

/// Helper: emit a single `InfraError`-shaped [`PostDeployReport`] on stdout.
/// Cog ≤8.
pub(super) fn emit_infra_error_json(
    command: PdrCommand,
    url: &str,
    message: String,
    elapsed: std::time::Duration,
) -> Result<()> {
    let subcommand = match command {
        PdrCommand::Check => "check",
        PdrCommand::Conformance => "conformance",
        PdrCommand::Apps => "apps",
    };
    let pdr = PostDeployReport {
        command,
        url: url.to_string(),
        mode: None,
        outcome: TestOutcome::InfraError,
        summary: None,
        failures: vec![FailureDetail {
            tool: None,
            message,
            reproduce: format!("cargo pmcp test {subcommand} {url}"),
        }],
        duration_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        schema_version: "1".to_string(),
    };
    println!("{}", serde_json::to_string_pretty(&pdr)?);
    Ok(())
}

/// Print the top-of-command banner (URL + transport) when output is enabled.
fn print_check_header(url: &str, transport: Option<&str>, global_flags: &GlobalFlags) {
    if !global_flags.should_output() {
        return;
    }
    println!();
    println!("{}", "MCP Server Check".bright_cyan().bold());
    println!(
        "{}",
        "────────────────────────────────────────".bright_cyan()
    );
    println!("  URL: {}", url.bright_white());
    match transport {
        Some(t) => println!("  Transport: {}", t.bright_white()),
        None => println!("  Transport: {} (auto-detect)", "default".bright_white()),
    }
    println!();
}

/// Resolve auth middleware and build the ServerTester.
async fn build_tester(
    url: &str,
    transport: Option<&str>,
    timeout: u64,
    auth_flags: &AuthFlags,
) -> Result<ServerTester> {
    let auth_method = auth_flags.resolve();
    let middleware = auth::resolve_auth_middleware(url, &auth_method).await?;

    ServerTester::new(
        url,
        Duration::from_secs(timeout),
        false, // insecure
        None,  // api_key -- auth handled via middleware for consistency
        transport,
        middleware,
    )
    .context("Failed to create server tester")
}

/// Print the "Step 1" banner.
fn print_step_connectivity(global_flags: &GlobalFlags) {
    if global_flags.should_output() {
        println!(
            "{}",
            "1. Testing connectivity and initialization...".bright_white()
        );
    }
}

/// Print quick-test results, return whether any test failed.
fn print_test_results(tests: &[mcp_tester::TestResult], verbose: bool) -> bool {
    let mut has_failures = false;
    for result in tests {
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
    has_failures
}

/// Print failure banner + transport-mismatch hint / troubleshooting tips,
/// plus verbose error details if requested.
fn print_failure_diagnostics(
    tests: &[mcp_tester::TestResult],
    url: &str,
    transport_was_auto: bool,
    verbose: bool,
    global_flags: &GlobalFlags,
) {
    if !global_flags.should_output() {
        return;
    }

    println!();
    println!(
        "{} {}",
        "✗".red().bold(),
        "Server check failed".red().bold()
    );
    println!();

    let has_transport_error = detect_transport_error(tests);
    if has_transport_error && transport_was_auto {
        print_transport_mismatch_hint(url);
    } else {
        print_troubleshooting_tips();
    }

    if verbose {
        print_verbose_failure_details(tests);
    }
}

/// Detect JSON-RPC/HTTP-transport shape errors that suggest the auto-detect
/// guessed wrong.
fn detect_transport_error(tests: &[mcp_tester::TestResult]) -> bool {
    tests.iter().filter_map(|r| r.error.as_deref()).any(|e| {
        e.contains("untagged enum RequestId")
            || e.contains("Invalid message format")
            || e.contains("missing field `id`")
            || e.contains("Invalid response")
    })
}

/// Print the transport-mismatch hint block (user-facing copy).
fn print_transport_mismatch_hint(url: &str) {
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
        "     cargo pmcp test check {} {}",
        url,
        "--transport jsonrpc".bright_green()
    );
    println!();
    println!("   {} Or try SSE streaming transport:", "→".bright_cyan());
    println!(
        "     cargo pmcp test check {} {}",
        url,
        "--transport http".bright_green()
    );
    println!();
}

/// Print the generic troubleshooting tips block.
fn print_troubleshooting_tips() {
    println!("{}", "Troubleshooting tips:".bright_white().bold());
    println!("  1. Verify the server is running at the URL");
    println!("  2. Check if the URL requires authentication");
    println!("  3. Try a different transport: --transport jsonrpc or --transport http");
    println!("  4. Use --verbose to see detailed error messages");
    println!();
}

/// Print per-failed-test error detail (verbose mode only).
fn print_verbose_failure_details(tests: &[mcp_tester::TestResult]) {
    println!("{}", "Detailed error information:".bright_white().bold());
    for result in tests {
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

/// Print the "Step 2" banner.
fn print_step_capabilities(global_flags: &GlobalFlags) {
    if global_flags.should_output() {
        println!();
        println!("{}", "2. Discovering server capabilities...".bright_white());
    }
}

/// Probe list_tools and print summary + per-tool details (verbose) + App hint.
async fn probe_and_print_tools(
    tester: &mut ServerTester,
    url: &str,
    verbose: bool,
    global_flags: &GlobalFlags,
) {
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

            if global_flags.should_output() {
                print_app_capable_hint(&tools_result.tools, url);
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
}

/// If the server advertises App-capable tools, print a one-line hint pointing
/// at `cargo pmcp test apps`.
fn print_app_capable_hint(tools: &[pmcp::types::ToolInfo], url: &str) {
    let app_count = tools
        .iter()
        .filter(|t| AppValidator::is_app_capable(t))
        .count();
    if app_count > 0 {
        println!(
            "   {} {} App-capable tool{} detected. Run `cargo pmcp test apps {}` for full validation.",
            "i".bright_cyan(),
            app_count,
            if app_count == 1 { "" } else { "s" },
            url
        );
    }
}

/// Probe list_resources and print summary + per-resource details (verbose).
async fn probe_and_print_resources(tester: &mut ServerTester, verbose: bool) {
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
}

/// Probe list_prompts and print summary + per-prompt details (verbose).
async fn probe_and_print_prompts(tester: &mut ServerTester, verbose: bool) {
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
}

/// Print the success banner and next-steps block.
fn print_check_success(url: &str, global_flags: &GlobalFlags) {
    if !global_flags.should_output() {
        return;
    }
    println!();
    println!(
        "{} {}",
        "✓".green().bold(),
        "Server check passed".green().bold()
    );
    println!();
    println!("{}", "Next steps:".bright_white().bold());
    println!(
        "  • Generate test scenarios: cargo pmcp test generate {}",
        url
    );
    println!(
        "  • Run full test suite:    cargo pmcp test run {} --scenarios <path>",
        url
    );
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp_tester::{TestCategory, TestResult};

    fn mk_test(name: &str, status: TestStatus, error: Option<String>) -> TestResult {
        TestResult {
            name: name.to_string(),
            category: TestCategory::Core,
            status,
            duration: Duration::from_millis(1),
            details: None,
            error,
        }
    }

    #[test]
    fn detect_transport_error_spots_untagged_enum_requestid() {
        let tests = vec![mk_test(
            "init",
            TestStatus::Failed,
            Some("failed: untagged enum RequestId".to_string()),
        )];
        assert!(detect_transport_error(&tests));
    }

    #[test]
    fn detect_transport_error_spots_missing_id_field() {
        let tests = vec![mk_test(
            "init",
            TestStatus::Failed,
            Some("missing field `id` in response".to_string()),
        )];
        assert!(detect_transport_error(&tests));
    }

    #[test]
    fn detect_transport_error_spots_invalid_message_format() {
        let tests = vec![mk_test(
            "init",
            TestStatus::Failed,
            Some("Invalid message format received".to_string()),
        )];
        assert!(detect_transport_error(&tests));
    }

    #[test]
    fn detect_transport_error_returns_false_for_unrelated_error() {
        let tests = vec![mk_test(
            "init",
            TestStatus::Failed,
            Some("connection refused".to_string()),
        )];
        assert!(!detect_transport_error(&tests));
    }

    #[test]
    fn detect_transport_error_returns_false_when_empty() {
        assert!(!detect_transport_error(&[]));
    }

    #[test]
    fn print_test_results_counts_failures_accurately() {
        let tests = vec![
            mk_test("a", TestStatus::Passed, None),
            mk_test("b", TestStatus::Failed, Some("err".to_string())),
            mk_test("c", TestStatus::Skipped, None),
        ];
        assert!(print_test_results(&tests, false));
    }

    #[test]
    fn print_test_results_false_when_all_pass() {
        let tests = vec![
            mk_test("a", TestStatus::Passed, None),
            mk_test("b", TestStatus::Warning, None),
        ];
        assert!(!print_test_results(&tests, false));
    }
}
