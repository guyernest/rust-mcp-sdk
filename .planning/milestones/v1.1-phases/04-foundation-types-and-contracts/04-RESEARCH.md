# Phase 4: Foundation Types and Contracts - Research

**Researched:** 2026-02-22
**Domain:** Rust type design, trait extension, composition patterns for task-prompt bridge
**Confidence:** HIGH

## Summary

Phase 4 defines the type contracts and composition boundary for the task-prompt bridge. The work spans two crates: `pmcp-tasks` (new types like `WorkflowProgress`, new `TaskRouter` methods) and `pmcp` (new `GetPromptResult._meta` field, new `TaskWorkflowPromptHandler` struct). The critical constraint is **zero modification** to the existing `WorkflowPromptHandler` -- all task-aware behavior must be additive via composition.

The existing codebase is well-structured for this extension. `WorkflowPromptHandler` has clean internal boundaries: `ExecutionContext` for bindings, `SequentialWorkflow` for step definitions, and separate `resolve_*`/`execute_*` methods. The `TaskRouter` trait in `pmcp/src/server/tasks.rs` uses `serde_json::Value` to avoid circular crate dependencies. The `ServerCoreBuilder` already has `with_task_store()` accepting `Arc<dyn TaskRouter>`. All of these provide natural extension points.

The CONTEXT.md decision to drop FNDX-02 (StepExecution enum) simplifies the phase significantly. Steps are just a plan; the server does best-effort execution at runtime, which is already the pattern in `WorkflowPromptHandler` lines 874-960 (break on unresolvable params, unsatisfied schema, or tool errors). The new `TaskWorkflowPromptHandler` will delegate to these same mechanisms, adding task variable persistence around the existing execution loop.

**Primary recommendation:** Implement foundation types in `pmcp-tasks` crate (WorkflowProgress, WorkflowStepStatus), extend `TaskRouter` trait with 3 new default-error methods, add `_meta` to `GetPromptResult`, and create `TaskWorkflowPromptHandler` in `pmcp` that composes with `WorkflowPromptHandler` internals via delegation -- all without modifying existing files beyond the `GetPromptResult` struct.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Variable schema (hybrid approach)**: Structured progress object under `_workflow.progress` key -- contains full step definitions: `{goal, steps: [{name, tool, status, execution_mode}], schema_version}`. Flat result keys under `_workflow.result.<step_name>` -- one key per completed step with its tool result. Prefix convention: `_workflow.` (single underscore, matches `_meta` convention). Result storage: Default to raw tool result JSON, but allow WorkflowStep to specify a `result_summary_fn` for large outputs (configurable per step).
- **Step execution mode (no StepExecution enum)**: Steps are a plan, not a fixed assignment. The server tries best-effort execution at runtime -- no pre-classification of steps as server/client. Pause is runtime-emergent: server runs until it can't resolve parameters, can't satisfy tool schema, or tool execution fails. This matches existing WorkflowPromptHandler behavior (lines 874-960 in prompt_handler.rs). Tool execution errors are pause points, not failures: task stays Working, error recorded in variables, client decides what to do. **FNDX-02 (StepExecution enum) is dropped** -- the runtime pause mechanism replaces it.
- **Handoff format (_meta on GetPromptResult)**: Message list stays as-is (MCP-standard prompt reply format). Task data goes in `_meta` on GetPromptResult -- requires adding `_meta: Option<serde_json::Map<String, Value>>` to `GetPromptResult`. Prompt reply `_meta` is simplified -- task_id, task_status, and a brief step plan. Tool call `_meta` is more complete (when `_task_id` present). `_meta` should be mostly consistent across prompt results and tool call results.
- **Composition model (per-workflow opt-in)**: Task store on builder enables the capability. Each workflow opts in individually. Backward compatibility is absolute: existing workflows registered via `prompt_workflow()` must work identically whether or not a task store is configured.

### Claude's Discretion
- Opt-in mechanism placement (workflow builder vs server builder method)
- Composition implementation (delegation vs independent with shared helpers)
- Exact `_meta` JSON structure (field names, nesting)
- Result summary function signature and defaults

