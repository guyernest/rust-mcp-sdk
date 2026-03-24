# Task Lifecycle and Polling

In this section, you will build a complete task-enabled tool from scratch. You will configure the server's TaskStore, declare task support on a tool, write a handler that branches between sync and async execution, and implement a fallback tool for clients that do not understand tasks.

## Learning Objectives

By the end of this section, you will be able to:

- Wire up a `TaskStore` in the server builder
- Declare `TaskSupport::Optional` on a tool using `.with_execution()`
- Inspect `extra.is_task_request()` to choose the execution path
- Return a `CreateTaskResult` for task-mode requests
- Implement `get_task_result` as a fallback tool for non-task-aware clients

## Setting Up TaskStore in the Server Builder

The builder's `.task_store()` method does two things: it stores the backend and it auto-configures the server's capabilities so clients can discover task support during initialization.

```rust
use pmcp::prelude::*;
use pmcp::server::task_store::InMemoryTaskStore;
use std::sync::Arc;

let store = Arc::new(InMemoryTaskStore::new());

let server = Server::builder()
    .name("satellite-analysis")
    .version("1.0.0")
    .task_store(store.clone())
    // ... register tools ...
    .build()?;
```

After calling `.task_store()`, the server advertises these capabilities during `initialize`:

```json
{
  "capabilities": {
    "tasks": {
      "list": {},
      "cancel": {},
      "requests": {
        "tools": {
          "call": {}
        }
      }
    }
  }
}
```

This tells clients: "I support tasks for `tools/call` requests, and I support `tasks/list` and `tasks/cancel`." Clients that understand tasks will check this before sending task-augmented requests.

**Try this:** Build a minimal server with just `.task_store()` configured but no tools registered. Run it with `mcp-tester` and inspect the capabilities response. You should see the `tasks` field in the server capabilities.

## Declaring Task Support on Tools

Each tool declares its own task support level through the `execution` field. This tells clients whether they should, may, or must not use tasks with this specific tool.

```rust
use pmcp::types::{ToolExecution, TaskSupport};

let tool = TypedTool::new("analyze_imagery", |args: AnalyzeArgs, extra| {
    Box::pin(async move {
        // handler implementation (shown below)
        Ok(json!({}))
    })
})
.with_description("Analyze satellite imagery for terrain classification")
.with_execution(
    ToolExecution::new().with_task_support(TaskSupport::Optional)
);
```

The three support levels control client behavior:

| Level | Wire Value | Meaning | When to Use |
|-------|-----------|---------|-------------|
| `TaskSupport::Required` | `"required"` | Client MUST send `task` parameter | Operations that always exceed timeout limits |
| `TaskSupport::Optional` | `"optional"` | Client MAY send `task` parameter | Operations with variable duration |
| `TaskSupport::Forbidden` | `"forbidden"` | Client MUST NOT send `task` parameter | Fast operations, side-effect-free lookups |

**The golden rule:** Use `Optional` unless you have a strong reason not to. It gives every client a path to your tool -- task-aware clients get the async path, sync-only clients get the sync path.

The `execution` field appears in the `tools/list` response:

```json
{
  "name": "analyze_imagery",
  "description": "Analyze satellite imagery for terrain classification",
  "inputSchema": { "type": "object", "properties": { ... } },
  "execution": {
    "taskSupport": "optional"
  }
}
```

## Writing a Dual-Path Tool Handler

The core pattern for `TaskSupport::Optional` tools is a branch inside the handler. When `extra.is_task_request()` returns `true`, the client sent a `task` parameter and expects a `CreateTaskResult`. When it returns `false`, the client expects a synchronous `CallToolResult`.

Here is the full pattern, using a satellite imagery analysis scenario:

