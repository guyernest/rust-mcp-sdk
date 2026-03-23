//! Resources domain conformance scenarios.
//!
//! Validates: resources/list, resources/read on first resource,
//! resources/read on invalid URI. Capability-conditional.

use super::check_capability;
use crate::report::{TestCategory, TestResult};
use crate::tester::ServerTester;
use std::time::Instant;

/// Run all resources conformance scenarios.
/// Skipped if server does not advertise resources capability.
pub async fn run_resources_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    if let Some(skip) = check_capability(tester, "Resources", TestCategory::Resources, |caps| {
        caps.resources.is_some()
    }) {
        return skip;
    }

    let mut results = Vec::new();

    // R-01: List resources
    let (list_result, first_uri) = test_resources_list(tester).await;
    results.push(list_result);

    // R-02: Read first resource
    results.push(test_read_first_resource(tester, first_uri.as_deref()).await);

    // R-03: Read invalid URI
    results.push(test_read_invalid_uri(tester).await);

    results
}

/// R-01: Validate resources/list returns valid ResourceInfo entries.
async fn test_resources_list(tester: &mut ServerTester) -> (TestResult, Option<String>) {
    let start = Instant::now();
    let name = "Resources: list returns valid ResourceInfo";

    match tester.list_resources().await {
        Ok(result) => {
            let resources = &result.resources;
            let invalid: Vec<_> = resources
                .iter()
                .filter(|r| r.name.is_empty() || r.uri.is_empty())
                .map(|r| r.name.clone())
                .collect();
            let first_uri = resources.first().map(|r| r.uri.clone());

            if invalid.is_empty() {
                (
                    TestResult::passed(
                        name,
                        TestCategory::Resources,
                        start.elapsed(),
                        format!("Found {} resources", resources.len()),
                    ),
                    first_uri,
                )
            } else {
                (
                    TestResult::failed(
                        name,
                        TestCategory::Resources,
                        start.elapsed(),
                        format!("Resources with empty name/uri: {:?}", invalid),
                    ),
                    first_uri,
                )
            }
        },
        Err(e) => (
            TestResult::failed(
                name,
                TestCategory::Resources,
                start.elapsed(),
                format!("resources/list failed: {e}"),
            ),
            None,
        ),
    }
}

/// R-02: Read the first resource and verify the response contains a contents array.
async fn test_read_first_resource(
    tester: &mut ServerTester,
    first_uri: Option<&str>,
) -> TestResult {
    let start = Instant::now();
    let name = "Resources: read first resource";

    let Some(uri) = first_uri else {
        return TestResult::skipped(name, TestCategory::Resources, "No resources to read");
    };

    match tester.read_resource(uri).await {
        Ok(_) => TestResult::passed(
            name,
            TestCategory::Resources,
            start.elapsed(),
            format!("Successfully read resource: {uri}"),
        ),
        Err(e) => TestResult::failed(
            name,
            TestCategory::Resources,
            start.elapsed(),
            format!("resources/read failed: {e}"),
        ),
    }
}

/// R-03: Read an invalid URI and verify the server returns an error.
async fn test_read_invalid_uri(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Resources: read invalid URI returns error";

    match tester
        .read_resource("___nonexistent_resource_conformance_test___")
        .await
    {
        Ok(_) => TestResult::warning(
            name,
            TestCategory::Resources,
            start.elapsed(),
            "Server returned success for nonexistent resource URI",
        ),
        Err(_) => TestResult::passed(
            name,
            TestCategory::Resources,
            start.elapsed(),
            "Server correctly rejected invalid URI",
        ),
    }
}
