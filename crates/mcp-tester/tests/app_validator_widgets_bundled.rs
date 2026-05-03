//! RED-phase regression tests for Phase 78 gap closure (G1+G2+G3).
//!
//! These tests encode the cost-coach Vite singlefile false-positive class
//! captured in feedback report `feedback-pmcp-test-apps-v1-false-positives.md`
//! (2026-05-02). They drive the bundled fixtures in
//! `tests/fixtures/widgets/bundled/` and assert the post-fix verdict shape.
//!
//! Until plan 78-06 lands, these tests are EXPECTED TO FAIL:
//!   - test_cost_summary_minified_passes_claude_desktop (G1+G2)
//!   - test_cost_over_time_minified_passes_claude_desktop (G1+G2)
//!   - test_synthetic_cascade_no_handler_cascade_when_sdk_absent (G3)
//!   - test_bundled_fixtures_pass_standard_mode (G1+G2 — first 2 fixtures)
//!
//! `test_bundled_fixtures_zero_results_chatgpt_mode` passes today (chatgpt
//! mode is a no-op per REVISION HIGH-1).
//!
//! After plan 06 lands, ALL FIVE must pass.

use mcp_tester::{AppValidationMode, AppValidator, TestStatus};

const COST_SUMMARY_MINIFIED: &str =
    include_str!("fixtures/widgets/bundled/cost_summary_minified.html");
const COST_OVER_TIME_MINIFIED: &str =
    include_str!("fixtures/widgets/bundled/cost_over_time_minified.html");
const SYNTHETIC_CASCADE_REPRO: &str =
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

/// G1+G2 RED: minified cost-summary widget must produce ZERO Failed rows in
/// claude-desktop mode after Plan 06 lands G1 (log-prefix / method-string SDK
/// signals) and G2 (mangled-id constructor regex). The fixture mirrors the
/// shape of cost-coach prod output: mangled `yl` constructor, intact
/// `{name,version}` payload, `[ext-apps]` log prefix, `ui/initialize` and
/// `ui/notifications/tool-result` method strings, all 5 handler member
/// assignments, and `connect()`. FAILS today because v1 detects the
/// constructor only via the literal `new App(` regex.
#[test]
fn test_cost_summary_minified_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "cost-coach-cost-summary",
        COST_SUMMARY_MINIFIED,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "cost_summary_minified.html must produce ZERO Failed rows in claude-desktop mode after G1+G2; got {failed}: {:?}",
        results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .map(|r| (&r.name, &r.details))
            .collect::<Vec<_>>()
    );
}

/// G2 RED across mangled-id variance: `gl` (cost-over-time) ≠ `yl`
/// (cost-summary). Same shape as test 1 — must pass after Plan 06 lands the
/// mangled-id-tolerant constructor regex.
#[test]
fn test_cost_over_time_minified_passes_claude_desktop() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "cost-coach-cost-over-time",
        COST_OVER_TIME_MINIFIED,
    );
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        0,
        "cost_over_time_minified.html must produce ZERO Failed rows in claude-desktop mode after G1+G2; got {failed}: {:?}",
        results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .map(|r| (&r.name, &r.details))
            .collect::<Vec<_>>()
    );
}

/// G3 RED — load-bearing cascade-elimination test.
///
/// Fixture has handlers + connect + ontoolresult present, but NO SDK presence
/// signals (no log prefix, no method strings, no `@modelcontextprotocol/ext-apps`)
/// and NO constructor (no `new App(` and no mangled-id `new <X>({name,version})`).
///
/// Post-G3 expected emission shape:
///   - Failed rows: exactly `MCP Apps SDK wiring` AND `App constructor` (2 rows).
///   - Passed rows: `handler: onteardown`, `handler: ontoolinput`,
///     `handler: ontoolcancelled`, `handler: onerror`, `handler: ontoolresult`,
///     `connect() call` (6 rows).
///
/// FAILS today because v1's SDK detection uses the `>=3 of 4 handlers`
/// fallback, which says SDK=present whenever any 3 handlers are written. After
/// Plan 06's G3 fix, SDK detection will require an actual SDK signal
/// independent of handler counting, so a fixture with handlers but no SDK
/// signals correctly reports SDK Failed without cascading to the handler rows.
#[test]
fn test_synthetic_cascade_no_handler_cascade_when_sdk_absent() {
    let results = validate(
        AppValidationMode::ClaudeDesktop,
        "synthetic-cascade-repro",
        SYNTHETIC_CASCADE_REPRO,
    );

    // Exact Failed count: SDK + constructor only — no cascade onto handlers/connect.
    let failed = count_status(&results, TestStatus::Failed);
    assert_eq!(
        failed,
        2,
        "G3: synthetic cascade fixture must emit EXACTLY 2 Failed rows (SDK + App constructor) — handlers, connect, ontoolresult must be detected independently. Got {failed}: {:?}",
        results
            .iter()
            .map(|r| (&r.name, &r.status))
            .collect::<Vec<_>>()
    );

    let sdk_failed = results
        .iter()
        .any(|r| r.status == TestStatus::Failed && r.name.contains("MCP Apps SDK wiring"));
    let ctor_failed = results
        .iter()
        .any(|r| r.status == TestStatus::Failed && r.name.contains("App constructor"));
    assert!(
        sdk_failed,
        "expected `MCP Apps SDK wiring` row to be Failed; results: {:?}",
        results
            .iter()
            .map(|r| (&r.name, &r.status))
            .collect::<Vec<_>>()
    );
    assert!(
        ctor_failed,
        "expected `App constructor` row to be Failed; results: {:?}",
        results
            .iter()
            .map(|r| (&r.name, &r.status))
            .collect::<Vec<_>>()
    );

    let onteardown_passed = results
        .iter()
        .any(|r| r.status == TestStatus::Passed && r.name.contains("handler: onteardown"));
    let ontoolinput_passed = results
        .iter()
        .any(|r| r.status == TestStatus::Passed && r.name.contains("handler: ontoolinput"));
    let ontoolcancelled_passed = results
        .iter()
        .any(|r| r.status == TestStatus::Passed && r.name.contains("handler: ontoolcancelled"));
    let onerror_passed = results
        .iter()
        .any(|r| r.status == TestStatus::Passed && r.name.contains("handler: onerror"));
    let ontoolresult_passed = results
        .iter()
        .any(|r| r.status == TestStatus::Passed && r.name.contains("handler: ontoolresult"));
    let connect_passed = results
        .iter()
        .any(|r| r.status == TestStatus::Passed && r.name.contains("connect() call"));

    assert!(
        onteardown_passed
            && ontoolinput_passed
            && ontoolcancelled_passed
            && onerror_passed
            && ontoolresult_passed
            && connect_passed,
        "G3: handlers + connect + ontoolresult must be detected independently of SDK signal; results: {:?}",
        results
            .iter()
            .map(|r| (&r.name, &r.status))
            .collect::<Vec<_>>()
    );
}

