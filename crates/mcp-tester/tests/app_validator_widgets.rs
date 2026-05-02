//! Integration tests for `AppValidator::validate_widgets`.
//!
//! Mode-driven emission shape (per RESEARCH Q4 RESOLVED — three-way split):
//! - ClaudeDesktop: one Failed row per missing signal/handler
//! - Standard: ONE summary Warning row per widget
//! - ChatGpt: ZERO widget-related rows (REVISION HIGH-1)
//!
//! Tuple shape: `(tool_name, uri, html)` per REVISION HIGH-4.

use mcp_tester::{AppValidationMode, AppValidator, TestStatus};

const BROKEN_NO_SDK: &str = include_str!("fixtures/widgets/broken_no_sdk.html");
const BROKEN_NO_HANDLERS: &str = include_str!("fixtures/widgets/broken_no_handlers.html");
const CORRECTED_MINIMAL: &str = include_str!("fixtures/widgets/corrected_minimal.html");

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

#[test]
fn test_broken_widget_fails_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "cost-coach",
        BROKEN_NO_SDK,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert!(
        failed >= 1,
        "broken_no_sdk.html must produce at least 1 Failed row in claude-desktop mode (got {failed})"
    );
    let any_handler_named = results.iter().any(|r| r.name.contains("onteardown"));
    assert!(
        any_handler_named,
        "must produce a row whose name names a missing handler"
    );
    // REVISION HIGH-4: every Failed row's name must contain the tool name.
    for r in results.iter().filter(|r| r.status == TestStatus::Failed) {
        assert!(
            r.name.contains("cost-coach"),
            "Failed row name must include tool name (REVISION HIGH-4): {}",
            r.name
        );
    }
}

#[test]
fn test_broken_widget_fails_claude_desktop_no_handlers() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "cost-coach",
        BROKEN_NO_HANDLERS,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert!(
        failed >= 4,
        "broken_no_handlers.html must produce >=4 Failed rows (one per missing handler) (got {failed})"
    );
}

#[test]
fn test_corrected_widget_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "cost-coach",
        CORRECTED_MINIMAL,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "corrected_minimal.html must produce ZERO Failed rows in claude-desktop mode; got {failed}: {:?}",
        results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .map(|r| &r.name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_standard_mode_one_summary_warn_for_broken() {
    let results = validate(AppValidationMode::Standard, "cost-coach", BROKEN_NO_SDK);
    let failed = count_status(&results, TestStatus::Failed);
    let warnings = count_status(&results, TestStatus::Warning);
    assert_eq!(
        failed, 0,
        "standard mode must NOT emit Failed rows from widget signals"
    );
    assert_eq!(
        warnings, 1,
        "standard mode must emit EXACTLY 1 summary Warning per widget (got {warnings})"
    );
}

#[test]
fn test_corrected_widget_passes_standard_too() {
    let results = validate(AppValidationMode::Standard, "cost-coach", CORRECTED_MINIMAL);
    let failed = count_status(&results, TestStatus::Failed);
    let warnings = count_status(&results, TestStatus::Warning);
    assert_eq!(
        failed, 0,
        "standard mode must NOT emit Failed rows on the corrected widget"
    );
    assert_eq!(
        warnings, 0,
        "standard mode must NOT emit Warning rows on the corrected widget"
    );
}

/// REVISION HIGH-1 LOAD-BEARING TEST: chatgpt mode must emit EXACTLY zero
/// widget-related results — not "no Failed rows" by some weaker measure, but
/// `results.len() == 0` against a broken widget that under ClaudeDesktop
/// would emit many rows. This is the tightened assertion REVIEWS demanded.
#[test]
fn test_chatgpt_mode_unchanged_zero_results() {
    let results = validate(AppValidationMode::ChatGpt, "cost-coach", BROKEN_NO_SDK);
    assert_eq!(
        results.len(),
        0,
        "REVISION HIGH-1: chatgpt mode must emit EXACTLY zero widget-related rows (got {} rows: {:?})",
        results.len(),
        results
            .iter()
            .map(|r| (&r.name, &r.status))
            .collect::<Vec<_>>(),
    );
}

/// REVISION HIGH-1 corollary: chatgpt mode is a no-op regardless of widget
/// shape — a fully corrected widget also yields zero results.
#[test]
fn test_chatgpt_mode_zero_results_corrected_too() {
    let results = validate(AppValidationMode::ChatGpt, "cost-coach", CORRECTED_MINIMAL);
    assert_eq!(
        results.len(),
        0,
        "REVISION HIGH-1: chatgpt mode is a no-op regardless of widget shape (got {} rows)",
        results.len()
    );
}
