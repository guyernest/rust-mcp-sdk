# Phase 5: Partial Execution Engine - Research

**Researched:** 2026-02-22
**Domain:** Rust async workflow execution, durable state persistence, structured pause/resume patterns
**Confidence:** HIGH

## Summary

Phase 5 transforms `TaskWorkflowPromptHandler` from a passive observer (Phase 4: creates task, delegates to inner handler, infers step statuses from message trace after the fact) into an active execution engine that controls the step loop, persists results to task variables, and pauses gracefully when a step cannot execute. The key architectural shift is that `TaskWorkflowPromptHandler` must now intercept the execution loop rather than wrapping it -- it cannot simply delegate to `WorkflowPromptHandler.handle()` and parse the output, because it needs to capture per-step results, detect failure reasons, and decide whether to continue or pause.

The existing `WorkflowPromptHandler` execution loop (lines 785-963 in `prompt_handler.rs`) provides the reference implementation for sequential step execution. It handles parameter resolution, schema validation, tool execution, binding storage, and break-on-failure. Phase 5 must replicate this loop inside `TaskWorkflowPromptHandler` but with task-aware instrumentation: accumulating step results in memory, building structured `PauseReason` on failure, and batch-writing all state to the task store at the end.

The CONTEXT.md decisions simplify the design significantly: (1) batch-at-end write ordering means no per-step store calls during execution, just memory accumulation; (2) no pre-classification of steps means the existing break-on-failure pattern is the pause mechanism; (3) current-state-only means no history tracking; (4) EXEC-04 becomes a runtime check (unresolved_dependency PauseReason variant) rather than build-time validation.

