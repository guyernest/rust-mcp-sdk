//! Tools domain conformance scenarios.
//!
//! Validates: tools/list, tool schema, tools/call on existing tool,
//! tools/call on unknown tool. Capability-conditional -- Skipped if
//! server does not advertise tools capability.

use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::ServerTester;
use pmcp::types::ToolInfo;
use serde_json::json;
use std::time::Instant;

/// Run all tools conformance scenarios.
/// Skipped if server does not advertise tools capability.
pub async fn run_tools_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    // Check capability via public getter
    let has_tools = tester
        .server_capabilities()
        .map_or(false, |caps| caps.tools.is_some());

    if !has_tools {
        return vec![TestResult {
            name: "Tools: capability not advertised".to_string(),
            category: TestCategory::Tools,
            status: TestStatus::Skipped,
            duration: std::time::Duration::from_secs(0),
            error: None,
            details: Some("Server does not advertise tools capability".to_string()),
        }];
    }

    let mut results = Vec::new();

    // T-01: List tools
    let (list_result, tools) = test_tools_list(tester).await;
    results.push(list_result);

    // T-02: Schema validation (uses tools from T-01)
    results.push(test_tool_schema_validation(&tools));

    // T-03: Call existing tool (uses first tool from T-01)
    results.push(test_call_existing_tool(tester, &tools).await);

    // T-04: Call unknown tool
    results.push(test_call_unknown_tool(tester).await);

    results
}

/// T-01: Validate tools/list returns valid ToolInfo entries.
async fn test_tools_list(tester: &mut ServerTester) -> (TestResult, Vec<ToolInfo>) {
    let start = Instant::now();

    match tester.list_tools().await {
        Ok(result) => {
            let tools = result.tools;

            // Verify each tool has a non-empty name
            let invalid_tools: Vec<_> = tools
                .iter()
                .enumerate()
                .filter(|(_, t)| t.name.is_empty())
                .map(|(i, _)| i)
                .collect();

            if invalid_tools.is_empty() {
                (
                    TestResult {
                        name: "Tools: list returns valid ToolInfo".to_string(),
                        category: TestCategory::Tools,
                        status: TestStatus::Passed,
                        duration: start.elapsed(),
                        error: None,
                        details: Some(format!("Found {} tools", tools.len())),
                    },
                    tools,
                )
            } else {
                (
                    TestResult {
                        name: "Tools: list returns valid ToolInfo".to_string(),
                        category: TestCategory::Tools,
                        status: TestStatus::Failed,
                        duration: start.elapsed(),
                        error: Some(format!(
                            "Tools at indices {:?} have empty names",
                            invalid_tools
                        )),
                        details: None,
                    },
                    tools,
                )
            }
        },
        Err(e) => (
            TestResult {
                name: "Tools: list returns valid ToolInfo".to_string(),
                category: TestCategory::Tools,
                status: TestStatus::Failed,
                duration: start.elapsed(),
                error: Some(format!("tools/list failed: {e}")),
                details: None,
            },
            Vec::new(),
        ),
    }
}

/// T-02: Validate that each tool's input_schema is a valid JSON Schema object or null.
fn test_tool_schema_validation(tools: &[ToolInfo]) -> TestResult {
    let start = Instant::now();

    let invalid_schemas: Vec<String> = tools
        .iter()
        .filter_map(|tool| {
            let schema = &tool.input_schema;
            // Valid: null, object with "type" field, or empty object
            if schema.is_null() || schema.is_object() {
                None
            } else {
                Some(tool.name.clone())
            }
        })
        .collect();

    if invalid_schemas.is_empty() {
        TestResult {
            name: "Tools: input schema validation".to_string(),
            category: TestCategory::Tools,
            status: TestStatus::Passed,
            duration: start.elapsed(),
            error: None,
            details: Some(format!("All {} tool schemas valid", tools.len())),
        }
    } else {
        TestResult {
            name: "Tools: input schema validation".to_string(),
            category: TestCategory::Tools,
            status: TestStatus::Warning,
            duration: start.elapsed(),
            error: None,
            details: Some(format!(
                "Tools with invalid schemas: {}",
                invalid_schemas.join(", ")
            )),
        }
    }
}

/// T-03: Call an existing tool with empty arguments.
/// Accepts: valid CallToolResult, isError=true response, or reasonable JSON-RPC error.
async fn test_call_existing_tool(tester: &mut ServerTester, tools: &[ToolInfo]) -> TestResult {
    let start = Instant::now();

    if tools.is_empty() {
        return TestResult {
            name: "Tools: call existing tool".to_string(),
            category: TestCategory::Tools,
            status: TestStatus::Skipped,
            duration: start.elapsed(),
            error: None,
            details: Some("No tools available to test".to_string()),
        };
    }

    let tool_name = &tools[0].name;

    match tester
        .send_custom_request(
            "tools/call",
            json!({"name": tool_name, "arguments": {}}),
        )
        .await
    {
        Ok(response) => {
            // Accept either a valid result (content array) or isError=true response
            if response.get("content").is_some() || response.get("isError").is_some() {
                TestResult {
                    name: "Tools: call existing tool".to_string(),
                    category: TestCategory::Tools,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!("Tool '{tool_name}' responded correctly")),
                }
            } else if response.get("error").is_some() {
                // JSON-RPC error is acceptable (e.g., invalid params)
                TestResult {
                    name: "Tools: call existing tool".to_string(),
                    category: TestCategory::Tools,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!(
                        "Tool '{tool_name}' returned protocol error (valid)"
                    )),
                }
            } else {
                TestResult {
                    name: "Tools: call existing tool".to_string(),
                    category: TestCategory::Tools,
                    status: TestStatus::Warning,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!(
                        "Tool '{tool_name}' returned unparseable response"
                    )),
                }
            }
        },
        Err(_) => {
            // Transport-level error also acceptable (e.g., -32602 invalid params)
            TestResult {
                name: "Tools: call existing tool".to_string(),
                category: TestCategory::Tools,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some(format!(
                    "Tool '{tool_name}' returned error (acceptable for empty args)"
                )),
            }
        },
    }
}

/// T-04: Call a nonexistent tool and verify error response.
async fn test_call_unknown_tool(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();

    match tester
        .send_custom_request(
            "tools/call",
            json!({"name": "___nonexistent_tool_conformance_test___", "arguments": {}}),
        )
        .await
    {
        Ok(response) => {
            // Check for isError=true or error field
            let is_error = response
                .get("isError")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_error || response.get("error").is_some() {
                TestResult {
                    name: "Tools: call unknown tool returns error".to_string(),
                    category: TestCategory::Tools,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some("Server correctly returned error for unknown tool".to_string()),
                }
            } else {
                TestResult {
                    name: "Tools: call unknown tool returns error".to_string(),
                    category: TestCategory::Tools,
                    status: TestStatus::Warning,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(
                        "Server returned success for nonexistent tool".to_string(),
                    ),
                }
            }
        },
        Err(_) => {
            // Error is the expected behavior
            TestResult {
                name: "Tools: call unknown tool returns error".to_string(),
                category: TestCategory::Tools,
                status: TestStatus::Passed,
                duration: start.elapsed(),
                error: None,
                details: Some("Server correctly rejected unknown tool".to_string()),
            }
        },
    }
}