### Deferred Ideas (OUT OF SCOPE)
- Cross-server task sharing on pmcp.run -- captured in PROJECT.md Future requirements
- Client-initiated task plans -- MCP client creates a task with a plan and shares it with the server for reference
- DataSource::TaskVariable for reading from task variable store -- v2 requirement (ADVW-01)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FNDX-01 | Workflow prompt can create a task when invoked, binding the task to the prompt execution | TaskRouter trait extension with `create_workflow_task` method; TaskWorkflowPromptHandler creates task at start of `get_prompt` |
| FNDX-02 | ~~WorkflowStep declares execution mode via StepExecution enum~~ | **DROPPED per CONTEXT.md** -- runtime best-effort execution replaces static classification. Existing `break` pattern in `WorkflowPromptHandler` already handles this. |
| FNDX-03 | Typed WorkflowProgress schema struct tracks goal, completed steps, remaining steps, and intermediate outputs in task variables | New `WorkflowProgress` struct in `pmcp-tasks` with `schema_version`, serializes to/from task variable JSON under `_workflow.progress` key |
| FNDX-04 | TaskRouter trait extended with workflow-specific methods (create_workflow_task, set_task_variables, complete_workflow_task) | 3 new methods on `TaskRouter` trait with default `Err` implementations; `TaskRouterImpl` provides concrete implementations |
| FNDX-05 | TaskWorkflowPromptHandler composes with (not modifies) existing WorkflowPromptHandler | New struct in `pmcp/src/server/workflow/` that delegates to `WorkflowPromptHandler` -- original file has zero diff |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp` | 1.10.3 | Main SDK crate -- `GetPromptResult`, `TaskRouter` trait, `WorkflowPromptHandler` | This is the project |
| `pmcp-tasks` | 0.1.0 | Task types, TaskStore, TaskRouterImpl | Where new foundation types live |
| `serde` | 1.0 | Serialization for WorkflowProgress, WorkflowStepStatus | Already in both crates |
| `serde_json` | 1.0 | JSON value types for `_meta`, task variables | Already in both crates |
| `async-trait` | 0.1 | Async trait methods on TaskRouter | Already in both crates |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `indexmap` | 2.10 | Deterministic step ordering in WorkflowProgress | Already in pmcp, matches SequentialWorkflow pattern |
| `smallvec` | 1.13 | Optimized step collections (2-4 steps typical) | Already in pmcp, matches SequentialWorkflow pattern |
| `proptest` | 1.7 | Property-based testing for serialization round-trips | Already in dev-dependencies |
| `pretty_assertions` | 1.4 | Better diff output for JSON comparison in tests | Already in dev-dependencies |
| `insta` | 1.43 | Snapshot testing for serialization format stability | Already in dev-dependencies |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `serde_json::Map<String, Value>` for `_meta` | `HashMap<String, Value>` | Map preserves insertion order (matches existing `_meta` patterns in pmcp-tasks); HashMap does not |
| Composition via delegation | Composition via independent handler + shared helpers | Delegation reuses `WorkflowPromptHandler`'s execution loop directly; independent requires duplicating or extracting the loop |

## Architecture Patterns

### Recommended Project Structure
```
crates/pmcp-tasks/src/
├── types/
│   ├── mod.rs              # (add workflow module)
│   └── workflow.rs          # NEW: WorkflowProgress, WorkflowStepStatus
├── router.rs                # MODIFY: add 3 new methods to TaskRouterImpl

src/server/
├── tasks.rs                 # MODIFY: add 3 new default methods to TaskRouter trait
├── workflow/
│   ├── prompt_handler.rs    # DO NOT MODIFY (zero diff requirement)
│   ├── task_prompt_handler.rs  # NEW: TaskWorkflowPromptHandler
│   └── mod.rs               # MODIFY: re-export TaskWorkflowPromptHandler

src/types/
├── protocol.rs              # MODIFY: add _meta to GetPromptResult
```

### Pattern 1: Trait Extension with Default Error Implementations
**What:** Add new methods to the existing `TaskRouter` trait with default implementations that return errors. Existing `TaskRouterImpl` code compiles without modification.
**When to use:** When extending a trait that already has implementations that must not break.
**Example:**
```rust
// Source: existing pattern in pmcp/src/server/tasks.rs
#[async_trait]
pub trait TaskRouter: Send + Sync {
    // ... existing methods unchanged ...

    /// Create a task for a workflow prompt execution.
    ///
    /// Default implementation returns an error -- callers must check
    /// task support before invoking.
    async fn create_workflow_task(
        &self,
        _workflow_name: &str,
        _owner_id: &str,
        _progress: Value,
    ) -> Result<Value> {
        Err(Error::internal("Task router does not support workflow tasks"))
    }

