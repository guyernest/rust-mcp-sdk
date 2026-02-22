# Phase 1: Foundation Types and Store Contract - Research

**Researched:** 2026-02-21
**Domain:** Rust type system, serde serialization, MCP Tasks protocol (2025-11-25 experimental), async trait design
**Confidence:** HIGH

## Summary

Phase 1 creates the `pmcp-tasks` crate as a workspace member with spec-compliant protocol types, a TaskStore async trait, state machine validation, and TaskWithVariables as a PMCP extension. The MCP 2025-11-25 Tasks specification has been thoroughly analyzed from the authoritative schema.ts and spec documentation. The existing PMCP crate provides clear patterns for module organization, serde conventions, error types, and capability structures that this phase must follow.

Key finding: The spec's `ttl` field is `number | null` (required, nullable) -- NOT optional. This means Rust must serialize `null` when unlimited, not omit the field. Additionally, `GetTaskResult` and `CancelTaskResult` are flat task fields at the result level (no wrapper), while `CreateTaskResult` wraps the task in a `task` field. The design document had some discrepancies with the spec that this research corrects.

**Primary recommendation:** Follow the existing PMCP crate conventions exactly (serde rename_all camelCase, thiserror for errors, async-trait, parking_lot, etc.) while implementing the spec-precise type definitions. Wire types must match spec JSON schema byte-for-byte; domain types (TaskRecord, TaskWithVariables) are separate.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Variables always included in `_meta` on every tasks/get response -- client always sees current state
- Bidirectional read/write: both client and server can read and write variables
- Ownership via prefix convention: `server.*` for server vars, `client.*` for client vars. Either side can read all, writes follow convention
- Variables placed at top level of `_meta` (not nested under a PMCP-specific key)
- Wire types and domain types are separate. Wire types map 1:1 to MCP spec JSON. Domain types include PMCP extensions (variables, owner). Convert between them at the boundary.
- TaskRecord (store level) has public variables/owner_id fields -- store implementors need full access. TaskContext is the primary ergonomic API for tool handlers.
- Capabilities via `experimental.tasks` only for now. Dedicated `ServerCapabilities.tasks` field deferred to spec stabilization.
- Rich context in errors: include task_id, current_status, attempted_action for debugging
- Public enum for TaskError -- idiomatic Rust, match-friendly. Variants: InvalidTransition { from, to, task_id }, NotFound { task_id }, Expired { task_id, expired_at }, NotReady { task_id, current_status }, OwnerMismatch, ResourceExhausted, VariableSizeExceeded, StoreError, etc.
- Distinct Expired error (not folded into NotFound) -- caller knows why the task is gone
- Errors include suggested_action field where possible
- TTL is adjustable after creation -- both server (via TaskContext) and client (via task params) can extend TTL during execution
- Crate name: `pmcp-tasks` -- direct dependency, independent versioning
- No re-export via pmcp feature flag (direct crate dependency only)
- Module structure matches existing PMCP SDK crate organization
- Export pattern matches main pmcp crate -- consistency with existing workspace conventions
- Rust best practices: public enums, exhaustive matching, clear ownership semantics, idiomatic patterns throughout

