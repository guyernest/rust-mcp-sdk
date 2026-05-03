//! RED-phase regression tests for Phase 78 gap-closure cycle 2 (Plan 09).
//!
//! These tests are bound to BYTES CAPTURED FROM REAL cost-coach prod
//! widget bundles (fixtures in `tests/fixtures/widgets/bundled/real-prod/`,
//! captured from `~/projects/mcp/cost-coach/widget/dist/` at commit `29f46efd`).
//! See `real-prod/CAPTURE.md` for capture provenance and `real-prod/README.md`
//! for the per-fixture mode-emission table.
//!
//! Cycle 2 scope is two load-bearing fixes:
//!   1. Make `strip_js_comments` string-literal aware (block + line
//!      comments). The cycle-1 regex destroys ~21 KB of SDK code in
//!      cost-over-time and savings-summary because a JS string
//!      `"/*.example.com..."` opens a phantom block comment that closes
//!      at a real license-header `*/` thousands of bytes later.
//!   2. Widen G2 constructor regex to accept non-literal `name`/`version`
//!      values — real prod uses `name:"cost-coach-"+t,version:"1.0.0"`
//!      (string concatenation), not `name:"literal"`.
//!
//! See `fixtures/widgets/bundled/real-prod/CAPTURE.md` "Root cause
//! discovered" section for the step-by-step probe evidence.
//!
//! Until Plan 78-10 lands, these 6 tests are EXPECTED TO FAIL today
//! (cost-summary / tag-coverage / connect-account / service-sankey fail
//! only the G2 constructor row; cost-over-time / savings-summary fail
//! the full cascade because the comment-stripper destroys their SDK
//! section):
//!   - test_real_prod_cost_summary_passes_claude_desktop
//!   - test_real_prod_cost_over_time_passes_claude_desktop
//!   - test_real_prod_savings_summary_passes_claude_desktop
//!   - test_real_prod_tag_coverage_passes_claude_desktop
//!   - test_real_prod_connect_account_passes_claude_desktop
//!   - test_real_prod_service_sankey_passes_claude_desktop
//!
//! `test_real_prod_no_regression_on_cycle1_synthetic_fixtures` PASSES today
//! and must continue to pass after Plan 78-10 — it asserts the cycle-1
//! synthetic fixtures (which the post-Plan-06 validator handles) are not
//! regressed by Plan 10's regex generalization.
//!
//! After Plan 78-10 lands, ALL SEVEN tests must pass.
//!
//! Source evidence: `78-VERIFICATION.md` Gap G6 +
//! `uat-evidence/2026-05-02-cost-coach-prod-rerun.md` (33 Failed rows on
//! the 8 cost-coach prod widgets, identical count to v1 — G2 universal miss
//! is the load-bearing failure pattern).

use mcp_tester::{AppValidationMode, AppValidator, TestStatus};

// Real-prod fixtures (Plan 78-09 Task 1 capture).
const COST_SUMMARY_PROD: &str =
    include_str!("fixtures/widgets/bundled/real-prod/cost-summary.html");
const COST_OVER_TIME_PROD: &str =
    include_str!("fixtures/widgets/bundled/real-prod/cost-over-time.html");
const SAVINGS_SUMMARY_PROD: &str =
    include_str!("fixtures/widgets/bundled/real-prod/savings-summary.html");
const TAG_COVERAGE_PROD: &str =
    include_str!("fixtures/widgets/bundled/real-prod/tag-coverage.html");
const CONNECT_ACCOUNT_PROD: &str =
    include_str!("fixtures/widgets/bundled/real-prod/connect-account.html");
const SERVICE_SANKEY_PROD: &str =
    include_str!("fixtures/widgets/bundled/real-prod/service-sankey.html");

// Cycle-1 synthetic fixtures (Plan 78-05 capture). Re-asserted by the
// no-regression test to prove Plan 10's regex generalization doesn't break
// cycle 1's synthetic shapes.
const COST_SUMMARY_SYNTHETIC: &str =
    include_str!("fixtures/widgets/bundled/cost_summary_minified.html");
