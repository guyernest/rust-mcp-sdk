# MCP Tasks Feature - Detailed Design Document

**Status**: Draft
**Protocol Version**: 2025-11-25 (experimental)
**SDK Version Target**: 1.11.0
**Date**: 2026-02-08

---

## Table of Contents

1. [Overview](#1-overview)
2. [Motivation & PMCP Vision](#2-motivation--pmcp-vision)
3. [Crate Architecture](#3-crate-architecture)
4. [Layer 1: Core Protocol Types](#4-layer-1-core-protocol-types)
5. [Layer 2: TaskStore Trait](#5-layer-2-taskstore-trait)
6. [Layer 3: TaskContext (Handler Integration)](#6-layer-3-taskcontext-handler-integration)
7. [Layer 4: Storage Backends](#7-layer-4-storage-backends)
8. [Integration with Existing SDK](#8-integration-with-existing-sdk)
9. [Security: Owner Binding](#9-security-owner-binding)
10. [Examples](#10-examples)
11. [Test Plan](#11-test-plan)
12. [Migration & Compatibility](#12-migration--compatibility)
13. [Open Questions](#13-open-questions)

---

## 1. Overview

The MCP Tasks specification (2025-11-25, experimental) introduces durable state machines
that allow requestors and receivers to coordinate long-running operations through polling
and deferred result retrieval. This document describes how to implement Tasks in the PMCP
SDK as a **separate crate** (`pmcp-tasks`) to keep the experimental feature isolated.

### Spec Summary

Tasks augment existing requests (e.g., `tools/call`) with a two-phase response pattern:

1. **Phase 1**: Receiver accepts the request, immediately returns a `CreateTaskResult`
   with a task ID, status, and polling metadata.
2. **Phase 2**: Requestor polls via `tasks/get`, then retrieves the final result via
   `tasks/result` once the task reaches a terminal status.

### Task Status State Machine

```
                    ┌──────────────┐
                    │   working    │ ◄── initial state
                    └──────┬───────┘
                           │
              ┌────────────┼────────────┐
              ▼            │            ▼
     ┌────────────────┐   │   ┌─────────────────┐
     │ input_required │───┘   │    terminal      │
     └────────────────┘       │  ┌─────────────┐ │
              │               │  │  completed   │ │
              └───────────────►  │  failed      │ │
                              │  │  cancelled   │ │
                              │  └─────────────┘ │
                              └─────────────────┘
```

---

## 2. Motivation & PMCP Vision

PMCP extends the spec's minimal task model in two strategic directions:

### Direction 1: Server → Client (Prompt-Driven Workflows with Task State)

The SDK's existing workflow system (`SequentialWorkflow`) guides clients through multi-step
sequences via prompts. Tasks add **durability and shared state** to these workflows:

```
Prompt "deploy-service" (backed by a Task)
  ├── Step 1: tools/call validate_config
  │   └── task.variables += {region: "us-east-1", service_name: "my-api"}
  ├── Step 2: tools/call provision_infra
  │   └── reads ${region}, ${service_name} from task.variables
  ├── Step 3: input_required → elicitation for approval
  │   └── task status: working → input_required → working
  └── Step 4: tools/call deploy
      └── reads accumulated variables, task → completed
```

The server has no LLM — **task variables are its memory**. Each tool handler reads from
and writes to the task's variable store. The server tells the client what to call next
with pre-populated arguments derived from task variables.

### Direction 2: Client → Server (Code Mode with Task Context)

The pmcp.run "code mode" pattern (validate_code → execute_code) benefits from tasks:

```
Task "analyze customer data"
  ├── Step 1: validate_code(SQL)
  │   └── task.variables += {query: "...", table_schema: {...}}
  ├── Step 2: execute_code(SQL)
  │   └── task.variables += {result_set_id: "rs-42"}
  ├── Step 3: validate_code(SQL) — follow-up query
  │   └── reads ${result_set_id} from task.variables
  └── Step 4: execute_code(SQL)
      └── final result, task → completed
```

Task variables accumulate context so the LLM client doesn't lose track across tool calls,
and the server can enforce consistency and detect mistakes from either side.

### PMCP Extension: Task Variables

The spec defines tasks as `{taskId, status, ttl}`. PMCP extends this with a **variable
store** visible to both client and server:

```
Standard Spec Task:           PMCP Extended Task:
┌──────────────────┐          ┌────────────────────────────────────┐
│ taskId           │          │ taskId                             │
│ status           │          │ status                             │
│ ttl              │          │ ttl                                │
│ statusMessage    │          │ statusMessage                      │
│ createdAt        │          │ createdAt                          │
│ lastUpdatedAt    │          │ lastUpdatedAt                      │
│ pollInterval     │          │ pollInterval                       │
│                  │          │ owner_id (OAuth sub / client ID)   │
│                  │          │ variables: {                       │
│                  │          │   region: "us-east-1",             │
│                  │          │   service_name: "my-api",          │
│                  │          │   last_result_id: "abc123"         │
│                  │          │ }                                  │
└──────────────────┘          └────────────────────────────────────┘
```

Variables are surfaced to the client through `_meta` in task responses, making the task
a shared scratchpad that helps both sides stay aligned. This is the key innovation that
makes Tasks practical for servers that lack LLM capabilities.

---

## 3. Crate Architecture

The feature lives in a **separate crate** within the workspace, keeping it isolated from
the stable SDK while allowing optional integration:

```
rust-mcp-sdk/
├── Cargo.toml                     # workspace root (add pmcp-tasks)
├── crates/
│   └── pmcp-tasks/                # NEW: separate crate
│       ├── Cargo.toml
│       ├── src/
│       │   ├── lib.rs             # crate root, feature gating
│       │   ├── types.rs           # Layer 1: protocol types
│       │   ├── store.rs           # Layer 2: TaskStore trait
│       │   ├── context.rs         # Layer 3: TaskContext for handlers
│       │   ├── capabilities.rs    # Task capability types
│       │   ├── error.rs           # Task-specific errors
│       │   ├── middleware.rs      # TaskMiddleware for pmcp integration
│       │   └── backends/          # Layer 4: storage backends
│       │       ├── mod.rs
│       │       ├── memory.rs      # In-memory (dev/tests)
│       │       └── dynamodb.rs    # DynamoDB (Lambda/serverless)
│       └── tests/
│           ├── protocol_types.rs  # Type serialization tests
│           ├── store_memory.rs    # In-memory store tests
│           ├── store_dynamodb.rs  # DynamoDB store tests (integration)
│           ├── context.rs         # TaskContext behavior tests
│           ├── lifecycle.rs       # Task state machine tests
│           └── security.rs        # Owner binding tests
├── examples/
│   ├── 60_tasks_basic.rs          # Basic task-augmented tool calls
│   ├── 61_tasks_workflow.rs       # Tasks with SequentialWorkflow
│   ├── 62_tasks_code_mode.rs      # Tasks with validate/execute code
│   └── 63_tasks_dynamodb.rs       # Tasks with DynamoDB backend
└── src/                           # main pmcp crate (unchanged)
```

### Cargo.toml for `pmcp-tasks`

```toml
[package]
name = "pmcp-tasks"
version = "0.1.0"
edition = "2021"
description = "MCP Tasks support for the PMCP SDK (experimental)"
license = "MIT"
rust-version = "1.82.0"

[features]
default = []
dynamodb = ["aws-sdk-dynamodb", "aws-config"]

[dependencies]
# Core (always included - zero extra deps beyond pmcp)
pmcp = { path = "../..", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
async-trait = "0.1"
thiserror = "2.0"
uuid = { version = "1.17", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["sync", "time"] }
tracing = "0.1"
parking_lot = "0.12"

# DynamoDB backend (optional)
aws-sdk-dynamodb = { version = "1", optional = true }
aws-config = { version = "1", optional = true }

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

### Integration with main `pmcp` crate

The main `pmcp` crate can optionally re-export `pmcp-tasks`:

```toml
# In pmcp/Cargo.toml
[dependencies]
pmcp-tasks = { path = "crates/pmcp-tasks", optional = true }

[features]
tasks = ["pmcp-tasks"]
```

This allows users to either:
- `pmcp = { features = ["tasks"] }` — convenience re-export
- `pmcp-tasks = "0.1"` — direct dependency (more explicit)

---

## 4. Layer 1: Core Protocol Types

These types map 1:1 to the MCP specification schema. They have **zero additional
dependencies** beyond serde.

### `types.rs`

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

// ─── Task Status ─────────────────────────────────────────────

/// Task execution status.
///
/// Tasks follow this state machine:
/// - `working` → {`input_required`, `completed`, `failed`, `cancelled`}
/// - `input_required` → {`working`, `completed`, `failed`, `cancelled`}
/// - `completed`, `failed`, `cancelled` are terminal (no further transitions)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// The request is currently being processed
    Working,
    /// The receiver needs input from the requestor
    InputRequired,
    /// The request completed successfully
    Completed,
    /// The request did not complete successfully
    Failed,
    /// The request was cancelled before completion
    Cancelled,
}

impl TaskStatus {
    /// Returns true if this is a terminal status (no further transitions).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns the set of valid next states from this status.
    pub fn valid_transitions(&self) -> &[TaskStatus] {
        match self {
            Self::Working => &[
                Self::InputRequired,
                Self::Completed,
                Self::Failed,
                Self::Cancelled,
            ],
            Self::InputRequired => &[
                Self::Working,
                Self::Completed,
                Self::Failed,
                Self::Cancelled,
            ],
            Self::Completed | Self::Failed | Self::Cancelled => &[],
        }
    }

    /// Check if transitioning to `next` is valid.
    pub fn can_transition_to(&self, next: &TaskStatus) -> bool {
        self.valid_transitions().contains(next)
    }
}

// ─── Task ────────────────────────────────────────────────────

/// A task representing the execution state of a request.
///
/// This is the core data type returned by `tasks/get` and `tasks/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique identifier for the task (generated by receiver)
    pub task_id: String,

    /// Current execution status
    pub status: TaskStatus,

    /// Optional human-readable status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// ISO 8601 timestamp when the task was created
    pub created_at: DateTime<Utc>,

    /// ISO 8601 timestamp when the task was last updated
    pub last_updated_at: DateTime<Utc>,

    /// Time in milliseconds from creation before task may be deleted.
    /// `None` means unlimited lifetime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,

    /// Suggested polling interval in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

// ─── PMCP Extension: Task with Variables ─────────────────────

/// PMCP-extended task that includes a variable store.
///
/// Variables are shared state visible to both client and server,
/// accumulated across tool calls within the same task. The server
/// uses variables as its memory (since it has no LLM), and the
/// client can use them to stay aligned with the server's state.
///
/// Variables are surfaced to the client via `_meta` in task responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWithVariables {
    /// Standard MCP task fields
    #[serde(flatten)]
    pub task: Task,

    /// PMCP extension: shared variable store.
    ///
    /// Keys are variable names, values are arbitrary JSON.
    /// Variables accumulate across tool calls within the task.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variables: HashMap<String, Value>,
}

// ─── Task Parameters (request augmentation) ──────────────────

/// Parameters for augmenting a request with task execution.
///
/// Included in `params.task` when creating a task-augmented request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskParams {
    /// Requested TTL in milliseconds from creation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
}

// ─── CreateTaskResult ────────────────────────────────────────

/// Result returned when a receiver accepts a task-augmented request.
///
/// This is the Phase 1 response — it contains task metadata but not
/// the actual operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    /// The created task
    pub task: Task,

    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<Map<String, Value>>,
}

// ─── tasks/get ───────────────────────────────────────────────

/// Request parameters for `tasks/get`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGetParams {
    /// Task ID to retrieve
    pub task_id: String,
}

/// Result for `tasks/get` — returns the current task state.
pub type TaskGetResult = Task;

// ─── tasks/result ────────────────────────────────────────────

/// Request parameters for `tasks/result`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResultParams {
    /// Task ID to retrieve the result for
    pub task_id: String,
}

/// Result for `tasks/result` — the actual operation result.
///
/// The structure matches the original request type. For `tools/call`
/// tasks, this is a `CallToolResult`. The `_meta` field MUST include
/// `io.modelcontextprotocol/related-task`.
///
/// We use `Value` here because the result type depends on the
/// original request type.
pub type TaskResultResponse = Value;

// ─── tasks/list ──────────────────────────────────────────────

/// Request parameters for `tasks/list`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListParams {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Result for `tasks/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListResult {
    /// List of tasks
    pub tasks: Vec<Task>,

    /// Next page cursor (if more tasks available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

// ─── tasks/cancel ────────────────────────────────────────────

/// Request parameters for `tasks/cancel`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCancelParams {
    /// Task ID to cancel
    pub task_id: String,
}

/// Result for `tasks/cancel` — returns the cancelled task.
pub type TaskCancelResult = Task;

// ─── notifications/tasks/status ──────────────────────────────

/// Notification parameters for `notifications/tasks/status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusNotification {
    /// Task ID
    pub task_id: String,

    /// Updated status
    pub status: TaskStatus,

    /// Optional status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// When the task was created
    pub created_at: DateTime<Utc>,

    /// When the task was last updated
    pub last_updated_at: DateTime<Utc>,

    /// TTL in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,

    /// Suggested polling interval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

// ─── Related Task Metadata ───────────────────────────────────

/// Metadata key for associating messages with tasks.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";

/// Related task metadata value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedTaskMeta {
    pub task_id: String,
}

/// Helper to create related-task metadata for `_meta` fields.
pub fn related_task_meta(task_id: &str) -> (String, Value) {
    (
        RELATED_TASK_META_KEY.to_string(),
        serde_json::to_value(RelatedTaskMeta {
            task_id: task_id.to_string(),
        })
        .expect("RelatedTaskMeta is always serializable"),
    )
}

// ─── Tool-Level Task Support ─────────────────────────────────

/// Tool-level declaration of task support.
///
/// Present in `tools/list` response under `execution.taskSupport`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSupport {
    /// Clients MUST NOT invoke the tool as a task (default)
    Forbidden,
    /// Clients MAY invoke the tool as a task or as a normal request
    Optional,
    /// Clients MUST invoke the tool as a task
    Required,
}

impl Default for TaskSupport {
    fn default() -> Self {
        Self::Forbidden
    }
}

/// Tool execution metadata (included in `tools/list` response).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    /// Whether/how the tool supports task augmentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_support: Option<TaskSupport>,
}

// ─── Model Immediate Response ────────────────────────────────

/// Meta key for providing an immediate response to the model while
/// a task executes in the background.
pub const MODEL_IMMEDIATE_RESPONSE_KEY: &str =
    "io.modelcontextprotocol/model-immediate-response";
```

### `capabilities.rs`

```rust
use serde::{Deserialize, Serialize};

/// Server-side task capabilities.
///
/// Declared in `capabilities.tasks` during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTaskCapabilities {
    /// Server supports `tasks/list`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<TaskListCapability>,

    /// Server supports `tasks/cancel`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<TaskCancelCapability>,

    /// Which request types support task augmentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ServerTaskRequests>,
}

/// Task list capability (empty object signals support).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskListCapability {}

/// Task cancel capability (empty object signals support).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskCancelCapability {}

/// Server-side request types that support task augmentation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTaskRequests {
    /// Tools namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsTaskRequests>,
}

/// Tool-specific task request support.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsTaskRequests {
    /// `tools/call` supports task augmentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<TaskRequestCapability>,
}

/// Empty capability marker for a specific request type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskRequestCapability {}

/// Client-side task capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTaskCapabilities {
    /// Client supports `tasks/list`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<TaskListCapability>,

    /// Client supports `tasks/cancel`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<TaskCancelCapability>,

    /// Which request types support task augmentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ClientTaskRequests>,
}

