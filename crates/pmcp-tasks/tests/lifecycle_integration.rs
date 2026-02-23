//! Full lifecycle integration tests for MCP Tasks.
//!
//! These tests exercise the complete task lifecycle through `ServerCore::handle_request()`,
//! verifying end-to-end correctness of create -> poll -> complete -> get_result flows,
//! as well as list, cancel, and error handling.

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::{CallToolParams, ClientRequest, Request, RequestId, ToolInfo};
use pmcp::RequestHandlerExtra;
use pmcp_tasks::task::TaskStatus;
use pmcp_tasks::{InMemoryTaskStore, TaskRouterImpl, TaskSecurityConfig, TaskStore};
use serde_json::{json, Value};
use std::sync::Arc;

/// A simple test tool that returns immediately.
///
/// The tool does not perform any long-running work itself -- the task system
/// handles lifecycle management. The tool returns metadata that would normally
/// be passed to an external service.
struct TestTool {
    store: Arc<InMemoryTaskStore>,
}

#[async_trait]
impl pmcp::ToolHandler for TestTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // In a real scenario, this would trigger external work.
        // For tests, we just return the arguments as confirmation.
        let _ = &self.store; // held for direct store access in tests
        Ok(json!({ "received": args }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "long_running_tool",
            Some("A test tool for task lifecycle".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                }
            }),
        ))
    }
}

/// A tool that declares `taskSupport: required` via execution metadata.
struct RequiredTaskTool;

#[async_trait]
impl pmcp::ToolHandler for RequiredTaskTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "status": "invoked" }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        let mut info = ToolInfo::new(
            "required_task_tool",
            Some("A tool that requires task augmentation".to_string()),
            json!({ "type": "object" }),
        );
        info.execution = Some(json!({ "taskSupport": "required" }));
        Some(info)
    }
}

/// A normal tool with no task awareness.
struct NormalTool;

#[async_trait]
impl pmcp::ToolHandler for NormalTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "result": "normal_output" }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "normal_tool",
            Some("A tool that does not use tasks".to_string()),
            json!({ "type": "object" }),
        ))
    }
}

/// Build a task-enabled ServerCore for testing.
///
/// Returns both the server and the underlying store for direct manipulation.
fn build_task_server() -> (pmcp::server::core::ServerCore, Arc<InMemoryTaskStore>) {
    let store = Arc::new(
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    );
    let router = Arc::new(TaskRouterImpl::new(store.clone()));

    let server = ServerCoreBuilder::new()
        .name("test-tasks")
        .version("1.0.0")
        .tool(
            "long_running_tool",
            TestTool {
                store: store.clone(),
            },
        )
        .tool("required_task_tool", RequiredTaskTool)
        .tool("normal_tool", NormalTool)
        .with_task_store(router)
        .stateless_mode(true)
        .build()
        .unwrap();

    (server, store)
}

/// Helper to extract a successful result value from a response.
fn unwrap_result(response: pmcp::types::JSONRPCResponse) -> Value {
    match response.payload {
        ResponsePayload::Result(v) => v,
        ResponsePayload::Error(e) => panic!("Expected success but got error: {}", e.message),
    }
}

/// Helper to extract an error from a response.
fn unwrap_error(response: pmcp::types::JSONRPCResponse) -> pmcp::types::JSONRPCError {
    match response.payload {
        ResponsePayload::Error(e) => e,
        ResponsePayload::Result(v) => panic!("Expected error but got result: {v}"),
    }
}

/// Build a tools/call request with task augmentation.
fn task_call_request(tool_name: &str, args: Value, task_params: Value) -> Request {
    Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
        name: tool_name.to_string(),
        arguments: args,
        _meta: None,
        task: Some(task_params),
    })))
}

/// Build a tools/call request without task augmentation.
fn normal_call_request(tool_name: &str, args: Value) -> Request {
    Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
        name: tool_name.to_string(),
        arguments: args,
        _meta: None,
        task: None,
    })))
}

/// Build a tasks/get request.
fn tasks_get_request(task_id: &str) -> Request {
    Request::Client(Box::new(ClientRequest::TasksGet(json!({
        "taskId": task_id
    }))))
}

