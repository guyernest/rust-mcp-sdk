//! Tools domain conformance scenarios.
//!
//! Validates: tools/list, tool schema, tools/call on existing tool,
//! tools/call on unknown tool. Capability-conditional -- Skipped if
//! server does not advertise tools capability.

use super::check_capability;
use crate::report::{TestCategory, TestResult};
use crate::tester::ServerTester;
use pmcp::types::ToolInfo;
use serde_json::json;
use std::time::Instant;

/// Run all tools conformance scenarios.
/// Skipped if server does not advertise tools capability.
pub async fn run_tools_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    if let Some(skip) = check_capability(tester, "Tools", TestCategory::Tools, |caps| {
        caps.tools.is_some()
    }) {
        return skip;
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
    let name = "Tools: list returns valid ToolInfo";

    match tester.list_tools().await {
        Ok(result) => {
            let tools = result.tools;
            let invalid_tools: Vec<_> = tools
                .iter()
                .enumerate()
                .filter(|(_, t)| t.name.is_empty())
                .map(|(i, _)| i)
                .collect();

            if invalid_tools.is_empty() {
                (
                    TestResult::passed(
                        name,
                        TestCategory::Tools,
                        start.elapsed(),
                        format!("Found {} tools", tools.len()),
                    ),
                    tools,
                )
            } else {
                (
                    TestResult::failed(
                        name,
                        TestCategory::Tools,
                        start.elapsed(),
                        format!("Tools at indices {:?} have empty names", invalid_tools),
                    ),
                    tools,
                )
            }
        },
        Err(e) => (
            TestResult::failed(
                name,
                TestCategory::Tools,
                start.elapsed(),
                format!("tools/list failed: {e}"),
            ),
            Vec::new(),
        ),
    }
}

/// T-02: Validate that each tool's input_schema is a valid JSON Schema object or null.
fn test_tool_schema_validation(tools: &[ToolInfo]) -> TestResult {
    let start = Instant::now();
    let name = "Tools: input schema validation";

    let invalid_schemas: Vec<String> = tools
        .iter()
        .filter_map(|tool| {
            if tool.input_schema.is_null() || tool.input_schema.is_object() {
                None
            } else {
                Some(tool.name.clone())
            }
        })
        .collect();

    if invalid_schemas.is_empty() {
        TestResult::passed(
            name,
            TestCategory::Tools,
            start.elapsed(),
            format!("All {} tool schemas valid", tools.len()),
        )
    } else {
        TestResult::warning(
            name,
            TestCategory::Tools,
            start.elapsed(),
            format!("Tools with invalid schemas: {}", invalid_schemas.join(", ")),
        )
    }
}

/// T-03: Call an existing tool with empty arguments.
/// Accepts: valid CallToolResult, isError=true response, or reasonable JSON-RPC error.
async fn test_call_existing_tool(tester: &mut ServerTester, tools: &[ToolInfo]) -> TestResult {
    let start = Instant::now();
    let name = "Tools: call existing tool";

    if tools.is_empty() {
        return TestResult::skipped(name, TestCategory::Tools, "No tools available to test");
    }

    let tool_name = &tools[0].name;

    match tester
        .send_custom_request("tools/call", json!({"name": tool_name, "arguments": {}}))
        .await
    {
        Ok(response) => {
            if response.get("content").is_some() || response.get("isError").is_some() {
                TestResult::passed(
                    name,
                    TestCategory::Tools,
                    start.elapsed(),
                    format!("Tool '{tool_name}' responded correctly"),
                )
            } else if response.get("error").is_some() {
                TestResult::passed(
                    name,
                    TestCategory::Tools,
                    start.elapsed(),
                    format!("Tool '{tool_name}' returned protocol error (valid)"),
                )
            } else {
                TestResult::warning(
                    name,
                    TestCategory::Tools,
                    start.elapsed(),
                    format!("Tool '{tool_name}' returned unparseable response"),
                )
            }
        },
        Err(_) => TestResult::passed(
            name,
            TestCategory::Tools,
            start.elapsed(),
            format!("Tool '{tool_name}' returned error (acceptable for empty args)"),
        ),
    }
}

/// T-04: Call a nonexistent tool and verify error response.
async fn test_call_unknown_tool(tester: &mut ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Tools: call unknown tool returns error";

    match tester
        .send_custom_request(
            "tools/call",
            json!({"name": "___nonexistent_tool_conformance_test___", "arguments": {}}),
        )
        .await
    {
        Ok(response) => {
            let is_error = response
                .get("isError")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if is_error || response.get("error").is_some() {
                TestResult::passed(
                    name,
                    TestCategory::Tools,
                    start.elapsed(),
                    "Server correctly returned error for unknown tool",
                )
            } else {
                TestResult::warning(
                    name,
                    TestCategory::Tools,
                    start.elapsed(),
                    "Server returned success for nonexistent tool",
                )
            }
        },
        Err(_) => TestResult::passed(
            name,
            TestCategory::Tools,
            start.elapsed(),
            "Server correctly rejected unknown tool",
        ),
    }
}