    /// Set task variables for a workflow task.
    async fn set_task_variables(
        &self,
        _task_id: &str,
        _owner_id: &str,
        _variables: Value,
    ) -> Result<()> {
        Err(Error::internal("Task router does not support workflow tasks"))
    }

    /// Complete a workflow task with a final result.
    async fn complete_workflow_task(
        &self,
        _task_id: &str,
        _owner_id: &str,
        _result: Value,
    ) -> Result<Value> {
        Err(Error::internal("Task router does not support workflow tasks"))
    }
}
```

### Pattern 2: Composition via Delegation (TaskWorkflowPromptHandler)
**What:** `TaskWorkflowPromptHandler` wraps a `WorkflowPromptHandler` and delegates to it, adding task lifecycle around the call.
**When to use:** When you must not modify the original handler but need to add behavior around it.
**Why delegation over independent:** `WorkflowPromptHandler` has a complex execution loop (parameter resolution, schema validation, tool execution, binding storage, resource fetching) spanning ~200 lines. Extracting shared helpers would require modifying `prompt_handler.rs` (violating zero-diff). Delegation wraps the existing handler as a black box and post-processes the result.
**Example:**
```rust
// Source: informed by existing WorkflowPromptHandler structure
pub struct TaskWorkflowPromptHandler {
    /// The inner workflow handler that does actual execution
    inner: WorkflowPromptHandler,
    /// Task router for creating/managing workflow tasks
    task_router: Arc<dyn TaskRouter>,
    /// The workflow definition (needed for step metadata)
    workflow: SequentialWorkflow,
}

#[async_trait]
impl PromptHandler for TaskWorkflowPromptHandler {
    async fn get_prompt(
        &self,
        args: HashMap<String, String>,
        extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // 1. Resolve owner from extra
        // 2. Build WorkflowProgress from workflow steps
        // 3. Create task via task_router.create_workflow_task()
        // 4. Delegate to inner.get_prompt(args, extra) -- existing execution
        // 5. Analyze messages to determine which steps completed
        // 6. Update task variables with step results (_workflow.result.*)
        // 7. Update task progress (_workflow.progress)
        // 8. Add _meta to GetPromptResult with task_id, status, step plan
        // 9. Return enriched GetPromptResult
    }

    fn metadata(&self) -> Option<PromptInfo> {
        self.inner.metadata() // Delegate unchanged
    }
}
```

### Pattern 3: Structured Progress in Task Variables
**What:** `WorkflowProgress` serializes to a specific key in task variables, using `_workflow.` prefix.
**When to use:** For all task-backed workflows to track execution state.
**Example:**
```rust
// In pmcp-tasks/src/types/workflow.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProgress {
    /// The workflow's overall goal description
    pub goal: String,
    /// Ordered list of steps in the workflow
    pub steps: Vec<WorkflowStepProgress>,
    /// Schema version for forward compatibility
    pub schema_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStepProgress {
    /// Step name (from WorkflowStep)
    pub name: String,
    /// Tool name (None for resource-only steps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Current status of this step
    pub status: StepStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Step has not been attempted yet
    Pending,
    /// Step completed successfully
    Completed,
    /// Step failed (error recorded in variables)
    Failed,
    /// Step was skipped (server couldn't execute, client should continue)
    Skipped,
}
```

### Pattern 4: Adding `_meta` to GetPromptResult
**What:** Add optional `_meta` field to `GetPromptResult` for experimental task data.
**When to use:** Needed for task-aware prompt replies.
**Example:**
```rust
// In pmcp/src/types/protocol.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    /// Prompt description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Prompt messages
    pub messages: Vec<PromptMessage>,
    /// Optional metadata (PMCP extension for task-aware workflows)
    #[serde(rename = "_meta")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Map<String, serde_json::Value>>,
}
```

### Anti-Patterns to Avoid
- **Modifying `prompt_handler.rs`:** The zero-diff constraint is absolute. Do not add task awareness to `WorkflowPromptHandler` -- compose around it.
- **Making StepExecution an enum:** DROPPED per CONTEXT.md. Steps are best-effort at runtime, not pre-classified.
- **Putting workflow types in `pmcp` core:** `WorkflowProgress` and `WorkflowStepProgress` belong in `pmcp-tasks` to keep the core crate clean. Only `_meta` on `GetPromptResult` goes in core.
- **Using `HashMap<String, Value>` for `_meta`:** Use `serde_json::Map<String, Value>` which preserves insertion order and matches the existing `_meta` pattern in `pmcp-tasks` Task type.
- **Breaking backward compatibility with default implementations:** New `TaskRouter` methods MUST have defaults returning errors, so existing `TaskRouterImpl` compiles unchanged.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON serialization with `camelCase` | Manual JSON construction | `#[serde(rename_all = "camelCase")]` with derive | Consistent with every other type in both crates |
| Task variable key naming | Ad-hoc string constants | `const WORKFLOW_PROGRESS_KEY: &str = "_workflow.progress"` | Prevents typos, single source of truth |
| Step status tracking | Custom bitflags or status arrays | `WorkflowStepProgress` struct with `StepStatus` enum | Matches the typed pattern used everywhere |
| `_meta` field pattern | Custom serialization logic | `#[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]` | Exact pattern already used in `pmcp-tasks::Task` |

