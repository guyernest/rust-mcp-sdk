# Phase 3: Handler, Middleware, and Server Integration - Research

**Researched:** 2026-02-22
**Domain:** PMCP server integration -- capability advertisement, JSON-RPC routing, tool middleware interception, task endpoint handlers
**Confidence:** HIGH

## Summary

Phase 3 wires the `pmcp-tasks` crate (types, store, context from Phases 1-2) into the `pmcp` server core. The integration touches five distinct subsystems: (1) `ServerCapabilities` for advertising `experimental.tasks`, (2) `ToolInfo` for per-tool `execution.taskSupport` metadata, (3) `ClientRequest` enum for routing `tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel`, (4) `handle_call_tool` in `ServerCore` for intercepting task-augmented `tools/call` requests, and (5) `RequestHandlerExtra` for injecting `Option<TaskContext>` into tool handlers.

The codebase is well-structured for this integration. The `ServerCoreBuilder` pattern already supports `with_observability(config)` and `tool_middleware(middleware)` -- adding `.with_task_store(store)` follows the same pattern. The `ClientRequest` enum uses `#[serde(tag = "method", content = "params")]` dispatching, so adding four new variants is mechanical. The primary risk is the interaction between task-augmented `tools/call` (which returns `CreateTaskResult` instead of `CallToolResult`) and the existing `handle_call_tool` flow that wraps results in `CallToolResult`.

**Primary recommendation:** Add task endpoint variants to `ClientRequest`, add `Option<Arc<dyn TaskStore>>` to `ServerCore`, handle task routing in `handle_request_internal`, and intercept task-augmented `tools/call` in `handle_call_tool` before the normal tool execution path.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Phase 3 implements TWO task creation patterns: (1) client-initiated (client sends `task` field in tools/call), (2) long-running tools (tools that declare `taskSupport: required` in ToolInfo)
- Other patterns (server-initiated via prompts, task_start/task_complete convenience tools, composite operations) deferred to Phase 5
- No tool-level task annotations -- tasks are NOT just "background execution" of a single tool. Tasks represent long-running, multi-step interactions that may span multiple tool calls
- TaskContext injected via RequestHandlerExtra (Option<TaskContext>) -- handler checks if task context is present
- Handler's responsibility: (1) create task in store, (2) trigger external job execution, (3) store job reference (execution ARN, job ID) in task variables, (4) return immediately. Handler does NOT wait for completion.
- Built-in middleware handles all four task endpoints (tasks/get, tasks/result, tasks/list, tasks/cancel) automatically
- Developer does NOT write any task routing code -- enabling tasks on the server is enough
- Execution model: Handler triggers external service, returns immediately. No tokio::spawn for long-running work inside the server. Simulated background (tokio::spawn + sleep) acceptable only in examples for demonstration purposes.
- Explicit builder config: developer calls .with_task_store() on the server builder to enable tasks
- Store required, security defaults: just .with_task_store(store) is enough. TaskSecurityConfig has sensible defaults.
- All four endpoints (get, result, list, cancel) always-on when tasks are enabled -- no individual control
- 60_tasks_basic.rs: minimal viable example -- simplest possible task-enabled server with one tool demonstrating create-poll-complete lifecycle
- Uses InMemoryTaskStore (self-contained, no external dependencies)
- Simulates background execution with tokio::spawn + sleep for demonstration (real Lambda pattern deferred to Phase 5 examples)

### Claude's Discretion
- Status change notification triggering (automatic vs explicit)
- Task capability placement in initialize response (per spec)
- Exact builder API boilerplate for enabling tasks
- Execution model details for the example