/// Client-side request types that support task augmentation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTaskRequests {
    /// Sampling namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingTaskRequests>,

    /// Elicitation namespace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<ElicitationTaskRequests>,
}

/// Sampling task request support.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingTaskRequests {
    /// `sampling/createMessage` supports task augmentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_message: Option<TaskRequestCapability>,
}

/// Elicitation task request support.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationTaskRequests {
    /// `elicitation/create` supports task augmentation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create: Option<TaskRequestCapability>,
}

// ─── Convenience constructors ────────────────────────────────

impl ServerTaskCapabilities {
    /// Full server task support (list + cancel + tools/call).
    pub fn full() -> Self {
        Self {
            list: Some(TaskListCapability {}),
            cancel: Some(TaskCancelCapability {}),
            requests: Some(ServerTaskRequests {
                tools: Some(ToolsTaskRequests {
                    call: Some(TaskRequestCapability {}),
                }),
            }),
        }
    }

    /// Tools-only task support (no list/cancel).
    pub fn tools_only() -> Self {
        Self {
            list: None,
            cancel: None,
            requests: Some(ServerTaskRequests {
                tools: Some(ToolsTaskRequests {
                    call: Some(TaskRequestCapability {}),
                }),
            }),
        }
    }
}
```

---

## 5. Layer 2: TaskStore Trait

The storage abstraction that backends implement. This is the extension point.

### `store.rs`

```rust
use crate::error::TaskError;
use crate::types::{Task, TaskStatus, TaskWithVariables};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

