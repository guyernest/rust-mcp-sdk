//! Core domain conformance scenarios.
//!
//! Validates: initialize handshake, protocol version, server info,
//! capabilities structure, unknown method error, malformed request.

use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::ServerTester;
use serde_json::json;
use std::time::Instant;

/// Run all core conformance scenarios.
/// Core domain handles initialization -- must run before other domains.
pub async fn run_core_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    let mut results = Vec::new();

    // C-01: Initialize handshake
    results.push(test_initialize_handshake(tester).await);

    // If init failed, skip remaining core tests
    if results.last().map_or(false, |r| r.status == TestStatus::Failed) {
        return results;
    }

    // C-02: Protocol version validation
    results.push(test_protocol_version(tester));

    // C-03: Server info validation
    results.push(test_server_info(tester));

    // C-04: Capabilities structure
    results.push(test_capabilities_structure(tester));

    // C-05: Unknown method returns -32601
    results.push(test_unknown_method(tester).await);

    // C-06: Malformed request handling
    results.push(test_malformed_request(tester).await);

    results
}

/// C-01: Validate that the server completes the initialize handshake.
/// The server must return a valid InitializeResult with protocolVersion,
/// capabilities, and serverInfo.
async fn test_initialize_handshake(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();

    let init_result = tester.test_initialize().await;

    // Convert the existing test_initialize result into a conformance result
    if init_result.status == TestStatus::Passed {
        TestResult {
            name: "Core: initialize handshake".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Passed,
            duration: start.elapsed(),
            error: None,
            details: init_result.details,
        }
    } else {
        TestResult {
            name: "Core: initialize handshake".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: init_result.error,
            details: None,
        }
    }
}

/// C-02: Validate the protocol version is a recognized MCP version.
/// Non-async -- reads cached state from the initialize response.
fn test_protocol_version(tester: &ServerTester) -> TestResult {
    let start = Instant::now();

    let supported_versions = ["2025-11-25", "2025-06-18", "2025-03-26"];
    let older_versions = ["2024-11-05"];

    match tester.server_info() {
        Some(info) => {
            let version = &info.protocol_version.0;
            if supported_versions.contains(&version.as_str()) {
                TestResult {
                    name: "Core: protocol version".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!("Protocol version: {version}")),
                }
            } else if older_versions.contains(&version.as_str()) {
                TestResult {
                    name: "Core: protocol version".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Warning,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!(
                        "Server uses older protocol version: {version}"
                    )),
                }
            } else {
                TestResult {
                    name: "Core: protocol version".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Warning,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!(
                        "Unrecognized protocol version: {version}"
                    )),
                }
            }
        },
        None => TestResult {
            name: "Core: protocol version".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some("No server info available (initialize not called?)".to_string()),
            details: None,
        },
    }
}

/// C-03: Validate server info has non-empty name and version.
fn test_server_info(tester: &ServerTester) -> TestResult {
    let start = Instant::now();

    match tester.server_info() {
        Some(info) => {
            let name = &info.server_info.name;
            let version = &info.server_info.version;

            if name.is_empty() || version.is_empty() {
                let mut missing = Vec::new();
                if name.is_empty() {
                    missing.push("name");
                }
                if version.is_empty() {
                    missing.push("version");
                }
                TestResult {
                    name: "Core: server info".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Failed,
                    duration: start.elapsed(),
                    error: Some(format!(
                        "Server info has empty field(s): {}",
                        missing.join(", ")
                    )),
                    details: None,
                }
            } else {
                TestResult {
                    name: "Core: server info".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!("{name} v{version}")),
                }
            }
        },
        None => TestResult {
            name: "Core: server info".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some("No server info available".to_string()),
            details: None,
        },
    }
}

/// C-04: Validate the capabilities structure is present and well-formed.
fn test_capabilities_structure(tester: &ServerTester) -> TestResult {
    let start = Instant::now();

    match tester.server_capabilities() {
        Some(caps) => {
            let mut advertised = Vec::new();
            if caps.tools.is_some() {
                advertised.push("tools");
            }
            if caps.resources.is_some() {
                advertised.push("resources");
            }
            if caps.prompts.is_some() {
                advertised.push("prompts");
            }
            if caps.tasks.is_some() {
                advertised.push("tasks");
            }

            let details = if advertised.is_empty() {
                "No optional capabilities advertised".to_string()
            } else {
                advertised.join(", ")
            };

            TestResult {
                name: "Core: capabilities structure".to_string(),
                category: TestCategory::Core,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some(details),
            }
        },
        None => TestResult {
            name: "Core: capabilities structure".to_string(),
            category: TestCategory::Core,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some("No capabilities available".to_string()),
            details: None,
        },
    }
}

/// C-05: Validate that the server returns -32601 (Method not found) for unknown methods.
async fn test_unknown_method(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();

    match tester
        .send_custom_request("nonexistent/method", json!({}))
        .await
    {
        Ok(response) => {
            // Check if the response itself contains an error field (JSON-RPC error)
            if let Some(error) = response.get("error") {
                if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
                    if code == -32601 {
                        TestResult {
                            name: "Core: unknown method returns -32601".to_string(),
                            category: TestCategory::Core,
                            status: TestStatus::Passed,
                            duration: start.elapsed(),
                            error: None,
                            details: Some("Correct -32601 Method not found error".to_string()),
                        }
                    } else {
                        TestResult {
                            name: "Core: unknown method returns -32601".to_string(),
                            category: TestCategory::Core,
                            status: TestStatus::Warning,
                            duration: start.elapsed(),
                            error: None,
                            details: Some(format!(
                                "Server returned error code {code} instead of -32601"
                            )),
                        }
                    }
                } else {
                    TestResult {
                        name: "Core: unknown method returns -32601".to_string(),
                        category: TestCategory::Core,
                        status: TestStatus::Warning,
                        duration: start.elapsed(),
                        error: None,
                        details: Some(
                            "Server returned error but no numeric code".to_string(),
                        ),
                    }
                }
            } else {
                TestResult {
                    name: "Core: unknown method returns -32601".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Warning,
                    duration: start.elapsed(),
                    error: None,
                    details: Some("Server did not reject unknown method".to_string()),
                }
            }
        },
        Err(_) => {
            // An Err result means the transport returned an error, which is
            // the expected behavior -- server rejected the unknown method
            TestResult {
                name: "Core: unknown method returns -32601".to_string(),
                category: TestCategory::Core,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some("Server correctly rejected unknown method".to_string()),
            }
        },
    }
}

/// C-06: Validate that the server handles malformed/empty method requests gracefully.
async fn test_malformed_request(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();

    match tester.send_custom_request("", json!({})).await {
        Ok(response) => {
            // Check if response contains an error
            if response.get("error").is_some() {
                TestResult {
                    name: "Core: malformed request handling".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some("Server returned error for malformed request".to_string()),
                }
            } else {
                TestResult {
                    name: "Core: malformed request handling".to_string(),
                    category: TestCategory::Core,
                    status: TestStatus::Warning,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(
                        "Server returned success for empty method name".to_string(),
                    ),
                }
            }
        },
        Err(_) => {
            // Error response is the expected behavior
            TestResult {
                name: "Core: malformed request handling".to_string(),
                category: TestCategory::Core,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some(
                    "Server correctly rejected malformed request".to_string(),
                ),
            }
        },
    }
}
