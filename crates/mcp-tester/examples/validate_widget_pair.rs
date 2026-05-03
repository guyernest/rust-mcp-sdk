//! Example: Validate broken / corrected / cost-coach prod-shape widgets.
//!
//! ALWAYS-requirement working example for Phase 78. Demonstrates the
//! silent-fail bug Cost Coach hit, the fix, AND the cost-coach prod-bundle
//! shape passing the post-Plan-78-06 validator.
//!
//! Usage:
//!   cargo run -p mcp-tester --example validate_widget_pair

use mcp_tester::{AppValidationMode, AppValidator, OutputFormat, TestReport, TestStatus};

const BROKEN: &str = include_str!("../tests/fixtures/widgets/broken_no_sdk.html");
const CORRECTED: &str = include_str!("../tests/fixtures/widgets/corrected_minimal.html");
/// Cost-coach prod-bundle shape (Vite singlefile minified). Captured in
/// Plan 78-05 (`crates/mcp-tester/tests/fixtures/widgets/bundled/`).
/// Under the post-Plan-78-06 validator (G1+G2+G3 fixes), this fixture must
/// produce zero Failed rows — that's the proof the cost-coach v1
/// false-positive class is closed.
const COST_SUMMARY_MINIFIED: &str =
    include_str!("../tests/fixtures/widgets/bundled/cost_summary_minified.html");

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
    println!("MCP Apps widget validator — broken/corrected/bundled-prod demo");
    println!("================================================================");
    run_one("broken_no_sdk", BROKEN);
    run_one("corrected_minimal", CORRECTED);
    run_one(
        "cost_summary_minified (cost-coach prod shape)",
        COST_SUMMARY_MINIFIED,
    );
    println!(
        "Done. broken: many Failed; corrected: zero Failed; cost_summary_minified: zero Failed (post-Plan-06 fix)."
    );
}