### Deferred Ideas (OUT OF SCOPE)
- Server-initiated task creation via prompts/workflows -- Phase 5
- Task mode convenience tools (task_start/task_complete) -- Phase 5
- Composite operation task patterns (code mode validate-then-execute) -- Phase 5
- Real Lambda + Step Functions integration example -- Phase 5
- Individual control over list/cancel capabilities -- not currently needed
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INTG-01 | Server task capabilities advertised via `experimental.tasks` field during initialization | ServerCapabilities has `experimental: Option<HashMap<String, Value>>`. Insert `ServerTaskCapabilities::full()` serialized as Value into this map under key "tasks". Builder `.with_task_store()` sets this automatically. |
| INTG-02 | Tool-level task support declared via `execution.taskSupport` in tools/list response | `ToolInfo` is `#[non_exhaustive]` and has `_meta`. Add optional `execution: Option<ToolExecution>` field to `ToolInfo` (from pmcp-tasks). `handle_list_tools` already iterates handler metadata. |
| INTG-03 | TaskMiddleware intercepts tools/call requests containing `task` field and creates task | `handle_call_tool` in `core.rs` is the interception point. Check `req.arguments` or add a `task` field to `CallToolRequest`. If task field present and store available, create task and return CreateTaskResult. |
| INTG-04 | TaskMiddleware returns CreateTaskResult immediately and spawns background tool execution | For the basic example, use tokio::spawn. In production Lambda pattern, handler triggers external service and returns. The CreateTaskResult is returned as the JSON-RPC result (not wrapped in CallToolResult). |
| INTG-05 | tasks/get endpoint returns current task state for polling | Add `TasksGet(TaskGetParams)` variant to `ClientRequest`. Route in `handle_request_internal` to `store.get()`, convert `TaskRecord` to `GetTaskResult` (which is just `Task`). |
| INTG-06 | tasks/result endpoint returns the operation result for terminal tasks | Add `TasksResult(TaskResultParams)` variant. Route to `store.get_result()`. Return the stored Value with `related_task_meta()` in `_meta`. |
| INTG-07 | tasks/list endpoint returns paginated tasks scoped to owner's authorization context | Add `TasksList(TaskListParams)` variant. Route to `store.list()` with owner from auth context. Return tasks array with next_cursor. |
| INTG-08 | tasks/cancel endpoint transitions non-terminal tasks to cancelled status | Add `TasksCancel(TaskCancelParams)` variant. Route to `store.cancel()`. Return `CancelTaskResult` (which is just `Task`). |
| INTG-09 | TTL enforcement: receivers respect requested TTL, can override with max, clean up expired tasks | Store already handles TTL clamping via StoreConfig. Task creation in the interception path passes client-requested TTL to `store.create()`. |
| INTG-10 | JSON-RPC routing handles tasks/get, tasks/result, tasks/list, tasks/cancel methods | Four new `ClientRequest` variants with `#[serde(rename = "tasks/get")]` etc. `parse_client_request` in protocol_helpers.rs uses serde tagged enum deserialization -- new variants parse automatically. |
| INTG-11 | progressToken from original request threaded through to background task execution | `CallToolRequest._meta.progress_token` is already parsed. Pass it through `RequestHandlerExtra` to the spawned background execution. TaskContext or handler can use it. |
| INTG-12 | Model immediate response supported via optional `_meta` field in CreateTaskResult | `CreateTaskResult._meta` already exists. When constructing the result, optionally include `MODEL_IMMEDIATE_RESPONSE_META_KEY` if the handler provides one. |
| TEST-08 | Full lifecycle integration tests (create -> poll -> complete -> get_result end-to-end) | Build a test ServerCore with task store, send tools/call with task field, poll tasks/get, verify terminal state, call tasks/result. All using `handle_request` directly. |
| EXMP-01 | Basic task-augmented tool call example (60_tasks_basic.rs) | ServerCoreBuilder with .with_task_store(InMemoryTaskStore). One tool that creates task, spawns background work, completes. Client-side polling loop. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| pmcp-tasks | 0.1.0 (local) | Task types, store, context, security | This is the crate we built in Phases 1-2 |
| pmcp | 1.10.3 (local) | Server core, builder, types, middleware | The SDK we are integrating into |
| serde / serde_json | 1.0 | JSON serialization for wire types | Already used throughout both crates |
| async-trait | 0.1 | Async trait definitions | Already used for ToolHandler, TaskStore |
| tokio | 1.x | Async runtime, spawn for example | Already a dependency |
| uuid | 1.17 | Task ID generation in store | Already in pmcp-tasks |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio-util | * | CancellationToken | Already used in RequestHandlerExtra |
| tracing | 0.1 | Structured logging | Already used throughout server |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Modifying ClientRequest enum | Raw JSON-RPC dispatch | Enum change is more type-safe, consistent with existing pattern |
| Adding task field to CallToolRequest | Checking _meta for task params | Explicit field is cleaner, but _meta approach avoids core type changes |

