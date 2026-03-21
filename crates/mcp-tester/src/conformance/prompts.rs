//! Prompts domain conformance scenarios.
//!
//! Validates: prompts/list, prompts/get on first prompt,
//! prompts/get on unknown prompt. Capability-conditional.

use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::ServerTester;
use serde_json::json;
use std::time::Instant;

/// Run all prompts conformance scenarios.
/// Skipped if server does not advertise prompts capability.
pub async fn run_prompts_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    // Check capability via public getter
    let has_prompts = tester
        .server_capabilities()
        .map_or(false, |caps| caps.prompts.is_some());

    if !has_prompts {
        return vec![TestResult {
            name: "Prompts: capability not advertised".to_string(),
            category: TestCategory::Prompts,
            status: TestStatus::Skipped,
            duration: std::time::Duration::from_secs(0),
            error: None,
            details: Some("Server does not advertise prompts capability".to_string()),
        }];
    }

    let mut results = Vec::new();

    // P-01: List prompts
    let (list_result, first_prompt_name) = test_prompts_list(tester).await;
    results.push(list_result);

    // P-02: Get first prompt
    results.push(test_get_first_prompt(tester, first_prompt_name.as_deref()).await);

    // P-03: Get unknown prompt
    results.push(test_get_unknown_prompt(tester).await);

    results
}

/// P-01: Validate prompts/list returns valid PromptInfo entries.
async fn test_prompts_list(tester: &mut ServerTester) -> (TestResult, Option<String>) {
    let start = Instant::now();

    match tester.list_prompts().await {
        Ok(result) => {
            let prompts = &result.prompts;

            // Verify each prompt has a non-empty name
            let invalid: Vec<_> = prompts
                .iter()
                .filter(|p| p.name.is_empty())
                .map(|p| p.name.clone())
                .collect();

            let first_name = prompts.first().map(|p| p.name.clone());

            if invalid.is_empty() {
                (
                    TestResult {
                        name: "Prompts: list returns valid PromptInfo".to_string(),
                        category: TestCategory::Prompts,
                        status: TestStatus::Passed,
                        duration: start.elapsed(),
                        error: None,
                        details: Some(format!("Found {} prompts", prompts.len())),
                    },
                    first_name,
                )
            } else {
                (
                    TestResult {
                        name: "Prompts: list returns valid PromptInfo".to_string(),
                        category: TestCategory::Prompts,
                        status: TestStatus::Failed,
                        duration: start.elapsed(),
                        error: Some(format!(
                            "{} prompts have empty names",
                            invalid.len()
                        )),
                        details: None,
                    },
                    first_name,
                )
            }
        },
        Err(e) => (
            TestResult {
                name: "Prompts: list returns valid PromptInfo".to_string(),
                category: TestCategory::Prompts,
                status: TestStatus::Failed,
                duration: start.elapsed(),
                error: Some(format!("prompts/list failed: {e}")),
                details: None,
            },
            None,
        ),
    }
}

/// P-02: Get the first prompt with empty arguments.
/// Some prompts require arguments, so an error is treated as Warning, not failure.
async fn test_get_first_prompt(tester: &mut ServerTester, first_name: Option<&str>) -> TestResult {
    let start = Instant::now();

    let Some(name) = first_name else {
        return TestResult {
            name: "Prompts: get first prompt".to_string(),
            category: TestCategory::Prompts,
            status: TestStatus::Skipped,
            duration: start.elapsed(),
            error: None,
            details: Some("No prompts to get".to_string()),
        };
    };

    match tester.get_prompt(name, json!({})).await {
        Ok(result) => {
            // GetPromptResult should contain a messages array
            TestResult {
                name: "Prompts: get first prompt".to_string(),
                category: TestCategory::Prompts,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some(format!(
                    "Prompt '{}' returned {} messages",
                    name,
                    result.messages.len()
                )),
            }
        },
        Err(e) => {
            // Error may be expected if the prompt requires arguments
            TestResult {
                name: "Prompts: get first prompt".to_string(),
                category: TestCategory::Prompts,
                status: TestStatus::Warning,
                duration: start.elapsed(),
                error: None,
                details: Some(format!(
                    "Prompt '{name}' returned error (may require arguments): {e}"
                )),
            }
        },
    }
}

/// P-03: Get a nonexistent prompt and verify error response.
async fn test_get_unknown_prompt(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();

    match tester
        .get_prompt("___nonexistent_prompt_conformance_test___", json!({}))
        .await
    {
        Ok(_) => {
            // Unexpected success for nonexistent prompt
            TestResult {
                name: "Prompts: get unknown prompt returns error".to_string(),
                category: TestCategory::Prompts,
                status: TestStatus::Warning,
                duration: start.elapsed(),
                error: None,
                details: Some(
                    "Server returned success for nonexistent prompt".to_string(),
                ),
            }
        },
        Err(_) => {
            // Error is the expected behavior
            TestResult {
                name: "Prompts: get unknown prompt returns error".to_string(),
                category: TestCategory::Prompts,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some("Server correctly rejected unknown prompt".to_string()),
            }
        },
    }
}