/// Build a tasks/result request.
fn tasks_result_request(task_id: &str) -> Request {
    Request::Client(Box::new(ClientRequest::TasksResult(json!({
        "taskId": task_id
    }))))
}

/// Build a tasks/list request.
fn tasks_list_request() -> Request {
    Request::Client(Box::new(ClientRequest::TasksList(json!({}))))
}

/// Build a tasks/cancel request.
fn tasks_cancel_request(task_id: &str) -> Request {
    Request::Client(Box::new(ClientRequest::TasksCancel(json!({
        "taskId": task_id
    }))))
}

// --------------------------------------------------------------------------
// Test 1: Full lifecycle -- create, poll, complete, get_result
// --------------------------------------------------------------------------

#[tokio::test]
async fn full_lifecycle_create_poll_complete_result() {
    let (server, store) = build_task_server();

    // Step 1: Create task via tools/call with task field
    let req = task_call_request(
        "long_running_tool",
        json!({ "query": "test" }),
        json!({ "ttl": 60000 }),
    );
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let create_result = unwrap_result(response);

    // Validate CreateTaskResult structure
    let task = &create_result["task"];
    assert_eq!(
        task["status"], "working",
        "new task should be in working state"
    );
    let task_id = task["taskId"].as_str().expect("taskId should be a string");
    assert!(!task_id.is_empty(), "taskId should not be empty");
    assert_eq!(task["ttl"], 60000);

    // Step 2: Poll via tasks/get -- should still be working
    let req = tasks_get_request(task_id);
    let response = server
        .handle_request(RequestId::from(2i64), req, None)
        .await;
    let get_result = unwrap_result(response);
    assert_eq!(get_result["status"], "working");
    assert_eq!(get_result["taskId"], task_id);

    // Step 3: Simulate background completion via direct store access
    let result_data = json!({
        "analysis": "complete",
        "rows_processed": 1_500_000
    });
    store
        .complete_with_result(
            task_id,
            "local",
            TaskStatus::Completed,
            Some("All done".to_string()),
            result_data.clone(),
        )
        .await
        .unwrap();

    // Step 4: Poll again -- should be completed
    let req = tasks_get_request(task_id);
    let response = server
        .handle_request(RequestId::from(3i64), req, None)
        .await;
    let get_result = unwrap_result(response);
    assert_eq!(get_result["status"], "completed");

    // Step 5: Get result via tasks/result
    let req = tasks_result_request(task_id);
    let response = server
        .handle_request(RequestId::from(4i64), req, None)
        .await;
    let result_response = unwrap_result(response);

    assert_eq!(result_response["result"], result_data);
    // Verify _meta contains related-task link
    let meta = &result_response["_meta"];
    assert!(
        meta["io.modelcontextprotocol/related-task"]["taskId"]
            .as_str()
            .is_some(),
        "should have related-task metadata"
    );
}

// --------------------------------------------------------------------------
// Test 2: tasks/list returns owner-scoped results
// --------------------------------------------------------------------------

#[tokio::test]
async fn tasks_list_returns_owner_scoped_tasks() {
    let (server, _store) = build_task_server();

    // Create first task
    let req = task_call_request(
        "long_running_tool",
        json!({ "query": "first" }),
        json!({ "ttl": 60000 }),
    );
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let _ = unwrap_result(response);

    // Create second task
    let req = task_call_request(
        "long_running_tool",
        json!({ "query": "second" }),
        json!({ "ttl": 60000 }),
    );
    let response = server
        .handle_request(RequestId::from(2i64), req, None)
        .await;
    let _ = unwrap_result(response);

    // List all tasks
    let req = tasks_list_request();
    let response = server
        .handle_request(RequestId::from(3i64), req, None)
        .await;
    let list_result = unwrap_result(response);

    let tasks = list_result["tasks"]
        .as_array()
        .expect("tasks should be an array");
    assert_eq!(tasks.len(), 2, "should have exactly 2 tasks");

    // Each task should have a taskId and status
    for task in tasks {
        assert!(task["taskId"].as_str().is_some());
        assert_eq!(task["status"], "working");
    }
}

