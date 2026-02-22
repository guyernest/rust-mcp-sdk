# Architecture Research: Task-Prompt Bridge Integration

**Domain:** Task-aware workflow prompts with partial execution for PMCP SDK
**Researched:** 2026-02-21
**Confidence:** HIGH (based entirely on existing codebase analysis -- all components already exist, this research maps how they connect)

## System Overview: Current vs. Proposed

### Current Architecture (v1.0) -- Two Disconnected Paths

```
PATH 1: prompts/get                    PATH 2: tools/call with task

  prompts/get request                    tools/call {task: {ttl: 60000}}
          |                                      |
          v                                      v
  +---------------------------+          +---------------------------+
  | WorkflowPromptHandler     |          | ServerCore                |
  | implements PromptHandler  |          | detects task field        |
  +---------------------------+          +---------------------------+
          |                                      |
          | for each step:                       v
          |   resolve -> execute -> bind  +---------------------------+
          v                              | Arc<dyn TaskRouter>       |
  +---------------------------+          | (trait in pmcp core)      |
  | ExecutionContext           |          +---------------------------+
  | bindings: HashMap<        |                  |
  |   BindingName, Value>     |                  v
  +---------------------------+          +---------------------------+
          |                              | TaskRouterImpl            |
          v                              | (in pmcp-tasks crate)     |
  GetPromptResult {                      +---------------------------+
    messages: [PromptMessage]                    |
    // full conversation trace                   v
  }                                      +---------------------------+
                                         | TaskStore                 |
                                         | InMemoryTaskStore         |
                                         +---------------------------+
                                                 |
                                                 v
                                         CreateTaskResult { task }
```

These paths are completely disconnected. Workflows execute all steps during `prompts/get` and return a full trace. Tasks manage long-running `tools/call` operations. The v1.1 bridge connects them.

### Proposed Architecture (v1.1 Task-Prompt Bridge)

```
  prompts/get request
          |
          v
  +------------------------------------+
  | WorkflowPromptHandler              |
  | (MODIFIED: optional task_router)   |
  +------------------------------------+
          |
          | 1. Create task via TaskRouter.create_workflow_task()
          | 2. For each step:
          |    a. resolve params from ExecutionContext
          |    b. CAN execute? yes: execute, bind, sync to task vars
          |    c. CAN execute? no:  record remaining steps, BREAK
          | 3. Build structured reply with task_id + step guidance
          |
          v
  +--------------------+     +---------------------------+
  | ExecutionContext    |     | Arc<dyn TaskRouter>       |
  | (local bindings,   | --> |  .set_task_variables()    |
  |  unchanged type)   |     |  syncs bindings to task   |
  +--------------------+     +---------------------------+
          |                              |
          v                              v
  GetPromptResult {              +---------------------------+
    messages: [                  | TaskStore                 |
      ...executed steps...,     |  variables: {             |
      ...structured guidance    |    "wf.goal": "...",      |
         with task_id,          |    "wf.steps.0.status":   |
         completed steps,       |      "completed",         |
         remaining steps...     |    "wf.steps.1.status":   |
    ]                           |      "pending",           |
  }                             |    "wf.steps.1.tool":     |
                                |      "provision_infra"    |
                                |  }                        |
                                +---------------------------+
                                         |
                                         v
                                Client reads task vars via tasks/get,
                                follows step guidance,
                                calls tools directly,
                                polls tasks/get for progress
```

## Integration Points: New vs. Modified vs. Unchanged

### New Components

| Component | Location | Purpose |
|-----------|----------|---------|
| 3 new methods on `TaskRouter` trait | `src/server/tasks.rs` | `create_workflow_task`, `set_task_variables`, `complete_workflow_task` -- bridge pmcp core to pmcp-tasks without circular dependency |
| Structured reply builder | `src/server/workflow/prompt_handler.rs` (internal to handler) | Constructs the final assistant message with task_id, completed/remaining steps |
| Workflow variable schema | Convention only (no new type) | `wf.goal`, `wf.steps.{idx}.status`, etc. -- flat keys with `wf.` prefix |