/// G1+G2 RED — Standard-mode bundled fixture coverage.
///
/// After Plan 06: cost_summary_minified and cost_over_time_minified are
/// correctly wired (per real cost-coach prod evidence) and must produce ZERO
/// Standard-mode rows (zero Failed AND zero Warning). synthetic_cascade_repro
/// is missing SDK + constructor so it must emit exactly 1 summary Warning.
///
/// FAILS today for the first two fixtures because v1 emits a 1-Warning
/// summary listing `new App({...})` as missing.
#[test]
fn test_bundled_fixtures_pass_standard_mode() {
    // Fully-wired fixtures: zero Standard-mode rows expected.
    for (label, html) in [
        ("cost_summary_minified.html", COST_SUMMARY_MINIFIED),
        ("cost_over_time_minified.html", COST_OVER_TIME_MINIFIED),
    ] {
        let results = validate(AppValidationMode::Standard, label, html);
        let failed = count_status(&results, TestStatus::Failed);
        let warnings = count_status(&results, TestStatus::Warning);
        assert_eq!(
            failed, 0,
            "{label}: standard mode must NOT emit Failed rows on a fully-wired bundled fixture; got {failed}: {:?}",
            results
                .iter()
                .filter(|r| r.status == TestStatus::Failed)
                .map(|r| (&r.name, &r.details))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            warnings, 0,
            "{label}: standard mode must NOT emit Warning rows on a fully-wired bundled fixture (post-G1+G2); got {warnings}: {:?}",
            results
                .iter()
                .filter(|r| r.status == TestStatus::Warning)
                .map(|r| (&r.name, &r.details))
                .collect::<Vec<_>>()
        );
    }

    // Cascade-repro fixture: 1 summary Warning expected (SDK + constructor missing).
    let results = validate(
        AppValidationMode::Standard,
        "synthetic-cascade-repro",
        SYNTHETIC_CASCADE_REPRO,
    );
    let failed = count_status(&results, TestStatus::Failed);
    let warnings = count_status(&results, TestStatus::Warning);
    assert_eq!(
        failed, 0,
        "synthetic_cascade_repro.html: standard mode must NOT emit Failed rows; got {failed}"
    );
    assert_eq!(
        warnings, 1,
        "synthetic_cascade_repro.html: standard mode must emit EXACTLY 1 summary Warning (SDK + constructor missing); got {warnings}: {:?}",
        results
            .iter()
            .map(|r| (&r.name, &r.status, &r.details))
            .collect::<Vec<_>>()
    );
}

/// AC-78-4 preservation: chatgpt mode is a no-op for widget validation.
/// Bundled fixtures must produce zero rows under ChatGpt mode (passes today
/// because the validator early-returns at the start of `validate_widgets`).
#[test]
fn test_bundled_fixtures_zero_results_chatgpt_mode() {
    for (label, html) in [
        ("cost_summary_minified.html", COST_SUMMARY_MINIFIED),
        ("cost_over_time_minified.html", COST_OVER_TIME_MINIFIED),
        ("synthetic_cascade_repro.html", SYNTHETIC_CASCADE_REPRO),
    ] {
        let results = validate(AppValidationMode::ChatGpt, label, html);
        assert_eq!(
            results.len(),
            0,
            "{label}: chatgpt mode must emit zero widget-related rows (REVISION HIGH-1); got {} rows: {:?}",
            results.len(),
            results
                .iter()
                .map(|r| (&r.name, &r.status))
                .collect::<Vec<_>>(),
        );
    }
}
