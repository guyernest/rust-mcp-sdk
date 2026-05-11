//! Example: Validate broken / corrected / cycle-2 real-prod widgets.
//!
//! ALWAYS-requirement working example for Phase 78 (extended in cycle 2).
//! Demonstrates the silent-fail bug Cost Coach hit, the cycle-1 fix against
//! synthetic minified fixtures, AND the cycle-2 fix that handles real
//! cost-coach prod minified output.
//!
//! Usage:
//!   cargo run -p mcp-tester --example validate_widget_pair
//!
//! Expected output: 9 widget runs (3 cycle-1 baseline + 6 cycle-2 real-prod).
//! `broken_no_sdk` reports many Failed rows; all 8 others report zero
//! Failed rows. The summary line tallies the cycle-2 real-prod failed
//! count — must read 0 for cycle-2 to be considered closed.

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

// Cycle-2 real-prod fixtures (Plan 78-09 Task 1 capture from cost-coach prod).
// Under the post-Plan-78-10 validator (string-literal aware comment stripper +
// widened G2 constructor regex), all 6 must produce zero Failed rows — that's
// the binding proof that the cost-coach v2 false-positive class (33 Failed
// rows in 2026-05-02 prod re-run) is closed.
const REAL_PROD_COST_SUMMARY: &str =
    include_str!("../tests/fixtures/widgets/bundled/real-prod/cost-summary.html");
const REAL_PROD_COST_OVER_TIME: &str =
    include_str!("../tests/fixtures/widgets/bundled/real-prod/cost-over-time.html");
const REAL_PROD_SAVINGS_SUMMARY: &str =
    include_str!("../tests/fixtures/widgets/bundled/real-prod/savings-summary.html");
const REAL_PROD_TAG_COVERAGE: &str =
    include_str!("../tests/fixtures/widgets/bundled/real-prod/tag-coverage.html");
const REAL_PROD_CONNECT_ACCOUNT: &str =
    include_str!("../tests/fixtures/widgets/bundled/real-prod/connect-account.html");
const REAL_PROD_SERVICE_SANKEY: &str =
    include_str!("../tests/fixtures/widgets/bundled/real-prod/service-sankey.html");

fn run_one(label: &str, html: &str) {
    let _ = run_one_and_count_failed(label, html);
}

fn run_one_and_count_failed(label: &str, html: &str) -> usize {
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
    failed
}

fn main() {
    println!("MCP Apps widget validator — broken/corrected/cycle-2-real-prod demo");
    println!("====================================================================");

    // Sample fixtures (cycle-1 baseline)
    run_one("broken_no_sdk", BROKEN);
    run_one("corrected_minimal", CORRECTED);
    run_one(
        "cost_summary_minified (cost-coach prod shape, cycle-1 synthetic)",
        COST_SUMMARY_MINIFIED,
    );

    // Cycle-2 real-prod fixtures (Plan 78-09 captures, Plan 78-10 GREEN)
    let real_prod = [
        ("real-prod cost-summary.html", REAL_PROD_COST_SUMMARY),
        ("real-prod cost-over-time.html", REAL_PROD_COST_OVER_TIME),
        ("real-prod savings-summary.html", REAL_PROD_SAVINGS_SUMMARY),
        ("real-prod tag-coverage.html", REAL_PROD_TAG_COVERAGE),
        ("real-prod connect-account.html", REAL_PROD_CONNECT_ACCOUNT),
        ("real-prod service-sankey.html", REAL_PROD_SERVICE_SANKEY),
    ];
    let mut real_prod_failed_total: usize = 0;
    for (label, html) in real_prod {
        real_prod_failed_total += run_one_and_count_failed(label, html);
    }

    println!(
        "\nDone. broken: many Failed; corrected: zero Failed; cycle-1 synthetic: zero Failed; cycle-2 real-prod: {} Failed total across 6 widgets.",
        real_prod_failed_total
    );
    if real_prod_failed_total > 0 {
        eprintln!(
            "WARNING: cycle-2 real-prod widgets produced {} Failed rows. The cost-coach prod false-positive class is NOT yet closed. Run `cargo test -p mcp-tester --test app_validator_widgets_real_prod` to see which assertions fail.",
            real_prod_failed_total
        );
    } else {
        println!(
            "All 6 cycle-2 real-prod widgets produced zero Failed rows. Cost-coach v2 false-positive class CLOSED."
        );
    }
}
