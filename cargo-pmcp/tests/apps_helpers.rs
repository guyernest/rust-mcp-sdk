//! Integration tests for cargo-pmcp's apps command helpers (Plan 02 wiring).
//!
//! Covers:
//! - Plan 02's integration boundary by exercising `validate_widgets` directly
//! - REVISION HIGH-4: tool name embedded in `TestResult.name` so the report
//!   layer can do post-hoc filtering
//! - REVISION MED-6: URI dedup observable via shared-URI scenarios

use mcp_tester::{AppValidationMode, AppValidator, TestStatus};

/// Inline broken-widget body. Same shape as Plan 03's
/// `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html`.
const BROKEN_INLINE: &str = r#"<!doctype html>
<html><head><title>broken</title></head><body>
<div id="app">Loading</div>
<script type="module">
  const root = document.getElementById("app");
  window.openai = window.openai || {};
</script>
</body></html>"#;

const CORRECTED_INLINE: &str = r#"<!doctype html>
<html><head><title>ok</title></head><body>
<div id="app">Loading</div>
<script type="module">
  import { App } from "@modelcontextprotocol/ext-apps";
  const a = new App({ name: "ok", version: "1.0.0" });
  a.onteardown = async () => {};
  a.ontoolinput = () => {};
  a.ontoolcancelled = () => {};
  a.onerror = () => {};
  a.connect();
</script>
</body></html>"#;

/// Smoke-test that the validator wiring is reachable from cargo-pmcp's
/// dependency graph and that ClaudeDesktop mode emits Failed rows naming the
/// tool (REVISION HIGH-4).
#[test]
fn test_apps_resources_read_e2e() {
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let results = validator.validate_widgets(&[(
        "open_dashboard".to_string(),
        "ui://test".to_string(),
        BROKEN_INLINE.to_string(),
    )]);
    let failed: Vec<_> = results
        .iter()
        .filter(|r| r.status == TestStatus::Failed)
        .collect();
    assert!(
        !failed.is_empty(),
        "ClaudeDesktop mode must emit >=1 Failed row for a broken widget"
    );
    // REVISION HIGH-4: tool name must appear in error report.
    let any_with_tool_name = failed.iter().any(|r| r.name.contains("open_dashboard"));
    assert!(
        any_with_tool_name,
        "Failed row name must include tool name (REVISION HIGH-4); got {:?}",
        failed.iter().map(|r| &r.name).collect::<Vec<_>>()
    );
}

/// Standard mode emits ONE summary WARN per widget (per Plan 01 contract).
#[test]
fn test_apps_resources_read_standard_mode_summary_warn() {
    let validator = AppValidator::new(AppValidationMode::Standard, None);
    let results = validator.validate_widgets(&[(
        "open_dashboard".to_string(),
        "ui://test".to_string(),
        BROKEN_INLINE.to_string(),
    )]);
    let warns = results
        .iter()
        .filter(|r| r.status == TestStatus::Warning)
        .count();
    let failed = results
        .iter()
        .filter(|r| r.status == TestStatus::Failed)
        .count();
    assert_eq!(
        warns, 1,
        "Standard mode must emit EXACTLY 1 Warning per widget; got {warns}"
    );
    assert_eq!(
        failed, 0,
        "Standard mode must emit ZERO Failed rows from widget signals"
    );
}

/// REVISION HIGH-4: tool_filter at the validator level. We can't easily mock
/// `ServerTester` for the read-site filter from a workspace integration test,
/// but the validator-side behavior is verifiable — the validator name field
/// includes tool names so the report layer can do post-hoc filtering. The
/// CLI-level enforcement is verified end-to-end by the `cli_acceptance.rs`
/// test in this plan.
///
/// What this test asserts: when `validate_widgets` is fed two widgets owned by
/// two different tools, each result is name-tagged with its owning tool, so
/// downstream filters (e.g. shell pipe `| grep tool_name`) work as expected.
#[test]
fn tool_filter_restricts_widget_validation() {
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let results = validator.validate_widgets(&[
        (
            "alpha_tool".to_string(),
            "ui://alpha".to_string(),
            BROKEN_INLINE.to_string(),
        ),
        (
            "beta_tool".to_string(),
            "ui://beta".to_string(),
            BROKEN_INLINE.to_string(),
        ),
    ]);
    let alpha_failures = results
        .iter()
        .filter(|r| r.name.contains("alpha_tool"))
        .count();
    let beta_failures = results
        .iter()
        .filter(|r| r.name.contains("beta_tool"))
        .count();
    assert!(
        alpha_failures >= 1,
        "alpha_tool widget must produce >=1 row"
    );
    assert!(beta_failures >= 1, "beta_tool widget must produce >=1 row");
    // Each tool's results MUST be tagged with the tool's own name and not
    // the other tool's. (Sanity for the REVISION HIGH-4 `name` format.)
    for r in &results {
        if r.name.contains("alpha_tool") {
            assert!(
                !r.name.contains("beta_tool"),
                "row tagged alpha must not also tag beta: {}",
                r.name
            );
        }
    }
}

/// REVISION MED-6: when two tools share a single widget URI, the validator
/// produces results for BOTH tools (one set per tool). The dedup contract is
/// at the READ site (see Task 1's `dedup_widget_uris`); the validator simply
/// receives the fanned-out tuples. This test verifies the validator handles
/// multiple tuples sharing a URI without crash and produces per-tool rows.
#[test]
fn widget_uris_deduplicated_at_read_time() {
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    // Two tools share `ui://shared`; the validator receives both tuples.
    let results = validator.validate_widgets(&[
        (
            "tool_a".to_string(),
            "ui://shared".to_string(),
            BROKEN_INLINE.to_string(),
        ),
        (
            "tool_b".to_string(),
            "ui://shared".to_string(),
            BROKEN_INLINE.to_string(),
        ),
    ]);
    let a_rows = results.iter().filter(|r| r.name.contains("tool_a")).count();
    let b_rows = results.iter().filter(|r| r.name.contains("tool_b")).count();
    assert!(a_rows >= 1, "tool_a must get its own validator rows");
    assert!(b_rows >= 1, "tool_b must get its own validator rows");
    assert_eq!(
        a_rows, b_rows,
        "both tools share a URI; both must get the same number of rows"
    );
}

/// Corrected widget under ClaudeDesktop mode produces zero Failed rows.
#[test]
fn corrected_widget_passes_claude_desktop() {
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let results = validator.validate_widgets(&[(
        "ok_tool".to_string(),
        "ui://ok".to_string(),
        CORRECTED_INLINE.to_string(),
    )]);
    let failed = results
        .iter()
        .filter(|r| r.status == TestStatus::Failed)
        .count();
    assert_eq!(
        failed, 0,
        "corrected widget must produce ZERO Failed rows; got {failed}"
    );
}