### Modified Components

| Component | File | Change |
|-----------|------|--------|
| `TaskRouter` trait | `src/server/tasks.rs` | +3 methods with default impls returning errors (non-breaking) |
| `TaskRouterImpl` | `crates/pmcp-tasks/src/router.rs` | Implement 3 new methods delegating to `TaskStore` |
| `WorkflowPromptHandler` | `src/server/workflow/prompt_handler.rs` | +`task_router: Option<Arc<dyn TaskRouter>>` field; task creation on entry; binding-to-variable sync; structured reply |
| `SequentialWorkflow` | `src/server/workflow/sequential.rs` | +`task_support: bool` field with builder method (opt-in) |
| `ServerCoreBuilder` | `src/server/builder.rs` | `prompt_workflow()` passes `task_router.clone()` to handler when configured |

### Unchanged Components (and Why)

| Component | Why Unchanged |
|-----------|---------------|
| `TaskRouter` existing 7 methods | Task CRUD routing is complete; bridge creates tasks through new methods |
| `TaskStore` trait | Already has `create`, `set_variables`, `complete_with_result` -- all ops the bridge needs |
| `TaskContext` (pmcp-tasks) | Bridge uses `TaskRouter` trait boundary, not `TaskContext` directly (avoids circular dep) |
| `DataSource` enum | `PromptArg`, `StepOutput`, `Constant` resolve from `ExecutionContext` which is unchanged |
| `ExecutionContext` struct | Still stores bindings during execution; sync to task vars is a post-binding side effect |
| `MiddlewareExecutor` trait | Tool execution path is unchanged; steps still execute through middleware |
| `ServerCore` request routing | `prompts/get` already routes to `PromptHandler`; `tasks/*` already routes to `TaskRouter` |
| `InMemoryTaskStore` | No new store operations needed; existing methods cover all bridge requirements |
| `WorkflowStep` | Step definition is unchanged; the handler decides whether to execute or defer |

## Detailed Design: Where Things Happen

### 1. Task Creation in WorkflowPromptHandler

**Location:** `WorkflowPromptHandler::handle()`, before the step loop.

**Current flow (prompt_handler.rs lines 757-774):**
```rust
async fn handle(&self, args: HashMap<String, String>, extra: RequestHandlerExtra)
    -> Result<GetPromptResult>
{
    let mut messages = Vec::new();
    let mut execution_context = ExecutionContext::new();
    messages.push(self.create_user_intent(&args));
    messages.push(self.create_assistant_plan()?);
    // ... step loop (lines 785-961) ...
    Ok(GetPromptResult { description, messages })
}
```

**Proposed flow:**
```rust
async fn handle(&self, args: HashMap<String, String>, extra: RequestHandlerExtra)
    -> Result<GetPromptResult>
{
    let mut messages = Vec::new();
    let mut execution_context = ExecutionContext::new();

    // NEW: Create task if task_router configured AND workflow has task_support
    let task_state = if self.workflow.task_support() {
        if let Some(ref router) = self.task_router {
            let owner_id = self.resolve_owner(&extra);
            let task_json = router.create_workflow_task(
                &owner_id,
                self.workflow.name(),
                self.workflow.steps().len(),
                serde_json::to_value(&args).unwrap_or(Value::Null),
            ).await?;
            let task_id = task_json["taskId"].as_str()
                .ok_or_else(|| crate::Error::internal("missing taskId"))?
                .to_string();
            Some(WorkflowTaskState { router: router.clone(), task_id, owner_id })
        } else { None }
    } else { None };

    messages.push(self.create_user_intent(&args));
    messages.push(self.create_assistant_plan()?);
    // ... step loop with task_state passed through ...
    // ... structured reply construction ...
    Ok(GetPromptResult { description, messages })
}
```