**Key insight:** The existing codebase has established patterns for every structural concern in this phase. The task is to apply those patterns to new types, not invent new patterns.

## Common Pitfalls

### Pitfall 1: Breaking Existing WorkflowPromptHandler Tests
**What goes wrong:** Adding `_meta` field to `GetPromptResult` could break existing tests that construct `GetPromptResult` without the field.
**Why it happens:** Struct literal construction in tests requires all fields.
**How to avoid:** The new `_meta` field defaults to `None`. Use `#[serde(skip_serializing_if = "Option::is_none")]` and `Default`. Every existing construction of `GetPromptResult { description: ..., messages: ... }` must be updated to include `_meta: None`. Alternatively, implement `Default` on `GetPromptResult` or use a builder. However, looking at the code, `GetPromptResult` is constructed directly in `prompt_handler.rs` at lines 832-835, 864-866, and 973-976. These MUST be updated to include `_meta: None` -- but this is a source change to `prompt_handler.rs`, not a behavioral change. The zero-diff constraint applies to the v1.0 release artifact. Adding `_meta: None` to struct literals is a non-behavioral field initialization, which should be allowed. **Clarification needed:** Verify whether "zero diff" means the file content or the behavior. If file content, then a `Default` implementation or a `new()` constructor on `GetPromptResult` would be needed instead.
**Warning signs:** Compilation errors in prompt_handler.rs after adding the field.

### Pitfall 2: Circular Crate Dependencies
**What goes wrong:** Putting `WorkflowProgress` in `pmcp` makes `pmcp` depend on task-specific types; putting task-router methods in `pmcp-tasks` makes it depend on `pmcp`.
**Why it happens:** The type boundary between the two crates uses `serde_json::Value` specifically to avoid this.
**How to avoid:** Keep `WorkflowProgress` and `WorkflowStepProgress` in `pmcp-tasks`. The `TaskRouter` trait in `pmcp` uses `Value` for all params/returns. `TaskRouterImpl` in `pmcp-tasks` deserializes `Value` into typed structs internally. This is the existing pattern -- follow it exactly.
**Warning signs:** `pmcp-tasks` appearing in `pmcp`'s `[dependencies]` (it should only be in `[dev-dependencies]`).

### Pitfall 3: Schema Version Migration
**What goes wrong:** `WorkflowProgress` evolves across versions but old serialized data can't be deserialized.
**Why it happens:** Task variables persist in the store and may outlive a server deployment.
**How to avoid:** Include `schema_version: u32` in `WorkflowProgress` (starting at 1). Use `#[serde(default)]` on new fields added in future versions. Document the schema version contract: "readers MUST tolerate unknown fields; writers MUST set schema_version".
**Warning signs:** Deserialization failures when reading task variables from a previous server version.

### Pitfall 4: Step Completion Detection in Delegation Model
**What goes wrong:** `TaskWorkflowPromptHandler` delegates to `WorkflowPromptHandler` and gets back a `GetPromptResult` with messages, but can't determine which steps completed vs. which were skipped.
**Why it happens:** The message list is conversational text, not structured step tracking.
**How to avoid:** The `TaskWorkflowPromptHandler` knows the workflow's step list. It can count tool call/result message pairs to determine how many steps completed. Each step produces: (1) an assistant message announcing the tool call, and (2) a user message with "Tool result:" prefix. The number of such pairs equals the number of completed steps. Steps after the last completed pair are remaining. This is a heuristic but reliable because the message format is controlled by the delegation target.
**Warning signs:** Mismatch between detected completed steps and actual execution when the message format changes.

