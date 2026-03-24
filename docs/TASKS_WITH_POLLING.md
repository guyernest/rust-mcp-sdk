# Tasks with Polling

MCP Tasks let servers manage long-running operations that outlive a single request/response cycle. Instead of SSE-based notifications, the client polls for status updates on a timer -- stateless, simple, and compatible with serverless deployments.

In v2.0, task detection is **requestor-driven**: the SDK returns `CreateTaskResult` only when the client sends a `task` field in the `tools/call` request. Without it, the tool result is wrapped as a normal `CallToolResult`. This is capability-based negotiation, not client sniffing.

## Quick Start

```rust
use pmcp::server::builder::ServerCoreBuilder;
use pmcp::types::{ToolExecution, TaskSupport};
use pmcp_tasks::{InMemoryTaskStore, TaskRouterImpl, TaskSecurityConfig};
use std::sync::Arc;

let store = Arc::new(
    InMemoryTaskStore::new()
        .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
);
let router = Arc::new(TaskRouterImpl::new(store.clone()));

let server = ServerCoreBuilder::new()
    .name("my-server")
    .version("1.0.0")
    .with_task_store(router)
    // ... register tools with .with_execution() ...
    .build()?;
```

The builder:
- Sets `ServerCapabilities.tasks` automatically (list, cancel, tools/call)
- Dispatches `tasks/get`, `tasks/list`, `tasks/cancel` through your store
- Resolves task owners from auth context for multi-tenant isolation
- Returns `CreateTaskResult` or `CallToolResult` based on the client's request

## Declaring Task Support on Tools

Every tool that participates in the task lifecycle must declare its `TaskSupport` level via `ToolExecution`:

```rust
use pmcp::server::typed_tool::TypedTool;
use pmcp::types::{ToolExecution, TaskSupport};
use serde_json::json;

// Required: tool MUST be called with task augmentation
let tool = TypedTool::new_with_schema(
    "long_analysis",
    json!({"type": "object"}),
    |args, extra| Box::pin(async move { /* ... */ Ok(json!({})) }),
)
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));

// Optional: tool works with or without task augmentation
let tool = TypedTool::new_with_schema(
    "flexible_query",
    json!({"type": "object"}),
    |args, extra| Box::pin(async move { /* ... */ Ok(json!({})) }),
)
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Optional));

// Forbidden: tool never creates tasks (default when execution is omitted)
let tool = TypedTool::new_with_schema(
    "quick_lookup",
    json!({"type": "object"}),
    |args, extra| Box::pin(async move { /* ... */ Ok(json!({})) }),
)
.with_execution(ToolExecution::new().with_task_support(TaskSupport::Forbidden));
```

The three levels:

| TaskSupport | Meaning |
|-------------|---------|
| `Required` | Tool always produces a task. If the client omits the `task` field, the SDK logs a warning and falls through to `CallToolResult` for compatibility. |
| `Optional` | Tool can work both ways. Use `extra.is_task_request()` in the handler to branch. |
| `Forbidden` | Tool never creates tasks. The SDK ignores any `task` field on the request. |

## Capability Negotiation

An MCP server should not ask "who is the client?" It should ask "what capabilities has the client declared?"

The `task` field in the `tools/call` request IS the per-request capability signal. No session state is needed, no capability handshake during `initialize`. The SDK enforces this with a four-part check before returning `CreateTaskResult`:

1. `task_store` is configured on the server
2. The tool declares `taskSupport` of `Required` or `Optional`
3. The client sent a `task` field in the request (explicit task-augmented call)
4. The tool handler returned a Task-shaped value (has `taskId` + `status`)

When any condition is not met, the SDK falls through to `CallToolResult`. This means:
- Task-aware clients (Claude Desktop, custom agents) get `CreateTaskResult` with polling
- Non-task-aware clients (ChatGPT, older clients) get `CallToolResult` with text content

The tool handler itself decides what to return based on `extra.is_task_request()`.

## How Polling Works

### Task-Aware Client (sends `task` field)

```
Client                              Server
  |                                    |
  |-- tools/call { task: {...} } ----->|
  |<-- CreateTaskResult { task } ------|  (status: working)
  |                                    |
  |-- tasks/get { task_id } ---------->|  (poll every 2-5s)
  |<-- Task { status: working } -------|
  |                                    |
  |-- tasks/get { task_id } ---------->|
  |<-- Task { status: completed } -----|
  |                                    |
  |   (client stops polling)           |
```

### Non-Task-Aware Client (no `task` field)

```
Client                              Server
  |                                    |
  |-- tools/call (no task field) ----->|
  |<-- CallToolResult { text } --------|  (task_id embedded in text)
  |                                    |
  |-- tools/call get_task_result ----->|  (fallback polling tool)
  |<-- CallToolResult { text } --------|  (status: working)
  |                                    |
  |-- tools/call get_task_result ----->|
  |<-- CallToolResult { text } --------|  (status: completed + result)
  |                                    |
  |   (client stops polling)           |
```