### Claude's Discretion
- Unknown JSON field handling strategy (preserve vs ignore) for spec forward-compatibility
- Whether to prepare a `stable-tasks` feature flag for future dedicated capability field
- Exact module layout within the crate (following existing pmcp patterns)
- Compression/optimization of variable serialization

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TYPE-01 | Protocol types serialize to match MCP 2025-11-25 schema exactly | Spec schema.ts analyzed: Task, CreateTaskResult, GetTaskResult, etc. Key finding: `ttl` is `number \| null` (required+nullable), not optional. GetTaskResult is flat (Result & Task), CreateTaskResult wraps in `task` field. |
| TYPE-02 | TaskStatus enum with 5 states + snake_case serde | Spec confirms: working, input_required, completed, failed, cancelled. Use `#[serde(rename_all = "snake_case")]` |
| TYPE-03 | State machine validates transitions | Spec defines exact transitions: working -> {input_required, completed, failed, cancelled}, input_required -> {working, completed, failed, cancelled}, terminal states reject all |
| TYPE-04 | Related-task metadata helper | Spec key: `io.modelcontextprotocol/related-task` with `{taskId: string}`. Required on tasks/result responses. |
| TYPE-05 | Task capability types with convenience constructors | Spec defines ServerCapabilities.tasks and ClientCapabilities.tasks with nested request type structure. full() and tools_only() constructors needed. |
| TYPE-06 | Request types (TaskGetParams, TaskResultParams, TaskListParams, TaskCancelParams) | Spec defines params for tasks/get, tasks/result, tasks/cancel as `{taskId: string}`, tasks/list as paginated `{cursor?: string}` |
| TYPE-07 | TaskStatusNotification | Spec method: `notifications/tasks/status`, params are Task fields (taskId, status, statusMessage, createdAt, lastUpdatedAt, ttl, pollInterval) |
| TYPE-08 | TaskSupport enum + ToolExecution metadata | Spec: `execution.taskSupport` with values `"forbidden" \| "optional" \| "required"`, default forbidden |
| TYPE-09 | TaskError variants map to JSON-RPC error codes | Spec errors: -32602 for invalid/nonexistent taskId, invalid cursor, terminal cancel; -32603 for internal; -32600 for missing required task augmentation |
| TYPE-10 | ModelImmediateResponse meta key constant | Spec key: `io.modelcontextprotocol/model-immediate-response` in CreateTaskResult._meta |
| STOR-01 | TaskStore async trait with all methods | Design doc provides trait definition; phase 1 defines the trait only (no backend impl) |
| STOR-02 | Atomic `complete_with_result` method | Required because spec says tasks/result MUST block until terminal, and result + status must be atomically consistent |
| STOR-03 | Configurable variable size limits | Trait-level enforcement, not per-backend. Design doc decided this. |
| STOR-04 | TaskRecord struct with protocol fields + extensions | TaskRecord = protocol Task + owner_id + variables + result + request_method |
| HNDL-01 | TaskWithVariables extends Task with variable store | Domain type with HashMap<String, Value> variables, serializes variables into `_meta` at boundary |
| TEST-01 | Protocol type serialization tests | Round-trip tests for all wire types against spec JSON examples |
| TEST-02 | State machine transition tests | Valid and invalid transitions, terminal state enforcement |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde | 1.0 | Serialization/deserialization | Already used by pmcp, derive macros for structs |
| serde_json | 1.0 (preserve_order) | JSON handling, Map type | Already used by pmcp, preserve_order for spec compliance |
| async-trait | 0.1 | Async trait methods | Already used throughout pmcp for all async traits |
| thiserror | 2.0 | Error derive macros | Already used by pmcp for Error enum |
| uuid | 1.17 (v4, serde) | Task ID generation | Already used by pmcp, UUIDv4 for 122-bit entropy |
| chrono | 0.4 (serde) | ISO 8601 timestamps | Already used by pmcp for DateTime<Utc> |
| tokio | 1 (sync, time) | Async runtime, channels | Already used by pmcp, needed for async trait |
| tracing | 0.1 | Structured logging | Already used by pmcp |
| parking_lot | 0.12 | Synchronization primitives | Already used by pmcp (RwLock) |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| pmcp | path="../.." | Core SDK types (CallToolResult, etc.) | For integration types, re-use existing protocol types |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| chrono | time 0.3 | pmcp already uses chrono, consistency wins |
| parking_lot | std::sync | parking_lot already in pmcp, faster, non-poisoning |
| thiserror 2.0 | anyhow | thiserror gives typed errors; anyhow is for application code |