### Pitfall 5: result_summary_fn Signature Design
**What goes wrong:** Designing `result_summary_fn` too early locks in a signature that doesn't work for real use cases.
**Why it happens:** Phase 4 defines contracts; the function won't actually be called until Phase 5.
**How to avoid:** Define `result_summary_fn` as `Option<fn(&Value) -> Value>` -- a simple function pointer that takes the raw tool result and returns a summarized version. This is enough for Phase 4's contract definition. Phase 5 can extend it if needed. Keep it as a field on `WorkflowStep` (or on a task-aware step wrapper) but don't implement the calling logic yet.
**Warning signs:** Over-engineering the summary function with closures, async, or trait objects when a function pointer suffices.

## Code Examples

### WorkflowProgress Serialization Round-Trip
```rust
// Expected JSON shape for _workflow.progress task variable
let progress = WorkflowProgress {
    goal: "Deploy service to us-east-1".to_string(),
    steps: vec![
        WorkflowStepProgress {
            name: "validate".to_string(),
            tool: Some("validate_config".to_string()),
            status: StepStatus::Completed,
        },
        WorkflowStepProgress {
            name: "deploy".to_string(),
            tool: Some("deploy_service".to_string()),
            status: StepStatus::Pending,
        },
    ],
    schema_version: 1,
};

let json = serde_json::to_value(&progress).unwrap();
// {
//   "goal": "Deploy service to us-east-1",
//   "steps": [
//     { "name": "validate", "tool": "validate_config", "status": "completed" },
//     { "name": "deploy", "tool": "deploy_service", "status": "pending" }
//   ],
//   "schemaVersion": 1
// }

let round_trip: WorkflowProgress = serde_json::from_value(json).unwrap();
assert_eq!(round_trip.goal, progress.goal);
assert_eq!(round_trip.steps.len(), 2);
```

### TaskRouter Extension Pattern
```rust
// In pmcp/src/server/tasks.rs -- adding default methods
#[async_trait]
pub trait TaskRouter: Send + Sync {
    // ... existing 8 methods unchanged ...

    /// Create a workflow-backed task. Returns CreateTaskResult as Value.
    async fn create_workflow_task(
        &self,
        _workflow_name: &str,
        _owner_id: &str,
        _progress: Value,
    ) -> Result<Value> {
        Err(Error::internal("workflow tasks not supported by this router"))
    }

    /// Update task variables with workflow step results.
    async fn set_task_variables(
        &self,
        _task_id: &str,
        _owner_id: &str,
        _variables: Value,
    ) -> Result<()> {
        Err(Error::internal("workflow tasks not supported by this router"))
    }

    /// Complete a workflow task.
    async fn complete_workflow_task(
        &self,
        _task_id: &str,
        _owner_id: &str,
        _result: Value,
    ) -> Result<Value> {
        Err(Error::internal("workflow tasks not supported by this router"))
    }
}
```

### GetPromptResult _meta Addition
```rust
// Existing GetPromptResult at src/types/protocol.rs:654
// Add _meta field following exact pattern from pmcp-tasks Task type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPromptResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub messages: Vec<PromptMessage>,
    /// Optional metadata for task-aware workflows
    #[serde(rename = "_meta")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<serde_json::Map<String, serde_json::Value>>,
}
```