The `Task` response includes `poll_interval` (milliseconds) to guide the client's polling frequency. Terminal statuses (`completed`, `failed`, `cancelled`) signal the client to stop polling.

## Using `extra.is_task_request()` in a Tool Handler

The `RequestHandlerExtra.is_task_request()` method is the primary tool for building dual-path handlers. It returns `true` when the client sent a `task` field in the request.

```rust
use pmcp::{TaskStore, RequestHandlerExtra};
use pmcp_tasks::InMemoryTaskStore;
use pmcp_tasks::task::TaskStatus;
use serde_json::{json, Value};
use std::sync::Arc;

struct FlexibleTool {
    task_store: Arc<InMemoryTaskStore>,
}

// Inside the tool's execute method:
async fn execute(
    &self,
    args: Value,
    extra: RequestHandlerExtra,
) -> pmcp::Result<Value> {
    if extra.is_task_request() {
        // Async path: create task, spawn background work, return immediately
        let task = self.task_store.create("local", Some(300_000)).await?;
        let task_id = task.task_id.clone();
        let store = self.task_store.clone();

        tokio::spawn(async move {
            // ... do expensive work ...
            let _ = store.update_status(
                &task_id, "local",
                TaskStatus::Completed,
                Some("Done processing".into()),
            ).await;
        });

        // Return task-shaped value; SDK wraps as CreateTaskResult
        Ok(json!({
            "taskId": task.task_id,
            "status": "working"
        }))
    } else {
        // Sync path: do the work inline, return CallToolResult text
        let result = do_work_inline(&args).await;
        Ok(json!({ "result": result }))
    }
}
```

This pattern is the foundation for `TaskSupport::Optional` tools. The same tool serves both task-aware and non-task-aware clients without separate implementations.

## Client Compatibility

### The `get_task_result` Fallback Tool Pattern

Non-task-aware clients (like ChatGPT) cannot send the `task` field or call `tasks/get`. For these clients, a recommended server-side pattern is to expose a fallback polling tool:

```rust
// This is a server-implemented pattern, NOT a built-in SDK feature.
// The server registers a regular tool that wraps task store reads.

fn get_task_result_tool(store: Arc<InMemoryTaskStore>) -> TypedTool {
    TypedTool::new_with_schema(
        "get_task_result",
        json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string", "description": "Task ID to check" }
            },
            "required": ["task_id"]
        }),
        move |args: Value, _extra| {
            let store = store.clone();
            Box::pin(async move {
                let task_id = args["task_id"].as_str()
                    .ok_or_else(|| pmcp::Error::validation("task_id required"))?;
                let task = store.get(task_id, "local").await?;
                Ok(serde_json::to_value(&task)?)
            })
        },
    )
    .with_description("Check the status and result of a background task")
    // Forbidden: this tool itself should NOT create tasks
    .with_execution(ToolExecution::new().with_task_support(TaskSupport::Forbidden))
}
```

The dual-path flow for `TaskSupport::Optional` tools in serverless deployments:

| Client sends `task` | Tool behavior | Response type |
|---------------------|---------------|---------------|
| Yes | Create task, return immediately | `CreateTaskResult` -- client polls `tasks/get` |
| No | Create task anyway, return task info as text | `CallToolResult` -- client calls `get_task_result` tool |

The SDK does not provide `get_task_result` as a built-in because it is a server policy decision (naming, access control, what fields to expose). Servers implement it as a regular tool.

## Task Status State Machine

```
                 +--------------+
                 |   Working    |
                 +------+-------+
                   +----+-----+------------+
                   |    |     |            |
                   v    |     v            v
          +------------+|  +--------+  +-----------+
          |InputRequired||  |Completed|  | Cancelled |
          +------+-----+|  +---------+  +-----------+
                 |      |                     ^
                 +------+     +-------+       |
                              | Failed |       |
                              +--------+       |
                   InputRequired --------------+
```

- **Working** -> InputRequired, Completed, Failed, Cancelled
- **InputRequired** -> Working, Completed, Failed, Cancelled
- **Completed, Failed, Cancelled** -> (terminal, no transitions)
- Self-transitions (Working -> Working) are rejected

```rust
use pmcp::types::tasks::TaskStatus;

assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Completed));
assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Working));
assert!(TaskStatus::Failed.is_terminal());
```

## TaskStore Trait

The `TaskStore` trait defines 7 methods for task lifecycle management:

| Method | Description |
|--------|-------------|
| `create(owner_id, ttl)` | Create a new task in `Working` state |
| `get(task_id, owner_id)` | Retrieve a task (owner-scoped) |
| `update_status(task_id, owner_id, status, message)` | Transition to a new status |
| `list(owner_id, cursor)` | List tasks with cursor pagination |
| `cancel(task_id, owner_id)` | Cancel a task |
| `cleanup_expired()` | Remove expired tasks |
| `config()` | Get store configuration |

All methods enforce **owner isolation**: accessing a task owned by someone else returns `NotFound` (never reveals existence).