**Why `TaskRouter`, not `TaskStore` directly:** The `WorkflowPromptHandler` lives in `pmcp` core. `TaskStore` is defined in `pmcp-tasks`. Using `TaskStore` directly would create a circular dependency (`pmcp-tasks` depends on `pmcp`). The `TaskRouter` trait in `pmcp` core uses `serde_json::Value` to avoid this, same as the existing v1.0 design for `handle_task_call`.

### 2. TaskRouter Trait Extension (Minimal)

**Current trait (src/server/tasks.rs):** 7 methods for tools/call task lifecycle + owner resolution + capabilities.

**3 new methods:**

```rust
#[async_trait]
pub trait TaskRouter: Send + Sync {
    // ... existing 7 methods unchanged ...

    /// Create a task for a workflow prompt execution.
    ///
    /// Returns a JSON object with at minimum { "taskId": "..." }.
    /// Called by WorkflowPromptHandler on prompts/get entry.
    async fn create_workflow_task(
        &self,
        owner_id: &str,
        workflow_name: &str,
        step_count: usize,
        prompt_args: Value,
    ) -> Result<Value> {
        // Default impl: tasks not supported
        Err(crate::Error::internal("task router does not support workflow tasks"))
    }

    /// Set variables on a task (used by workflow handler to sync step results).
    ///
    /// Variables is a JSON object of flat key-value pairs.
    async fn set_task_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: Value,
    ) -> Result<()> {
        let _ = (task_id, owner_id, variables);
        Err(crate::Error::internal("task router does not support variable sync"))
    }

    /// Complete a workflow task with the structured result.
    async fn complete_workflow_task(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<()> {
        let _ = (task_id, owner_id, result);
        Err(crate::Error::internal("task router does not support workflow completion"))
    }
}
```

**Non-breaking:** Default implementations return errors. Existing `TaskRouterImpl` continues to compile without changes until Phase 2 implements the new methods.

**Value-based interface:** Same rationale as existing methods -- avoids importing `pmcp-tasks` types into `pmcp` core.

### 3. Partial Execution: How the Handler Flow Changes

**Current behavior (prompt_handler.rs lines 785-961):** Execute ALL steps in sequence. On parameter resolution failure, `break`. On tool execution error, `break`. On schema mismatch, `break`.

**Key insight:** The current code already implements partial execution. When it `break`s out of the step loop, it returns whatever messages have been built so far. The task-prompt bridge makes this _explicit_ by recording progress in task variables.

**Modified step loop (pseudocode showing only changes):**

```rust
let mut completed_steps: Vec<StepSummary> = Vec::new();
let mut remaining_steps: Vec<StepSummary> = Vec::new();

for (step_index, step) in self.workflow.steps().iter().enumerate() {
    // ... existing: cancellation check, progress report, guidance ...

    // Tool execution attempt (logic unchanged from current code)
    match self.create_tool_call_announcement(step, &args, &execution_context) {
        Ok(announcement) => {
            // ... existing: schema check, execute tool ...
            match self.execute_tool_step(step, &args, &execution_context, &extra).await {
                Ok(result) => {
                    // ... existing: add result message, store binding ...

                    // NEW: sync to task variables
                    if let Some(ref ts) = task_state {
                        let mut vars = serde_json::Map::new();
                        vars.insert(
                            format!("wf.steps.{}.status", step_index),
                            json!("completed"),
                        );
                        vars.insert(
                            format!("wf.steps.{}.binding", step_index),
                            json!(step.binding().map(|b| b.as_str())),
                        );
                        // Store key result fields (not full result to avoid size limits)
                        if let Some(binding) = step.binding() {
                            vars.insert(
                                format!("wf.steps.{}.result_summary", step_index),
                                summarize_result(&result),
                            );
                        }
                        let _ = ts.router.set_task_variables(
                            &ts.task_id, &ts.owner_id, Value::Object(vars),
                        ).await; // Non-critical; log warning on failure
                    }
                    completed_steps.push(StepSummary::completed(step, step_index));
                },
                Err(e) => {
                    // ... existing: error message, break ...
                    // NEW: record this step as failed
                    remaining_steps.push(StepSummary::failed(step, step_index, &e));
                    // Record all subsequent steps as pending
                    for (j, s) in self.workflow.steps().iter().enumerate().skip(step_index + 1) {
                        remaining_steps.push(StepSummary::pending(s, j));
                    }
                    break;
                },
            }
        },
        Err(_) => {
            // Cannot resolve parameters -- same break as today
            // NEW: record this and remaining steps
            for (j, s) in self.workflow.steps().iter().enumerate().skip(step_index) {
                remaining_steps.push(StepSummary::pending(s, j));
            }
            break;
        },
    }
}
```