/// Owned task data in the store (superset of protocol Task).
///
/// The store manages both the protocol-visible fields and PMCP
/// extensions (variables, owner, result).
#[derive(Debug, Clone)]
pub struct TaskRecord {
    /// Protocol-visible task data
    pub task: Task,

    /// PMCP extension: owner identity (OAuth sub or client ID)
    pub owner_id: String,

    /// PMCP extension: shared variable store
    pub variables: HashMap<String, Value>,

    /// The final result (set when task reaches terminal status).
    /// Stored as the raw JSON-RPC response value.
    pub result: Option<Value>,

    /// The original request method (e.g., "tools/call") for
    /// determining the result type.
    pub request_method: String,
}

/// Options for listing tasks.
#[derive(Debug, Clone, Default)]
pub struct ListTasksOptions {
    /// Filter by owner
    pub owner_id: String,
    /// Pagination cursor
    pub cursor: Option<String>,
    /// Maximum number of results
    pub limit: usize,
}

/// A page of task results.
#[derive(Debug, Clone)]
pub struct TaskPage {
    /// Tasks in this page
    pub tasks: Vec<Task>,
    /// Cursor for the next page (None if no more results)
    pub next_cursor: Option<String>,
}

/// Abstract storage backend for tasks.
///
/// Implementors must handle:
/// - Thread safety (Send + Sync)
/// - TTL enforcement (cleanup of expired tasks)
/// - Ownership enforcement (owner_id matching)
/// - Atomic status transitions (no invalid state changes)
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Create a new task record.
    ///
    /// The store MUST:
    /// - Generate a unique task ID
    /// - Set initial status to `Working`
    /// - Set `created_at` and `last_updated_at` to now
    /// - Associate the task with the given owner
    async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError>;

    /// Get a task by ID, enforcing owner access.
    ///
    /// Returns `TaskError::NotFound` if the task doesn't exist or
    /// belongs to a different owner.
    async fn get(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskError>;

    /// Update task status with an atomic transition.
    ///
    /// The store MUST:
    /// - Validate the transition (reject invalid state changes)
    /// - Update `last_updated_at`
    /// - Enforce owner access
    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        new_status: TaskStatus,
        status_message: Option<String>,
    ) -> Result<TaskRecord, TaskError>;

    /// Set task variables (merge with existing).
    ///
    /// Variables are merged: new keys are added, existing keys
    /// are overwritten. To delete a variable, set it to `Value::Null`.
    async fn set_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError>;

    /// Store the final result for a completed/failed task.
    ///
    /// The store MUST reject this if the task is not in a terminal status.
    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<(), TaskError>;

    /// Get the final result for a task.
    ///
    /// Returns `TaskError::NotReady` if the task hasn't reached
    /// a terminal status yet.
    async fn get_result(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<Value, TaskError>;

    /// List tasks for an owner with pagination.
    async fn list(
        &self,
        options: ListTasksOptions,
    ) -> Result<TaskPage, TaskError>;

    /// Cancel a task.
    ///
    /// The store MUST:
    /// - Reject if already in a terminal status
    /// - Transition to `Cancelled`
    /// - Enforce owner access
    async fn cancel(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskError>;

    /// Clean up expired tasks.
    ///
    /// Called periodically or lazily. Removes tasks whose
    /// `created_at + ttl` has elapsed.
    async fn cleanup_expired(&self) -> Result<usize, TaskError>;
}
```

---

## 6. Layer 3: TaskContext (Handler Integration)

The `TaskContext` is what tool handlers interact with. It wraps a `TaskStore` reference
and provides an ergonomic API for reading/writing task state within a tool call.

### `context.rs`

```rust
use crate::error::TaskError;
use crate::store::TaskStore;
use crate::types::TaskStatus;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Context passed to tool handlers for task-augmented requests.
///
/// Provides read/write access to the task's variable store and
/// status management. This is the primary interface tool handlers
/// use to interact with tasks.
///
/// # Example
///
/// ```rust,ignore
/// async fn handle(
///     &self,
///     args: Value,
///     extra: RequestHandlerExtra,
///     task_ctx: Option<TaskContext>,
/// ) -> Result<Value, Error> {
///     if let Some(ctx) = &task_ctx {
///         // Read a variable set by a previous step
///         let region = ctx.get_variable("region").await?;
///
///         // Set variables for the next step
///         ctx.set_variable("instance_id", json!("i-1234")).await?;
///
///         // Read all variables
///         let vars = ctx.variables().await?;
///     }
///     Ok(json!({"status": "done"}))
/// }
/// ```
#[derive(Clone)]
pub struct TaskContext {
    store: Arc<dyn TaskStore>,
    task_id: String,
    owner_id: String,
}

impl TaskContext {
    /// Create a new task context.
    pub fn new(
        store: Arc<dyn TaskStore>,
        task_id: String,
        owner_id: String,
    ) -> Self {
        Self {
            store,
            task_id,
            owner_id,
        }
    }

    /// Get the task ID.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Get a single variable value.
    pub async fn get_variable(&self, key: &str) -> Result<Option<Value>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).cloned())
    }

    /// Set a single variable.
    pub async fn set_variable(
        &self,
        key: impl Into<String>,
        value: Value,
    ) -> Result<(), TaskError> {
        let mut vars = HashMap::new();
        vars.insert(key.into(), value);
        self.store
            .set_variables(&self.task_id, &self.owner_id, vars)
            .await?;
        Ok(())
    }

    /// Set multiple variables at once (atomic merge).
    pub async fn set_variables(
        &self,
        variables: HashMap<String, Value>,
    ) -> Result<(), TaskError> {
        self.store
            .set_variables(&self.task_id, &self.owner_id, variables)
            .await?;
        Ok(())
    }

    /// Get all variables.
    pub async fn variables(&self) -> Result<HashMap<String, Value>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables)
    }

    /// Transition the task to `input_required` status.
    ///
    /// Use this when the tool needs input from the client (e.g.,
    /// user confirmation via elicitation).
    pub async fn require_input(
        &self,
        message: impl Into<String>,
    ) -> Result<(), TaskError> {
        self.store
            .update_status(
                &self.task_id,
                &self.owner_id,
                TaskStatus::InputRequired,
                Some(message.into()),
            )
            .await?;
        Ok(())
    }

    /// Mark the task as failed.
    pub async fn fail(&self, message: impl Into<String>) -> Result<(), TaskError> {
        self.store
            .update_status(
                &self.task_id,
                &self.owner_id,
                TaskStatus::Failed,
                Some(message.into()),
            )
            .await?;
        Ok(())
    }

    /// Store the final result and mark the task as completed.
    pub async fn complete(&self, result: Value) -> Result<(), TaskError> {
        self.store
            .update_status(
                &self.task_id,
                &self.owner_id,
                TaskStatus::Completed,
                None,
            )
            .await?;
        self.store
            .set_result(&self.task_id, &self.owner_id, result)
            .await?;
        Ok(())
    }
}
```

---

## 7. Layer 4: Storage Backends

### 7.1 In-Memory Backend (`backends/memory.rs`)

For development, testing, and single-instance deployments.

```rust
use crate::error::TaskError;
use crate::store::{ListTasksOptions, TaskPage, TaskRecord, TaskStore};
use crate::types::{Task, TaskStatus};
use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// In-memory task store.
///
/// Suitable for development, testing, and single-instance servers.
/// Tasks are lost on process restart.
pub struct InMemoryTaskStore {
    tasks: RwLock<HashMap<String, TaskRecord>>,
    default_poll_interval: u64,
    max_ttl: Option<u64>,
}