### Implementing a Custom TaskStore

```rust
use pmcp::server::task_store::{TaskStore, TaskStoreError, StoreConfig};
use pmcp::types::tasks::{Task, TaskStatus};
use async_trait::async_trait;

struct MyDatabaseStore { /* ... */ }

#[async_trait]
impl TaskStore for MyDatabaseStore {
    async fn create(&self, owner_id: &str, ttl: Option<u64>) -> Result<Task, TaskStoreError> {
        // INSERT INTO tasks ...
        todo!()
    }

    async fn get(&self, task_id: &str, owner_id: &str) -> Result<Task, TaskStoreError> {
        // SELECT FROM tasks WHERE id = ? AND owner_id = ?
        todo!()
    }

    // ... implement remaining methods ...
    # async fn update_status(&self, _: &str, _: &str, _: TaskStatus, _: Option<String>) -> Result<Task, TaskStoreError> { todo!() }
    # async fn list(&self, _: &str, _: Option<&str>) -> Result<(Vec<Task>, Option<String>), TaskStoreError> { todo!() }
    # async fn cancel(&self, _: &str, _: &str) -> Result<Task, TaskStoreError> { todo!() }
    # async fn cleanup_expired(&self) -> Result<usize, TaskStoreError> { todo!() }
    # fn config(&self) -> &StoreConfig { todo!() }
}
```

For production backends (DynamoDB, Redis), see the `pmcp-tasks` crate.

## Configuration

`StoreConfig` controls task store behavior:

```rust
use pmcp::StoreConfig;
use pmcp::InMemoryTaskStore;

let store = InMemoryTaskStore::with_config(StoreConfig {
    default_ttl_ms: Some(300_000),      // 5 minutes (default: 1 hour)
    max_ttl_ms: Some(3_600_000),        // 1 hour max (default: 24 hours)
    default_poll_interval_ms: 3000,     // 3 seconds (default: 5 seconds)
    max_tasks_per_owner: 50,            // Per-owner limit (default: 100)
});
```

| Setting | Default | Description |
|---------|---------|-------------|
| `default_ttl_ms` | 3,600,000 (1h) | Applied when `create()` receives `None` for TTL |
| `max_ttl_ms` | 86,400,000 (24h) | Requested TTL values are clamped to this ceiling |
| `default_poll_interval_ms` | 5,000 (5s) | Suggested to clients in `Task.poll_interval` |
| `max_tasks_per_owner` | 100 | Active (non-expired) tasks per owner |

## Cleanup

`InMemoryTaskStore` does not run automatic background cleanup. Call `cleanup_expired()` periodically if you use TTL:

```rust
use std::sync::Arc;
use pmcp::InMemoryTaskStore;

let store = Arc::new(InMemoryTaskStore::new());
let cleanup_store = store.clone();

tokio::spawn(async move {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    loop {
        interval.tick().await;
        let _ = cleanup_store.cleanup_expired().await;
    }
});
```

Expired tasks are filtered from `get()`, `list()`, and the `max_tasks_per_owner` count, so cleanup primarily reclaims memory.

## Error Handling

`TaskStoreError` variants map to appropriate SDK error types:

| TaskStoreError | SDK Error | JSON-RPC | When |
|----------------|-----------|----------|------|
| `NotFound` | `Error::not_found` | -32602 | Task ID doesn't exist or wrong owner |
| `InvalidTransition` | `Error::validation` | -32602 | Invalid state machine transition |
| `Expired` | `Error::not_found` | -32602 | Task TTL elapsed (same as NotFound for privacy) |
| `Internal` | `Error::internal` | -32603 | Quota exceeded, storage failure |

## Owner Resolution

When a request arrives, the server resolves the task owner from the auth context:

| Auth Present | Owner ID |
|-------------|----------|
| OAuth token with `client_id` | `client_id` |
| OAuth token without `client_id` | `subject` (sub claim) |
| No auth | `"local"` (single-tenant) |

This means multi-tenant deployments with OAuth get automatic task isolation -- User A cannot see or cancel User B's tasks.

## Architecture

```
+--------------------------------------------------+
|                   pmcp (SDK)                      |
|                                                   |
|  types/tasks.rs     Task, TaskStatus (wire types) |
|  types/tools.rs     ToolExecution, TaskSupport    |
|  server/cancellation.rs  is_task_request()        |
|  server/core        Dispatch + capability neg.    |
|                     CreateTaskResult / CallToolResult
|                     routing based on request       |
+--------------------------------------------------+
          |
          |  (optional, for production)
          v
+--------------------------------------------------+
|              pmcp-tasks (extension)                |
|                                                   |
|  InMemoryTaskStore   TaskRouterImpl               |
|  DynamoDB backend    Redis backend                |
|  Task variables      Task result storage          |
|  Security config     Owner binding                |
+--------------------------------------------------+
```

The SDK provides everything needed for development and testing. Add `pmcp-tasks` when you need production persistence or PMCP extensions (task variables, result storage).