### 4. Step State Storage in Task Variables

**Schema (flat keys with `wf.` prefix):**

```json
{
    "wf.goal": "Deploy a service",
    "wf.total_steps": 3,
    "wf.completed_count": 1,
    "wf.prompt_args": {"region": "us-east-1", "service": "my-api"},

    "wf.steps.0.name": "validate_config",
    "wf.steps.0.status": "completed",
    "wf.steps.0.tool": "validate_config",
    "wf.steps.0.binding": "config",
    "wf.steps.0.result_summary": {"valid": true},

    "wf.steps.1.name": "provision_infra",
    "wf.steps.1.status": "pending",
    "wf.steps.1.tool": "provision_infra",
    "wf.steps.1.guidance": "Provision with the validated config",

    "wf.steps.2.name": "deploy",
    "wf.steps.2.status": "pending",
    "wf.steps.2.tool": "deploy_service",
    "wf.steps.2.guidance": "Deploy using the provisioned infrastructure"
}
```

**Why flat keys:** Matches the validated v1.0 decision (PROJECT.md: "flat keys with convention recommendation in docs"). DynamoDB attribute-level operations work better with flat keys. Individual step updates are independent operations.

**Why `result_summary` not full result:** Task variables have a 1MB limit (StoreConfig.max_variable_size_bytes). Full tool results can be large. The summary stores only key output fields needed by subsequent steps. The full result is already in the conversation trace (messages array) and in the ExecutionContext bindings.

### 5. Structured Prompt Reply

**Constraint:** `GetPromptResult { description: Option<String>, messages: Vec<PromptMessage> }` is an MCP protocol type. We cannot add fields without a breaking change.

**Solution:** Embed structured guidance as the final assistant message in the messages array. This is both human-readable (for the LLM) and machine-parseable (JSON block for task-aware clients).

```rust
// After the step loop, if task is active:
if let Some(ref ts) = task_state {
    let structured = json!({
        "task_id": ts.task_id,
        "status": if remaining_steps.is_empty() { "all_completed" } else { "partial" },
        "completed": completed_steps.iter().map(|s| json!({
            "name": s.name,
            "tool": s.tool_name,
            "binding": s.binding_name,
        })).collect::<Vec<_>>(),
        "remaining": remaining_steps.iter().map(|s| json!({
            "index": s.index,
            "name": s.name,
            "tool": s.tool_name,
            "guidance": s.guidance,
        })).collect::<Vec<_>>(),
        "continuation": format!(
            "Call each remaining tool in order. \
             Poll tasks/get with taskId '{}' to check variable accumulation.",
            ts.task_id,
        ),
    });

    messages.push(PromptMessage {
        role: Role::Assistant,
        content: MessageContent::Text {
            text: format!(
                "## Workflow Progress\n\n\
                 Task ID: `{}`\n\
                 Completed: {}/{} steps\n\n\
                 {}\n\n\
                 ```json\n{}\n```",
                ts.task_id,
                completed_steps.len(),
                completed_steps.len() + remaining_steps.len(),
                remaining_steps_text,
                serde_json::to_string_pretty(&structured).unwrap(),
            ),
        },
    });

    // If ALL steps completed, mark task as completed
    if remaining_steps.is_empty() {
        let _ = ts.router.complete_workflow_task(
            &ts.task_id,
            &ts.owner_id,
            serde_json::to_value(&completed_steps).unwrap_or(Value::Null),
        ).await;
    }
    // Otherwise task stays in "working" -- client continues
}
```

