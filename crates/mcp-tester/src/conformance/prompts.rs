//! Prompts domain conformance scenarios.
//!
//! Validates: prompts/list, prompts/get on first prompt,
//! prompts/get on unknown prompt. Capability-conditional.

use crate::report::TestResult;
use crate::tester::ServerTester;

/// Run all prompts conformance scenarios.
/// Skipped if server does not advertise prompts capability.
pub async fn run_prompts_conformance(_tester: &mut ServerTester) -> Vec<TestResult> {
    // Implemented in Task 3
    Vec::new()
}
