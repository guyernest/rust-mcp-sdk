# Stack Research: Task-Prompt Bridge

**Domain:** Bridging task lifecycle with workflow prompt execution in PMCP SDK
**Researched:** 2026-02-21
**Confidence:** HIGH
**Mode:** Subsequent milestone -- only NEW stack needs, not re-researching v1.0 foundations

## Executive Assessment

**No new crate dependencies are needed.** The task-prompt bridge is an architectural integration, not a technology addition. Every capability required -- partial execution control flow, step state serialization, structured prompt reply formatting, and WorkflowPromptHandler modification -- is achievable with the existing dependency graph in `pmcp-tasks` and `pmcp` core.

The work is entirely about wiring existing types together: `WorkflowPromptHandler` (which already executes steps sequentially and already has a graceful "break" pattern for unresolvable steps) needs to optionally create a `TaskContext` at the start of execution and persist step progress into task variables as it runs. The return format changes from a bare `GetPromptResult` to one that includes structured step metadata in `_meta`.

## Stack Status: No Changes Required

### Dependencies Already Sufficient

| Capability Needed | Already Available In | How |
|---|---|---|
| Task creation during prompt execution | `pmcp-tasks` (`TaskStore::create`, `TaskContext::new`) | WorkflowPromptHandler gets optional `Arc<dyn TaskStore>` |
| Step state serialization | `serde` + `serde_json` (already deps) | Serialize step progress as `Value` into task variables |
| Structured prompt reply with `_meta` | `serde_json::Map` (already used in `GetPromptResult`) | Add `_meta` field to `GetPromptResult` or embed in messages |
| Variable tracking per step | `TaskContext::set_variable` / `set_variables` | One store call per completed step |
| Partial execution detection | Already exists in `WorkflowPromptHandler::handle()` | The `break` pattern at lines 891-958 already handles "cannot resolve parameters" |
| Owner ID resolution | `pmcp-tasks::resolve_owner_id` | Extract from `RequestHandlerExtra` auth context |
| Typed variable accessors | `TaskContext::get_string`, `get_typed` | Read step results back from task variables |

### Why No New Dependencies

1. **Serialization of step state:** `serde_json::Value` already represents arbitrary step outputs. The `WorkflowPromptHandler::ExecutionContext` already stores `HashMap<BindingName, Value>`. Converting this to task variables is a `HashMap<String, Value>` which `TaskStore::set_variables` accepts directly.

2. **Structured prompt reply:** The MCP `GetPromptResult` type already has a `description: Option<String>` field. For richer metadata, the prompt messages themselves can carry structured JSON in text content. No schema validation library is needed because the reply format is a convention between server and client (the LLM interprets it), not a wire protocol contract.

3. **Partial execution control flow:** The existing `break` statements in `WorkflowPromptHandler::handle()` already implement this pattern. When parameter resolution fails (line 958: "Cannot resolve parameters deterministically") or schema satisfaction fails (line 891: "Params resolved but incomplete"), the handler breaks out of the step loop and returns partial results. The bridge simply adds "persist progress to task before breaking."

4. **Step state schema:** The task variables schema (`goal`, `completed_steps`, `remaining_steps`, `step_results`) is pure `serde_json::Value` construction. No schema definition library (like `schemars` or `jsonschema`) is needed because the schema is a PMCP convention, not a validated contract.

## Integration Points (Where Code Changes)

### 1. WorkflowPromptHandler -- Primary Integration

**Current state:** Owns `SequentialWorkflow`, `tools`, `middleware_executor`, `tool_handlers`, `resource_handler`. Executes all steps during `prompts/get`, returns conversation trace as `GetPromptResult`.

**What changes:**

| Field/Method | Change | Why |
|---|---|---|
| New field: `task_store: Option<Arc<dyn TaskStore>>` | Add | Optional task backing for workflow execution |
| New constructor: `with_task_store()` | Add builder method | Pass store when registering workflow |
| `handle()` method | Modify | Create task at start, persist step results as variables, include task ID and step metadata in reply |
| New method: `persist_step_progress()` | Add private helper | Writes completed step name + result to task variables |
| New method: `build_structured_reply()` | Add private helper | Constructs the step guidance metadata |

**Dependency path:**
```
WorkflowPromptHandler (in pmcp crate)
  -> needs Arc<dyn TaskStore> (trait defined in pmcp-tasks)
```

**Problem:** This creates a dependency from `pmcp` -> `pmcp-tasks`, which violates the current one-directional dependency (`pmcp-tasks` -> `pmcp`).

**Solution:** Use the same pattern as `TaskRouter`. Define a `WorkflowTaskBridge` trait in `pmcp` (with `serde_json::Value` params), implement it in `pmcp-tasks`. This maintains the one-directional dependency.