**Why this differs from current full-trace reply:** Today, the reply is an opaque conversation trace. The LLM reads it and decides what to do. With the bridge, the reply includes explicit machine-readable guidance: here is the task ID, here are the steps you still need to do, here are the tools to call. This transforms the prompt from "here's what happened" to "here's what happened AND here's what to do next."

### 6. Does TaskRouter Need Changes? -- YES, Minimal

**Added methods (3):**

| Method | Params (Value-based) | Implementation in TaskRouterImpl |
|--------|---------------------|----------------------------------|
| `create_workflow_task` | owner_id, workflow_name, step_count, prompt_args | `store.create(owner_id, "prompts/get", None)` + `store.set_variables(task_id, owner_id, {wf.goal, wf.total_steps, wf.prompt_args})` |
| `set_task_variables` | task_id, owner_id, variables (JSON object) | `store.set_variables(task_id, owner_id, convert_json_to_hashmap(variables))` |
| `complete_workflow_task` | task_id, owner_id, result | `store.complete_with_result(task_id, owner_id, Completed, None, result)` |

**Existing methods unchanged:** `handle_task_call`, `handle_tasks_get`, `handle_tasks_result`, `handle_tasks_list`, `handle_tasks_cancel`, `resolve_owner`, `tool_requires_task`, `task_capabilities` -- all remain exactly as they are.

**Why not modify `handle_task_call`:** That method is for `tools/call` with task augmentation. Workflow task creation has different semantics: it stores workflow metadata (goal, steps, args), not tool context (tool_name, arguments, progress_token).

## Data Flow: Complete Lifecycle

```
CLIENT                    SERVER (prompts/get handler)         TaskRouter/Store
  |                              |                                  |
  |--- prompts/get "deploy" --->|                                  |
  |                              |                                  |
  |                              |--- create_workflow_task() ------>|
  |                              |<-- {taskId: "t-123"} -----------|
  |                              |                                  |
  |                              |--- set_task_variables ---------->|
  |                              |    {wf.goal, wf.total_steps}    |
  |                              |                                  |
  |                              |  STEP LOOP:                      |
  |                              |  step 0: validate_config         |
  |                              |    resolve params -> OK          |
  |                              |    execute tool -> OK            |
  |                              |    store binding "config"        |
  |                              |--- set_task_variables ---------->|
  |                              |    {wf.steps.0.status:completed} |
  |                              |                                  |
  |                              |  step 1: provision_infra         |
  |                              |    resolve params -> FAIL        |
  |                              |    (needs LLM reasoning)         |
  |                              |--- set_task_variables ---------->|
  |                              |    {wf.steps.1.status:pending,   |
  |                              |     wf.steps.2.status:pending}   |
  |                              |  BREAK                           |
  |                              |                                  |
  |<-- GetPromptResult ---------|                                  |
  |    messages: [               |                                  |
  |      user intent,            |                                  |
  |      assistant plan,         |                                  |
  |      assistant: calling      |                                  |
  |        validate_config,      |                                  |
  |      user: tool result,      |                                  |
  |      assistant: structured   |                                  |
  |        guidance {            |                                  |
  |          task_id: "t-123",   |                                  |
  |          completed: [0],     |                                  |
  |          remaining: [1, 2]   |                                  |
  |        }                     |                                  |
  |    ]                         |                                  |
  |                              |                                  |
  |                              |                                  |
  |--- tools/call provision_infra (direct, no task field) -------->|
  |<-- result -----------------------------------------------------|
  |                              |                                  |
  |--- tasks/get {taskId: "t-123"} ---->|                          |
  |                              |--- store.get("t-123") --------->|
  |<-- task with variables ------|<-- TaskRecord with wf.steps.* --|
  |                              |                                  |
  |--- tools/call deploy_service (direct) ----------------------->|
  |<-- result -----------------------------------------------------|
```

