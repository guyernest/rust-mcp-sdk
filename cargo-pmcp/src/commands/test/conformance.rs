//! MCP protocol conformance validation subcommand for cargo-pmcp
//!
//! Validates any MCP server against the MCP protocol spec (2025-11-25).
//! Runs 5 domain groups: Core, Tools, Resources, Prompts, Tasks.

use anyhow::{Context, Result};
use colored::Colorize;
use mcp_tester::post_deploy_report::{
    FailureDetail, PostDeployReport, TestCommand as PdrCommand, TestOutcome,
};
use mcp_tester::{ConformanceDomain, ConformanceRunner, TestCategory, TestReport, TestStatus};
use std::time::Duration;

use super::check::emit_infra_error_json;
use super::TestFormatValue;
use crate::commands::auth;
use crate::commands::flags::AuthFlags;
use crate::commands::GlobalFlags;

/// Execute the `cargo pmcp test conformance` command. Branches on `format`:
/// - [`TestFormatValue::Pretty`] (default) — preserves the existing terminal UX byte-for-byte.
/// - [`TestFormatValue::Json`] — emits a single [`PostDeployReport`] on stdout
///   carrying the per-domain summary + per-failure detail for Phase 79
///   verifier consumption (Plan 79-03).
pub async fn execute(
    url: String,
    strict: bool,
    domain: Option<Vec<String>>,
    transport: Option<String>,
    timeout: u64,
    format: TestFormatValue,
    auth_flags: &AuthFlags,
    global_flags: &GlobalFlags,
) -> Result<()> {
    match format {
        TestFormatValue::Json => {
            execute_json(url, strict, domain, transport, timeout, auth_flags).await
        },
        TestFormatValue::Pretty => {
            execute_pretty(
                url,
                strict,
                domain,
                transport,
                timeout,
                auth_flags,
                global_flags,
            )
            .await
        },
    }
}

/// Pretty (human-readable) execution path. Behavior is byte-identical to the
/// pre-Phase-79 implementation — no UX regression.
async fn execute_pretty(
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
/// Output format: `Conformance: Core=PASS Transport=PASS Tools=PASS Resources=SKIP Prompts=PASS Tasks=SKIP`
/// This line is easy to grep/parse in CI pipelines.
fn print_domain_summary(report: &TestReport) {
    let domains = [
        ("Core", TestCategory::Core),
        ("Transport", TestCategory::Transport),
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
        } else if domain_tests.iter().any(|t| t.status == TestStatus::Failed) {
            "FAIL"
        } else if domain_tests.iter().all(|t| t.status == TestStatus::Skipped) {
            "SKIP"
        } else if domain_tests.iter().any(|t| t.status == TestStatus::Warning) {
            "WARN"
        } else {
            "PASS"
        };

        parts.push(format!("{}={}", name, status));
    }

    println!();
    println!("Conformance: {}", parts.join(" "));
}

/// JSON execution path. Builds the same `TestReport` via the same
/// `ConformanceRunner`, then wraps it in a [`PostDeployReport`] on stdout.
async fn execute_json(
    url: String,
    strict: bool,
    domain: Option<Vec<String>>,
    transport: Option<String>,
    timeout: u64,
    auth_flags: &AuthFlags,
) -> Result<()> {
    let started = std::time::Instant::now();

    let auth_method = auth_flags.resolve();
    let middleware = match auth::resolve_auth_middleware(&url, &auth_method).await {
        Ok(m) => m,
        Err(e) => {
            emit_infra_error_json(
                PdrCommand::Conformance,
                &url,
                e.to_string(),
                started.elapsed(),
            )?;
            std::process::exit(2);
        },
    };

    let mut tester = match mcp_tester::ServerTester::new(
        &url,
        Duration::from_secs(timeout),
        false,
        None,
        transport.as_deref(),
        middleware,
    ) {
        Ok(t) => t,
        Err(e) => {
            emit_infra_error_json(
                PdrCommand::Conformance,
                &url,
                e.to_string(),
                started.elapsed(),
            )?;
            std::process::exit(2);
        },
    };

    let parsed_domains = domain.map(|ds| {
        ds.iter()
            .filter_map(|s| ConformanceDomain::from_str_loose(s))
            .collect::<Vec<_>>()
    });

    let runner = ConformanceRunner::new(strict, parsed_domains);
    let report = runner.run(&mut tester).await;
    let dur_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let failures: Vec<FailureDetail> = report
        .tests
        .iter()
        .filter(|t| t.status == TestStatus::Failed)
        .map(|t| FailureDetail {
            tool: Some(category_label(&t.category)),
            message: t.error.clone().unwrap_or_else(|| t.name.clone()),
            reproduce: format!(
                "cargo pmcp test conformance {url} --domain {}",
                category_to_kebab(&t.category)
            ),
        })
        .collect();

    let outcome = if report.has_failures() {
        TestOutcome::TestFailed
    } else {
        TestOutcome::Passed
    };
    let pdr = PostDeployReport {
        command: PdrCommand::Conformance,
        url: url.clone(),
        mode: None,
        outcome,
        summary: Some(report.summary.clone()),
        failures,
        duration_ms: dur_ms,
        schema_version: "1".to_string(),
    };

    println!("{}", serde_json::to_string_pretty(&pdr)?);
    if report.has_failures() {
        std::process::exit(1);
    }
    Ok(())
}

/// Map a [`TestCategory`] to its display label for the `tool` field of a
/// `FailureDetail` (e.g. `Tools` → `"Tools"`).
fn category_label(category: &TestCategory) -> String {
    format!("{category:?}")
}

/// Map a [`TestCategory`] to its `--domain` flag value (lowercase).
/// `Apps`, `Performance`, and `Compatibility` aren't valid `--domain` values
/// in the existing parser; they fall back to `core` so the reproduce string
/// is still well-formed.
fn category_to_kebab(category: &TestCategory) -> &'static str {
    match category {
        TestCategory::Core => "core",
        TestCategory::Transport => "transport",
        TestCategory::Tools => "tools",
        TestCategory::Resources => "resources",
        TestCategory::Prompts => "prompts",
        TestCategory::Tasks => "tasks",
        // Categories that conformance does not filter on:
        TestCategory::Protocol
        | TestCategory::Performance
        | TestCategory::Compatibility
        | TestCategory::Apps => "core",
    }
}