// --------------------------------------------------------------------------
// Test 3: tasks/cancel transitions to cancelled
// --------------------------------------------------------------------------

#[tokio::test]
async fn tasks_cancel_transitions_to_cancelled() {
    let (server, _store) = build_task_server();

    // Create a task
    let req = task_call_request(
        "long_running_tool",
        json!({ "query": "to_cancel" }),
        json!({ "ttl": 60000 }),
    );
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let create_result = unwrap_result(response);
    let task_id = create_result["task"]["taskId"].as_str().unwrap();

    // Cancel the task
    let req = tasks_cancel_request(task_id);
    let response = server
        .handle_request(RequestId::from(2i64), req, None)
        .await;
    let cancel_result = unwrap_result(response);
    assert_eq!(cancel_result["status"], "cancelled");

    // Verify it is still cancelled via tasks/get
    let req = tasks_get_request(task_id);
    let response = server
        .handle_request(RequestId::from(3i64), req, None)
        .await;
    let get_result = unwrap_result(response);
    assert_eq!(get_result["status"], "cancelled");

    // Cancelling again should fail (already terminal)
    let req = tasks_cancel_request(task_id);
    let response = server
        .handle_request(RequestId::from(4i64), req, None)
        .await;
    let err = unwrap_error(response);
    assert!(
        err.message.contains("invalid transition") || err.message.contains("terminal"),
        "should get transition error, got: {}",
        err.message,
    );
}

// --------------------------------------------------------------------------
// Test 4: Auto-task for required tools (no task field, tool requires task)
// --------------------------------------------------------------------------

#[tokio::test]
async fn auto_task_for_required_tool() {
    let (server, _store) = build_task_server();

    // Call the required_task_tool WITHOUT a task field
    let req = normal_call_request("required_task_tool", json!({}));
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let result = unwrap_result(response);

    // Should get a CreateTaskResult (auto-created) with a task wrapper
    assert!(
        result.get("task").is_some(),
        "required tool should auto-create task even without task field"
    );
    assert_eq!(result["task"]["status"], "working");
    assert!(result["task"]["taskId"].as_str().is_some());
}

// --------------------------------------------------------------------------
// Test 5: Normal tool call still works (no task field, no required)
// --------------------------------------------------------------------------

#[tokio::test]
async fn normal_tool_call_unaffected_by_task_system() {
    let (server, _store) = build_task_server();

    // Call normal_tool without task field
    let req = normal_call_request("normal_tool", json!({}));
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let result = unwrap_result(response);

    // Should get a normal CallToolResult (content array), NOT a CreateTaskResult
    assert!(
        result.get("task").is_none(),
        "normal tool should not create a task"
    );
    // CallToolResult has 'content' field
    assert!(
        result.get("content").is_some(),
        "normal tool should return CallToolResult with content"
    );
}

// --------------------------------------------------------------------------
// Test 6: tasks/get with wrong owner returns error
// --------------------------------------------------------------------------

#[tokio::test]
async fn tasks_get_nonexistent_returns_error() {
    let (server, _store) = build_task_server();

    // Try to get a task that does not exist
    let req = tasks_get_request("nonexistent-task-id");
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let err = unwrap_error(response);
    assert!(
        err.message.contains("not found"),
        "should get not found error, got: {}",
        err.message,
    );
}

// --------------------------------------------------------------------------
// Test 7: tasks/result on non-terminal task returns error
// --------------------------------------------------------------------------

#[tokio::test]
async fn tasks_result_on_working_task_returns_error() {
    let (server, _store) = build_task_server();

    // Create a task (still working)
    let req = task_call_request(
        "long_running_tool",
        json!({ "query": "still_working" }),
        json!({ "ttl": 60000 }),
    );
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let create_result = unwrap_result(response);
    let task_id = create_result["task"]["taskId"].as_str().unwrap();

    // Try to get result before completion
    let req = tasks_result_request(task_id);
    let response = server
        .handle_request(RequestId::from(2i64), req, None)
        .await;
    let err = unwrap_error(response);
    assert!(
        err.message.contains("not in terminal state") || err.message.contains("not ready"),
        "should get not-ready error, got: {}",
        err.message,
    );
}