```rust
// In pmcp::server::tasks (or new module pmcp::server::workflow_bridge)
#[async_trait]
pub trait WorkflowTaskBridge: Send + Sync {
    /// Create a task for a workflow execution
    async fn create_workflow_task(
        &self,
        owner_id: &str,
        workflow_name: &str,
        ttl: Option<u64>,
    ) -> Result<Value>;

    /// Store step progress as task variables
    async fn persist_step_result(
        &self,
        task_id: &str,
        owner_id: &str,
        step_name: &str,
        step_index: usize,
        result: Value,
    ) -> Result<()>;

    /// Get all step progress for a task
    async fn get_step_progress(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<Value>;

    /// Complete the workflow task with final result
    async fn complete_workflow(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<()>;
}
```

### 2. SequentialWorkflow -- Minor Extension

**Current state:** Pure data structure. Holds name, description, arguments, steps, instructions.

**What changes:**

| Change | Why |
|---|---|
| New field: `task_support: Option<TaskSupportMode>` | Declare whether this workflow creates tasks |
| New method: `with_task_support(mode)` | Builder chain |

`TaskSupportMode` is a simple enum (`Disabled`, `Optional`, `Required`) defined in `pmcp` core. Not the same as `pmcp-tasks::TaskSupport` (which is for tools), but semantically similar.

### 3. GetPromptResult -- Structured Reply Embedding

**Current state:** `{ description: Option<String>, messages: Vec<PromptMessage> }`.

**What changes:** No structural change to `GetPromptResult`. Instead, embed step metadata in the conversation trace messages themselves. This keeps the protocol type unchanged while providing structured guidance.

The last assistant message becomes the "step guidance" message:

```json
{
  "role": "assistant",
  "content": {
    "type": "text",
    "text": "## Workflow Progress\n\n**Task ID:** task-abc-123\n**Status:** partial (2/4 steps completed)\n\n### Completed Steps\n1. validate_config: {\"valid\": true, \"region\": \"us-east-1\"}\n2. check_resources: {\"available\": true}\n\n### Remaining Steps\n3. deploy_service (needs: approval, config from step 1)\n4. verify_deployment (needs: deployment_id from step 3)\n\n### Next Action\nCall tool `deploy_service` with arguments:\n- config: (from task variable `validate_config_result`)\n- region: \"us-east-1\"\n\nUse task ID `task-abc-123` to track progress."
  }
}
```

**Why text, not `_meta`:** MCP `PromptMessage` does not have a `_meta` field in the current spec. Adding one would be a protocol extension. Using structured text in the message is MCP-compliant, LLM-readable, and requires no protocol changes.

### 4. DataSource -- New Variant (Optional)

**Current state:** `PromptArg`, `StepOutput`, `Constant`.

**Potential addition:** `TaskVariable { key: String }` -- resolve from task variables instead of execution context bindings. This would allow workflows that resume from a task to read previously stored state.

**Assessment:** Defer this. For v1.1, step outputs stored as task variables can be read back via `TaskContext::get_variable()` inside the handler. A new `DataSource` variant would be valuable for v1.2 when workflows support true resume-from-checkpoint, but it is unnecessary for the initial bridge.

## Step State Variable Schema

The standard schema for workflow progress in task variables uses flat keys with a `workflow.` prefix convention. No schema validation library needed -- this is a convention enforced by `WorkflowTaskBridge` implementation code.

```json
{
  "workflow.name": "deploy-service",
  "workflow.total_steps": 4,
  "workflow.completed_count": 2,
  "workflow.status": "partial",
  "workflow.step.0.name": "validate_config",
  "workflow.step.0.status": "completed",
  "workflow.step.0.result": {"valid": true, "region": "us-east-1"},
  "workflow.step.1.name": "check_resources",
  "workflow.step.1.status": "completed",
  "workflow.step.1.result": {"available": true},
  "workflow.step.2.name": "deploy_service",
  "workflow.step.2.status": "pending",
  "workflow.step.3.name": "verify_deployment",
  "workflow.step.3.status": "pending"
}
```

**Why flat keys with prefix:** Validated in v1.0 -- flat keys are sufficient, namespace via convention in docs. The `workflow.` prefix avoids collision with tool-level variables (`tool_name`, `arguments`, `progress_token` stored by `TaskRouterImpl::handle_task_call`).