**Primary recommendation:** Refactor `TaskWorkflowPromptHandler.handle()` to run its own step execution loop (reusing `WorkflowPromptHandler`'s helper methods where possible) that accumulates `(step_name, result_or_error)` pairs in memory, constructs a `PauseReason` enum on failure/unresolvable params, batch-writes all accumulated state plus updated `WorkflowProgress` to the task store at the end, and auto-completes the task when all steps succeed.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Batch write after execution completes or pauses** -- accumulate step results in memory during execution, write all to task store in one batch at the end
- **Single batch includes both WorkflowProgress (step statuses) and step results** -- consistency between progress and results is guaranteed
- **If batch write fails: return prompt result anyway** -- graceful degradation, consistent with Phase 4's pattern. Log the error, return _meta with in-memory state. Client gets results even if task store persistence failed.
- **Store raw tool results as-is** -- no summarization or truncation. Existing `TaskSecurityConfig.max_variable_size_bytes` handles overflow.
- **Sequential execution, stop at first unresolvable step** -- matches existing WorkflowPromptHandler break-on-failure pattern. No skip-and-try-remaining.
- **Task stays in Working status** when paused -- step statuses in variables tell the story (Completed/Pending/Failed)
- **Structured pause reason** stored in `_workflow.pause_reason` variable -- a `PauseReason` enum with specific variants: `unresolvable_params`, `schema_mismatch`, `tool_error`, `unresolved_dependency`
- **Pause reason includes actionable guidance**: blocked step name + missing parameter + tool name the client should call (with `_task_id`).
- **Auto-complete when all steps succeed** -- if every step executes without pause, mark task Completed automatically. No client action needed for fully-resolvable workflows.
- **Tool errors are just another pause reason** -- same model as unresolvable params. Step marked Failed, remaining steps stay Pending, task stays Working.
- **Error stored in same `_workflow.result.<step_name>` key** -- contains either success result OR error. WorkflowStepProgress.status (Completed/Failed) distinguishes them.
- **Current state only, no history** -- when a client retries a failed step and it succeeds, the step status changes from Failed to Completed and the error is replaced by the success result. No audit trail.
- **Per-tool retry hint** -- specific tools can declare themselves as retryable (e.g., transient network errors). Most tools don't need this.
- **EXEC-04 reinterpreted as runtime check** -- since steps have no pre-classification, dependency issues surface at runtime when a step can't resolve params because the producing step failed/skipped
- **`unresolved_dependency` is a specific PauseReason variant** -- distinct from generic `unresolvable_params`. Includes: blocked_step, missing_output, producing_step, suggested_tool

### Claude's Discretion
- WorkflowProgress and step results batch write implementation details (single `set_task_variables` call vs multiple)
- PauseReason struct field names and exact JSON serialization format
- How auto-complete interacts with the _meta response (completed task vs working task _meta shape)
- Whether per-tool retry hint is a field on WorkflowStep or on the tool definition

### Deferred Ideas (OUT OF SCOPE)
- Step-level retry policies with exponential backoff -- over-engineering for v1.1
- Parallel step execution within a workflow -- sequential-only for v1.1
- Resume from task state (re-invoke prompt to continue from last step) -- v2 requirement (ADVW-02)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| EXEC-01 | Server executes server-mode steps sequentially, storing each result in task variables (durable-first write order) | Batch-at-end write ordering (per CONTEXT.md refinement): step results accumulated in memory during execution loop, then batch-written to task store via single `set_task_variables` call. The existing `WorkflowPromptHandler` execution loop (lines 785-963) provides the step execution mechanics; `TaskWorkflowPromptHandler` replicates the loop with result accumulation. |
| EXEC-02 | Execution pauses at client-deferred steps without failing the task (task remains Working) | No pre-classification of steps; pause is runtime-emergent. When parameter resolution fails (`resolve_tool_parameters` returns Err) or schema validation fails (`params_satisfy_tool_schema` returns false), execution stops and a structured `PauseReason` is recorded. Task stays Working; step statuses distinguish Completed from Pending/Failed. |
| EXEC-03 | Step failure during partial execution keeps task in Working state and records error details in task variables | Tool execution errors (from `execute_tool_step`) become `PauseReason::ToolError`. The error is stored in `_workflow.result.<step_name>` alongside the step's Failed status in WorkflowProgress. Per-tool retry hint indicates whether the client should retry the same call. |
| EXEC-04 | Extended validation checks that client-deferred steps don't depend on outputs of other client-deferred steps | Reinterpreted as runtime check: when step N can't resolve a parameter because step M (which produces the needed binding) has status Failed or Skipped, the `PauseReason::UnresolvedDependency` variant is emitted with `blocked_step`, `missing_output`, `producing_step`, and `suggested_tool` fields. This is distinct from generic `UnresolvableParams`. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `pmcp` | 1.10.x | Main SDK crate -- `TaskWorkflowPromptHandler`, `WorkflowPromptHandler`, `TaskRouter` | This is the project; Phase 5 modifies `task_prompt_handler.rs` |
| `pmcp-tasks` | 0.1.x | `WorkflowProgress`, `WorkflowStepProgress`, `StepStatus`, `workflow_result_key` | Types and constants already defined in Phase 4 |
| `serde` | 1.0 | Serialization for `PauseReason` enum | Already in both crates |
| `serde_json` | 1.0 | JSON values for step results, PauseReason serialization | Already in both crates |
| `async-trait` | 0.1 | Async trait methods on `PromptHandler`, `TaskRouter` | Already in both crates |
| `tracing` | 0.1 | Structured logging for execution flow and error reporting | Already in pmcp |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `indexmap` | 2.x | Deterministic key ordering in batch variable writes | Already in pmcp |
| `proptest` | 1.x | Property tests for PauseReason serialization round-trips | Already in dev-dependencies |
| `thiserror` | 2.x | Error types for workflow errors | Already in pmcp (WorkflowError) |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Replicating step loop in `TaskWorkflowPromptHandler` | Extracting shared step loop into a function both handlers call | Extraction would require modifying `WorkflowPromptHandler` (violates composition constraint) or creating a third module. Replication with task-aware instrumentation is simpler and keeps the composition boundary clean. The step loop is ~180 lines of straightforward sequential code. |
| Single `set_task_variables` batch call | Multiple `set_task_variables` calls (one for progress, one for results) | Single call guarantees atomicity between progress and results. The `set_task_variables` method accepts a `HashMap<String, Value>` so multiple keys can be set in one call. |
| `PauseReason` as enum with serde | `PauseReason` as raw `serde_json::Value` | Typed enum provides compile-time variant exhaustiveness, better documentation, and `camelCase` serialization consistency. Raw Value would be fragile. |

## Architecture Patterns

### Recommended Project Structure
```
src/server/workflow/
    task_prompt_handler.rs   # MODIFIED: Full execution engine (replaces delegation)
    prompt_handler.rs        # UNCHANGED: Inner handler methods reused

crates/pmcp-tasks/src/types/
    workflow.rs              # MODIFIED: Add PauseReason, WORKFLOW_PAUSE_REASON_KEY
```

### Pattern 1: Active Execution with Result Accumulation

**What:** `TaskWorkflowPromptHandler.handle()` runs its own step execution loop, accumulating `(step_name, Value)` pairs and step statuses in memory. At the end, it batch-writes everything to the task store.

**When to use:** Always -- this is the core Phase 5 pattern.

**Why needed:** Phase 4's delegation pattern (`self.inner.handle(args, extra).await`) cannot provide per-step results or structured pause reasons. The inner handler returns a flat message trace; by the time we parse it, the execution context (bindings, intermediate values) is gone.

**Implementation approach:**

The key challenge is that `WorkflowPromptHandler`'s execution loop uses private helpers (`resolve_tool_parameters`, `execute_tool_step`, `params_satisfy_tool_schema`, `create_tool_call_announcement`, etc.). `TaskWorkflowPromptHandler` cannot call these directly because they are methods on `WorkflowPromptHandler` (not `pub(crate)` in all cases).

Two approaches:
1. **Make helpers `pub(crate)`** on `WorkflowPromptHandler` so `TaskWorkflowPromptHandler` can call them on `self.inner`. This is the cleanest approach -- minimal code duplication, reuses validated logic.
2. **Duplicate the loop** in `TaskWorkflowPromptHandler`. More code but zero changes to `WorkflowPromptHandler`.

**Recommendation:** Approach 1 (make helpers `pub(crate)`). This is NOT modifying `WorkflowPromptHandler`'s behavior -- it's only changing visibility of existing methods from private to `pub(crate)`. The Phase 4 constraint was "zero behavioral modification"; visibility changes are not behavioral.

```rust
// In task_prompt_handler.rs (Phase 5 version):
async fn handle(&self, args: HashMap<String, String>, extra: RequestHandlerExtra) -> Result<GetPromptResult> {
    let owner_id = self.resolve_owner(&extra);
    let progress = self.build_initial_progress_typed();

    // Create task (graceful degradation)
    let task_id = match self.task_router.create_workflow_task(...).await { ... };

    // === Active execution loop (replaces inner delegation) ===
    let mut messages = Vec::new();
    let mut execution_context = ExecutionContext::new();
    let mut step_results: Vec<(String, Value)> = Vec::new();
    let mut step_statuses: Vec<StepStatus> = vec![StepStatus::Pending; step_count];
    let mut pause_reason: Option<PauseReason> = None;

    messages.push(self.inner.create_user_intent(&args));
    messages.push(self.inner.create_assistant_plan()?);

    for (idx, step) in self.workflow.steps().iter().enumerate() {
        // Try resolve params
        match self.inner.resolve_tool_parameters(step, &args, &execution_context) {
            Ok(params) => {
                // Check schema satisfaction
                if !self.inner.params_satisfy_tool_schema(step, &params)? {
                    pause_reason = Some(PauseReason::SchemaMismatch { ... });
                    break;
                }
                // Execute tool
                match self.inner.execute_tool_step(step, &args, &execution_context, &extra).await {
                    Ok(result) => {
                        step_results.push((step.name().to_string(), result.clone()));
                        step_statuses[idx] = StepStatus::Completed;
                        // Store binding
                        if let Some(binding) = step.binding() {
                            execution_context.store_binding(binding.clone(), result);
                        }
                    }
                    Err(e) => {
                        step_results.push((step.name().to_string(), error_to_value(&e)));
                        step_statuses[idx] = StepStatus::Failed;
                        pause_reason = Some(PauseReason::ToolError { ... });
                        break;
                    }
                }
            }
            Err(_) => {
                // Determine if this is an unresolved dependency or generic unresolvable
                pause_reason = Some(classify_resolution_failure(step, &step_statuses, ...));
                break;
            }
        }
    }

    // === Batch write ===
    let mut variables = HashMap::new();
    variables.insert(WORKFLOW_PROGRESS_KEY.to_string(), updated_progress_value);
    for (step_name, result) in &step_results {
        variables.insert(workflow_result_key(step_name), result.clone());
    }
    if let Some(ref reason) = pause_reason {
        variables.insert(WORKFLOW_PAUSE_REASON_KEY.to_string(), serde_json::to_value(reason)?);
    }

    // Write to store (graceful degradation on failure)
    if let Err(e) = self.task_router.set_task_variables(&task_id, &owner_id, variables_value).await {
        tracing::warn!("batch write failed: {}", e);
    }

    // Auto-complete if all steps succeeded
    if pause_reason.is_none() {
        let _ = self.task_router.complete_workflow_task(&task_id, &owner_id, final_result).await;
    }

    // Build _meta and return
    ...
}
```

### Pattern 2: PauseReason as Typed Enum with Serde

**What:** A `PauseReason` enum in `pmcp-tasks/src/types/workflow.rs` with four variants matching CONTEXT.md decisions. Serializes to `camelCase` JSON with `#[serde(tag = "type", rename_all = "camelCase")]`.

**When to use:** Whenever execution cannot complete all steps.

```rust
/// Structured reason why workflow execution paused.
///
/// Stored in task variables under `_workflow.pause_reason`.
/// Includes actionable guidance for the client to continue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PauseReason {
    /// Step cannot resolve its parameters from available context.
    UnresolvableParams {
        /// Name of the step that could not execute.
        blocked_step: String,
        /// Parameter that could not be resolved.
        missing_param: String,
        /// Tool the client should call to provide the missing value.
        suggested_tool: String,
    },
    /// Resolved parameters don't satisfy the tool's input schema.
    SchemaMismatch {
        /// Name of the step that could not execute.
        blocked_step: String,
        /// Required fields that are missing from resolved params.
        missing_fields: Vec<String>,
        /// Tool the client should call.
        suggested_tool: String,
    },
    /// Tool execution returned an error.
    ToolError {
        /// Name of the step that failed.
        failed_step: String,
        /// Error message from the tool.
        error: String,
        /// Whether this tool declares itself as retryable.
        retryable: bool,
        /// Tool the client should retry or use to fix the issue.
        suggested_tool: String,
    },
    /// Step depends on output from a failed/skipped step.
    UnresolvedDependency {
        /// Name of the step that is blocked.
        blocked_step: String,
        /// The binding/output that is missing.
        missing_output: String,
        /// The step that should have produced the output.
        producing_step: String,
        /// Tool the client should call to provide the missing value.
        suggested_tool: String,
    },
}
```

### Pattern 3: Batch Variable Write

**What:** Accumulate all variable updates in a `HashMap<String, Value>`, then call `set_task_variables` once with the full map.

**When to use:** At the end of every execution (whether paused or completed).

```rust
let mut batch: HashMap<String, Value> = HashMap::new();

// 1. Updated WorkflowProgress with step statuses
batch.insert(WORKFLOW_PROGRESS_KEY.to_string(), progress_json);

// 2. Per-step results (success or error)
for (step_name, result) in &accumulated_results {
    batch.insert(workflow_result_key(step_name), result.clone());
}

// 3. Pause reason (if execution paused)
if let Some(reason) = &pause_reason {
    batch.insert(WORKFLOW_PAUSE_REASON_KEY.to_string(),
        serde_json::to_value(reason).unwrap());
}

// Single batch write
let variables_value = serde_json::to_value(batch)?;
self.task_router.set_task_variables(&task_id, &owner_id, variables_value).await?;
```

### Pattern 4: Unresolved Dependency Detection (EXEC-04)

**What:** When parameter resolution fails, inspect the DataSource to determine if the failure is due to a dependency on a failed/skipped step (vs a genuinely missing prompt argument).

**When to use:** In the error path of parameter resolution, to produce `UnresolvedDependency` instead of `UnresolvableParams`.

```rust
fn classify_resolution_failure(
    step: &WorkflowStep,
    step_statuses: &[StepStatus],
    step_names: &[String],
    workflow: &SequentialWorkflow,
) -> PauseReason {
    // Check each argument's DataSource
    for (arg_name, source) in step.arguments() {
        if let DataSource::StepOutput { step: binding_name, .. } = source {
            // Find the producing step's index and check its status
            if let Some(producer_idx) = step_names.iter().position(|n| n == binding_name.as_str()) {
                match step_statuses[producer_idx] {
                    StepStatus::Failed | StepStatus::Skipped => {
                        return PauseReason::UnresolvedDependency {
                            blocked_step: step.name().to_string(),
                            missing_output: binding_name.to_string(),
                            producing_step: step_names[producer_idx].clone(),
                            suggested_tool: find_tool_name(step, workflow),
                        };
                    }
                    _ => {}
                }
            }
        }
    }

    // Default: generic unresolvable params
    PauseReason::UnresolvableParams { ... }
}
```

### Pattern 5: Auto-Complete on Full Success

**What:** When all steps execute successfully (no pause_reason), automatically transition the task to Completed status.

**When to use:** After the batch write, when `pause_reason.is_none()` and all step statuses are `Completed`.

```rust
if pause_reason.is_none() {
    // All steps succeeded -- auto-complete
    let final_result = build_completion_result(&step_results);
    match self.task_router.complete_workflow_task(&task_id, &owner_id, final_result).await {
        Ok(_) => {
            // Update _meta to reflect completed status
            task_status = "completed";
        }
        Err(e) => {
            tracing::warn!("auto-complete failed: {}", e);
            // Task stays Working -- client can poll and see all steps completed
        }
    }
}
```

### Anti-Patterns to Avoid

- **Modifying WorkflowPromptHandler's execution logic:** The composition constraint means we can only change method visibility (private to `pub(crate)`), not behavior. All new execution logic goes in `TaskWorkflowPromptHandler`.
- **Per-step store writes:** The CONTEXT.md decision is batch-at-end. Do not call `set_task_variables` inside the step loop -- accumulate in memory and write once.
- **Failing the task on pause:** Paused workflows stay in Working status. Only auto-complete transitions to a terminal state. The task should never transition to Failed during execution -- that's a client decision.
- **Storing step history:** Current state only. When a step is retried and succeeds, overwrite the result. No array of attempts.
- **Build-time dependency validation:** EXEC-04 is a runtime check. Do not add validation to `SequentialWorkflow::validate()` or the builder.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Step status tracking | Custom status tracking | `pmcp-tasks::WorkflowStepProgress` + `StepStatus` enum | Already defined in Phase 4, has serde round-trip tests |
| Variable key generation | String concatenation | `pmcp-tasks::workflow_result_key()` | Consistent key format, already tested |
| Progress structure | Raw `serde_json::json!()` | `pmcp-tasks::WorkflowProgress` struct | Typed, versioned, has proptest coverage |
| Tool parameter resolution | Custom resolution | `WorkflowPromptHandler::resolve_tool_parameters()` | Handles all DataSource variants, error paths |
| Tool execution with middleware | Direct handler calls | `WorkflowPromptHandler::execute_tool_step()` | Routes through middleware executor correctly |
| Schema validation | Custom field checking | `WorkflowPromptHandler::params_satisfy_tool_schema()` | Handles required fields from JSON schema |

**Key insight:** Phase 5 is primarily orchestration glue between existing Phase 4 types and existing `WorkflowPromptHandler` execution helpers. The core logic for parameter resolution, tool execution, and schema validation already exists -- Phase 5 adds the task lifecycle wrapper around it.

## Common Pitfalls

### Pitfall 1: Breaking the Composition Constraint
**What goes wrong:** Modifying `WorkflowPromptHandler` to support task-aware execution, breaking existing non-task workflows.
**Why it happens:** It seems cleaner to add task logic directly to the execution loop rather than replicating it.
**How to avoid:** Only change method visibility on `WorkflowPromptHandler` (private to `pub(crate)`). All new behavior goes in `TaskWorkflowPromptHandler`. Run existing workflow tests to verify zero behavioral change.
**Warning signs:** Any `diff` to `WorkflowPromptHandler` that changes method bodies or control flow.

### Pitfall 2: ExecutionContext Visibility
**What goes wrong:** `ExecutionContext` is a private struct in `prompt_handler.rs`. `TaskWorkflowPromptHandler` needs to create and manage its own `ExecutionContext` for the step loop.
**Why it happens:** `ExecutionContext` was designed as an implementation detail of `WorkflowPromptHandler`.
**How to avoid:** Either make `ExecutionContext` `pub(crate)` or create a parallel type in `task_prompt_handler.rs`. The struct is simple (just a `HashMap<BindingName, Value>` wrapper), so duplication is acceptable.
**Warning signs:** Compilation errors about private types when `TaskWorkflowPromptHandler` tries to pass `ExecutionContext` to inner helper methods.

### Pitfall 3: Batch Write Exceeding Variable Size Limit
**What goes wrong:** Raw tool results can be large. The batch write of all step results could exceed `StoreConfig::max_variable_size_bytes` (default 1 MB).
**Why it happens:** CONTEXT.md says "store raw tool results as-is" and "existing `TaskSecurityConfig.max_variable_size_bytes` handles overflow."
**How to avoid:** Let the store's `set_variables` method enforce the limit and return `TaskError::VariableSizeExceeded`. The batch write uses graceful degradation (log error, return result without persistence), so this won't crash. Document the limitation.
**Warning signs:** `VariableSizeExceeded` errors in logs during testing with large tool results.

### Pitfall 4: Resource-Only Steps in the Execution Loop
**What goes wrong:** Resource-only steps (no tool, no binding) need special handling -- they don't produce results to store, don't have tools to suggest in PauseReason.
**Why it happens:** The execution loop focuses on tool steps; resource-only steps are an edge case.
**How to avoid:** Follow the existing `WorkflowPromptHandler` pattern: check `step.is_resource_only()` early and handle separately (fetch resources, add messages, continue to next step). Resource-only steps always succeed or fail at the resource-fetch level.
**Warning signs:** Panic or incorrect PauseReason when a resource-only step fails.

### Pitfall 5: PauseReason `suggested_tool` for Non-Tool Steps
**What goes wrong:** PauseReason variants require a `suggested_tool` field. If the blocked step doesn't have a tool (resource-only), or the suggestion should be the *next* step's tool, the field could be incorrect.
**Why it happens:** Not all steps have tools, and the "best tool to call" depends on context.
**How to avoid:** For `UnresolvableParams` and `SchemaMismatch`, use the blocked step's own tool name. For `UnresolvedDependency`, use the blocked step's tool (the client needs to provide the missing value by calling that tool with the task_id). For resource-only steps that fail, the `suggested_tool` can reference the next tool step or be empty with a descriptive guidance message.
**Warning signs:** Empty or misleading `suggested_tool` values in test output.

### Pitfall 6: Auto-Complete Race with Batch Write
**What goes wrong:** `complete_workflow_task` is called after `set_task_variables`. If the auto-complete call fails, the task has step results but is not marked Completed. The client might see a Working task with all steps Completed.
**Why it happens:** Two separate store calls are not atomic.
**How to avoid:** This is acceptable behavior per CONTEXT.md (graceful degradation). The client can detect all-steps-completed by inspecting WorkflowProgress. Document that auto-complete is best-effort.
**Warning signs:** Tasks stuck in Working status despite all steps being Completed in variables.

## Code Examples

### Example 1: PauseReason Serialization

```rust
// Source: CONTEXT.md decisions, following pmcp-tasks serde conventions
use serde_json::json;

let reason = PauseReason::ToolError {
    failed_step: "deploy".to_string(),
    error: "connection timeout".to_string(),
    retryable: true,
    suggested_tool: "deploy_service".to_string(),
};

let json = serde_json::to_value(&reason).unwrap();
assert_eq!(json, json!({
    "type": "toolError",
    "failedStep": "deploy",
    "error": "connection timeout",
    "retryable": true,
    "suggestedTool": "deploy_service"
}));
```

### Example 2: Batch Variable Write Shape

```rust
// The batch written to set_task_variables at end of execution
let batch = json!({
    "_workflow.progress": {
        "goal": "deploy: Deploy a service",
        "steps": [
            {"name": "validate", "tool": "validate_config", "status": "completed"},
            {"name": "deploy", "tool": "deploy_service", "status": "failed"},
            {"name": "notify", "tool": "send_notification", "status": "pending"}
        ],
        "schemaVersion": 1
    },
    "_workflow.result.validate": {
        "valid": true,
        "region": "us-east-1"
    },
    "_workflow.result.deploy": {
        "error": "connection timeout",
        "code": "ETIMEOUT"
    },
    "_workflow.pause_reason": {
        "type": "toolError",
        "failedStep": "deploy",
        "error": "connection timeout",
        "retryable": true,
        "suggestedTool": "deploy_service"
    }
});
```

### Example 3: UnresolvedDependency Detection

```rust
// Step 2 depends on step 1's output, but step 1 failed
// step_statuses = [Failed, Pending, Pending]
// step 2 args: { "config": DataSource::StepOutput { step: "validated", field: None } }

let reason = PauseReason::UnresolvedDependency {
    blocked_step: "deploy".to_string(),
    missing_output: "validated".to_string(),
    producing_step: "validate".to_string(),
    suggested_tool: "deploy_service".to_string(),
};
// Client sees: "deploy is blocked because validate failed to produce 'validated'.
//               Call deploy_service with _task_id to provide the value manually."
```

### Example 4: Auto-Complete _meta Shape

```rust
// When all steps succeed, _meta reflects completed status
let meta = json!({
    "task_id": "task-abc-123",
    "task_status": "completed",
    "steps": [
        {"name": "validate", "status": "completed"},
        {"name": "deploy", "status": "completed"},
        {"name": "notify", "status": "completed"}
    ]
});
```

### Example 5: Paused _meta Shape

```rust
// When execution pauses, _meta shows working + pause reason
let meta = json!({
    "task_id": "task-abc-123",
    "task_status": "working",
    "steps": [
        {"name": "validate", "status": "completed"},
        {"name": "deploy", "status": "failed"},
        {"name": "notify", "status": "pending"}
    ],
    "pause_reason": {
        "type": "toolError",
        "failedStep": "deploy",
        "error": "connection timeout",
        "retryable": true,
        "suggestedTool": "deploy_service"
    }
});
```

## State of the Art

| Old Approach (Phase 4) | New Approach (Phase 5) | Impact |
|------------------------|----------------------|--------|
| Delegate to `self.inner.handle()` | Run own step loop calling inner helpers | Per-step result capture, structured pause reasons |
| Infer step statuses from message trace | Track step statuses directly during execution | Accurate status for Failed vs Pending, no heuristic parsing |
| Single `set_task_variables` with progress only | Batch write with progress + results + pause_reason | Full execution state persisted |
| Task always stays Working | Auto-complete when all steps succeed | Zero client action for fully-resolvable workflows |
| No error detail in task variables | Structured PauseReason enum | Actionable guidance for clients |
| `build_initial_progress` returns `serde_json::Value` | Use typed `WorkflowProgress` struct | Type safety, schema_version consistency |

## Open Questions

1. **`ExecutionContext` access pattern**
   - What we know: `ExecutionContext` is private in `prompt_handler.rs`. `TaskWorkflowPromptHandler` needs bindings storage for its own execution loop.
   - What's unclear: Should we make the existing `ExecutionContext` `pub(crate)` or create a parallel type?
   - Recommendation: Make it `pub(crate)` -- it's a simple HashMap wrapper, and making it visible within the crate is a non-behavioral change. If the inner helper methods (`resolve_tool_parameters`, `execute_tool_step`) take `&ExecutionContext`, we need the same type.

2. **Helper method visibility**
   - What we know: `resolve_tool_parameters`, `execute_tool_step`, `params_satisfy_tool_schema`, `create_tool_call_announcement`, `create_user_intent`, `create_assistant_plan`, `fetch_step_resources`, `substitute_arguments` are all private methods on `WorkflowPromptHandler`.
   - What's unclear: Which specific methods need to be `pub(crate)` for the task-aware loop?
   - Recommendation: At minimum: `create_user_intent`, `create_assistant_plan`, `resolve_tool_parameters`, `execute_tool_step`, `params_satisfy_tool_schema`, `create_tool_call_announcement`, `fetch_step_resources`, `substitute_arguments`. Making all execution-related helpers `pub(crate)` is safe since they are stateless (take `&self` for tool registry access only).

3. **Per-tool retry hint placement**
   - What we know: CONTEXT.md says "specific tools can declare themselves as retryable." Claude's discretion area.
   - What's unclear: Is this a field on `WorkflowStep` (set by the workflow author) or on the tool definition (set by the tool author)?
   - Recommendation: Field on `WorkflowStep` (`retryable: bool`, default false). The workflow author knows which steps are transient vs deterministic. Adding it to tool definitions would require modifying `ToolInfo` across the SDK. A `WorkflowStep` field is isolated and opt-in.

4. **Pause reason clearing on resume**
   - What we know: CONTEXT.md says "current state only, no history" -- when a step is retried successfully, its result replaces the error.
   - What's unclear: Should `_workflow.pause_reason` be cleared when a tool call with `_task_id` succeeds on a previously-failed step? That's Phase 6 (CONT-02) territory.
   - Recommendation: Phase 5 only writes the pause_reason. Phase 6 (continuation) handles clearing it. Document this boundary.

## Sources

### Primary (HIGH confidence)
- `src/server/workflow/prompt_handler.rs` -- full execution loop implementation (lines 785-963), helper methods
- `src/server/workflow/task_prompt_handler.rs` -- Phase 4 composition handler, current delegation pattern
- `crates/pmcp-tasks/src/types/workflow.rs` -- `WorkflowProgress`, `WorkflowStepProgress`, `StepStatus`, key constants
- `src/server/tasks.rs` -- `TaskRouter` trait with `create_workflow_task`, `set_task_variables`, `complete_workflow_task`
- `crates/pmcp-tasks/src/router.rs` -- `TaskRouterImpl` concrete implementations of workflow methods
- `crates/pmcp-tasks/src/store/mod.rs` -- `TaskStore` trait, `StoreConfig` (max_variable_size_bytes)

### Secondary (MEDIUM confidence)
- `.planning/phases/05-partial-execution-engine/05-CONTEXT.md` -- user decisions constraining implementation
- `.planning/phases/04-foundation-types-and-contracts/04-VERIFICATION.md` -- Phase 4 verification confirming all artifacts exist
- `.planning/REQUIREMENTS.md` -- EXEC-01 through EXEC-04 definitions

### Tertiary (LOW confidence)
- None -- all findings based on direct codebase inspection

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies needed
- Architecture: HIGH -- pattern directly derived from existing codebase inspection; execution loop mechanics verified in source
- Pitfalls: HIGH -- identified through direct analysis of the code paths that will change
- PauseReason design: MEDIUM -- enum design is discretionary; field names and serde tag strategy are reasonable but could be refined during implementation

**Research date:** 2026-02-22
**Valid until:** 2026-03-22 (stable -- internal codebase, no external API changes expected)