impl InMemoryTaskStore {
    pub fn new() -> Self {
        Self {
            tasks: RwLock::new(HashMap::new()),
            default_poll_interval: 5000,
            max_ttl: None,
        }
    }

    /// Set the default poll interval (milliseconds).
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.default_poll_interval = ms;
        self
    }

    /// Set maximum allowed TTL (milliseconds).
    pub fn with_max_ttl(mut self, ms: u64) -> Self {
        self.max_ttl = Some(ms);
        self
    }
}

#[async_trait]
impl TaskStore for InMemoryTaskStore {
    // ... implementation validates state transitions,
    // enforces owner_id matching, handles TTL, etc.
}
```

### 7.2 DynamoDB Backend (`backends/dynamodb.rs`)

For Lambda/serverless deployments. Feature-gated behind `dynamodb`.

#### DynamoDB Table Schema

```
Table: mcp-tasks

Primary Key:
  PK (String): TASK#{taskId}       — partition key
  SK (String): OWNER#{owner_id}    — sort key (enables owner enforcement)

GSI: owner-index
  GSI1PK (String): OWNER#{owner_id}  — for tasks/list by owner
  GSI1SK (String): CREATED#{iso8601}  — for chronological ordering

Attributes:
  status       (String)  — TaskStatus enum value
  statusMessage (String) — optional
  variables    (Map)     — PMCP task variables
  result       (String)  — JSON-encoded final result
  requestMethod (String) — original request method
  createdAt    (String)  — ISO 8601
  lastUpdatedAt (String) — ISO 8601
  ttl_epoch    (Number)  — DynamoDB TTL (Unix epoch seconds)
  pollInterval (Number)  — suggested polling interval