**Installation (Cargo.toml for pmcp-tasks):**
```toml
[package]
name = "pmcp-tasks"
version = "0.1.0"
edition = "2021"
description = "MCP Tasks support for the PMCP SDK (experimental)"
license = "MIT"
rust-version = "1.82.0"

[dependencies]
pmcp = { path = "../..", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
async-trait = "0.1"
thiserror = "2.0"
uuid = { version = "1.17", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["sync", "time"] }
tracing = "0.1"
parking_lot = "0.12"

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
pretty_assertions = "1.4"
proptest = "1.7"
```

## Architecture Patterns

### Recommended Project Structure
```
crates/pmcp-tasks/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Crate root: module declarations, re-exports
│   ├── types/
│   │   ├── mod.rs           # Wire types module
│   │   ├── task.rs          # Task, TaskStatus, CreateTaskResult
│   │   ├── params.rs        # TaskParams, TaskGetParams, etc.
│   │   ├── capabilities.rs  # ServerTaskCapabilities, ClientTaskCapabilities
│   │   ├── notification.rs  # TaskStatusNotification
│   │   └── execution.rs     # TaskSupport, ToolExecution
│   ├── domain/
│   │   ├── mod.rs           # Domain types module
│   │   ├── record.rs        # TaskRecord (store-level)
│   │   └── variables.rs     # TaskWithVariables
│   ├── store.rs             # TaskStore trait, ListTasksOptions, TaskPage
│   ├── error.rs             # TaskError enum
│   └── constants.rs         # Meta keys, method names
└── tests/
    ├── protocol_types.rs    # TYPE-01 serialization round-trip
    └── state_machine.rs     # TYPE-03 transition tests
```

### Pattern 1: Wire Types vs Domain Types (LOCKED DECISION)
**What:** Separate types for protocol JSON (wire) and internal storage (domain).
**When to use:** Always. Wire types serialize byte-for-byte to spec JSON. Domain types carry PMCP extensions.
**Example:**
```rust
// Wire type: matches spec exactly
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub task_id: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub created_at: String,        // ISO 8601 string, not DateTime
    pub last_updated_at: String,   // ISO 8601 string, not DateTime
    pub ttl: Option<u64>,          // number | null in spec
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

// Domain type: PMCP extensions
pub struct TaskRecord {
    pub task: Task,
    pub owner_id: String,
    pub variables: HashMap<String, Value>,
    pub result: Option<Value>,
    pub request_method: String,
}
```

### Pattern 2: Nullable vs Optional Field Handling
**What:** The spec distinguishes between `ttl: number | null` (REQUIRED, can be null) and `pollInterval?: number` (OPTIONAL, omitted when absent).
**When to use:** For `ttl` field specifically, and any other required-but-nullable fields.
**Example:**
```rust
// ttl is REQUIRED but nullable -> Option<u64> with NO skip_serializing_if
// This ensures "ttl": null appears in JSON when unlimited
pub ttl: Option<u64>,

// pollInterval is truly optional -> skip when None
#[serde(skip_serializing_if = "Option::is_none")]
pub poll_interval: Option<u64>,
```

### Pattern 3: State Machine as Methods on Enum
**What:** Implement transition validation as methods on TaskStatus.
**When to use:** Always for status transitions.
**Example:**
```rust
impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_transition_to(&self, next: &TaskStatus) -> bool {
        match self {
            Self::Working => matches!(next,
                Self::InputRequired | Self::Completed | Self::Failed | Self::Cancelled
            ),
            Self::InputRequired => matches!(next,
                Self::Working | Self::Completed | Self::Failed | Self::Cancelled
            ),
            Self::Completed | Self::Failed | Self::Cancelled => false,
        }
    }
}
```

### Pattern 4: Error Context Pattern (LOCKED DECISION)
**What:** Rich error variants with context fields.
**When to use:** All TaskError variants.
**Example:**
```rust
#[derive(Debug, thiserror::Error)]
pub enum TaskError {
    #[error("invalid transition from {from} to {to} for task {task_id}")]
    InvalidTransition {
        task_id: String,
        from: TaskStatus,
        to: TaskStatus,
        suggested_action: Option<String>,
    },

    #[error("task not found: {task_id}")]
    NotFound { task_id: String },

    #[error("task expired: {task_id} (expired at {expired_at})")]
    Expired {
        task_id: String,
        expired_at: String,
    },
    // ... etc
}
```