## Architecture Patterns

### Recommended Integration Structure
```
src/
├── types/
│   ├── protocol.rs      # Add 4 ClientRequest variants, add execution field to ToolInfo
│   └── capabilities.rs  # No changes (use experimental field)
├── server/
│   ├── builder.rs        # Add with_task_store() method
│   ├── core.rs           # Add task_store field, handle task routing + interception
│   └── mod.rs            # Re-export task types if needed
├── shared/
│   └── protocol_helpers.rs  # No changes needed (serde handles new variants)
crates/
└── pmcp-tasks/
    └── src/              # Already complete from Phases 1-2
examples/
└── 60_tasks_basic.rs    # New example
```

### Pattern 1: Builder Extension for Task Store
**What:** Add `.with_task_store(store)` to `ServerCoreBuilder` that stores the task store and auto-configures capabilities.
**When to use:** Always -- this is the only way to enable tasks.
**Example:**
```rust
// In builder.rs
pub fn with_task_store(mut self, store: Arc<dyn TaskStore>) -> Self {
    self.task_store = Some(store);
    // Auto-set experimental.tasks capability
    let task_caps = serde_json::to_value(ServerTaskCapabilities::full()).unwrap();
    let experimental = self.capabilities.experimental.get_or_insert_with(HashMap::new);
    experimental.insert("tasks".to_string(), task_caps);
    self
}
```
**Source:** Follows existing `with_observability()` pattern in builder.rs.

### Pattern 2: Task-Augmented tools/call Interception
**What:** In `handle_call_tool`, check for task field in request before normal tool execution. If task field present and store available, create task and return `CreateTaskResult`.
**When to use:** When client sends `tools/call` with a `task` field.
**Example:**
```rust
// In core.rs handle_call_tool
// Check for task-augmented call
if let (Some(task_store), Some(task_params)) = (&self.task_store, req.task.as_ref()) {
    let owner_id = resolve_owner_from_auth(&auth_context);
    let record = task_store.create(&owner_id, "tools/call", task_params.ttl).await?;
    let task_context = TaskContext::new(task_store.clone(), record.task.task_id.clone(), owner_id);

    // Inject TaskContext into extra
    extra.task_context = Some(task_context.clone());

    // Spawn background execution (example pattern)
    let handler = handler.clone();
    let args = req.arguments.clone();
    tokio::spawn(async move {
        match handler.handle(args, extra).await {
            Ok(result) => { task_context.complete(result).await.ok(); }
            Err(e) => { task_context.fail(e.to_string()).await.ok(); }
        }
    });

    // Return CreateTaskResult immediately
    return Ok(CreateTaskResult { task: record.task, _meta: None });
}
```

### Pattern 3: Built-in Task Endpoint Routing
**What:** Route `tasks/get`, `tasks/result`, `tasks/list`, `tasks/cancel` directly in `handle_request_internal` to the task store.
**When to use:** Always when task store is configured. No developer code needed.
**Example:**
```rust
// In core.rs handle_request_internal match arms
ClientRequest::TasksGet(req) => {
    if let Some(store) = &self.task_store {
        let owner_id = resolve_owner_from_auth(&auth_context);
        match store.get(&req.task_id, &owner_id).await {
            Ok(record) => Self::success_response(id, serde_json::to_value(record.task).unwrap()),
            Err(e) => Self::error_response(id, e.error_code(), e.to_string()),
        }
    } else {
        Self::error_response(id, -32601, "Tasks not enabled".to_string())
    }
}
```

### Pattern 4: Adding task Field to CallToolRequest
**What:** Add an optional `task` field to `CallToolRequest` for task augmentation.
**When to use:** This is how clients signal task creation intent.
**Example:**
```rust
// In protocol.rs
pub struct CallToolRequest {
    pub name: String,
    #[serde(default)]
    pub arguments: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _meta: Option<RequestMeta>,
    /// Task augmentation parameters (experimental)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<pmcp_tasks::TaskParams>,
}
```