```rust
use pmcp::prelude::*;
use pmcp::server::task_store::TaskStore;
use pmcp::types::tasks::{Task, TaskStatus};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Deserialize, JsonSchema)]
struct AnalyzeArgs {
    /// S3 URI of the satellite image to analyze
    image_uri: String,
    /// Classification model to use
    #[serde(default = "default_model")]
    model: String,
}

fn default_model() -> String {
    "terrain-v3".to_string()
}

fn build_analyze_tool(
    store: Arc<dyn TaskStore>,
) -> TypedTool<AnalyzeArgs, impl Fn(AnalyzeArgs, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>> + Send + Sync> {
    let store_clone = store.clone();

    TypedTool::new("analyze_imagery", move |args: AnalyzeArgs, extra| {
        let store = store_clone.clone();
        Box::pin(async move {
            if extra.is_task_request() {
                // ── TASK PATH ──
                // Create a task record, spawn background work, return immediately
                let owner = extra.auth_context()
                    .map(|ctx| ctx.subject.clone())
                    .unwrap_or_else(|| "anonymous".to_string());

                let task = store.create(&owner, Some(300_000)).await?; // 5 min TTL

                // Spawn the actual analysis in the background
                let task_id = task.task_id.clone();
                let store_bg = store.clone();
                tokio::spawn(async move {
                    // Simulate long-running analysis
                    let result = run_analysis(&args.image_uri, &args.model).await;
                    match result {
                        Ok(_) => {
                            let _ = store_bg.update_status(
                                &task_id, &owner,
                                TaskStatus::Completed,
                                Some("Analysis complete".into()),
                            ).await;
                        }
                        Err(e) => {
                            let _ = store_bg.update_status(
                                &task_id, &owner,
                                TaskStatus::Failed,
                                Some(format!("Analysis failed: {e}")),
                            ).await;
                        }
                    }
                });

                // Return CreateTaskResult immediately
                Ok(json!({
                    "task": {
                        "taskId": task.task_id,
                        "status": "working",
                        "createdAt": task.created_at,
                        "lastUpdatedAt": task.last_updated_at,
                        "pollInterval": task.poll_interval,
                        "ttl": task.ttl
                    }
                }))
            } else {
                // ── SYNC PATH ──
                // Run the analysis inline and return the result directly
                let result = run_analysis(&args.image_uri, &args.model).await
                    .map_err(|e| Error::internal(format!("Analysis failed: {e}")))?;

                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string_pretty(&result)?
                    }]
                }))
            }
        })
    })
    .with_description("Analyze satellite imagery for terrain classification")
    .with_execution(
        ToolExecution::new().with_task_support(TaskSupport::Optional)
    )
}
```

Let's walk through the key decisions:

1. **Owner resolution** -- The `extra.auth_context()` provides the OAuth subject. For unauthenticated servers, fall back to a default. The owner ID ensures task isolation between users.

2. **TTL selection** -- The `300_000` (5 minutes) is the task lifetime, not the operation timeout. Choose based on how long a client might reasonably poll before giving up.

3. **Background spawn** -- The `tokio::spawn` runs the analysis after the handler returns. For Lambda deployments, you would instead enqueue to SQS and have a separate worker update the task.

4. **Sync fallback** -- The `else` branch runs the same analysis inline. This is why `Optional` works: every client gets a result, but task-aware clients get it faster (immediate acknowledgment instead of waiting).

## The Task Status State Machine

The `TaskStore` enforces valid status transitions. Your handler code only needs to call `update_status` -- the store rejects invalid transitions automatically.

```
                +--------------+
                |   working    |  <-- initial state from store.create()
                +------+-------+
                       |
          +------------+------------+
          v            |            v
  +----------------+   |   +-----------------+
  | input_required |---+   |    terminal     |
  +----------------+       | +-------------+ |
          |                | | completed   | |
          +--------------->  | failed      | |
                           | | cancelled   | |
                           | +-------------+ |
                           +-----------------+
```

Valid transitions:
- `working` -> `input_required`, `completed`, `failed`, `cancelled`
- `input_required` -> `working`, `completed`, `failed`, `cancelled`
- Terminal states (`completed`, `failed`, `cancelled`) -> nothing (any attempt returns `InvalidTransition`)

**Try this:** Write a test that creates a task, transitions it to `completed`, then tries to transition back to `working`. Verify that the store returns a `TaskStoreError::InvalidTransition`.

## The get_task_result Fallback Tool Pattern

Not every MCP client understands tasks. Claude Desktop does. A custom CLI tool you wrote last month probably does not. The `get_task_result` fallback tool bridges this gap by exposing task retrieval as a plain tool call.