### Pattern 5: GetTaskResult is Flat, CreateTaskResult Wraps
**What:** The spec defines `GetTaskResult = Result & Task` (flat fields) but `CreateTaskResult = Result & { task: Task }` (wrapped).
**When to use:** Critical for correct serialization.
**Example:**
```rust
// GetTaskResult: Task fields ARE the result (flat, no wrapper)
// Serializes as: { "taskId": "...", "status": "...", ... }
// Use Task directly as the get result type, or alias it
pub type GetTaskResult = Task;

// CancelTaskResult: same flat pattern
pub type CancelTaskResult = Task;

// CreateTaskResult: Task is WRAPPED in a `task` field
// Serializes as: { "task": { "taskId": "...", ... } }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    pub task: Task,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<serde_json::Map<String, Value>>,
}
```

### Pattern 6: Capability Types for experimental.tasks
**What:** Task capabilities are serialized into `experimental.tasks` (not a dedicated field).
**When to use:** Phase 1 defines the types; Phase 3 uses them for capability advertisement.
**Example:**
```rust
impl ServerTaskCapabilities {
    pub fn full() -> Self {
        Self {
            list: Some(EmptyObject {}),
            cancel: Some(EmptyObject {}),
            requests: Some(ServerTaskRequests {
                tools: Some(ToolsTaskRequests {
                    call: Some(EmptyObject {}),
                }),
            }),
        }
    }
}

// At integration time (Phase 3):
// caps.experimental["tasks"] = serde_json::to_value(ServerTaskCapabilities::full())
```

### Anti-Patterns to Avoid
- **DateTime<Utc> in wire types:** The spec uses ISO 8601 strings. Use `String` in wire types, `DateTime<Utc>` in domain types. This avoids serde format mismatches.
- **skip_serializing_if on `ttl`:** The spec says `ttl: number | null` is REQUIRED. Do NOT skip it. Serialize `null` for unlimited.
- **Wrapping GetTaskResult:** The spec says `GetTaskResult = Result & Task` -- the task fields ARE the result. Do not wrap in a `task` field.
- **Single error type for everything:** Keep TaskError separate from pmcp::Error. Convert at the integration boundary (Phase 3).
- **HashMap for variables in wire types:** Variables go into `_meta` (a serde_json::Map). Wire types should not have a variables field.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| UUID generation | Custom ID generator | `uuid::Uuid::new_v4()` | Cryptographically secure, 122 bits entropy per spec requirement |
| ISO 8601 timestamps | Manual string formatting | `chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)` | Handles timezone, leap seconds, formatting correctly |
| Error derive boilerplate | Manual Display/Error impl | `thiserror::Error` derive | Already used in pmcp, consistent error patterns |
| JSON Map type | `HashMap<String, Value>` for `_meta` | `serde_json::Map<String, Value>` | Preserves insertion order with `preserve_order` feature, matches spec behavior |
| Async trait signatures | Manual Pin<Box<dyn Future>> | `async_trait` macro | Already used throughout pmcp, consistent interface |

**Key insight:** Every dependency in pmcp-tasks already exists in the pmcp workspace. Zero new dependencies are introduced. This is intentional -- the crate extends the ecosystem without adding supply chain risk.

## Common Pitfalls

### Pitfall 1: ttl Serialization Bug
**What goes wrong:** Using `#[serde(skip_serializing_if = "Option::is_none")]` on `ttl` causes it to be omitted when `None`, but the spec requires `"ttl": null` for unlimited TTL.
**Why it happens:** Natural Rust instinct is to skip None fields.
**How to avoid:** Do NOT add skip_serializing_if to `ttl`. `Option<u64>` serializes as `null` by default, which is correct.
**Warning signs:** Tests against spec JSON examples will fail with missing `ttl` field.

