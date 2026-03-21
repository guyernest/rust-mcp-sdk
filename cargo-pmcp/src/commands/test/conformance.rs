//! MCP protocol conformance validation subcommand for cargo-pmcp
//!
//! Validates any MCP server against the MCP protocol spec (2025-11-25).
//! Runs 5 domain groups: Core, Tools, Resources, Prompts, Tasks.

use anyhow::{Context, Result};
use colored::Colorize;
use mcp_tester::{ConformanceDomain, ConformanceRunner, TestCategory, TestReport, TestStatus};
use std::time::Duration;

use crate::commands::auth;
use crate::commands::flags::AuthFlags;
use crate::commands::GlobalFlags;

/// Execute the `cargo pmcp test conformance` command.
pub async fn execute(
    url: String,
    strict: bool,
    domain: Option<Vec<String>>,
    transport: Option<String>,
    timeout: u64,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!();
        println!("{}", "MCP Protocol Conformance".bright_cyan().bold());
        println!(
            "{}",
            "--------------------------------------------".bright_cyan()
        );
        println!("  URL: {}", url.bright_white());
        if strict {
            println!("  Strict: {}", "yes".bright_yellow());
        }
        if let Some(ref domains) = domain {
            println!("  Domains: {}", domains.join(", ").bright_white());
        }
        println!();
    }

    // Resolve authentication middleware
    let auth_method = auth_flags.resolve();
    let middleware = auth::resolve_auth_middleware(&url, &auth_method).await?;

    // Create server tester
    let mut tester = mcp_tester::ServerTester::new(
        &url,
        Duration::from_secs(timeout),
        false, // insecure
        None,  // api_key -- auth handled via middleware
        transport.as_deref(),
        middleware,
    )
    .context("Failed to create server tester")?;

    // Parse domain filter
    let parsed_domains = domain.map(|ds| {
        ds.iter()
            .filter_map(|s| ConformanceDomain::from_str_loose(s))
            .collect::<Vec<_>>()
    });

    // Run conformance suite
    let runner = ConformanceRunner::new(strict, parsed_domains);
    let report = runner.run(&mut tester).await;

    // Print report
    report.print(mcp_tester::OutputFormat::Pretty);

    // Print per-domain summary line for CI consumption (D-13)
    if global_flags.should_output() {
        print_domain_summary(&report);
    }

    if report.has_failures() {
        anyhow::bail!("Conformance validation failed - see errors above");
    }

    if global_flags.should_output() {
        println!(
            "{} {}",
            "OK".green().bold(),
            "Conformance validation passed".green().bold()
        );
        println!();
    }

    Ok(())
}

/// Print a single-line per-domain summary for CI consumption.
///
/// Output format: `Conformance: Core=PASS Tools=PASS Resources=SKIP Prompts=PASS Tasks=SKIP`
/// This line is easy to grep/parse in CI pipelines.
fn print_domain_summary(report: &TestReport) {
    let domains = [
        ("Core", TestCategory::Core),
        ("Tools", TestCategory::Tools),
        ("Resources", TestCategory::Resources),
        ("Prompts", TestCategory::Prompts),
        ("Tasks", TestCategory::Tasks),
    ];

    let mut parts = Vec::new();
    for (name, category) in &domains {
        let domain_tests: Vec<_> = report
            .tests
            .iter()
            .filter(|t| t.category == *category)
            .collect();

        let status = if domain_tests.is_empty() {
            "N/A"
        } else if domain_tests
            .iter()
            .any(|t| t.status == TestStatus::Failed)
        {
            "FAIL"
        } else if domain_tests
            .iter()
            .all(|t| t.status == TestStatus::Skipped)
        {
            "SKIP"
        } else if domain_tests
            .iter()
            .any(|t| t.status == TestStatus::Warning)
        {
            "WARN"
        } else {
            "PASS"
        };

        parts.push(format!("{}={}", name, status));
    }

    println!();
    println!("Conformance: {}", parts.join(" "));
}
