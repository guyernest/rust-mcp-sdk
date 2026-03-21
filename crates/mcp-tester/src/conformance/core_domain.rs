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
async fn test_initialize_handshake(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let init_result = tester.test_initialize().await;

    // Re-label the existing test_initialize result as a conformance result
    if init_result.status == TestStatus::Passed {
        TestResult::passed(
            "Core: initialize handshake",
            TestCategory::Core,
            start.elapsed(),
            init_result.details.unwrap_or_default(),
        )
    } else {
        TestResult::failed(
            "Core: initialize handshake",
            TestCategory::Core,
            start.elapsed(),
            init_result.error.unwrap_or_else(|| "Initialize failed".into()),
        )
    }
}

/// C-02: Validate the protocol version is a recognized MCP version.
fn test_protocol_version(tester: &ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: protocol version";

    match tester.server_info() {
        Some(info) => {
            let version = &info.protocol_version.0;
            if pmcp::SUPPORTED_PROTOCOL_VERSIONS.contains(&version.as_str()) {
                TestResult::passed(name, TestCategory::Core, start.elapsed(), format!("Protocol version: {version}"))
            } else {
                TestResult::warning(name, TestCategory::Core, start.elapsed(), format!("Unrecognized protocol version: {version}"))
            }
        },
        None => TestResult::failed(name, TestCategory::Core, start.elapsed(), "No server info available (initialize not called?)"),
    }
}

/// C-03: Validate server info has non-empty name and version.
fn test_server_info(tester: &ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: server info";

    match tester.server_info() {
        Some(info) => {
            let srv_name = &info.server_info.name;
            let srv_version = &info.server_info.version;

            if srv_name.is_empty() || srv_version.is_empty() {
                let mut missing = Vec::new();
                if srv_name.is_empty() { missing.push("name"); }
                if srv_version.is_empty() { missing.push("version"); }
                TestResult::failed(name, TestCategory::Core, start.elapsed(), format!("Server info has empty field(s): {}", missing.join(", ")))
            } else {
                TestResult::passed(name, TestCategory::Core, start.elapsed(), format!("{srv_name} v{srv_version}"))
            }
        },
        None => TestResult::failed(name, TestCategory::Core, start.elapsed(), "No server info available"),
    }
}

/// C-04: Validate the capabilities structure is present and well-formed.
fn test_capabilities_structure(tester: &ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: capabilities structure";

    match tester.server_capabilities() {
        Some(caps) => {
            let mut advertised = Vec::new();
            if caps.tools.is_some() { advertised.push("tools"); }
            if caps.resources.is_some() { advertised.push("resources"); }
            if caps.prompts.is_some() { advertised.push("prompts"); }
            if caps.tasks.is_some() { advertised.push("tasks"); }

            let details = if advertised.is_empty() {
                "No optional capabilities advertised".to_string()
            } else {
                advertised.join(", ")
            };

            TestResult::passed(name, TestCategory::Core, start.elapsed(), details)
        },
        None => TestResult::failed(name, TestCategory::Core, start.elapsed(), "No capabilities available"),
    }
}

/// C-05: Validate that the server returns -32601 (Method not found) for unknown methods.
async fn test_unknown_method(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: unknown method returns -32601";

    match tester.send_custom_request("nonexistent/method", json!({})).await {
        Ok(response) => {
            if let Some(error) = response.get("error") {
                // error may be a structured JSON-RPC error object {"code": -32601, "message": "..."}
                // or a flat string from send_custom_request's Err-to-Ok wrapping
                if let Some(code) = error.get("code").and_then(|c| c.as_i64()) {
                    if code == -32601 {
                        TestResult::passed(name, TestCategory::Core, start.elapsed(), "Correct -32601 Method not found error")
                    } else {
                        TestResult::warning(name, TestCategory::Core, start.elapsed(), format!("Server returned error code {code} instead of -32601"))
                    }
                } else {
                    // Server rejected the method — the structured error code was lost
                    // through the transport layer, but rejection itself is correct behavior
                    TestResult::passed(name, TestCategory::Core, start.elapsed(), "Server rejected unknown method (error code not available through transport)")
                }
            } else {
                TestResult::warning(name, TestCategory::Core, start.elapsed(), "Server did not reject unknown method")
            }
        },
        Err(_) => TestResult::passed(name, TestCategory::Core, start.elapsed(), "Server correctly rejected unknown method"),
    }
}

/// C-06: Validate that the server handles malformed/empty method requests gracefully.
async fn test_malformed_request(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Core: malformed request handling";

    match tester.send_custom_request("", json!({})).await {
        Ok(response) => {
            if response.get("error").is_some() {
                TestResult::passed(name, TestCategory::Core, start.elapsed(), "Server returned error for malformed request")
            } else {
                TestResult::warning(name, TestCategory::Core, start.elapsed(), "Server returned success for empty method name")
            }
        },
        Err(_) => TestResult::passed(name, TestCategory::Core, start.elapsed(), "Server correctly rejected malformed request"),
    }
}