```

#### Key Design Decisions

1. **DynamoDB TTL**: The `ttl_epoch` attribute uses DynamoDB's native TTL feature.
   DynamoDB automatically deletes expired items within ~48 hours, so we also check
   expiration on read for immediate enforcement.

2. **Conditional Writes**: Status transitions use `ConditionExpression` to enforce
   the state machine atomically:
   ```
   UpdateExpression: SET #status = :new_status
   ConditionExpression: #status IN (:valid_from_states)
   ```

3. **Owner Enforcement**: The sort key `OWNER#{owner_id}` means a `GetItem` with
   the wrong owner simply returns no item (treated as not found).

4. **Pagination**: The GSI supports cursor-based pagination via `ExclusiveStartKey`.

```rust
/// DynamoDB-backed task store.
///
/// Suitable for Lambda, ECS, and other serverless/container deployments.
/// Tasks persist across invocations and survive process restarts.
///
/// # Table Setup
///
/// The table must be created with the schema documented in the design doc.
/// Use `DynamoDbTaskStore::create_table()` for development, or provision
/// via CloudFormation/CDK in production.
#[cfg(feature = "dynamodb")]
pub struct DynamoDbTaskStore {
    client: aws_sdk_dynamodb::Client,
    table_name: String,
    default_poll_interval: u64,
    max_ttl: Option<u64>,
}

#[cfg(feature = "dynamodb")]
impl DynamoDbTaskStore {
    pub async fn new(table_name: impl Into<String>) -> Self {
        let config = aws_config::load_defaults(
            aws_config::BehaviorVersion::latest()
        ).await;
        let client = aws_sdk_dynamodb::Client::new(&config);
        Self {
            client,
            table_name: table_name.into(),
            default_poll_interval: 5000,
            max_ttl: None,
        }
    }

    /// Create the DynamoDB table (for development/testing).
    pub async fn create_table(&self) -> Result<(), TaskError> {
        // Creates table + GSI with the documented schema
    }
}
```

---

## 8. Integration with Existing SDK

### 8.1 Capability Negotiation

Tasks capabilities must be added to `ServerCapabilities` and `ClientCapabilities`.
Since `pmcp-tasks` is a separate crate, we integrate via the existing `experimental`
field or by adding a `tasks` field behind a feature flag.

**Approach**: Use the `experimental` capability field for now (since Tasks are
experimental in the spec), with a migration path to a dedicated field:

```rust
// In the server builder, when tasks are enabled:
let mut caps = ServerCapabilities::tools_only();

// Add task capabilities via experimental (spec-compliant for experimental features)
let task_caps = ServerTaskCapabilities::full();
caps.experimental = Some(HashMap::from([(
    "tasks".to_string(),
    serde_json::to_value(task_caps).unwrap(),
)]));

// When the spec stabilizes, we add a dedicated field:
// caps.tasks = Some(task_caps);
```

**Future (post-stabilization)**: Add `tasks: Option<ServerTaskCapabilities>` directly
to `ServerCapabilities` and `ClientCapabilities` in the main `pmcp` crate.

### 8.2 Tool Registration with Task Support

Tools declare task support via `execution.taskSupport` in `tools/list`. We extend
`ToolInfo` (or use `_meta`) to carry this:

```rust
// Using ToolAnnotations extension or _meta
let tool = ToolInfo::new("long_running_analysis", "Analyze data", schema)
    .with_task_support(TaskSupport::Optional);

// Internally, this adds to the ToolInfo JSON:
// { "execution": { "taskSupport": "optional" } }
```

### 8.3 Request Routing

New JSON-RPC methods to handle in the server router:

| Method | Handler |
|--------|---------|
| `tasks/get` | `TaskHandler::get` |
| `tasks/result` | `TaskHandler::result` |
| `tasks/list` | `TaskHandler::list` |
| `tasks/cancel` | `TaskHandler::cancel` |

These are handled by a `TaskHandler` that wraps a `TaskStore`:

```rust
/// Handles task-related JSON-RPC methods.
pub struct TaskHandler {
    store: Arc<dyn TaskStore>,
}

impl TaskHandler {
    pub async fn handle_get(&self, params: TaskGetParams, owner_id: &str)
        -> Result<Task, TaskError>;

    pub async fn handle_result(&self, params: TaskResultParams, owner_id: &str)
        -> Result<Value, TaskError>;

    pub async fn handle_list(&self, params: TaskListParams, owner_id: &str)
        -> Result<TaskListResult, TaskError>;

    pub async fn handle_cancel(&self, params: TaskCancelParams, owner_id: &str)
        -> Result<Task, TaskError>;
}
```

### 8.4 Tool Middleware Integration

A `TaskMiddleware` intercepts `tools/call` requests that include the `task` field:

```rust
/// Middleware that detects task-augmented tool calls and manages
/// the task lifecycle around tool execution.
pub struct TaskMiddleware {
    store: Arc<dyn TaskStore>,
}

impl ToolMiddleware for TaskMiddleware {
    async fn on_request(
        &self,
        tool_name: &str,
        args: &mut Value,
        extra: &mut RequestHandlerExtra,
    ) -> Result<()> {
        // 1. Check if args contains "task" field
        // 2. If yes, create task in store
        // 3. Attach TaskContext to extra.metadata
        // 4. Return CreateTaskResult immediately (short-circuit)
        // 5. Spawn background execution of the actual tool
    }
}
```

### 8.5 Workflow Integration

The existing `SequentialWorkflow` system can be enhanced to optionally back workflows
with tasks. When a workflow prompt is invoked with task support:

1. A task is created for the workflow
2. Each step reads/writes task variables via `TaskContext`
3. The workflow's `DataSource::StepOutput` resolves from task variables
4. If a step needs client input, the task moves to `input_required`

```rust
let workflow = SequentialWorkflow::new("deploy", "Deploy a service")
    .with_task_support(TaskSupport::Optional)  // NEW
    .argument("region", "AWS region", true)
    .step(
        WorkflowStep::new("validate", ToolHandle::new("validate_config"))
            .arg("region", prompt_arg("region"))
            .bind("config")  // stored in task.variables["config"]
    )
    .step(
        WorkflowStep::new("deploy", ToolHandle::new("deploy_service"))
            .arg("config", from_step("config"))  // read from task.variables
            .bind("deployment")
    );
```

### 8.6 Owner ID Resolution

The owner is resolved from the request context:

```rust
/// Resolve the task owner from the request context.
///
/// Priority:
/// 1. OAuth token `sub` claim (most secure)
/// 2. Client ID from transport headers
/// 3. Session ID (least secure, single-tenant only)
pub fn resolve_owner_id(extra: &RequestHandlerExtra) -> String {
    // 1. Try OAuth subject
    if let Some(auth_ctx) = &extra.auth_context {
        if let Some(user_id) = &auth_ctx.user_id {
            return user_id.clone();
        }
    }

    // 2. Try client ID from auth info
    if let Some(auth_info) = &extra.auth_info {
        if let Some(client_id) = &auth_info.client_id {
            return client_id.clone();
        }
    }

    // 3. Fall back to session ID
    extra
        .session_id
        .clone()
        .unwrap_or_else(|| "anonymous".to_string())
}
```

---

## 9. Security: Owner Binding

### Threat Model

| Threat | Mitigation |
|--------|------------|
| Task ID guessing | UUIDv4 (122 bits of entropy) |
| Cross-owner access | Owner ID enforced on every operation |
| Task enumeration | `tasks/list` scoped to owner |
| Resource exhaustion | Max concurrent tasks per owner, max TTL |
| Stale task accumulation | TTL enforcement (DynamoDB native + lazy check) |

### Configuration

```rust
/// Security configuration for the task system.
pub struct TaskSecurityConfig {
    /// Maximum concurrent tasks per owner (default: 100)
    pub max_tasks_per_owner: usize,

    /// Maximum allowed TTL in milliseconds (default: 24 hours)
    pub max_ttl_ms: u64,

    /// Default TTL when not specified (default: 1 hour)
    pub default_ttl_ms: u64,

    /// Whether to allow anonymous owners (default: false)
    pub allow_anonymous: bool,
}

impl Default for TaskSecurityConfig {
    fn default() -> Self {
        Self {
            max_tasks_per_owner: 100,
            max_ttl_ms: 86_400_000,      // 24 hours
            default_ttl_ms: 3_600_000,    // 1 hour
            allow_anonymous: false,
        }
    }
}
```

