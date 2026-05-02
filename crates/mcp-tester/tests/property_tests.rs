//! Property-based tests for `AppValidator::validate_widgets`.

use mcp_tester::{AppValidationMode, AppValidator, TestStatus};
use proptest::prelude::*;

fn run_validator(html: &str) -> Vec<TestStatus> {
    let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
    validator
        .validate_widgets(&[(
            "prop-tool".to_string(),
            "ui://prop-test".to_string(),
            html.to_string(),
        )])
        .into_iter()
        .map(|r| r.status)
        .collect()
}

proptest! {
    #[test]
    fn prop_scan_never_panics(html in r"\PC{0,4096}") {
        let _ = run_validator(&html);
    }

    #[test]
    fn prop_whitespace_idempotent(html in r"[a-zA-Z<>/= .]{0,500}") {
        let s1 = run_validator(&html);
        let html2 = html.replace(' ', "  ").replace('\n', "\n\n");
        let s2 = run_validator(&html2);
        prop_assert_eq!(s1.len(), s2.len(), "result vec length must be stable under whitespace doubling");
        let count1_failed = s1.iter().filter(|s| **s == TestStatus::Failed).count();
        let count2_failed = s2.iter().filter(|s| **s == TestStatus::Failed).count();
        prop_assert_eq!(count1_failed, count2_failed, "Failed count must match across whitespace-doubled bodies");
        let count1_warn = s1.iter().filter(|s| **s == TestStatus::Warning).count();
        let count2_warn = s2.iter().filter(|s| **s == TestStatus::Warning).count();
        prop_assert_eq!(count1_warn, count2_warn, "Warning count must match across whitespace-doubled bodies");
    }
}