### Pitfall 2: GetTaskResult vs CreateTaskResult Shape Confusion
**What goes wrong:** Wrapping GetTaskResult in `{ "task": {...} }` when it should be flat task fields.
**Why it happens:** CreateTaskResult wraps in `task`, so devs assume all results do.
**How to avoid:** `GetTaskResult = Task` (type alias or newtype). `CreateTaskResult` has `task: Task` field.
**Warning signs:** JSON round-trip tests will produce `{"task":{"taskId":...}}` instead of `{"taskId":...}`.

### Pitfall 3: State Machine allows self-transitions
**What goes wrong:** Allowing `working -> working` as a valid transition.
**Why it happens:** The spec says "from working: may move to..." -- it does NOT list self-transitions.
**How to avoid:** `can_transition_to` must return false for same-status transitions. The spec only allows transitions TO different states.
**Warning signs:** Property tests catch this if they verify transition counts.

### Pitfall 4: Variables in Wire Type
**What goes wrong:** Adding a `variables` field to the wire Task type, breaking spec compliance.
**Why it happens:** The PMCP extension (variables) feels like it belongs on Task.
**How to avoid:** Variables exist only on domain types (TaskWithVariables, TaskRecord). They are injected into `_meta` at the serialization boundary.
**Warning signs:** Serialized JSON contains unexpected `variables` key.

### Pitfall 5: Timestamp Format Mismatch
**What goes wrong:** Using `DateTime<Utc>` in wire types with default serde, producing `"2025-02-21T10:30:00.000Z"` when spec examples show `"2025-11-25T10:30:00Z"` (no milliseconds).
**Why it happens:** Chrono's default format includes milliseconds.
**How to avoid:** Use `String` for timestamps in wire types. Parse/format at domain boundary. Or use a custom serde format. The spec says "ISO 8601" which allows both, but consistency matters.
**Warning signs:** Snapshot tests differ on timestamp format.

### Pitfall 6: TaskStore trait requires Send + Sync but parking_lot is not async
**What goes wrong:** Using `parking_lot::RwLock` directly in async code causes blocking.
**Why it happens:** The in-memory backend (Phase 2) will use parking_lot, but the trait is async.
**How to avoid:** The trait uses async methods. The in-memory backend wraps parking_lot in sync blocks (locks held briefly). This is acceptable per tokio guidance when lock hold times are short.
**Warning signs:** None in Phase 1 (trait only), but document for Phase 2.

## Code Examples

### Task Wire Type (spec-compliant)
```rust
// Source: MCP 2025-11-25 schema.ts - Task interface
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Working,
    InputRequired,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub task_id: String,
    pub status: TaskStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    pub created_at: String,
    pub last_updated_at: String,
    /// Required but nullable: number | null in spec.
    /// None serializes as null, Some(ms) serializes as number.
    pub ttl: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    pub task: Task,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<Map<String, Value>>,
}

/// GetTaskResult = Result & Task (flat, no wrapper)
pub type GetTaskResult = Task;

/// CancelTaskResult = Result & Task (flat, no wrapper)
pub type CancelTaskResult = Task;
```

