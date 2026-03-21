//! Resources domain conformance scenarios.
//!
//! Validates: resources/list, resources/read on first resource,
//! resources/read on invalid URI. Capability-conditional.

use crate::report::TestResult;
use crate::tester::ServerTester;

/// Run all resources conformance scenarios.
/// Skipped if server does not advertise resources capability.
pub async fn run_resources_conformance(_tester: &mut ServerTester) -> Vec<TestResult> {
    // Implemented in Task 3
    Vec::new()
}