// --------------------------------------------------------------------------
// Test 8: TTL is respected from task params
// --------------------------------------------------------------------------

#[tokio::test]
async fn ttl_respected_from_task_params() {
    let (server, store) = build_task_server();

    // Create task with very short TTL (1ms)
    let req = task_call_request(
        "long_running_tool",
        json!({ "query": "expiring" }),
        json!({ "ttl": 1 }),
    );
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let create_result = unwrap_result(response);
    let task_id = create_result["task"]["taskId"].as_str().unwrap();

    // Sleep to allow TTL to expire
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // The task should still be readable (get works on expired tasks)
    // but mutations should fail
    let record = store.get(task_id, "local").await.unwrap();
    assert!(
        record.is_expired(),
        "task should be expired after TTL elapses"
    );

    // Attempting to complete an expired task should fail
    let err = store
        .complete_with_result(
            task_id,
            "local",
            TaskStatus::Completed,
            None,
            json!("late_result"),
        )
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("expired") || err.to_string().contains("Expired"),
        "should get expired error, got: {}",
        err,
    );
}

// --------------------------------------------------------------------------
// Test: tasks/list returns empty when no tasks exist
// --------------------------------------------------------------------------

#[tokio::test]
async fn tasks_list_empty_when_no_tasks() {
    let (server, _store) = build_task_server();

    let req = tasks_list_request();
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let list_result = unwrap_result(response);

    let tasks = list_result["tasks"]
        .as_array()
        .expect("tasks should be an array");
    assert!(tasks.is_empty(), "should have no tasks initially");
}

// --------------------------------------------------------------------------
// Test: Task endpoint without task router returns METHOD_NOT_FOUND
// --------------------------------------------------------------------------

#[tokio::test]
async fn task_endpoints_without_router_return_method_not_found() {
    // Build a server WITHOUT a task router
    let server = ServerCoreBuilder::new()
        .name("no-tasks-server")
        .version("1.0.0")
        .tool("normal_tool", NormalTool)
        .stateless_mode(true)
        .build()
        .unwrap();

    // Try tasks/get
    let req = tasks_get_request("some-id");
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let err = unwrap_error(response);
    assert_eq!(err.code, -32601, "should return METHOD_NOT_FOUND (-32601)");

    // Try tasks/list
    let req = tasks_list_request();
    let response = server
        .handle_request(RequestId::from(2i64), req, None)
        .await;
    let err = unwrap_error(response);
    assert_eq!(err.code, -32601);

    // Try tasks/cancel
    let req = tasks_cancel_request("some-id");
    let response = server
        .handle_request(RequestId::from(3i64), req, None)
        .await;
    let err = unwrap_error(response);
    assert_eq!(err.code, -32601);

    // Try tasks/result
    let req = tasks_result_request("some-id");
    let response = server
        .handle_request(RequestId::from(4i64), req, None)
        .await;
    let err = unwrap_error(response);
    assert_eq!(err.code, -32601);
}

// --------------------------------------------------------------------------
// Test: Task-augmented call stores tool context as variables
// --------------------------------------------------------------------------

#[tokio::test]
async fn task_call_stores_tool_context_variables() {
    let (server, store) = build_task_server();

    let args = json!({ "query": "context_test", "limit": 100 });
    let req = task_call_request("long_running_tool", args.clone(), json!({ "ttl": 60000 }));
    let response = server
        .handle_request(RequestId::from(1i64), req, None)
        .await;
    let create_result = unwrap_result(response);
    let task_id = create_result["task"]["taskId"].as_str().unwrap();

    // Verify the store has tool context variables
    let record = store.get(task_id, "local").await.unwrap();
    assert_eq!(
        record.variables.get("tool_name").unwrap(),
        &Value::String("long_running_tool".to_string()),
        "should store tool_name variable"
    );
    assert_eq!(
        record.variables.get("arguments").unwrap(),
        &args,
        "should store arguments variable"
    );
}