### Pattern 5: Adding execution Field to ToolInfo
**What:** Add optional `execution` field to `ToolInfo` for declaring task support.
**When to use:** Tools that want to advertise their task support level.
**Example:**
```rust
// In protocol.rs
pub struct ToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Value,
    pub annotations: Option<ToolAnnotations>,
    pub _meta: Option<serde_json::Map<String, Value>>,
    /// Execution metadata (task support declaration)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<pmcp_tasks::ToolExecution>,
}
```

### Anti-Patterns to Avoid
- **Creating a separate TaskMiddleware struct:** The task interception lives directly in `handle_call_tool` and `handle_request_internal`. No separate middleware layer. The "built-in middleware" terminology from CONTEXT.md means the server handles it internally, not that we create a ToolMiddleware impl.
- **Blocking in handle_call_tool for task completion:** The server MUST return immediately with CreateTaskResult. Background execution is external (Lambda, Step Functions) or simulated with tokio::spawn in examples only.
- **Duplicating owner resolution logic:** Use `pmcp_tasks::resolve_owner_id()` everywhere, bridging from `AuthContext` fields.
- **Making pmcp depend on pmcp-tasks at the crate level:** pmcp-tasks already depends on pmcp. The integration should use feature flags or keep the dependency one-directional. The types needed in pmcp (TaskParams, ToolExecution) can be added as optional fields using `serde_json::Value` parsing, or pmcp-tasks re-exports can be used through the example's dependency.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Task ID generation | Custom UUID logic | `uuid::Uuid::new_v4()` via TaskStore::create | Already handles 122-bit entropy requirement |
| Owner resolution from auth | Custom auth extraction | `pmcp_tasks::resolve_owner_id()` | Already handles priority chain and empty-string skipping |
| State machine validation | Manual status checks | `TaskStatus::validate_transition()` | Already encodes the full spec state machine |
| TTL clamping | Manual comparison | StoreConfig in InMemoryTaskStore | Already handles default/max TTL |
| JSON-RPC error code mapping | Manual match | `TaskError::error_code()` | Already maps all variants to -32602 or -32603 |
| Task creation + initial state | Manual struct construction | `TaskRecord::new()` via `TaskStore::create()` | Handles UUID, timestamps, TTL defaults atomically |

**Key insight:** Phase 1-2 deliberately pushed all business logic into `pmcp-tasks`. Phase 3's job is pure wiring -- connecting the existing store operations to the server's request routing. If you find yourself reimplementing store logic in `core.rs`, you are doing it wrong.

## Common Pitfalls

