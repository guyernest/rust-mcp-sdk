//! Tools domain conformance scenarios.
//!
//! Validates: tools/list, tool schema, tools/call on existing tool,
//! tools/call on unknown tool. Capability-conditional.

use crate::report::TestResult;
use crate::tester::ServerTester;

/// Run all tools conformance scenarios.
/// Skipped if server does not advertise tools capability.
pub async fn run_tools_conformance(_tester: &mut ServerTester) -> Vec<TestResult> {
    // Implemented in Task 2
    Vec::new()
}