### TaskError with Rich Context
```rust
// Source: CONTEXT.md decisions + spec error codes
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("invalid transition from {from:?} to {to:?} for task {task_id}")]
    InvalidTransition {
        task_id: String,
        from: TaskStatus,
        to: TaskStatus,
        suggested_action: Option<String>,
    },

    #[error("task not found: {task_id}")]
    NotFound {
        task_id: String,
    },

    #[error("task expired: {task_id}")]
    Expired {
        task_id: String,
        expired_at: Option<String>,
    },

    #[error("task not in terminal state: {task_id} (status: {current_status:?})")]
    NotReady {
        task_id: String,
        current_status: TaskStatus,
    },

    #[error("owner mismatch for task {task_id}")]
    OwnerMismatch {
        task_id: String,
    },

    #[error("resource exhausted")]
    ResourceExhausted {
        suggested_action: Option<String>,
    },

    #[error("variable size limit exceeded")]
    VariableSizeExceeded {
        limit_bytes: usize,
        actual_bytes: usize,
    },

    #[error("store error: {0}")]
    StoreError(String),
}

impl TaskError {
    /// Map to JSON-RPC error code per spec
    pub fn error_code(&self) -> i32 {
        match self {
            Self::InvalidTransition { .. } => -32602,
            Self::NotFound { .. } => -32602,
            Self::Expired { .. } => -32602,
            Self::NotReady { .. } => -32602,
            Self::OwnerMismatch { .. } => -32602,
            Self::ResourceExhausted { .. } => -32603,
            Self::VariableSizeExceeded { .. } => -32602,
            Self::StoreError(_) => -32603,
        }
    }
}
```

### TaskStore Trait (with atomic complete_with_result)
```rust
// Source: Design doc + STOR-01/02/03 requirements
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

#[async_trait]
pub trait TaskStore: Send + Sync {
    async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError>;

    async fn get(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskError>;

    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        new_status: TaskStatus,
        status_message: Option<String>,
    ) -> Result<TaskRecord, TaskError>;

    async fn set_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError>;

    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<(), TaskError>;

    async fn get_result(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<Value, TaskError>;

    /// Atomic status transition + result storage.
    /// MUST atomically set status to terminal AND store result.
    /// If either fails, neither should be applied.
    async fn complete_with_result(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        result: Value,
    ) -> Result<TaskRecord, TaskError>;

    async fn list(
        &self,
        options: ListTasksOptions,
    ) -> Result<TaskPage, TaskError>;

    async fn cancel(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskError>;

    async fn cleanup_expired(&self) -> Result<usize, TaskError>;
}
```

### Serialization Test Pattern
```rust
// Source: Spec JSON examples from tasks documentation
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_create_task_result_matches_spec() {
        let result = CreateTaskResult {
            task: Task {
                task_id: "786512e2-9e0d-44bd-8f29-789f320fe840".to_string(),
                status: TaskStatus::Working,
                status_message: Some("The operation is now in progress.".to_string()),
                created_at: "2025-11-25T10:30:00Z".to_string(),
                last_updated_at: "2025-11-25T10:40:00Z".to_string(),
                ttl: Some(60000),
                poll_interval: Some(5000),
            },
            _meta: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["task"]["taskId"], "786512e2-9e0d-44bd-8f29-789f320fe840");
        assert_eq!(json["task"]["status"], "working");
        assert_eq!(json["task"]["ttl"], 60000);
        assert_eq!(json["task"]["pollInterval"], 5000);
    }

    #[test]
    fn test_ttl_null_serialization() {
        let task = Task {
            task_id: "test-id".to_string(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:30:00Z".to_string(),
            ttl: None,  // unlimited
            poll_interval: None,
        };

        let json = serde_json::to_value(&task).unwrap();
        // ttl MUST be present as null, not omitted
        assert!(json.get("ttl").is_some());
        assert!(json["ttl"].is_null());
        // pollInterval SHOULD be omitted when None
        assert!(json.get("pollInterval").is_none());
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No async tasks in MCP | Tasks primitive (experimental) | 2025-11-25 spec | New protocol feature, first implementation in this SDK |
| tools/call returns result directly | tools/call can return CreateTaskResult | 2025-11-25 spec | Two-phase response pattern for long-running operations |
| No task capabilities | tasks field in ServerCapabilities/ClientCapabilities | 2025-11-25 spec | Capability negotiation for task support |

**Deprecated/outdated:**
- The design document used `DateTime<Utc>` for timestamp fields in wire types. The spec uses ISO 8601 strings. Use `String` in wire types.
- The design document used `Option<u64>` with skip_serializing_if for `ttl`. The spec requires `ttl` to always be present (null or number).
- The design document showed `TaskWithVariables` using `#[serde(flatten)]` on `task: Task` and a `variables` HashMap. Per CONTEXT.md, variables go into `_meta`, not flattened.