## Build Order (Dependency-Aware)

### Phase 1: TaskRouter trait extension (pmcp core, ~30 LOC)

**File:** `src/server/tasks.rs`

Add 3 new methods with default implementations returning errors. Non-breaking -- existing `TaskRouterImpl` compiles unchanged.

**Why first:** Everything downstream depends on this interface. It is the smallest change and the one that all other phases import.

### Phase 2: TaskRouterImpl new methods (pmcp-tasks, ~80 LOC)

**File:** `crates/pmcp-tasks/src/router.rs`

Implement the 3 new methods:
- `create_workflow_task` -> `store.create()` + `store.set_variables()` for initial workflow metadata
- `set_task_variables` -> convert `Value` to `HashMap<String, Value>` + `store.set_variables()`
- `complete_workflow_task` -> `store.complete_with_result()`

**Why second:** Provides the concrete implementation the handler tests against.

### Phase 3: WorkflowPromptHandler modifications (pmcp core, ~150 LOC)

**File:** `src/server/workflow/prompt_handler.rs`

1. Add `task_router: Option<Arc<dyn TaskRouter>>` field
2. Add `with_task_router()` constructor (parallel to existing `with_middleware_executor()`)
3. Add helper struct `WorkflowTaskState { router, task_id, owner_id }`
4. Add helper struct `StepSummary { name, tool_name, binding_name, index, status, guidance }`
5. Modify `handle()`: task creation, step sync, structured reply

**Why third:** Depends on Phase 1 (trait) and tested against Phase 2 (impl).

### Phase 4: SequentialWorkflow opt-in flag (pmcp core, ~15 LOC)

**File:** `src/server/workflow/sequential.rs`

Add `task_support: bool` field (default false) with builder method `with_task_support(bool) -> Self`. When false, handler skips task creation even if router is configured.

**Why fourth:** Simple field addition. Depends on nothing, but Phase 3 reads this flag.

### Phase 5: ServerCoreBuilder wiring (pmcp core, ~10 LOC)

**File:** `src/server/builder.rs`

Modify the `prompt_workflow()` method to pass `self.task_router.clone()` to `WorkflowPromptHandler::with_task_router()` when the task router is configured.

**Why fifth:** Simple wiring. Depends on Phase 3 (handler accepts router) and Phase 4 (workflow has flag).

### Phase 6: Example and tests

**Files:** `examples/62_tasks_workflow.rs`, tests in `prompt_handler.rs` and `crates/pmcp-tasks/src/router.rs`

End-to-end: define workflow with `with_task_support(true)`, configure task store + router, invoke prompt, verify task created, verify variables populated, verify structured reply content.

## Anti-Patterns to Avoid

### Anti-Pattern 1: Importing TaskStore in pmcp Core

**What people do:** Import `TaskStore` or `TaskContext` from `pmcp-tasks` directly in workflow handler code.
**Why wrong:** `pmcp-tasks` depends on `pmcp`. If `pmcp` imports from `pmcp-tasks`, circular dependency.
**Do instead:** All task operations from `pmcp` core go through `Arc<dyn TaskRouter>` with `serde_json::Value`. This is the established pattern from v1.0.

### Anti-Pattern 2: Storing Full Tool Results in Task Variables

**What people do:** Store the complete tool output JSON as a task variable for each step.
**Why wrong:** Task variables have a 1MB limit. Accumulating full results across steps can exceed this.
**Do instead:** Store summaries or key fields in task variables. Full results are in the conversation trace (messages array) and the execution context bindings.

### Anti-Pattern 3: Making Task Creation Mandatory

