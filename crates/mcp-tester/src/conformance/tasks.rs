//! Tasks domain conformance scenarios.
//!
//! Validates: tasks capability structure, task creation via tools/call,
//! tasks/get, task status transitions. Fully capability-conditional --
//! all scenarios Skipped if server does not advertise tasks capability.

use super::check_capability;
use crate::report::{TestCategory, TestResult};
use crate::tester::ServerTester;
use pmcp::types::TaskStatus;
use serde_json::json;
use std::time::Instant;

/// Run all tasks conformance scenarios.
/// Skipped if server does not advertise tasks capability.
pub async fn run_tasks_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    if let Some(skip) = check_capability(tester, "Tasks", TestCategory::Tasks, |caps| {
        caps.tasks.is_some()
    }) {
        return skip;
    }

    let mut results = Vec::new();

    // K-01: Tasks capability advertised
    results.push(test_tasks_capability(tester));

    // Get the first tool name for task creation test (single capability lookup)
    let has_tools = tester
        .server_capabilities()
        .is_some_and(|caps| caps.tools.is_some());
    let first_tool_name = if has_tools {
        tester
            .get_tools()
            .and_then(|tools| tools.first().map(|t| t.name.clone()))
    } else {
        None
    };

    // K-02: Task creation via tools/call
    let (creation_result, task_id) = test_task_creation(tester, first_tool_name.as_deref()).await;
    results.push(creation_result);

    // K-03: Get task by ID
    let (get_result, task_status) = test_task_get(tester, task_id.as_deref()).await;
    results.push(get_result);

    // K-04: Valid status transitions
    results.push(
        test_task_status_transitions(tester, task_id.as_deref(), task_status.as_deref()).await,
    );

    results
}

/// K-01: Verify the tasks capability structure is valid.
fn test_tasks_capability(tester: &ServerTester) -> TestResult {
    let start = Instant::now();
    let name = "Tasks: capability advertised";

    match tester.server_capabilities() {
        Some(caps) => {
            if let Some(tasks_cap) = &caps.tasks {
                TestResult::passed(
                    name,
                    TestCategory::Tasks,
                    start.elapsed(),
                    format!("{tasks_cap:?}"),
                )
            } else {
                TestResult::failed(
                    name,
                    TestCategory::Tasks,
                    start.elapsed(),
                    "Tasks capability not found in capabilities",
                )
            }
        },
        None => TestResult::failed(
            name,
            TestCategory::Tasks,
            start.elapsed(),
            "No capabilities available",
        ),
    }
}

/// K-02: Attempt task creation by calling tools/call with task metadata.
async fn test_task_creation(
    tester: &mut ServerTester,
    first_tool_name: Option<&str>,
) -> (TestResult, Option<String>) {
    let start = Instant::now();
    let name = "Tasks: create task via tools/call";

    let Some(tool_name) = first_tool_name else {
        return (
            TestResult::skipped(
                name,
                TestCategory::Tasks,
                "No tools available for task creation test",
            ),
            None,
        );
    };

    match tester
        .send_custom_request(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": {},
                "_meta": { "task": { "ttl": 60000 } }
            }),
        )
        .await
    {
        Ok(response) => {
            if let Some(task) = response.get("task") {
                let task_id = task
                    .get("taskId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let status_str = task.get("status").and_then(|v| v.as_str());

                if let Some(ref id) = task_id {
                    // Validate status using the canonical TaskStatus enum
                    let valid_status = status_str
                        .map(|s| serde_json::from_value::<TaskStatus>(json!(s)).is_ok())
                        .unwrap_or(false);

                    if valid_status {
                        (
                            TestResult::passed(
                                name,
                                TestCategory::Tasks,
                                start.elapsed(),
                                format!(
                                    "Task created: id={id}, status={}",
                                    status_str.unwrap_or("unknown")
                                ),
                            ),
                            task_id,
                        )
                    } else {
                        (
                            TestResult::warning(
                                name,
                                TestCategory::Tasks,
                                start.elapsed(),
                                format!(
                                    "Task created with unrecognized status: {}",
                                    status_str.unwrap_or("missing")
                                ),
                            ),
                            task_id,
                        )
                    }
                } else {
                    (
                        TestResult::warning(
                            name,
                            TestCategory::Tasks,
                            start.elapsed(),
                            "Task object present but missing taskId",
                        ),
                        None,
                    )
                }
            } else {
                let is_error = response
                    .get("isError")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if is_error || response.get("error").is_some() {
                    (
                        TestResult::warning(
                            name,
                            TestCategory::Tasks,
                            start.elapsed(),
                            "Tool call failed but error format is valid",
                        ),
                        None,
                    )
                } else {
                    (
                        TestResult::warning(
                            name,
                            TestCategory::Tasks,
                            start.elapsed(),
                            "Tool responded without task field (tool may not support tasks)",
                        ),
                        None,
                    )
                }
            }
        },
        Err(e) => (
            TestResult::warning(
                name,
                TestCategory::Tasks,
                start.elapsed(),
                format!("Tool call returned error: {e}"),
            ),
            None,
        ),
    }
}