### Pitfall 1: Circular Dependency Between pmcp and pmcp-tasks
**What goes wrong:** pmcp-tasks already depends on pmcp. Adding pmcp -> pmcp-tasks creates a cycle.
**Why it happens:** Wanting to use `TaskParams`, `ToolExecution`, `TaskStore` types directly in pmcp's protocol types.
**How to avoid:** Two approaches: (A) Use `serde_json::Value` for the `task` field on `CallToolRequest` and parse it in `core.rs` where pmcp-tasks is available, or (B) Move the tiny wire types (TaskParams, ToolExecution) into pmcp itself (they are small, spec-compliant, and don't depend on pmcp). Approach B is cleaner. The `CallToolRequest.task` field and `ToolInfo.execution` field are spec types that belong in the protocol layer.
**Warning signs:** Cargo complains about cyclic dependencies during build.

### Pitfall 2: Returning CreateTaskResult vs CallToolResult
**What goes wrong:** The `handle_call_tool` method currently returns `Result<CallToolResult>`. Task-augmented calls need to return `CreateTaskResult` which has a completely different shape (wraps `Task`, not `Content`).
**Why it happens:** The method signature assumes all tools/call responses are `CallToolResult`.
**How to avoid:** Intercept task-augmented calls BEFORE entering the normal `handle_call_tool` flow. Return the `CreateTaskResult` directly as a `serde_json::Value` via `Self::success_response()` in `handle_request_internal`, bypassing the `CallToolResult` wrapper. This means the task interception should be in the `ClientRequest::CallTool` match arm of `handle_request_internal`, not deep inside `handle_call_tool`.
**Warning signs:** Client receives a `CallToolResult` wrapper around task data instead of a clean `CreateTaskResult`.

### Pitfall 3: Missing Owner Resolution in Task Endpoints
**What goes wrong:** Task endpoints return tasks from other owners because owner_id is not extracted from auth context.
**Why it happens:** Forgetting to bridge `AuthContext` to `resolve_owner_id()` for every task endpoint.
**How to avoid:** Create a single helper function `fn resolve_owner(auth_context: &Option<AuthContext>) -> String` that calls `pmcp_tasks::resolve_owner_id()`. Use it in ALL task endpoint handlers and in the tools/call interception.
**Warning signs:** Test shows tasks visible across different owners.

### Pitfall 4: Serde Compatibility of New ClientRequest Variants
**What goes wrong:** Adding task variants to `ClientRequest` breaks existing JSON-RPC parsing because `parse_client_request` tries all variants via serde.
**Why it happens:** `ClientRequest` uses `#[serde(tag = "method", content = "params")]`. New variants with new method names parse independently and should not conflict. BUT the params types must be correctly deserializable from the JSON.
**How to avoid:** Ensure all new param types (`TaskGetParams`, etc.) match the JSON structure exactly. Test round-trip serialization of each variant.
**Warning signs:** Existing tests fail with "Invalid client request" errors after adding variants.

### Pitfall 5: Long-Running Tool Detection Without Task Field
**What goes wrong:** Tools declared with `taskSupport: required` are called without a `task` field, and the server doesn't create a task automatically.
**Why it happens:** Two task creation patterns exist -- client-initiated (task field) and long-running (tool metadata). Both need handling.
**How to avoid:** In the interception logic, check BOTH `req.task.is_some()` (client-initiated) AND the tool's metadata `execution.task_support == Required` (long-running tool). For required tools called without task field, auto-create a task.
**Warning signs:** Tools with `taskSupport: required` return normal CallToolResult instead of CreateTaskResult.

### Pitfall 6: tasks/result Must Return the Original Tool Result
**What goes wrong:** `tasks/result` returns the Task object instead of the stored tool execution result.
**Why it happens:** Confusing `tasks/get` (returns Task) with `tasks/result` (returns the operation result).
**How to avoid:** `tasks/result` calls `store.get_result()` which returns the `Value` stored via `complete_with_result()`. It returns this value wrapped with `related_task_meta` in `_meta`. `tasks/get` returns the Task wire type.
**Warning signs:** Client gets task metadata from tasks/result instead of the tool's output.

## Code Examples

### Example 1: ServerCoreBuilder with Task Store
```rust
// Source: Follows existing builder patterns in builder.rs
use pmcp::server::builder::ServerCoreBuilder;
use pmcp_tasks::{InMemoryTaskStore, TaskSecurityConfig};
use std::sync::Arc;

let store = Arc::new(
    InMemoryTaskStore::new()
        .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
);

let server = ServerCoreBuilder::new()
    .name("task-server")
    .version("1.0.0")
    .tool("long_running_tool", MyTool)
    .with_task_store(store)  // Enables tasks + auto-configures capabilities
    .build()?;
```

### Example 2: Owner Resolution Bridge
```rust
// Source: Bridges pmcp AuthContext to pmcp-tasks resolve_owner_id
fn resolve_owner(auth_context: &Option<AuthContext>) -> String {
    match auth_context {
        Some(ctx) => pmcp_tasks::resolve_owner_id(
            Some(&ctx.subject),
            ctx.client_id.as_deref(),
            None, // session_id from extra if needed
        ),
        None => pmcp_tasks::DEFAULT_LOCAL_OWNER.to_string(),
    }
}
```

### Example 3: Task-Augmented tools/call Interception (in handle_request_internal)
```rust
// Source: Interception point in core.rs handle_request_internal
ClientRequest::CallTool(req) => {
    // Check for task-augmented call FIRST
    if let Some(ref task_store) = self.task_store {
        if req.task.is_some() || self.tool_requires_task(&req.name) {
            match self.handle_task_augmented_call(req, auth_context, task_store).await {
                Ok(result) => return Self::success_response(id, result),
                Err(e) => return Self::error_response(id, e.error_code(), e.to_string()),
            }
        }
    }
    // Normal tool call path (existing code)
    match self.handle_call_tool(req, auth_context).await {
        Ok(result) => Self::success_response(id, serde_json::to_value(result).unwrap()),
        Err(e) => Self::error_response(id, -32603, e.to_string()),
    }
}
```

### Example 4: tasks/result Endpoint Handler
```rust
// Source: Pattern for tasks/result in core.rs
async fn handle_tasks_result(
    &self,
    req: &TaskResultParams,
    auth_context: Option<AuthContext>,
) -> Result<Value, TaskError> {
    let store = self.task_store.as_ref()
        .ok_or_else(|| TaskError::StoreError("Tasks not enabled".to_string()))?;
    let owner_id = resolve_owner(&auth_context);

    // get_result returns the stored Value and validates terminal state
    let result = store.get_result(&req.task_id, &owner_id).await?;

    // Wrap result with related-task meta
    let meta = pmcp_tasks::related_task_meta(&req.task_id);
    let mut response = serde_json::Map::new();
    response.insert("content".to_string(), result);
    response.insert("_meta".to_string(), serde_json::to_value(meta)?);

    Ok(serde_json::Value::Object(response))
}
```

### Example 5: 60_tasks_basic.rs Skeleton
```rust
// Source: Follows example patterns in examples/57_tool_middleware_oauth.rs
use pmcp::server::builder::ServerCoreBuilder;
use pmcp_tasks::{InMemoryTaskStore, TaskSecurityConfig, TaskContext};

struct LongRunningTool;

#[async_trait]
impl ToolHandler for LongRunningTool {
    async fn handle(&self, args: Value, extra: RequestHandlerExtra) -> Result<Value> {
        // In a real server, this triggers an external service
        // For the example, simulate with tokio::spawn
        Ok(json!({"status": "processing", "job_id": "sim-123"}))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "long_running_tool",
            Some("A tool that demonstrates task lifecycle".to_string()),
            json!({"type": "object", "properties": {"input": {"type": "string"}}}),
        ))
    }
}

#[tokio::main]
async fn main() {
    let store = Arc::new(
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    );

    let server = ServerCoreBuilder::new()
        .name("tasks-basic-example")
        .version("1.0.0")
        .tool("long_running_tool", LongRunningTool)
        .with_task_store(store.clone())
        .build()
        .unwrap();

    // Demonstrate lifecycle:
    // 1. Initialize
    // 2. tools/call with task field -> CreateTaskResult
    // 3. tasks/get polling -> Working -> Completed
    // 4. tasks/result -> operation result
}
```

## Critical Design Decisions for Planner

### Decision 1: Where Does Task Interception Live?

The task-augmented `tools/call` interception MUST happen in `handle_request_internal` (the match on `ClientRequest::CallTool`), NOT inside `handle_call_tool`. This is because:
- `handle_call_tool` returns `Result<CallToolResult>` -- wrong type for task responses
- The interception needs to return `CreateTaskResult` serialized as `Value` via `Self::success_response`
- This keeps the existing `handle_call_tool` untouched for normal (non-task) tool calls

### Decision 2: Task Field on CallToolRequest vs _meta

The `task` field should be a proper Optional field on `CallToolRequest`, not hidden in `_meta`. Reasons:
- The MCP spec defines it as a named field
- Explicit typing prevents parsing errors
- Serde can deserialize it automatically
- This does change `CallToolRequest` in the core `pmcp` crate, but the field is `Option` with `skip_serializing_if`, so it is backward compatible

### Decision 3: Dependency Direction

Since `pmcp-tasks` already depends on `pmcp`, we cannot make `pmcp` depend on `pmcp-tasks`. Options:
- **Option A:** Duplicate the tiny `TaskParams` and `ToolExecution` structs in `pmcp` (4 simple structs, ~40 lines total). Wire types belong in the protocol layer.
- **Option B:** Use `serde_json::Value` for `CallToolRequest.task` and `ToolInfo.execution`, parse them in integration code.
- **Option C:** Feature-flag pmcp-tasks dependency in pmcp behind `tasks` feature.
- **Recommendation:** Option A for `TaskParams` (needed in CallToolRequest), Option B for `ToolExecution` (can stay in _meta or be Value-typed). Or restructure: move the 4 wire types out of pmcp-tasks into a shared location. But the simplest path is Option B: `CallToolRequest.task: Option<Value>` parsed as `TaskParams` in core.rs. This avoids any crate restructuring.

### Decision 4: Background Execution Model

For the example, use `tokio::spawn` with sleep to simulate background work. For real servers:
- Handler receives `TaskContext` via `RequestHandlerExtra`
- Handler creates task in store, triggers external service (SFN, SQS), stores job reference in task variables
- Handler returns immediately
- External service updates task store when done (separate code path, not in this phase)

The interception in `core.rs` for client-initiated tasks (task field in tools/call) should:
1. Create task in store
2. Build TaskContext
3. Inject into extra
4. Execute handler normally (handler returns immediately)
5. Return CreateTaskResult

The handler's returned Value can be used as the "immediate result" if the tool returns quickly, or the handler can store it as task variables.

## Open Questions

1. **tasks/result response shape**
   - What we know: MCP spec says tasks/result returns the original tool result. `TaskStore::get_result()` returns the `Value` stored by `complete_with_result()`.
   - What's unclear: The exact response JSON shape -- is it just the raw Value, or wrapped in CallToolResult structure, or with _meta containing related-task?
   - Recommendation: Return as `{ "content": <stored_value>, "_meta": { "io.modelcontextprotocol/related-task": { "taskId": "..." } } }` following the spec pattern. Verify against MCP 2025-11-25 spec if possible.

2. **ServerCore::new() parameter explosion**
   - What we know: ServerCore::new() already has 11 parameters. Adding task_store makes 12.
   - What's unclear: Whether this should trigger a refactor to a config struct.
   - Recommendation: Keep as-is for now (matches existing pattern), note for future cleanup. The builder shields users from this.

3. **Feature flag for tasks in pmcp crate**
   - What we know: Tasks are experimental. pmcp-tasks is a separate crate.
   - What's unclear: Whether the changes to `CallToolRequest`, `ToolInfo`, and `ClientRequest` should be behind a feature flag.
   - Recommendation: Make the new fields unconditionally present (they are `Option` and skip when None). The enum variants in `ClientRequest` should always be parseable even if tasks are not enabled. The server returns "Tasks not enabled" error if no store is configured. This follows the principle that protocol types should always be complete.

## Sources

### Primary (HIGH confidence)
- `src/server/core.rs` -- ServerCore implementation, handle_request_internal routing, handle_call_tool
- `src/server/builder.rs` -- ServerCoreBuilder pattern, with_observability precedent
- `src/types/protocol.rs` -- ClientRequest enum, CallToolRequest, ToolInfo, content types
- `src/types/capabilities.rs` -- ServerCapabilities with experimental field
- `src/server/cancellation.rs` -- RequestHandlerExtra structure
- `src/server/tool_middleware.rs` -- ToolMiddleware trait and chain
- `src/shared/protocol_helpers.rs` -- parse_client_request dispatching
- `crates/pmcp-tasks/src/` -- All task types, store, context, security from Phases 1-2

### Secondary (MEDIUM confidence)
- `crates/pmcp-tasks/Cargo.toml` -- Dependency direction (pmcp-tasks depends on pmcp)
- MCP 2025-11-25 Tasks specification -- Referenced in design docs and type comments

### Tertiary (LOW confidence)
- Exact JSON shape for tasks/result response -- inferred from spec comments in types, needs validation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries already in use, no new dependencies
- Architecture: HIGH -- Direct examination of all integration surfaces in the codebase
- Pitfalls: HIGH -- Identified from actual code structure and type system constraints

**Research date:** 2026-02-22
**Valid until:** 2026-03-22 (stable internal codebase, spec unlikely to change)