### Variable Key Convention
```rust
// In pmcp-tasks/src/constants.rs (or new workflow module)
/// Task variable key for structured workflow progress
pub const WORKFLOW_PROGRESS_KEY: &str = "_workflow.progress";

/// Prefix for per-step result variables
/// Full key: _workflow.result.<step_name>
pub const WORKFLOW_RESULT_PREFIX: &str = "_workflow.result.";

/// Build the variable key for a step result
pub fn workflow_result_key(step_name: &str) -> String {
    format!("{}{}", WORKFLOW_RESULT_PREFIX, step_name)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| StepExecution enum (ServerSide/ClientDeferred) | Runtime best-effort (server tries, breaks gracefully) | Phase 4 CONTEXT.md discussion | Eliminated an entire type and validation layer; matches existing WorkflowPromptHandler break pattern |
| Modifying WorkflowPromptHandler | Composition via TaskWorkflowPromptHandler | Phase 4 CONTEXT.md discussion | Zero-diff constraint on existing handler |
| Task types in pmcp core | Task types in separate pmcp-tasks crate | v1.0 design | Keeps experimental feature isolated |

**Deprecated/outdated:**
- FNDX-02 (StepExecution enum): Dropped per Phase 4 context discussion. The requirement still exists in REQUIREMENTS.md but is overridden by CONTEXT.md decisions.

## Open Questions

1. **Zero-diff interpretation for GetPromptResult field addition**
   - What we know: Adding `_meta: Option<...>` to `GetPromptResult` requires updating struct literal constructions in `prompt_handler.rs` (3 locations: lines 832, 864, 973) to include `_meta: None`
   - What's unclear: Whether "zero diff from v1.0" means file-content-identical or behavior-identical
   - Recommendation: **Treat as behavior-identical.** Adding `_meta: None` to struct literals is a mechanical, non-behavioral change. It does not alter execution flow, message content, or any observable behavior. The alternative (using `..Default::default()` or a constructor) would also require a change to how `GetPromptResult` is constructed. The simplest, clearest approach is to add `_meta: None` to the 3 literal sites.

2. **Opt-in mechanism placement (Claude's Discretion)**
   - What we know: Builder already has `with_task_store()` for task router. Workflow needs per-workflow opt-in. `SequentialWorkflow` uses builder pattern (`.argument()`, `.step()`, `.instruction()`).
   - Recommendation: **Add `.with_task_support(true)` on `SequentialWorkflow` builder.** This is a boolean flag on the workflow definition. The builder's `prompt_workflow()` checks if task support is requested AND a task router is configured; if both true, it wraps in `TaskWorkflowPromptHandler` instead of `WorkflowPromptHandler`. If task support is requested but no router is configured, return an error at build time. This keeps opt-in at the workflow level (not server level) and matches the existing builder pattern.

3. **Composition implementation (Claude's Discretion)**
   - What we know: Delegation wraps `WorkflowPromptHandler` and calls its `get_prompt()`. Independent implementation would duplicate the execution loop.
   - Recommendation: **Use delegation.** `TaskWorkflowPromptHandler` holds a `WorkflowPromptHandler` and a reference to the `TaskRouter`. On `get_prompt()`, it (1) creates task, (2) calls inner handler, (3) post-processes messages to update task variables, (4) adds `_meta` to result. This is ~50 lines of new code vs. ~200 lines of duplicated loop logic. The downside is that step completion detection is heuristic (counting message pairs), but this is reliable given the controlled message format.

4. **result_summary_fn type signature**
   - What we know: CONTEXT.md says "allow WorkflowStep to specify a `result_summary_fn` for large outputs (configurable per step)"
   - Recommendation: Define as `Option<fn(&serde_json::Value) -> serde_json::Value>` on a new field. Function pointers are `Clone + Copy + Send + Sync` with zero overhead. In Phase 4, define the field; in Phase 5, implement the calling logic. If closures are needed later, upgrade to `Option<Arc<dyn Fn(&Value) -> Value + Send + Sync>>`.

## Sources

### Primary (HIGH confidence)
- Codebase: `src/server/workflow/prompt_handler.rs` -- WorkflowPromptHandler execution loop, composition target
- Codebase: `src/server/tasks.rs` -- TaskRouter trait definition, extension point for new methods
- Codebase: `src/types/protocol.rs` -- GetPromptResult struct definition (line 654)
- Codebase: `crates/pmcp-tasks/src/router.rs` -- TaskRouterImpl implementation pattern
- Codebase: `crates/pmcp-tasks/src/types/task.rs` -- Task struct with `_meta` field pattern
- Codebase: `crates/pmcp-tasks/src/domain/record.rs` -- TaskRecord and variable injection pattern
- Codebase: `src/server/builder.rs` -- `with_task_store()` and `prompt_workflow()` builder methods

### Secondary (MEDIUM confidence)
- `.planning/phases/04-foundation-types-and-contracts/04-CONTEXT.md` -- Locked decisions from user discussion
- `docs/design/tasks-feature-design.md` -- Original design document (v1.0 scope, some concepts evolved)
- `.planning/REQUIREMENTS.md` -- Requirement definitions and phase mapping

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies needed
- Architecture: HIGH -- direct examination of existing code shows clear extension points
- Pitfalls: HIGH -- identified from actual code structure analysis, not theoretical
- Composition model: MEDIUM -- delegation heuristic for step detection needs validation in Phase 5

**Research date:** 2026-02-22
**Valid until:** 2026-03-22 (stable domain, patterns well-established in codebase)