**What people do:** Every `prompts/get` creates a task, even for simple 1-step workflows.
**Why wrong:** Simple workflows that always execute fully server-side gain nothing. Extra latency, extra storage.
**Do instead:** Task creation is opt-in via `SequentialWorkflow.with_task_support(true)`. Default is false.

### Anti-Pattern 4: Adding Fields to GetPromptResult

**What people do:** Add `task_id`, `completed_steps`, `remaining_steps` fields to `GetPromptResult`.
**Why wrong:** `GetPromptResult` is an MCP protocol type. Adding fields is a spec violation.
**Do instead:** Embed structured guidance as the final assistant message text. Both human-readable and machine-parseable via JSON block.

### Anti-Pattern 5: Blocking prompts/get on Task Completion

**What people do:** Create a task and wait for it to complete before returning the prompt result.
**Why wrong:** The point is partial execution. Return immediately with what the server could do.
**Do instead:** Execute steps synchronously (as today), record progress, return immediately. Task stays in `working` state for client continuation.

## Component Boundary Summary

```
+--- pmcp (core crate) --------------------------------------------------+
|                                                                         |
|  TaskRouter trait              WorkflowPromptHandler                    |
|  (src/server/tasks.rs)         (src/server/workflow/prompt_handler.rs)  |
|  +3 new methods:               +task_router: Option<Arc<dyn TaskRouter>>|
|  - create_workflow_task()      +with_task_router() constructor          |
|  - set_task_variables()        Modified handle(): task create, sync,    |
|  - complete_workflow_task()      structured reply                       |
|                                                                         |
|  SequentialWorkflow            ServerCoreBuilder                        |
|  +task_support: bool           passes task_router to handler            |
|  +with_task_support()                                                   |
|                                                                         |
+--- pmcp-tasks (separate crate, one-way dep on pmcp) -------------------+
|                                                                         |
|  TaskRouterImpl                TaskStore trait (UNCHANGED)               |
|  (src/router.rs)               (src/store/mod.rs)                       |
|  +3 new method impls           InMemoryTaskStore (UNCHANGED)            |
|   delegates to store                                                    |
|                                                                         |
|  TaskContext (UNCHANGED)                                                |
|  (src/context.rs)                                                       |
|  Used internally by router                                              |
|                                                                         |
+-------------------------------------------------------------------------+
```

## Sources

All findings derived from direct codebase analysis:

- `src/server/workflow/prompt_handler.rs` -- WorkflowPromptHandler, ExecutionContext, step loop, break behavior
- `src/server/tasks.rs` -- TaskRouter trait definition (7 existing methods)
- `src/server/builder.rs` -- ServerCoreBuilder.with_task_store(), prompt_workflow()
- `src/server/core.rs` -- ServerCore request routing, task_router field usage
- `src/server/middleware_executor.rs` -- MiddlewareExecutor trait
- `src/server/workflow/sequential.rs` -- SequentialWorkflow struct, validate()
- `src/server/workflow/workflow_step.rs` -- WorkflowStep, DataSource resolution
- `src/server/workflow/data_source.rs` -- DataSource enum (PromptArg, StepOutput, Constant)
- `crates/pmcp-tasks/src/router.rs` -- TaskRouterImpl, handle_task_call, store delegation
- `crates/pmcp-tasks/src/context.rs` -- TaskContext, variable accessors, status transitions
- `crates/pmcp-tasks/src/store/mod.rs` -- TaskStore trait, StoreConfig, TaskPage
- `crates/pmcp-tasks/src/domain/record.rs` -- TaskRecord, to_wire_task_with_variables
- `crates/pmcp-tasks/src/lib.rs` -- pmcp-tasks module organization
- `docs/design/tasks-feature-design.md` -- Design document for tasks feature
- `.planning/PROJECT.md` -- v1.1 milestone definition, requirements, constraints

---
*Architecture research for: Task-Prompt Bridge Integration in PMCP SDK*
*Researched: 2026-02-21*