## Discretion Recommendations

### Unknown JSON Field Handling
**Recommendation:** Use `#[serde(deny_unknown_fields)]` on wire types during development/testing to catch unexpected fields early. Remove or replace with `#[serde(flatten)] extra: HashMap<String, Value>` before release for forward-compatibility with future spec versions.

**Rationale:** The spec's Result base type has `[key: string]: unknown` (index signature), meaning unknown fields should be preserved. However, for Phase 1, strict validation catches bugs faster. The forward-compat strategy can be added as a feature flag later.

### stable-tasks Feature Flag
**Recommendation:** Do NOT prepare this flag in Phase 1. It adds complexity for a speculative future. When the spec stabilizes, add the flag then.

### Module Layout
**Recommendation:** Follow the structure in Architecture Patterns above: `types/` for wire types (mirroring pmcp's `types/` module), `domain/` for PMCP extensions, `store.rs` for trait, `error.rs` for errors. This matches pmcp's `types/`, `error/`, `server/` pattern.

### Variable Serialization Optimization
**Recommendation:** No optimization in Phase 1. Use standard serde_json::to_value for variables. Compression can be added behind a feature flag in a later phase if variable sizes become a concern.

## Open Questions

1. **GetTaskResult _meta field**
   - What we know: Spec's `Result` base type includes `_meta?: { [key: string]: unknown }`. GetTaskResult = Result & Task. This means GetTaskResult could include `_meta`.
   - What's unclear: Should the Rust Task wire type include `_meta` for GetTaskResult responses? The PMCP extension puts variables into `_meta` for get responses.
   - Recommendation: Add `_meta: Option<Map<String, Value>>` to the Task wire type with `skip_serializing_if`. This is where variables get injected at the boundary. The spec allows it via the Result base type's `_meta`.

2. **tasks/result blocking in trait design**
   - What we know: Spec says `tasks/result` MUST block until terminal status. This is a transport/handler concern, not a store concern.
   - What's unclear: Should TaskStore have a `wait_for_terminal` method, or is that handled at the handler level?
   - Recommendation: The store trait should NOT handle blocking. The handler (Phase 3) uses `tasks/get` polling or tokio::sync::watch to wait. Store just stores.

3. **Self-transitions (e.g., working -> working)**
   - What we know: Spec says "from working: may move to input_required, completed, failed, or cancelled". Self-transitions not listed.
   - What's unclear: Are self-transitions intentionally excluded or just not mentioned?
   - Recommendation: Reject self-transitions. The spec lists valid targets explicitly and working is not listed as a target from working. Being strict is safer.

## Sources

### Primary (HIGH confidence)
- [MCP 2025-11-25 Tasks Specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks) - Full normative spec text, all MUST/SHOULD/MAY requirements
- [MCP 2025-11-25 schema.ts](https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/schema/2025-11-25/schema.ts) - Authoritative TypeScript type definitions
- Existing PMCP codebase (`src/types/`, `src/error/`, `Cargo.toml`) - Established patterns for serde, errors, capabilities

### Secondary (MEDIUM confidence)
- [PMCP Tasks Design Document](docs/design/tasks-feature-design.md) - Internal design doc; some details diverge from spec (corrected in this research)
- CONTEXT.md decisions from /gsd:discuss-phase - User's locked design decisions

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All dependencies already in workspace, zero new deps
- Architecture: HIGH - Patterns derived from existing pmcp crate + spec analysis
- Pitfalls: HIGH - Verified against spec JSON examples and TypeScript schema
- Wire type shapes: HIGH - Verified against authoritative schema.ts definitions

**Research date:** 2026-02-21
**Valid until:** 2026-04-21 (spec is stable experimental, 60-day validity for non-moving target)