---

## 10. Examples

### Example 60: Basic Task-Augmented Tool Call

**File**: `examples/60_tasks_basic.rs`

Demonstrates the simplest task flow:
1. Server registers a "slow" tool with `TaskSupport::Optional`
2. Client sends `tools/call` with `task: { ttl: 60000 }`
3. Server returns `CreateTaskResult` immediately
4. Client polls `tasks/get` until status is `completed`
5. Client retrieves result via `tasks/result`

Key concepts: task creation, polling, result retrieval.

### Example 61: Tasks with Sequential Workflow

**File**: `examples/61_tasks_workflow.rs`

Demonstrates task variables in a multi-step workflow:
1. Server defines a `SequentialWorkflow` backed by a task
2. Step 1 writes variables (e.g., `config`) to the task store
3. Step 2 reads variables from the task store to populate arguments
4. Steps use `TaskContext` to read/write shared state
5. Client observes variable accumulation across steps

Key concepts: task variables, workflow integration, server-side state.

### Example 62: Tasks with Code Mode

**File**: `examples/62_tasks_code_mode.rs`

Demonstrates the pmcp.run code mode pattern with tasks:
1. Server exposes `validate_code` and `execute_code` tools
2. Client creates a task for a multi-step analysis
3. Step 1: `validate_code(SQL)` → stores `query` and `table_schema` in task variables
4. Step 2: `execute_code(SQL)` → stores `result_set_id` in task variables
5. Step 3: `validate_code(SQL)` → reads `result_set_id` for follow-up query
6. Step 4: `execute_code(SQL)` → final result, task completes

Key concepts: code mode, variable accumulation, multi-step consistency.

### Example 63: Tasks with DynamoDB Backend

**File**: `examples/63_tasks_dynamodb.rs`

Demonstrates production deployment with DynamoDB:
1. Server uses `DynamoDbTaskStore` (requires `dynamodb` feature)
2. Multiple Lambda invocations share task state
3. OAuth-based owner binding
4. DynamoDB TTL handles automatic cleanup
5. Shows table creation for development

Key concepts: serverless deployment, persistent storage, OAuth integration.

---

## 11. Test Plan

### 11.1 Unit Tests

#### Protocol Type Tests (`tests/protocol_types.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_task_status_serialization` | All TaskStatus variants serialize to `snake_case` |
| `test_task_status_deserialization` | Round-trip for all status values |
| `test_task_serialization` | Task struct serializes with all fields |
| `test_task_optional_fields` | `None` fields are skipped in JSON |
| `test_create_task_result_shape` | Matches spec JSON examples |
| `test_task_params_minimal` | Empty `task: {}` is valid |
| `test_task_params_with_ttl` | `task: { ttl: 60000 }` serializes correctly |
| `test_related_task_meta_helper` | `related_task_meta()` produces correct JSON |
| `test_task_support_enum` | Forbidden/Optional/Required serialize correctly |
| `test_task_with_variables_serialization` | PMCP extension serializes with standard fields |
| `test_task_capabilities_serialization` | Capabilities match spec JSON examples |
| `test_task_list_result_pagination` | `nextCursor` present when more results exist |
| `test_task_status_notification_shape` | Notification matches spec structure |

#### State Machine Tests (`tests/lifecycle.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_valid_transitions_from_working` | working → {input_required, completed, failed, cancelled} |
| `test_valid_transitions_from_input_required` | input_required → {working, completed, failed, cancelled} |
| `test_terminal_states_have_no_transitions` | completed/failed/cancelled → nothing |
| `test_is_terminal` | Only completed/failed/cancelled are terminal |
| `test_cannot_transition_completed_to_working` | Rejects invalid transitions |
| `test_cannot_transition_failed_to_completed` | Rejects cross-terminal transitions |
| `test_working_to_input_required_roundtrip` | working → input_required → working |

#### TaskContext Tests (`tests/context.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_get_variable_empty` | Returns `None` for nonexistent key |
| `test_set_and_get_variable` | Round-trip for single variable |
| `test_set_multiple_variables` | Atomic multi-variable set |
| `test_variable_merge_semantics` | New keys added, existing overwritten |
| `test_null_variable_deletion` | Setting `Value::Null` removes the key |
| `test_require_input` | Transitions to `input_required` |
| `test_fail` | Transitions to `failed` with message |
| `test_complete_stores_result` | Stores result and transitions to `completed` |

### 11.2 Store Tests (per backend)

These tests run against **every** `TaskStore` implementation (memory, DynamoDB).
They use a shared test harness to ensure consistent behavior.

#### `tests/store_memory.rs` and `tests/store_dynamodb.rs`

| Test | What it verifies |
|------|-----------------|
| `test_create_task` | Returns task with `Working` status, valid timestamps |
| `test_create_task_unique_ids` | 1000 tasks all have unique IDs |
| `test_get_task_by_owner` | Owner can retrieve their task |
| `test_get_task_wrong_owner` | Returns `NotFound` for different owner |
| `test_get_task_nonexistent` | Returns `NotFound` for invalid ID |
| `test_update_status_valid` | working → completed succeeds |
| `test_update_status_invalid` | completed → working returns `InvalidTransition` |
| `test_update_status_updates_timestamp` | `last_updated_at` changes |
| `test_set_variables_merge` | Merges with existing variables |
| `test_set_variables_overwrite` | Overwrites existing keys |
| `test_set_result_on_completed` | Stores result for completed task |
| `test_set_result_on_working_fails` | Rejects result for non-terminal task |
| `test_get_result_completed` | Returns stored result |
| `test_get_result_not_ready` | Returns `NotReady` for working task |
| `test_list_tasks_by_owner` | Returns only tasks for the specified owner |
| `test_list_tasks_pagination` | Cursor-based pagination works |
| `test_list_tasks_empty` | Returns empty list for owner with no tasks |
| `test_cancel_working_task` | Transitions to cancelled |
| `test_cancel_completed_task_fails` | Rejects cancellation of terminal task |
| `test_cleanup_expired` | Removes tasks past their TTL |
| `test_expired_task_not_returned` | `get()` returns `NotFound` for expired tasks |
| `test_ttl_enforcement` | Store respects max TTL override |
| `test_concurrent_updates` | Multiple threads can safely update different tasks |
| `test_concurrent_same_task` | Race condition on same task is handled (one wins) |

