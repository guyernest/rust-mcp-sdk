# Tasks with Polling

MCP Tasks let servers manage long-running operations that outlive a single request/response cycle. Instead of SSE-based notifications, the client polls for status updates on a timer — stateless, simple, and compatible with serverless deployments.

## Quick Start

```rust
use pmcp::{Server, InMemoryTaskStore};
use std::sync::Arc;

let store = Arc::new(InMemoryTaskStore::new());
let server = Server::builder()
    .name("my-server")
    .version("1.0.0")
    .task_store(store.clone())
    // ... register tools, prompts, resources ...
    .build()?;
```

That's it. The builder:
- Sets `ServerCapabilities.tasks` automatically (list, cancel, tools/call)
- Dispatches `tasks/get`, `tasks/list`, `tasks/cancel` through your store
- Resolves task owners from auth context for multi-tenant isolation

## How Polling Works

```
Client                              Server
  |                                    |
  |-- tools/call (long-running) ------>|
  |<-- 202 + Task { status: working } -|
  |                                    |
  |-- tasks/get { task_id } ---------->|  (poll every 2-5s)
  |<-- Task { status: working } -------|
  |                                    |
  |-- tasks/get { task_id } ---------->|
  |<-- Task { status: completed } -----|
  |                                    |
  |   (client stops polling)           |
```

The `Task` response includes `poll_interval` (milliseconds) to guide the client's polling frequency. Terminal statuses (`completed`, `failed`, `cancelled`) signal the client to stop polling.

## Task Status State Machine

```
                 ┌──────────────┐
                 │   Working    │
                 └──────┬───────┘
                   ┌────┼─────┬────────────┐
                   │    │     │            │
                   v    │     v            v
          ┌────────────┐│  ┌────────┐  ┌───────────┐
          │InputRequired││  │Completed│  │ Cancelled │
          └──────┬─────┘│  └─────────┘  └───────────┘
                 │      │                     ^
                 └──────┘     ┌───────┐       │
                              │ Failed │       │
                              └────────┘       │
                   InputRequired ──────────────┘
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

## Using TaskStore in a Tool Handler

A typical pattern: the tool creates a task, spawns background work, and returns the task immediately. The client polls `tasks/get` until completion.

```rust
use pmcp::{TaskStore, InMemoryTaskStore};
use pmcp::types::tasks::TaskStatus;
use std::sync::Arc;

struct MyLongRunningTool {
    task_store: Arc<InMemoryTaskStore>,
}

// Inside the tool's execute method:
async fn execute(&self, owner_id: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    // 1. Create a task
    let task = self.task_store.create(owner_id, Some(300_000)).await?; // 5 min TTL
    let task_id = task.task_id.clone();
    let store = self.task_store.clone();

    // 2. Spawn background work
    tokio::spawn(async move {
        // ... do expensive work ...
        let _ = store.update_status(
            &task_id, owner_id,
            TaskStatus::Completed,
            Some("Done processing".into()),
        ).await;
    });

    // 3. Return the task immediately
    Ok(serde_json::to_value(&task)?)
}
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

This means multi-tenant deployments with OAuth get automatic task isolation — User A cannot see or cancel User B's tasks.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                   pmcp (SDK)                     │
│                                                  │
│  types/tasks.rs     Task, TaskStatus (wire types)│
│  server/task_store  TaskStore trait               │
│                     InMemoryTaskStore (dev/test)  │
│  server/builder     .task_store() integration     │
│  server/core        Dispatch + capability neg.    │
└─────────────────────────────────────────────────┘
          │
          │  (optional, for production)
          v
┌─────────────────────────────────────────────────┐
│              pmcp-tasks (extension)               │
│                                                  │
│  DynamoDB backend    Redis backend               │
│  Task variables      Task result storage         │
│  Security config     Owner binding               │
└─────────────────────────────────────────────────┘
```

The SDK provides everything needed for development and testing. Add `pmcp-tasks` when you need production persistence or PMCP extensions (task variables, result storage).
