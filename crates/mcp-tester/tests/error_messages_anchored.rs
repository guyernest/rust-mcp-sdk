//! End-to-end test: validator emits `[guide:SLUG]` tokens, the report printer
//! expander turns them into absolute GitHub URLs.
//!
//! Phase 78 Plan 04 Task 4 — integration test asserting the contract between
//! `AppValidator::validate_widgets` (Plan 01) and `report::expand_guide_anchor`
//! (Plan 04 Task 1). Also asserts REVISION HIGH-4: every Failed row's `name`
//! field includes the tool name passed in the tuple.

use mcp_tester::{expand_guide_anchor, AppValidationMode, AppValidator, TestStatus};

const BROKEN_INLINE: &str = r#"<!doctype html>
<html><head><title>broken</title></head><body>
<div id="app">Loading</div>
<script type="module">
  const root = document.getElementById("app");
  window.openai = window.openai || {};
</script>
</body></html>"#;

#[test]
fn error_messages_anchored() {
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let results = validator.validate_widgets(&[(
        "open_dashboard".to_string(),
        "ui://test".to_string(),
        BROKEN_INLINE.to_string(),
    )]);
    let any_expanded_to_url = results.iter().filter_map(|r| r.details.as_ref()).any(|d| {
        let expanded = expand_guide_anchor(d);
        expanded.contains(
            "https://github.com/paiml/rust-mcp-sdk/blob/main/src/server/mcp_apps/GUIDE.md#handlers-before-connect",
        )
    });
    assert!(
        any_expanded_to_url,
        "At least one validation result must emit an [guide:handlers-before-connect] token \
         that expand_guide_anchor turns into the canonical GUIDE.md URL."
    );
}

#[test]
fn no_orphaned_guide_tokens() {
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    let results = validator.validate_widgets(&[(
        "open_dashboard".to_string(),
        "ui://test".to_string(),
        BROKEN_INLINE.to_string(),
    )]);
    for r in &results {
        if let Some(d) = &r.details {
            let expanded = expand_guide_anchor(d);
            assert!(
                !expanded.contains("[guide:"),
                "Result {:?} contains an unexpanded [guide:...] token after expander pass; \
                 every slug emitted by validator must appear in expand_guide_anchor's \
                 KNOWN_SLUGS list. Token block: {}",
                r.name,
                expanded
            );
        }
    }
}

/// REVISION HIGH-4: every Failed row's `name` field must include the tool
/// name (`open_dashboard`), so error reports identify which tool the widget
/// belongs to.
#[test]
fn error_messages_include_tool_name() {
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
    assert!(!failed.is_empty(), "must produce >=1 Failed row");
    for r in &failed {
        assert!(
            r.name.contains("open_dashboard"),
            "REVISION HIGH-4: Failed row name must include tool name; got: {}",
            r.name
        );
    }
}
