//! Example: Validate a broken vs. corrected widget pair under --mode claude-desktop.
//!
//! ALWAYS-requirement working example for Phase 78. Demonstrates the
//! silent-fail bug Cost Coach hit and the fix.
//!
//! Usage:
//!   cargo run -p mcp-tester --example validate_widget_pair

use mcp_tester::{AppValidationMode, AppValidator, OutputFormat, TestReport, TestStatus};

const BROKEN: &str = include_str!("../tests/fixtures/widgets/broken_no_sdk.html");
const CORRECTED: &str = include_str!("../tests/fixtures/widgets/corrected_minimal.html");

fn run_one(label: &str, html: &str) {
    println!("\n=== {label} (mode = claude-desktop) ===\n");
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let results = validator.validate_widgets(&[(
        format!("example-{label}"),
        format!("ui://example-{label}"),
        html.to_string(),
    )]);
    let failed = results
        .iter()
        .filter(|r| r.status == TestStatus::Failed)
        .count();
    let warnings = results
        .iter()
        .filter(|r| r.status == TestStatus::Warning)
        .count();
    let passed = results
        .iter()
        .filter(|r| r.status == TestStatus::Passed)
        .count();

    let mut report = TestReport::new();
    for r in results {
        report.add_test(r);
    }
    report.print(OutputFormat::Pretty);
    println!("\nSummary for {label}: {passed} passed, {warnings} warnings, {failed} failed.\n");
}

fn main() {
    println!("MCP Apps widget validator — broken/corrected demo");
    println!("==================================================");
    run_one("broken_no_sdk", BROKEN);
    run_one("corrected_minimal", CORRECTED);
    println!("Done. The broken widget produced Failed rows; the corrected one did not.");
}
