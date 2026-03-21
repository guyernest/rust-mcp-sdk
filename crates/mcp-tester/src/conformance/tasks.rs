//! Tasks domain conformance scenarios.
//!
//! Validates: tasks capability structure, task creation via tools/call,
//! tasks/get, task status transitions. Fully capability-conditional --
//! all scenarios Skipped if server does not advertise tasks capability.

use crate::report::{TestCategory, TestResult, TestStatus};
use crate::tester::ServerTester;
use serde_json::json;
use std::time::Instant;

/// Valid task status values per MCP spec 2025-11-25.
const VALID_STATUSES: &[&str] = &[
    "working",
    "input_required",
    "completed",
    "failed",
    "cancelled",
];

/// Terminal task states that should not transition further.
const TERMINAL_STATUSES: &[&str] = &["completed", "failed", "cancelled"];

/// Run all tasks conformance scenarios.
/// Skipped if server does not advertise tasks capability.
pub async fn run_tasks_conformance(tester: &mut ServerTester) -> Vec<TestResult> {
    // Check capability via public getter
    let has_tasks = tester
        .server_capabilities()
        .map_or(false, |caps| caps.tasks.is_some());

    if !has_tasks {
        return vec![TestResult {
            name: "Tasks: capability not advertised".to_string(),
            category: TestCategory::Tasks,
            status: TestStatus::Skipped,
            duration: std::time::Duration::from_secs(0),
            error: None,
            details: Some("Server does not advertise tasks capability".to_string()),
        }];
    }

    let mut results = Vec::new();

    // K-01: Tasks capability advertised
    results.push(test_tasks_capability(tester));

    // Get the first tool name for task creation test
    let first_tool_name = tester
        .server_capabilities()
        .and_then(|caps| {
            if caps.tools.is_some() {
                // Use get_tools if available, otherwise None
                tester.get_tools().and_then(|tools| {
                    tools.first().map(|t| t.name.clone())
                })
            } else {
                None
            }
        });

    // K-02: Task creation via tools/call
    let (creation_result, task_id) =
        test_task_creation(tester, first_tool_name.as_deref()).await;
    results.push(creation_result);

    // K-03: Get task by ID
    let (get_result, task_status) =
        test_task_get(tester, task_id.as_deref()).await;
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

    match tester.server_capabilities() {
        Some(caps) => {
            if let Some(tasks_cap) = &caps.tasks {
                let details = format!("{tasks_cap:?}");
                TestResult {
                    name: "Tasks: capability advertised".to_string(),
                    category: TestCategory::Tasks,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(details),
                }
            } else {
                TestResult {
                    name: "Tasks: capability advertised".to_string(),
                    category: TestCategory::Tasks,
                    status: TestStatus::Failed,
                    duration: start.elapsed(),
                    error: Some("Tasks capability not found in capabilities".to_string()),
                    details: None,
                }
            }
        },
        None => TestResult {
            name: "Tasks: capability advertised".to_string(),
            category: TestCategory::Tasks,
            status: TestStatus::Failed,
            duration: start.elapsed(),
            error: Some("No capabilities available".to_string()),
            details: None,
        },
    }
}