```rust
use pmcp::prelude::*;
use pmcp::server::task_store::TaskStore;
use std::sync::Arc;

#[derive(Deserialize, JsonSchema)]
struct GetTaskResultArgs {
    /// The task ID returned by the original tool call
    task_id: String,
}

fn build_get_task_result_tool(
    store: Arc<dyn TaskStore>,
) -> TypedTool<GetTaskResultArgs, impl Fn(GetTaskResultArgs, RequestHandlerExtra) -> Pin<Box<dyn Future<Output = Result<Value>> + Send>> + Send + Sync> {
    let store_clone = store.clone();

    TypedTool::new("get_task_result", move |args: GetTaskResultArgs, extra| {
        let store = store_clone.clone();
        Box::pin(async move {
            let owner = extra.auth_context()
                .map(|ctx| ctx.subject.clone())
                .unwrap_or_else(|| "anonymous".to_string());

            let task = store.get(&args.task_id, &owner).await
                .map_err(|e| Error::not_found(format!("Task not found: {e}")))?;

            match task.status {
                TaskStatus::Completed => {
                    // Return the stored result
                    // (In practice, you would read from a result store)
                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": format!(
                                "Task {} completed: {}",
                                task.task_id,
                                task.status_message.unwrap_or_default()
                            )
                        }]
                    }))
                }
                TaskStatus::Failed => {
                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": format!(
                                "Task {} failed: {}",
                                task.task_id,
                                task.status_message.unwrap_or_else(|| "Unknown error".into())
                            )
                        }],
                        "isError": true
                    }))
                }
                _ => {
                    // Still running -- tell the client to wait
                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": format!(
                                "Task {} is still {}. Try again in {} seconds.",
                                task.task_id,
                                task.status,
                                task.poll_interval.unwrap_or(5000) / 1000
                            )
                        }]
                    }))
                }
            }
        })
    })
    .with_description("Check the status or retrieve the result of a background task")
    .with_execution(
        ToolExecution::new().with_task_support(TaskSupport::Forbidden)
    )
}
```

Note that `get_task_result` itself uses `TaskSupport::Forbidden` -- it is a synchronous lookup tool, not a long-running operation. The pattern works like this:

```
Non-task-aware client flow:

1. Client calls analyze_imagery (sync path, tool returns inline)
   OR
1. Client calls analyze_imagery (sync path times out)
2. Server internally created a task anyway (defensive pattern)
3. Client retries, gets task_id in error message
4. Client calls get_task_result(task_id: "...")
5. If still working: "Try again in 5 seconds"
6. If completed: returns the analysis result
```

**When to use this pattern:** Always include `get_task_result` if any of your tools use `TaskSupport::Optional` or `TaskSupport::Required`. It costs nothing to register and provides a universal fallback.

## Putting It All Together

Here is how the pieces connect in a complete server:

```rust
use pmcp::prelude::*;
use pmcp::server::task_store::InMemoryTaskStore;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let store = Arc::new(InMemoryTaskStore::new());

    let server = Server::builder()
        .name("satellite-analysis")
        .version("1.0.0")
        .task_store(store.clone())
        .tool("analyze_imagery", build_analyze_tool(store.clone()))
        .tool("get_task_result", build_get_task_result_tool(store.clone()))
        .build()?;

    server.run_stdio().await
}
```

The server advertises task support in its capabilities. The `analyze_imagery` tool declares `Optional` task support. Clients that send the `task` parameter get immediate acknowledgment. Clients that do not get synchronous results. And any client can use `get_task_result` as a universal polling mechanism.

## Key Takeaways

- `.task_store()` on the builder auto-configures server capabilities -- you do not set them manually
- `.with_execution(ToolExecution::new().with_task_support(...))` on the tool declares per-tool task behavior
- `extra.is_task_request()` is the branch point in your handler -- one handler, two execution paths
- The `get_task_result` fallback tool ensures every client can access async results, regardless of task support
- The `TaskStore` enforces the state machine -- your handler only calls `create`, `update_status`, and `get`

---

*Continue to [Capability Negotiation](./ch21-02-capability-negotiation.md) ->*
