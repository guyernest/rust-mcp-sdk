//! Example: Task-Prompt Bridge Lifecycle
//!
//! Demonstrates the complete task-prompt bridge lifecycle, from workflow
//! invocation through structured handoff to client continuation and task
//! completion. This replaces the former `62_task_workflow_opt_in` example
//! with a full end-to-end walkthrough.
//!
//! **Scenario:** A 3-step data processing pipeline:
//!   1. `fetch_data` -- fetch raw data from a source (fails server-side)
//!   2. `transform_data` -- transform the raw data (depends on fetch output)
//!   3. `store_data` -- persist the transformed result
//!
//! **What happens:**
//!   - The server creates a task and attempts to execute steps automatically.
//!   - `fetch_data` returns an error, triggering `PauseReason::ToolError`.
//!   - `transform_data` cannot resolve its input (depends on `raw_data` from
//!     the failed fetch step), so it stays pending.
//!   - The server returns a structured handoff with `_meta` (task_id, step
//!     progress, pause reason) and a narrative message list.
//!   - The client calls `fetch_data` with `_task_id` in `_meta` to continue.
//!   - The client sends `tasks/cancel` with a result to mark the task complete.
//!   - The client polls `tasks/result` to verify the final state.
//!
//! **Key concepts demonstrated:**
//!   - `_meta` field on `GetPromptResult` for task-aware workflows
//!   - Full message list: user intent, assistant plan, tool call/result, handoff
//!   - Client continuation via `_task_id` in `CallTool._meta`
//!   - Cancel-with-result for explicit task completion
//!
//! Run: `cargo run --example 62_task_workflow_lifecycle`

use async_trait::async_trait;
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::server::core::ProtocolHandler;
use pmcp::server::workflow::{DataSource, SequentialWorkflow, ToolHandle, WorkflowStep};
use pmcp::types::jsonrpc::ResponsePayload;
use pmcp::types::{
    CallToolParams, ClientRequest, GetPromptParams, Request, RequestId, RequestMeta, ToolInfo,
};
use pmcp::RequestHandlerExtra;
use pmcp_tasks::{InMemoryTaskStore, TaskRouterImpl, TaskSecurityConfig};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Tool handlers
// ---------------------------------------------------------------------------

/// Fetches raw data from a source.
///
/// On the server-side execution path, this tool always returns an error to
/// simulate an external API being unavailable. This triggers the handoff:
/// the server pauses with `PauseReason::ToolError` and the client must
/// call this tool separately with `_task_id` to provide the data.
struct FetchDataTool;

#[async_trait]
impl pmcp::ToolHandler for FetchDataTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        // Intentionally fail during server-side execution.
        // The client will retry this tool with _task_id after receiving the handoff.
        Err(pmcp::Error::internal(
            "External API unavailable - client must provide data",
        ))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "fetch_data",
            Some("Fetch raw data from a source".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Data source identifier"
                    }
                },
                "required": ["source"]
            }),
        ))
    }
}

/// Transforms raw data into a processed format.
struct TransformDataTool;

#[async_trait]
impl pmcp::ToolHandler for TransformDataTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "transformed": true,
            "result": args.get("input").cloned().unwrap_or(Value::Null)
        }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "transform_data",
            Some("Transform raw data into processed format".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Raw data to transform"
                    }
                },
                "required": ["input"]
            }),
        ))
    }
}

/// Stores processed data to a persistent location.
struct StoreDataTool;

#[async_trait]
impl pmcp::ToolHandler for StoreDataTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({
            "stored": true,
            "location": "db://processed"
        }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "store_data",
            Some("Store processed data to database".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "data": {
                        "type": "string",
                        "description": "Processed data to store"
                    }
                },
                "required": ["data"]
            }),
        ))
    }
}

fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        println!("=== Task-Prompt Bridge: Full Lifecycle ===\n");

        // =====================================================================
        // Stage 1: Build the server
        // =====================================================================
        //
        // Create a task store and router, define a 3-step data pipeline workflow
        // with task support enabled, register tools and the workflow on the
        // server builder, and use stateless_mode(true) for direct handle_request
        // calls without the initialization handshake.
        // =====================================================================

        println!("--- Stage 1: Build the server ---\n");

        let store = Arc::new(
            InMemoryTaskStore::new()
                .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
        );
        let router = Arc::new(TaskRouterImpl::new(store));

        // Define the 3-step data pipeline:
        //   fetch (binds "raw_data") -> transform (uses raw_data, binds "transformed") -> store
        let workflow = SequentialWorkflow::new("data_pipeline", "Fetch, transform, and store data")
            .argument("source", "Data source identifier", true)
            .step(
                WorkflowStep::new("fetch", ToolHandle::new("fetch_data"))
                    .arg("source", DataSource::prompt_arg("source"))
                    .bind("raw_data")
                    .retryable(true),
            )
            .step(
                WorkflowStep::new("transform", ToolHandle::new("transform_data"))
                    .arg("input", DataSource::from_step("raw_data"))
                    .bind("transformed"),
            )
            .step(
                WorkflowStep::new("store", ToolHandle::new("store_data"))
                    .arg("data", DataSource::from_step("transformed")),
            )
            .with_task_support(true);

        let server = ServerCoreBuilder::new()
            .name("lifecycle-example")
            .version("1.0.0")
            .tool("fetch_data", FetchDataTool)
            .tool("transform_data", TransformDataTool)
            .tool("store_data", StoreDataTool)
            .with_task_store(router)
            .prompt_workflow(workflow)
            .expect("workflow should register")
            .stateless_mode(true)
            .build()
            .expect("server should build");

        println!("  Server built with task router and 3-step data pipeline workflow");
        println!("  Tools: fetch_data, transform_data, store_data");
        println!("  Workflow: data_pipeline (task_support = true)\n");

        // =====================================================================
        // Stage 2: Invoke the workflow prompt
        // =====================================================================
        //
        // Construct a GetPrompt request for "data_pipeline" with the "source"
        // argument. The server creates a task, then attempts to execute steps
        // server-side:
        //   - fetch_data fails -> PauseReason::ToolError at step 1
        //   - transform_data cannot resolve "input" (depends on "raw_data"
        //     from the failed fetch step) -> stays pending
        //   - store_data also stays pending
        // =====================================================================

        println!("--- Stage 2: Invoke the workflow prompt ---\n");

        let prompt_args = HashMap::from([("source".to_string(), "production_api".to_string())]);

        let get_prompt_req = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptParams {
            name: "data_pipeline".to_string(),
            arguments: prompt_args,
            _meta: None,
        })));

        let response = server
            .handle_request(RequestId::from(1i64), get_prompt_req, None)
            .await;

        let prompt_result: Value = match response.payload {
            ResponsePayload::Result(v) => v,
            ResponsePayload::Error(e) => panic!("GetPrompt failed: {}", e.message),
        };

        println!("  GetPrompt response received.");
        println!(
            "  Description: {}",
            prompt_result
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("(none)")
        );

        // =====================================================================
        // Stage 3: Inspect the handoff
        // =====================================================================
        //
        // Extract _meta from the GetPromptResult. This contains:
        //   - task_id: the durable task backing this workflow
        //   - task_status: "working" (not all steps completed)
        //   - steps: array with each step's name and status
        //   - pause_reason: why execution paused (toolError on fetch)
        //
        // Then print the full message list so the reader sees exactly what
        // an LLM client receives: user intent, assistant plan, tool call/result
        // pairs, and the handoff narrative.
        // =====================================================================

        println!("\n--- Stage 3: Inspect the handoff ---\n");

        let meta = prompt_result
            .get("_meta")
            .expect("task workflow should include _meta");

        let task_id = meta["task_id"]
            .as_str()
            .expect("_meta should contain task_id");
        let task_status = meta["task_status"]
            .as_str()
            .expect("_meta should contain task_status");

        println!("  _meta.task_id:     {}", task_id);
        println!("  _meta.task_status: {}", task_status);

        // Print step progress
        if let Some(steps) = meta["steps"].as_array() {
            println!("  _meta.steps:");
            for step in steps {
                println!(
                    "    - {} : {}",
                    step["name"].as_str().unwrap_or("?"),
                    step["status"].as_str().unwrap_or("?")
                );
            }
        }

        // Print pause reason
        if let Some(pause) = meta.get("pause_reason") {
            println!(
                "\n  _meta.pause_reason:\n{}",
                serde_json::to_string_pretty(pause).unwrap_or_default()
            );
        }

        // Print the full message list from the handoff.
        // This is the core of the teaching example: the reader sees exactly
        // what an LLM client receives when a workflow pauses.
        println!("\n  Full message list ({} messages):", {
            prompt_result["messages"].as_array().map_or(0, |m| m.len())
        });
        println!("  {}", "=".repeat(60));

        if let Some(messages) = prompt_result["messages"].as_array() {
            for (i, msg) in messages.iter().enumerate() {
                let role = msg["role"].as_str().unwrap_or("unknown");
                let role_label = match role {
                    "user" => "USER",
                    "assistant" => "ASSISTANT",
                    "system" => "SYSTEM",
                    other => other,
                };

                println!("\n  Message {} [{}]:", i + 1, role_label);

                // Handle both text content and other content types
                if let Some(text) = msg["content"]["text"].as_str() {
                    // Indent each line of the message for readability
                    for line in text.lines() {
                        println!("    {}", line);
                    }
                } else {
                    println!(
                        "    {}",
                        serde_json::to_string_pretty(&msg["content"]).unwrap_or_default()
                    );
                }
            }
        }

        println!("\n  {}", "=".repeat(60));

        // =====================================================================
        // Stage 4: Client continuation
        // =====================================================================
        //
        // The client now calls "fetch_data" with _task_id in _meta to indicate
        // this is a continuation of the workflow task. The server executes the
        // tool (which still fails in this example since FetchDataTool always
        // errors), but the continuation is recorded fire-and-forget against the
        // task. In a real scenario, the client would use a different tool
        // handler or provide the data directly.
        //
        // For this example, we demonstrate the request shape and the server's
        // fire-and-forget recording behavior. The tool result (success or error)
        // is returned to the client regardless.
        // =====================================================================

        println!("\n--- Stage 4: Client continuation ---\n");

        let continuation_req = Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
            name: "fetch_data".to_string(),
            arguments: json!({"source": "production_api"}),
            _meta: Some(RequestMeta {
                progress_token: None,
                _task_id: Some(task_id.to_string()),
            }),
            task: None,
        })));

        println!("  Sending CallTool 'fetch_data' with _task_id: {}", task_id);

        let response = server
            .handle_request(RequestId::from(2i64), continuation_req, None)
            .await;

        match &response.payload {
            ResponsePayload::Result(v) => {
                println!("  Tool succeeded:");
                println!(
                    "    {}",
                    serde_json::to_string_pretty(v).unwrap_or_default()
                );
            },
            ResponsePayload::Error(e) => {
                // Expected: FetchDataTool always fails. The continuation
                // recording is fire-and-forget -- the tool error is returned
                // to the client as usual.
                println!("  Tool returned error (expected -- FetchDataTool always fails):");
                println!("    {}", e.message);
                println!("  The continuation was recorded fire-and-forget against the task.");
            },
        }

        // =====================================================================
        // Stage 5: Complete the workflow
        // =====================================================================
        //
        // The client sends tasks/cancel with a result payload to mark the task
        // as completed. Per the cancel-with-result convention, providing a
        // result field transitions the task to Completed (not Cancelled).
        //
        // Then we poll tasks/result to verify the final state and print the
        // completed task with its stored result.
        // =====================================================================

        println!("\n--- Stage 5: Complete the workflow ---\n");

        let cancel_result_payload = json!({
            "summary": "Data pipeline completed by client",
            "fetched_data": {"source": "production_api", "rows": 1500},
            "transformed": true,
            "stored_at": "db://processed/production_api"
        });

        let cancel_req = Request::Client(Box::new(ClientRequest::TasksCancel(json!({
            "taskId": task_id,
            "result": cancel_result_payload
        }))));

        println!("  Sending tasks/cancel with result for task: {}", task_id);

        let response = server
            .handle_request(RequestId::from(3i64), cancel_req, None)
            .await;

        let cancel_response: Value = match response.payload {
            ResponsePayload::Result(v) => v,
            ResponsePayload::Error(e) => panic!("tasks/cancel failed: {}", e.message),
        };

        println!(
            "  Cancel response status: {}",
            cancel_response["status"].as_str().unwrap_or("unknown")
        );
        println!(
            "  Task transitioned to: {} (cancel-with-result = Completed, not Cancelled)",
            cancel_response["status"].as_str().unwrap_or("unknown")
        );

        // Poll tasks/result to verify final state
        println!("\n  Polling tasks/result to verify final state...");

        let result_req = Request::Client(Box::new(ClientRequest::TasksResult(json!({
            "taskId": task_id
        }))));

        let response = server
            .handle_request(RequestId::from(4i64), result_req, None)
            .await;

        let final_result: Value = match response.payload {
            ResponsePayload::Result(v) => v,
            ResponsePayload::Error(e) => panic!("tasks/result failed: {}", e.message),
        };

        println!("  Final task result:");
        println!(
            "{}",
            serde_json::to_string_pretty(&final_result)
                .unwrap_or_default()
                .lines()
                .map(|line| format!("    {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        );

        println!("\n=== Lifecycle Complete ===");
        println!("\nSummary of what happened:");
        println!("  1. Server built with task router and data pipeline workflow");
        println!("  2. GetPrompt created a task, executed steps until fetch_data failed");
        println!("  3. Handoff returned _meta with task_id, step progress, and pause reason");
        println!("  4. Client called fetch_data with _task_id for continuation");
        println!("  5. Client sent tasks/cancel with result to complete the task");
    });
}