**Why individual step keys (not a single JSON array):** Task variable merge semantics are key-level. Storing all steps in a single array variable means every update overwrites the entire array. Individual keys allow atomic per-step updates without read-modify-write races.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|---|---|---|---|
| Bridge pattern | `WorkflowTaskBridge` trait in pmcp | Direct `pmcp-tasks` dependency from pmcp | Violates one-directional dependency. Would make experimental task code a hard dependency of the stable SDK. |
| Step metadata in reply | Structured text in last assistant message | New `_meta` field on `PromptMessage` | `PromptMessage` has no `_meta` in the MCP spec. Protocol extension is out of scope. Text is LLM-readable. |
| Step state storage | Flat task variables with `workflow.` prefix | Nested JSON object in single variable | Flat keys support atomic per-step updates. Nested object requires read-modify-write for each step. |
| Execution resumption | Client follows step list, calls tools directly | Server-side resume from checkpoint | Server resume requires request-response lifecycle management beyond `prompts/get`. Defer to v1.2. Client-driven continuation is simpler and works with existing MCP flow. |
| Schema validation | Convention in code (no library) | `schemars` + `jsonschema` | Over-engineering. The variable schema is internal convention, not a validated contract. 20 lines of code vs. 2 new dependencies. |
| New DataSource variant | Defer `TaskVariable` to v1.2 | Add now | Unnecessary for v1.1. Step outputs are stored in task variables AND in execution context bindings. The handler reads from bindings during execution, stores to variables for persistence. |

## What NOT to Add

| Technology | Why Not |
|---|---|
| `jsonschema` crate | No runtime schema validation needed for task variable conventions |
| `schemars` crate | JSON Schema generation not needed -- variable keys are code convention |
| `state_machine` / `rust-fsm` | The "partial execution" flow is a simple break-from-loop, not a state machine |
| New serialization format | `serde_json::Value` handles all step result serialization already |
| `pmcp-tasks` as dependency of `pmcp` | Violates crate isolation. Use trait bridge pattern. |
| Channels / `tokio::mpsc` for step progress | Step execution is synchronous-sequential within `handle()`. No async coordination needed. |
| `dashmap` in prompt handler | The execution context is single-threaded (one `handle()` call). `HashMap` is correct. |

## Integration Dependency Graph

```
pmcp (core SDK)
  |-- server::tasks::TaskRouter         (trait, existing)
  |-- server::tasks::WorkflowTaskBridge (trait, NEW)
  |-- server::workflow::WorkflowPromptHandler
  |     |-- uses WorkflowTaskBridge (via Option<Arc<dyn WorkflowTaskBridge>>)
  |     |-- creates task at prompts/get start
  |     |-- persists step results as task variables
  |     |-- returns structured step guidance in messages
  |
pmcp-tasks (tasks crate, depends on pmcp)
  |-- router::TaskRouterImpl            (impl TaskRouter, existing)
  |-- bridge::WorkflowTaskBridgeImpl    (impl WorkflowTaskBridge, NEW)
  |     |-- wraps TaskStore
  |     |-- manages workflow.* variable keys
  |     |-- provides step progress accessors
```

**Direction of dependency:** `pmcp-tasks` -> `pmcp` (unchanged). The `WorkflowTaskBridge` trait lives in `pmcp` so that `WorkflowPromptHandler` can use it without depending on `pmcp-tasks`.

## Version Compatibility

No version changes from v1.0 research. All dependencies remain the same:

| Package | Version | Status |
|---|---|---|
| `serde` | 1.0 | Unchanged |
| `serde_json` | 1.0 | Unchanged |
| `async-trait` | 0.1 | Unchanged, needed for `WorkflowTaskBridge` trait |
| `thiserror` | 2.0 | Unchanged |
| `tokio` | 1 | Unchanged |
| `tracing` | 0.1 | Unchanged |
| All pmcp-tasks deps | Same as v1.0 | No additions |

## Sources

- Codebase analysis: `src/server/workflow/prompt_handler.rs` -- existing execution flow with break-on-failure pattern (lines 838-961)
- Codebase analysis: `src/server/tasks.rs` -- existing `TaskRouter` trait pattern for cross-crate integration
- Codebase analysis: `crates/pmcp-tasks/src/router.rs` -- existing `TaskRouterImpl` as reference for bridge implementation
- Codebase analysis: `crates/pmcp-tasks/src/context.rs` -- `TaskContext` API for variable read/write
- Codebase analysis: `crates/pmcp-tasks/src/domain/record.rs` -- `TaskRecord::to_wire_task_with_variables()` for variable injection into `_meta`
- Codebase analysis: `crates/pmcp-tasks/Cargo.toml` -- current dependency set
- Project doc: `.planning/PROJECT.md` -- v1.1 milestone scope and validated decisions
- Design doc: `docs/design/tasks-feature-design.md` -- workflow integration vision (section 8.5)

---
*Stack research for: Task-Prompt Bridge (v1.1 milestone)*
*Researched: 2026-02-21*
*Key finding: Zero new dependencies. This is an architecture problem, not a technology problem.*
