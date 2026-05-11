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

    /// G3 invariant: handler detection is INDEPENDENT of SDK-signal presence.
    ///
    /// For any combination of SDK-presence signals (present or absent) and any
    /// subset of the four required handler member-assignments (0..=4), the
    /// validator must report `Passed` for EXACTLY the handlers that were
    /// injected and `Failed` for those that were not. This holds regardless of
    /// whether any SDK-presence signal fires — i.e. there is no SDK→handler
    /// cascade.
    ///
    /// This is the property-level encoding of `78-VERIFICATION.md` Gap G3:
    /// "SDK-detection failure cascades to all 8 handler/connect checks".
    ///
    /// Search space: 8 booleans -> 256 input shapes per proptest run.
    #[test]
    fn prop_g3_handler_detection_independent_of_sdk(
        // 4 booleans: include each handler member-assignment?
        include_onteardown in any::<bool>(),
        include_ontoolinput in any::<bool>(),
        include_ontoolcancelled in any::<bool>(),
        include_onerror in any::<bool>(),
        // 4 booleans: include each SDK-presence signal?
        include_log_prefix in any::<bool>(),
        include_method_initialize in any::<bool>(),
        include_method_tool_result in any::<bool>(),
        include_legacy_import in any::<bool>(),
    ) {
        // Build a synthetic widget body from the booleans.
        let mut script = String::from("\"use strict\";");
        if include_log_prefix {
            script.push_str(r#"console.log("[ext-apps] boot");"#);
        }
        if include_method_initialize {
            script.push_str(r#"var m="ui/initialize";"#);
        }
        if include_method_tool_result {
            script.push_str(r#"var n="ui/notifications/tool-result";"#);
        }
        if include_legacy_import {
            // Synthesize the literal in a non-import context (e.g. a string
            // literal) so the SDK-presence signal fires without requiring
            // valid JS module syntax inside the proptest body.
            script.push_str(r#"var p="@modelcontextprotocol/ext-apps";"#);
        }
        script.push_str("var obj={};");
        if include_onteardown {
            script.push_str("obj.onteardown=async()=>({});");
        }
        if include_ontoolinput {
            script.push_str("obj.ontoolinput=function(p){};");
        }
        if include_ontoolcancelled {
            script.push_str("obj.ontoolcancelled=function(p){};");
        }
        if include_onerror {
            script.push_str("obj.onerror=function(e){};");
        }
        let html = format!("<html><body><script>{script}</script></body></html>");

        let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
        let results = validator.validate_widgets(&[(
            "prop-g3".to_string(),
            "ui://prop-g3".to_string(),
            html,
        )]);

        let handler_status = |name: &str| -> Option<TestStatus> {
            results
                .iter()
                .find(|r| r.name.contains(&format!("handler: {name}")))
                .map(|r| r.status.clone())
        };

        // Each handler row must exist AND its status must match injection.
        // Inlined (not closured) so prop_assert_eq! returns directly from
        // the test body — proptest's `?` propagation from nested closures
        // is brittle across versions.
        let s_onteardown = handler_status("onteardown")
            .expect("handler row for onteardown must exist");
        if include_onteardown {
            prop_assert_eq!(s_onteardown, TestStatus::Passed,
                "onteardown should be Passed when injected (regardless of SDK signals)");
        } else {
            prop_assert_eq!(s_onteardown, TestStatus::Failed,
                "onteardown should be Failed when not injected");
        }

        let s_ontoolinput = handler_status("ontoolinput")
            .expect("handler row for ontoolinput must exist");
        if include_ontoolinput {
            prop_assert_eq!(s_ontoolinput, TestStatus::Passed,
                "ontoolinput should be Passed when injected (regardless of SDK signals)");
        } else {
            prop_assert_eq!(s_ontoolinput, TestStatus::Failed,
                "ontoolinput should be Failed when not injected");
        }

        let s_ontoolcancelled = handler_status("ontoolcancelled")
            .expect("handler row for ontoolcancelled must exist");
        if include_ontoolcancelled {
            prop_assert_eq!(s_ontoolcancelled, TestStatus::Passed,
                "ontoolcancelled should be Passed when injected (regardless of SDK signals)");
        } else {
            prop_assert_eq!(s_ontoolcancelled, TestStatus::Failed,
                "ontoolcancelled should be Failed when not injected");
        }

        let s_onerror = handler_status("onerror")
            .expect("handler row for onerror must exist");
        if include_onerror {
            prop_assert_eq!(s_onerror, TestStatus::Passed,
                "onerror should be Passed when injected (regardless of SDK signals)");
        } else {
            prop_assert_eq!(s_onerror, TestStatus::Failed,
                "onerror should be Failed when not injected");
        }
    }

    /// G2 cycle-2 false-positive guard: the widened constructor regex
    /// `[^}]{0,200}\bname\s*:[^,}]{0,100},\s*version\s*:` must NOT match
    /// arbitrary `new <Class>({<key1>:<val1>,<key2>:<val2>})` shapes when
    /// neither key is `name`+`version`.
    ///
    /// This guards against the Plan 78-10 Task 1 widening over-matching
    /// benign code that uses `new SomeClass({...})` with arbitrary keys.
    /// The regex requires `name` AND `version` keys in adjacent positions
    /// — non-`name`/`version` keys must not satisfy the constructor signal.
    ///
    /// Search space: ~12 keys × ~12 keys × class-name space. Default
    /// proptest config (256 cases) covers a wide swath.
    #[test]
    fn prop_g2_cycle2_no_false_positive_on_unrelated_keys(
        class_name in "[A-Za-z][A-Za-z0-9]{0,15}",
        key1 in "(foo|bar|baz|qux|year|month|day|color|size|width|height|tag)",
        val1 in "[A-Za-z0-9]{1,10}",
        key2 in "(foo|bar|baz|qux|year|month|day|color|size|width|height|tag)",
        val2 in "[A-Za-z0-9]{1,10}",
    ) {
        let html = format!(
            "<html><body><script>var i = new {class_name}({{{key1}:\"{val1}\",{key2}:\"{val2}\"}});</script></body></html>"
        );
        let validator = AppValidator::new(AppValidationMode::ClaudeDesktop, None);
        let results = validator.validate_widgets(&[(
            "prop-g2-cycle2".to_string(),
            "ui://prop-g2-cycle2".to_string(),
            html,
        )]);
        let ctor_row = results.iter().find(|r| r.name.contains("App constructor"));
        prop_assert!(
            ctor_row.is_some(),
            "App constructor row must be emitted under ClaudeDesktop mode",
        );
        prop_assert_eq!(
            ctor_row.unwrap().status.clone(),
            TestStatus::Failed,
            "G2 cycle-2: must NOT match new <Class>({{<key1>,<key2>}}) without name+version keys",
        );
    }
}