const COST_OVER_TIME_SYNTHETIC: &str =
    include_str!("fixtures/widgets/bundled/cost_over_time_minified.html");
const SYNTHETIC_CASCADE: &str =
    include_str!("fixtures/widgets/bundled/synthetic_cascade_repro.html");

fn validate(mode: AppValidationMode, tool_name: &str, html: &str) -> Vec<mcp_tester::TestResult> {
    let validator = AppValidator::new(mode, None);
    validator.validate_widgets(&[(
        tool_name.to_string(),
        "ui://test".to_string(),
        html.to_string(),
    )])
}

fn count_status(results: &[mcp_tester::TestResult], wanted: TestStatus) -> usize {
    results.iter().filter(|r| r.status == wanted).count()
}

fn diagnostic_dump(results: &[mcp_tester::TestResult]) -> String {
    results
        .iter()
        .map(|r| format!("  [{:?}] {}", r.status, r.name))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_real_prod_cost_summary_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "get_spend_summary",
        COST_SUMMARY_PROD,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed, 0,
        "real-prod cost-summary.html must produce ZERO Failed rows in claude-desktop mode after Plan 78-10; got {failed}.\nResults:\n{}",
        diagnostic_dump(&results),
    );
}

#[test]
fn test_real_prod_cost_over_time_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "get_spend_over_time",
        COST_OVER_TIME_PROD,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "real-prod cost-over-time.html must produce ZERO Failed rows; got {failed}.\nResults:\n{}",
        diagnostic_dump(&results),
    );
}

#[test]
fn test_real_prod_savings_summary_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "find_savings_opportunities",
        SAVINGS_SUMMARY_PROD,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "real-prod savings-summary.html must produce ZERO Failed rows; got {failed}.\nResults:\n{}",
        diagnostic_dump(&results),
    );
}

#[test]
fn test_real_prod_tag_coverage_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "assess_tag_strategy",
        TAG_COVERAGE_PROD,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "real-prod tag-coverage.html must produce ZERO Failed rows; got {failed}.\nResults:\n{}",
        diagnostic_dump(&results),
    );
}

#[test]
fn test_real_prod_connect_account_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "connect_aws_account",
        CONNECT_ACCOUNT_PROD,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "real-prod connect-account.html must produce ZERO Failed rows; got {failed}.\nResults:\n{}",
        diagnostic_dump(&results),
    );
}

#[test]
fn test_real_prod_service_sankey_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "get_service_breakdown",
        SERVICE_SANKEY_PROD,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "real-prod service-sankey.html must produce ZERO Failed rows; got {failed}.\nResults:\n{}",
        diagnostic_dump(&results),
    );
}

#[test]
fn test_real_prod_no_regression_on_cycle1_synthetic_fixtures() {
    // Cycle-1 synthetic fixtures (Plan 78-05) are handled by the post-
    // Plan-06 validator. Plan 78-10's regex generalization must NOT
    // regress them. This test passes today and must continue to pass
    // after Plan 78-10 lands.
    for (label, tool_name, html, expected_failed) in [
        (
            "cost_summary_minified",
            "cost-coach-cost-summary",
            COST_SUMMARY_SYNTHETIC,
            0usize,
        ),
        (
            "cost_over_time_minified",
            "cost-coach-cost-over-time",
            COST_OVER_TIME_SYNTHETIC,
            0usize,
        ),
        // synthetic_cascade is the G3 fixture: SDK + constructor missing,
        // handlers + connect present. Expected: exactly 2 Failed (SDK +
        // constructor); handlers + connect Passed; ontoolresult Passed.
        (
            "synthetic_cascade",
            "synthetic-cascade",
            SYNTHETIC_CASCADE,
            2usize,
        ),
    ] {
        let results = validate(AppValidationMode::ClaudeDesktop, tool_name, html);
        let failed = count_status(&results, TestStatus::Failed);
        assert_eq!(
            failed, expected_failed,
            "cycle-1 synthetic fixture `{label}` regressed: expected {expected_failed} Failed, got {failed}.\nResults:\n{}",
            diagnostic_dump(&results),
        );
    }
}
