//! Core domain conformance scenarios.
//!
//! Validates: initialize handshake, protocol version, server info,
//! capabilities structure, unknown method error, malformed request.

use crate::report::TestResult;
use crate::tester::ServerTester;

/// Run all core conformance scenarios.
/// Core domain handles initialization -- must run before other domains.
pub async fn run_core_conformance(_tester: &mut ServerTester) -> Vec<TestResult> {
    // Implemented in Task 2
    Vec::new()
}
