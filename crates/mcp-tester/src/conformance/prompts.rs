//! Prompts domain conformance scenarios.
//!
//! Validates: prompts/list, prompts/get on first prompt,
//! prompts/get on unknown prompt. Capability-conditional.

use super::check_capability;
use crate::report::{TestCategory, TestResult};
use crate::tester::ServerTester;
use serde_json::json;
use std::time::Instant;

/// Run all prompts conformance scenarios.
/// Skipped if server does not advertise prompts capability.
pub async fn run_prompts_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    if let Some(skip) = check_capability(tester, "Prompts", TestCategory::Prompts, |caps| caps.prompts.is_some()) {
        return skip;
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
    let name = "Prompts: list returns valid PromptInfo";

    match tester.list_prompts().await {
        Ok(result) => {
            let prompts = &result.prompts;
            let invalid: Vec<_> = prompts.iter().filter(|p| p.name.is_empty()).map(|p| p.name.clone()).collect();
            let first_name = prompts.first().map(|p| p.name.clone());

            if invalid.is_empty() {
                (TestResult::passed(name, TestCategory::Prompts, start.elapsed(), format!("Found {} prompts", prompts.len())), first_name)
            } else {
                (TestResult::failed(name, TestCategory::Prompts, start.elapsed(), format!("{} prompts have empty names", invalid.len())), first_name)
            }
        },
        Err(e) => (TestResult::failed(name, TestCategory::Prompts, start.elapsed(), format!("prompts/list failed: {e}")), None),
    }
}

/// P-02: Get the first prompt with empty arguments.
/// Some prompts require arguments, so an error is treated as Warning, not failure.
async fn test_get_first_prompt(tester: &mut ServerTester, first_name: Option<&str>) -> TestResult {
    let start = Instant::now();
    let name = "Prompts: get first prompt";

    let Some(prompt_name) = first_name else {
        return TestResult::skipped(name, TestCategory::Prompts, "No prompts to get");
    };

    match tester.get_prompt(prompt_name, json!({})).await {
        Ok(result) => TestResult::passed(name, TestCategory::Prompts, start.elapsed(), format!("Prompt '{}' returned {} messages", prompt_name, result.messages.len())),
        Err(e) => TestResult::warning(name, TestCategory::Prompts, start.elapsed(), format!("Prompt '{prompt_name}' returned error (may require arguments): {e}")),
    }
}

/// P-03: Get a nonexistent prompt and verify error response.
async fn test_get_unknown_prompt(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Prompts: get unknown prompt returns error";

    match tester.get_prompt("___nonexistent_prompt_conformance_test___", json!({})).await {
        Ok(_) => TestResult::warning(name, TestCategory::Prompts, start.elapsed(), "Server returned success for nonexistent prompt"),
        Err(_) => TestResult::passed(name, TestCategory::Prompts, start.elapsed(), "Server correctly rejected unknown prompt"),
    }
}