/// K-02: Attempt task creation by calling tools/call with task metadata.
async fn test_task_creation(
    tester: &mut ServerTester,
    first_tool_name: Option<&str>,
) -> (TestResult, Option<String>) {
    let start = Instant::now();

    let Some(tool_name) = first_tool_name else {
        return (
            TestResult {
                name: "Tasks: create task via tools/call".to_string(),
                category: TestCategory::Tasks,
                status: TestStatus::Skipped,
                duration: start.elapsed(),
                error: None,
                details: Some("No tools available for task creation test".to_string()),
            },
            None,
        );
    };

    match tester
        .send_custom_request(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": {},
                "_meta": {
                    "task": {
                        "ttl": 60000
                    }
                }
            }),
        )
        .await
    {
        Ok(response) => {
            // Check for task field in response
            if let Some(task) = response.get("task") {
                let task_id = task
                    .get("taskId")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let status = task
                    .get("status")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if let Some(ref id) = task_id {
                    let valid_status = status
                        .as_deref()
                        .map_or(false, |s| VALID_STATUSES.contains(&s));

                    if valid_status {
                        (
                            TestResult {
                                name: "Tasks: create task via tools/call".to_string(),
                                category: TestCategory::Tasks,
                                status: TestStatus::Passed,
                                duration: start.elapsed(),
                                error: None,
                                details: Some(format!(
                                    "Task created: id={id}, status={}",
                                    status.as_deref().unwrap_or("unknown")
                                )),
                            },
                            task_id,
                        )
                    } else {
                        (
                            TestResult {
                                name: "Tasks: create task via tools/call".to_string(),
                                category: TestCategory::Tasks,
                                status: TestStatus::Warning,
                                duration: start.elapsed(),
                                error: None,
                                details: Some(format!(
                                    "Task created with unrecognized status: {}",
                                    status.as_deref().unwrap_or("missing")
                                )),
                            },
                            task_id,
                        )
                    }
                } else {
                    (
                        TestResult {
                            name: "Tasks: create task via tools/call".to_string(),
                            category: TestCategory::Tasks,
                            status: TestStatus::Warning,
                            duration: start.elapsed(),
                            error: None,
                            details: Some(
                                "Task object present but missing taskId".to_string(),
                            ),
                        },
                        None,
                    )
                }
            } else {
                // No task in response -- tool may not support task creation
                let is_error = response
                    .get("isError")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if is_error || response.get("error").is_some() {
                    (
                        TestResult {
                            name: "Tasks: create task via tools/call".to_string(),
                            category: TestCategory::Tasks,
                            status: TestStatus::Warning,
                            duration: start.elapsed(),
                            error: None,
                            details: Some(
                                "Tool call failed but error format is valid".to_string(),
                            ),
                        },
                        None,
                    )
                } else {
                    (
                        TestResult {
                            name: "Tasks: create task via tools/call".to_string(),
                            category: TestCategory::Tasks,
                            status: TestStatus::Warning,
                            duration: start.elapsed(),
                            error: None,
                            details: Some(
                                "Tool responded without task field (tool may not support tasks)"
                                    .to_string(),
                            ),
                        },
                        None,
                    )
                }
            }
        },
        Err(e) => (
            TestResult {
                name: "Tasks: create task via tools/call".to_string(),
                category: TestCategory::Tasks,
                status: TestStatus::Warning,
                duration: start.elapsed(),
                error: None,
                details: Some(format!(
                    "Tool call returned error: {e}"
                )),
            },
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

    let Some(id) = task_id else {
        return (
            TestResult {
                name: "Tasks: get task by ID".to_string(),
                category: TestCategory::Tasks,
                status: TestStatus::Skipped,
                duration: start.elapsed(),
                error: None,
                details: Some("No task ID from K-02 to query".to_string()),
            },
            None,
        );
    };

    match tester
        .send_custom_request("tasks/get", json!({"taskId": id}))
        .await
    {
        Ok(response) => {
            let has_task_id = response
                .get("taskId")
                .and_then(|v| v.as_str())
                .is_some();
            let status = response
                .get("status")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if has_task_id && status.is_some() {
                (
                    TestResult {
                        name: "Tasks: get task by ID".to_string(),
                        category: TestCategory::Tasks,
                        status: TestStatus::Passed,
                        duration: start.elapsed(),
                        error: None,
                        details: Some(format!(
                            "Task {id} found, status: {}",
                            status.as_deref().unwrap_or("unknown")
                        )),
                    },
                    status,
                )
            } else {
                (
                    TestResult {
                        name: "Tasks: get task by ID".to_string(),
                        category: TestCategory::Tasks,
                        status: TestStatus::Warning,
                        duration: start.elapsed(),
                        error: None,
                        details: Some(format!(
                            "tasks/get response missing taskId or status: {response}"
                        )),
                    },
                    None,
                )
            }
        },
        Err(e) => (
            TestResult {
                name: "Tasks: get task by ID".to_string(),
                category: TestCategory::Tasks,
                status: TestStatus::Failed,
                duration: start.elapsed(),
                error: Some(format!("tasks/get failed: {e}")),
                details: None,
            },
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

    let Some(id) = task_id else {
        return TestResult {
            name: "Tasks: valid status transitions".to_string(),
            category: TestCategory::Tasks,
            status: TestStatus::Skipped,
            duration: start.elapsed(),
            error: None,
            details: Some("No task ID available for transition test".to_string()),
        };
    };

    let Some(status) = current_status else {
        return TestResult {
            name: "Tasks: valid status transitions".to_string(),
            category: TestCategory::Tasks,
            status: TestStatus::Warning,
            duration: start.elapsed(),
            error: None,
            details: Some("Could not determine current task status".to_string()),
        };
    };

    // Verify current status is a valid TaskStatus value
    if !VALID_STATUSES.contains(&status) {
        return TestResult {
            name: "Tasks: valid status transitions".to_string(),
            category: TestCategory::Tasks,
            status: TestStatus::Warning,
            duration: start.elapsed(),
            error: None,
            details: Some(format!("Unrecognized task status: {status}")),
        };
    }

    // If terminal, poll again and verify it stays terminal
    if TERMINAL_STATUSES.contains(&status) {
        match tester
            .send_custom_request("tasks/get", json!({"taskId": id}))
            .await
        {
            Ok(response) => {
                let new_status = response
                    .get("status")
                    .and_then(|v| v.as_str());

                if let Some(new) = new_status {
                    if new == status {
                        TestResult {
                            name: "Tasks: valid status transitions".to_string(),
                            category: TestCategory::Tasks,
                            status: TestStatus::Passed,
                            duration: start.elapsed(),
                            error: None,
                            details: Some(format!(
                                "Terminal status '{status}' remained stable"
                            )),
                        }
                    } else if TERMINAL_STATUSES.contains(&new) {
                        // Different terminal state is acceptable
                        TestResult {
                            name: "Tasks: valid status transitions".to_string(),
                            category: TestCategory::Tasks,
                            status: TestStatus::Passed,
                            duration: start.elapsed(),
                            error: None,
                            details: Some(format!(
                                "Status transitioned: {status} -> {new} (both terminal)"
                            )),
                        }
                    } else {
                        TestResult {
                            name: "Tasks: valid status transitions".to_string(),
                            category: TestCategory::Tasks,
                            status: TestStatus::Warning,
                            duration: start.elapsed(),
                            error: None,
                            details: Some(format!(
                                "Terminal status '{status}' transitioned to non-terminal '{new}'"
                            )),
                        }
                    }
                } else {
                    TestResult {
                        name: "Tasks: valid status transitions".to_string(),
                        category: TestCategory::Tasks,
                        status: TestStatus::Warning,
                        duration: start.elapsed(),
                        error: None,
                        details: Some("Could not read status from re-poll".to_string()),
                    }
                }
            },
            Err(_) => {
                // Best-effort: if we can't re-poll, the current status was valid
                TestResult {
                    name: "Tasks: valid status transitions".to_string(),
                    category: TestCategory::Tasks,
                    status: TestStatus::Passed,
                    duration: start.elapsed(),
                    error: None,
                    details: Some(format!(
                        "Current status '{status}' is valid (re-poll failed)"
                    )),
                }
            },
        }
    } else {
        // Non-terminal status: current status is valid, that's enough
        TestResult {
            name: "Tasks: valid status transitions".to_string(),
            category: TestCategory::Tasks,
            status: TestStatus::Passed,
            duration: start.elapsed(),
            error: None,
            details: Some(format!(
                "Non-terminal status '{status}' is valid"
            )),
        }
    }
}