### 11.3 Security Tests (`tests/security.rs`)

| Test | What it verifies |
|------|-----------------|
| `test_owner_isolation_get` | Owner A cannot get Owner B's task |
| `test_owner_isolation_list` | Owner A's list doesn't include Owner B's tasks |
| `test_owner_isolation_cancel` | Owner A cannot cancel Owner B's task |
| `test_owner_isolation_set_variables` | Owner A cannot write Owner B's variables |
| `test_owner_isolation_set_result` | Owner A cannot set Owner B's result |
| `test_anonymous_owner_rejected` | Anonymous owner rejected when not allowed |
| `test_anonymous_owner_allowed` | Anonymous owner works in single-tenant mode |
| `test_max_tasks_per_owner` | 101st task creation fails with resource exhaustion |
| `test_task_id_entropy` | Task IDs are UUIDv4 format |
| `test_oauth_subject_binding` | Task bound to OAuth `sub` claim |
| `test_client_id_fallback` | Falls back to client ID when no OAuth |

### 11.4 Integration Tests

| Test | What it verifies |
|------|-----------------|
| `test_full_task_lifecycle` | Create → poll → complete → get_result end-to-end |
| `test_task_with_input_required` | Create → working → input_required → working → completed |
| `test_task_cancellation_flow` | Create → working → cancel → verify cancelled |
| `test_task_failure_flow` | Create → working → failed → get_result returns error |
| `test_task_with_variables_across_calls` | Variables persist across multiple tool calls |
| `test_task_with_workflow` | Workflow steps read/write task variables |
| `test_task_middleware_intercept` | Middleware detects `task` field and creates task |
| `test_task_polling_interval` | Client respects `pollInterval` from server |
| `test_task_ttl_cleanup` | Expired tasks are cleaned up |

### 11.5 Property Tests

| Property | What it verifies |
|----------|-----------------|
| `prop_status_transitions_only_valid` | Random transitions only succeed if valid |
| `prop_terminal_is_forever` | Once terminal, no transition succeeds |
| `prop_variables_merge_is_commutative` | Order of variable sets doesn't matter |
| `prop_task_ids_always_unique` | Creating N tasks produces N unique IDs |
| `prop_owner_isolation_holds` | Random operations never cross owner boundaries |

### 11.6 DynamoDB Integration Tests

These require a local DynamoDB (e.g., `docker run -p 8000:8000 amazon/dynamodb-local`).

| Test | What it verifies |
|------|-----------------|
| `test_dynamodb_create_table` | Table and GSI are created correctly |
| `test_dynamodb_conditional_write_race` | Concurrent status updates — one wins |
| `test_dynamodb_ttl_attribute` | `ttl_epoch` is set correctly for DynamoDB TTL |
| `test_dynamodb_gsi_pagination` | GSI-based owner listing with cursor |
| `test_dynamodb_large_variables` | Variables with 400KB payload (DynamoDB item limit) |

---

## 12. Migration & Compatibility

### Phase 1: Experimental (current plan)

- Tasks live in `pmcp-tasks` crate
- Capabilities advertised via `experimental.tasks`
- No changes to core `pmcp` crate types
- Feature flag in `pmcp`: `tasks = ["pmcp-tasks"]`

### Phase 2: Stabilization (when spec stabilizes)

- Add `tasks: Option<TaskCapabilities>` to `ServerCapabilities` / `ClientCapabilities`
- Add `task: Option<TaskParams>` to `CallToolRequest`
- Add `execution: Option<ToolExecution>` to `ToolInfo`
- Keep `pmcp-tasks` for storage backends, but move types into core
- Deprecate `experimental.tasks` with migration guide

### Phase 3: Advanced Features

- Redis backend
- Task progress streaming (SSE integration)
- Workflow-backed tasks with automatic step management
- Task analytics and monitoring hooks

---

## 13. Open Questions

### Resolved

1. **Crate structure**: Separate `pmcp-tasks` crate (decided: yes, experimental isolation)
2. **Variables visibility**: Shared between client and server (decided: yes, via `_meta`)
3. **Primary backend**: DynamoDB (decided: yes, given PMCP's AWS focus)

### Open

1. **`tasks/result` blocking behavior for Lambda**: Lambda has a max execution time.
   Should `tasks/result` always return immediately with current state (polling-only),
   or support a `timeout` parameter for bounded blocking?
   **Leaning**: Polling-only for Lambda, with optional bounded blocking for long-running
   servers.

2. **Variable size limits**: DynamoDB has a 400KB item limit. Should we enforce a
   variable size limit in the `TaskStore` trait, or leave it to backends?
   **Leaning**: Backend-specific, but document recommendations (e.g., keep variables
   under 100KB, use S3 for large data).

3. **Task status notifications**: The spec says notifications are optional. For
   stateless HTTP servers, should we skip them entirely and rely on polling?
   **Leaning**: Support notifications where the transport allows (SSE), skip for
   pure Lambda.

4. **CloudFormation/CDK template**: Should we ship a CloudFormation template for
   the DynamoDB table as part of the crate?
   **Leaning**: Yes, as a static JSON/YAML file in the crate, plus a `create_table()`
   helper for development.

5. **Variable namespacing**: Should task variables use flat keys (`region`) or
   namespaced keys (`step.validate_config.region`)?
   **Leaning**: Flat keys for simplicity, with a convention recommendation for
   namespacing in docs.
