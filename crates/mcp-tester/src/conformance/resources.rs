//! Resources domain conformance scenarios.
//!
//! Validates: resources/list, resources/read on first resource,
//! resources/read on invalid URI. Capability-conditional.

use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::ServerTester;
use std::time::Instant;

/// Run all resources conformance scenarios.
/// Skipped if server does not advertise resources capability.
pub async fn run_resources_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    // Check capability via public getter
    let has_resources = tester
        .server_capabilities()
        .map_or(false, |caps| caps.resources.is_some());

    if !has_resources {
        return vec![TestResult {
            name: "Resources: capability not advertised".to_string(),
            category: TestCategory::Resources,
            status: TestStatus::Skipped,
            duration: std::time::Duration::from_secs(0),
            error: None,
            details: Some("Server does not advertise resources capability".to_string()),
        }];
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

    match tester.list_resources().await {
        Ok(result) => {
            let resources = &result.resources;

            // Verify each resource has non-empty name and uri
            let invalid: Vec<_> = resources
                .iter()
                .filter(|r| r.name.is_empty() || r.uri.is_empty())
                .map(|r| r.name.clone())
                .collect();

            let first_uri = resources.first().map(|r| r.uri.clone());

            if invalid.is_empty() {
                (
                    TestResult {
                        name: "Resources: list returns valid ResourceInfo".to_string(),
                        category: TestCategory::Resources,
                        status: TestStatus::Passed,
                        duration: start.elapsed(),
                        error: None,
                        details: Some(format!("Found {} resources", resources.len())),
                    },
                    first_uri,
                )
            } else {
                (
                    TestResult {
                        name: "Resources: list returns valid ResourceInfo".to_string(),
                        category: TestCategory::Resources,
                        status: TestStatus::Failed,
                        duration: start.elapsed(),
                        error: Some(format!(
                            "Resources with empty name/uri: {:?}",
                            invalid
                        )),
                        details: None,
                    },
                    first_uri,
                )
            }
        },
        Err(e) => (
            TestResult {
                name: "Resources: list returns valid ResourceInfo".to_string(),
                category: TestCategory::Resources,
                status: TestStatus::Failed,
                duration: start.elapsed(),
                error: Some(format!("resources/list failed: {e}")),
                details: None,
            },
            None,
        ),
    }
}

/// R-02: Read the first resource and verify the response contains a contents array.
async fn test_read_first_resource(tester: &mut ServerTester, first_uri: Option<&str>) -> TestResult {
    let start = Instant::now();

    let Some(uri) = first_uri else {
        return TestResult {
            name: "Resources: read first resource".to_string(),
            category: TestCategory::Resources,
            status: TestStatus::Skipped,
            duration: start.elapsed(),
            error: None,
            details: Some("No resources to read".to_string()),
        };
    };

    match tester.read_resource(uri).await {
        Ok(_result) => {
            // ReadResourceResult exists with contents -- valid response
            TestResult {
                name: "Resources: read first resource".to_string(),
                category: TestCategory::Resources,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some(format!("Successfully read resource: {uri}")),
            }
        },
        Err(e) => TestResult {
            name: "Resources: read first resource".to_string(),
            category: TestCategory::Resources,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some(format!("resources/read failed: {e}")),
            details: None,
        },
    }
}

/// R-03: Read an invalid URI and verify the server returns an error.
async fn test_read_invalid_uri(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();

    match tester
        .read_resource("___nonexistent_resource_conformance_test___")
        .await
    {
        Ok(_) => {
            // Unexpected success for invalid URI
            TestResult {
                name: "Resources: read invalid URI returns error".to_string(),
                category: TestCategory::Resources,
                status: TestStatus::Warning,
                duration: start.elapsed(),
                error: None,
                details: Some(
                    "Server returned success for nonexistent resource URI".to_string(),
                ),
            }
        },
        Err(_) => {
            // Error is the expected behavior
            TestResult {
                name: "Resources: read invalid URI returns error".to_string(),
                category: TestCategory::Resources,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some("Server correctly rejected invalid URI".to_string()),
            }
        },
    }
}