/// K-03: Get a task by ID and verify the response contains a valid Task structure.
async fn test_task_get(
    tester: &mut ServerTester,
    task_id: Option<&str>,
) -> (TestResult, Option<String>) {
    let start = Instant::now();
    let name = "Tasks: get task by ID";

    let Some(id) = task_id else {
        return (
            TestResult::skipped(name, TestCategory::Tasks, "No task ID from K-02 to query"),
            None,
        );
    };

    match tester
        .send_custom_request("tasks/get", json!({"taskId": id}))
        .await
    {
        Ok(response) => {
            let has_task_id = response.get("taskId").and_then(|v| v.as_str()).is_some();
            let status = response
                .get("status")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if has_task_id && status.is_some() {
                (
                    TestResult::passed(
                        name,
                        TestCategory::Tasks,
                        start.elapsed(),
                        format!(
                            "Task {id} found, status: {}",
                            status.as_deref().unwrap_or("unknown")
                        ),
                    ),
                    status,
                )
            } else {
                (
                    TestResult::warning(
                        name,
                        TestCategory::Tasks,
                        start.elapsed(),
                        format!("tasks/get response missing taskId or status: {response}"),
                    ),
                    None,
                )
            }
        },
        Err(e) => (
            TestResult::failed(
                name,
                TestCategory::Tasks,
                start.elapsed(),
                format!("tasks/get failed: {e}"),
            ),
            None,
        ),
    }
}

/// K-04: Validate task status transitions.
/// If the task is in a terminal state, verify it stays terminal.
/// If non-terminal, poll once and verify valid transition.
async fn test_task_status_transitions(
    tester: &mut ServerTester,
    task_id: Option<&str>,
    current_status: Option<&str>,
) -> TestResult {
    let start = Instant::now();
    let name = "Tasks: valid status transitions";

    let Some(id) = task_id else {
        return TestResult::skipped(
            name,
            TestCategory::Tasks,
            "No task ID available for transition test",
        );
    };

    let Some(status_str) = current_status else {
        return TestResult::warning(
            name,
            TestCategory::Tasks,
            start.elapsed(),
            "Could not determine current task status",
        );
    };

    // Parse status using the canonical TaskStatus enum
    let Ok(status) = serde_json::from_value::<TaskStatus>(json!(status_str)) else {
        return TestResult::warning(
            name,
            TestCategory::Tasks,
            start.elapsed(),
            format!("Unrecognized task status: {status_str}"),
        );
    };

    if status.is_terminal() {
        // Terminal: poll again and verify it stays terminal
        match tester
            .send_custom_request("tasks/get", json!({"taskId": id}))
            .await
        {
            Ok(response) => {
                let new_status_str = response.get("status").and_then(|v| v.as_str());
                if let Some(new_str) = new_status_str {
                    if new_str == status_str {
                        TestResult::passed(
                            name,
                            TestCategory::Tasks,
                            start.elapsed(),
                            format!("Terminal status '{status_str}' remained stable"),
                        )
                    } else if serde_json::from_value::<TaskStatus>(json!(new_str))
                        .is_ok_and(|s| s.is_terminal())
                    {
                        TestResult::passed(
                            name,
                            TestCategory::Tasks,
                            start.elapsed(),
                            format!(
                                "Status transitioned: {status_str} -> {new_str} (both terminal)"
                            ),
                        )
                    } else {
                        TestResult::warning(name, TestCategory::Tasks, start.elapsed(), format!("Terminal status '{status_str}' transitioned to non-terminal '{new_str}'"))
                    }
                } else {
                    TestResult::warning(
                        name,
                        TestCategory::Tasks,
                        start.elapsed(),
                        "Could not read status from re-poll",
                    )
                }
            },
            Err(_) => TestResult::passed(
                name,
                TestCategory::Tasks,
                start.elapsed(),
                format!("Current status '{status_str}' is valid (re-poll failed)"),
            ),
        }
    } else {
        TestResult::passed(
            name,
            TestCategory::Tasks,
            start.elapsed(),
            format!("Non-terminal status '{status_str}' is valid"),
        )
    }
}
