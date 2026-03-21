//! Tasks domain conformance scenarios.
//!
//! Validates: tasks capability structure, task creation via tools/call,
//! tasks/get, task status transitions. Fully capability-conditional.

use crate::report::TestResult;
use crate::tester::ServerTester;

/// Run all tasks conformance scenarios.
/// Skipped if server does not advertise tasks capability.
pub async fn run_tasks_conformance(_tester: &mut ServerTester) -> Vec<TestResult> {
    // Implemented in Task 3
    Vec::new()
}
