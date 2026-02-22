//! Example: Basic Task-Augmented Tool Call
//!
//! Demonstrates the complete MCP Tasks lifecycle:
//! 1. Server advertises task support via experimental.tasks
//! 2. Client sends tools/call with a task field
//! 3. Server creates task, returns CreateTaskResult immediately
//! 4. Background service simulates work (tokio::spawn + sleep)
//! 5. Client polls tasks/get until completion
//! 6. Client retrieves result via tasks/result
//!
//! This is the simplest possible task-enabled server. It uses
//! InMemoryTaskStore (no external dependencies) and simulates
//! background execution with tokio::spawn + sleep.
//!
//! In production, the background work would be handled by an
//! external service (AWS Step Functions, SQS consumer, etc.)
//! that picks up work from task variables and calls
//! `store.complete_with_result()` when done.
//!
//! Run: `cargo run --example 60_tasks_basic`

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

/// A tool that simulates a long-running data analysis operation.
///
/// In production, this tool would trigger an AWS Step Functions execution
/// or enqueue a message to SQS. The task system creates the task and returns
/// a CreateTaskResult immediately -- the tool handler itself is not called
/// for task-augmented requests (the task router intercepts them).
///
/// For this example, we hold a reference to the store so the background
/// simulation can call `complete_with_result()` directly.
struct LongRunningAnalysis {
    store: Arc<InMemoryTaskStore>,
}

#[async_trait]
impl pmcp::ToolHandler for LongRunningAnalysis {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // This method is called for non-task tool calls.
        // For task-augmented calls, the TaskRouter intercepts
        // the request before this handler is reached.
        let _ = &self.store;
        Ok(json!({
            "message": "Analysis tool invoked (direct, non-task path)",
            "input": args
        }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "analyze_data",
            Some("Analyze a dataset (long-running operation)".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "dataset": {
                        "type": "string",
                        "description": "Name of the dataset to analyze"
                    }
                },
                "required": ["dataset"]
            }),
        ))
    }
}

#[tokio::main]
async fn main() {
    // 1. Create task store and router
    let store = Arc::new(
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    );
    let router = Arc::new(TaskRouterImpl::new(store.clone()));

    // 2. Build server with task support
    let server = ServerCoreBuilder::new()
        .name("tasks-basic-example")
        .version("1.0.0")
        .tool(
            "analyze_data",
            LongRunningAnalysis {
                store: store.clone(),
            },
        )
        .with_task_store(router)
        .stateless_mode(true)
        .build()
        .unwrap();

    println!("=== MCP Tasks Basic Example ===\n");

    // 3. Send tools/call with task augmentation
    println!("Step 1: Call tool with task augmentation...");
    let call_req = Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
        name: "analyze_data".to_string(),
        arguments: json!({"dataset": "sales_2024"}),
        _meta: None,
        task: Some(json!({"ttl": 60000})),
    })));

    let response = server
        .handle_request(RequestId::from(1i64), call_req, None)
        .await;
    let create_result: Value = match response.payload {
        ResponsePayload::Result(v) => v,
        ResponsePayload::Error(e) => panic!("Create failed: {}", e.message),
    };

    let task_id = create_result["task"]["taskId"]
        .as_str()
        .expect("taskId should be present");
    println!("  Task created: {task_id}");
    println!("  Status: {}", create_result["task"]["status"]);

    // 4. Poll task status
    println!("\nStep 2: Poll task status...");
    let get_req = Request::Client(Box::new(ClientRequest::TasksGet(
        json!({"taskId": task_id}),
    )));
    let response = server
        .handle_request(RequestId::from(2i64), get_req, None)
        .await;
    let task: Value = match response.payload {
        ResponsePayload::Result(v) => v,
        ResponsePayload::Error(e) => panic!("Get failed: {}", e.message),
    };
    println!("  Status: {}", task["status"]);

    // 5. Simulate background completion with tokio::spawn + sleep
    //    Per locked decision: "Simulates background execution with
    //    tokio::spawn + sleep for demonstration."
    //    In production, an external service (Step Functions, SQS consumer)
    //    would do this work and call complete_with_result when done.
    println!("\nStep 3: Simulate background work (tokio::spawn + sleep)...");
    let bg_store = store.clone();
    let bg_task_id = task_id.to_string();
    let bg_handle = tokio::spawn(async move {
        // Simulate work taking some time
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        bg_store
            .complete_with_result(
                &bg_task_id,
                "local",
                TaskStatus::Completed,
                Some("Analysis finished successfully".to_string()),
                json!({
                    "analysis": "complete",
                    "rows_processed": 1_500_000,
                    "anomalies_found": 42,
                    "summary": "Sales data shows 3.2% YoY growth with seasonal patterns"
                }),
            )
            .await
            .expect("complete_with_result should succeed");
    });
    // Wait for the background task to finish
    // (in production, the client would poll tasks/get periodically)
    bg_handle.await.expect("background task should complete");
    println!("  Task completed by background service");

    // 6. Poll again to see completion
    println!("\nStep 4: Poll after completion...");
    let get_req = Request::Client(Box::new(ClientRequest::TasksGet(
        json!({"taskId": task_id}),
    )));
    let response = server
        .handle_request(RequestId::from(3i64), get_req, None)
        .await;
    let task: Value = match response.payload {
        ResponsePayload::Result(v) => v,
        ResponsePayload::Error(e) => panic!("Get failed: {}", e.message),
    };
    println!("  Status: {}", task["status"]);

    // 7. Get the result
    println!("\nStep 5: Retrieve task result...");
    let result_req = Request::Client(Box::new(ClientRequest::TasksResult(
        json!({"taskId": task_id}),
    )));
    let response = server
        .handle_request(RequestId::from(4i64), result_req, None)
        .await;
    let result: Value = match response.payload {
        ResponsePayload::Result(v) => v,
        ResponsePayload::Error(e) => panic!("Result failed: {}", e.message),
    };
    println!(
        "  Result: {}",
        serde_json::to_string_pretty(&result["result"]).unwrap()
    );

    println!("\n=== Lifecycle Complete ===");
}
