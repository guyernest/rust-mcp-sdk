# Pitfalls Research

**Domain:** Task-prompt bridge -- partial execution, structured prompt replies, variable schema, and backward compatibility when adding task-aware workflows to existing PMCP SDK
**Researched:** 2026-02-21
**Confidence:** HIGH (codebase analysis of existing WorkflowPromptHandler, TaskContext, TaskStore, and SequentialWorkflow; domain research on state machine serialization, LLM prompt structuring, and Rust trait evolution)

## Critical Pitfalls

### Pitfall 1: Modifying WorkflowPromptHandler Breaks All Existing Non-Task Workflows

**What goes wrong:**
The existing `WorkflowPromptHandler` implements `PromptHandler` with a signature `fn handle(&self, args: HashMap<String, String>, extra: RequestHandlerExtra) -> Result<GetPromptResult>`. It executes ALL steps server-side and returns a complete conversation trace. Adding task awareness directly into this handler (e.g., checking for a `TaskStore`, creating tasks, pausing mid-execution) means every existing workflow -- including ones that have no task association -- now runs through task-aware code paths. If the task-aware code path has even a subtle behavior change (different message ordering, different error propagation, different binding resolution), existing workflows silently break. Since workflows produce conversation traces consumed by LLMs, even a whitespace change in message formatting can alter LLM behavior.

**Why it happens:**
The temptation is to add an `Option<Arc<dyn TaskStore>>` field to `WorkflowPromptHandler` and branch on `Some`/`None` in `handle()`. This is the "one handler to rule them all" anti-pattern. It couples two distinct execution modes (full-execution and partial-execution) into a single code path, making it impossible to test or reason about them independently. The existing `ExecutionContext` (in-memory `HashMap<BindingName, Value>`) is fundamentally different from `TaskContext` (durable store-backed), but the merge makes them share code that makes assumptions about one or the other.

**How to avoid:**
Create a **separate** `TaskWorkflowPromptHandler` that composes with (not inherits from) `WorkflowPromptHandler`. The original handler remains unchanged and untouched. The task-aware handler delegates full-execution steps to the original handler's step execution logic, but owns the task lifecycle and partial execution decisions. The key insight: `SequentialWorkflow` is data (step definitions), not behavior. Both handlers consume the same `SequentialWorkflow`, but execute it differently.

```
SequentialWorkflow (data)
    |
    +--- WorkflowPromptHandler (full execution, returns conversation trace)
    |     - Existing behavior, ZERO changes
    |     - All steps executed server-side
    |     - Returns complete trace
    |
    +--- TaskWorkflowPromptHandler (partial execution, returns structured guidance)
          - Creates task, binds variables
          - Executes resolvable steps
          - Pauses on unresolvable steps
          - Returns completed steps + remaining steps + task ID
```

**Warning signs:**
- `WorkflowPromptHandler` gaining new constructor parameters or fields
- `#[cfg(feature = "tasks")]` guards inside `WorkflowPromptHandler::handle()`
- Existing workflow tests needing modification to pass
- `ExecutionContext` gaining an `Option<TaskContext>` field

**Phase to address:**
Phase 1 (Architecture). Must decide handler composition pattern before writing any code. This is the most important architectural decision in the milestone.

---

### Pitfall 2: ExecutionContext and TaskContext Dual-Write Inconsistency

**What goes wrong:**
During partial execution, the task-aware handler must track step results in TWO places simultaneously: (1) the in-memory `ExecutionContext` (needed for resolving `DataSource::StepOutput` references in subsequent steps), and (2) the durable `TaskContext` / task variables (needed for persistence across client continuation calls). If a step result is stored in `ExecutionContext` but the `TaskContext::set_variable()` call fails (store error, variable size exceeded, task expired), the in-memory state diverges from the durable state. Subsequent steps execute successfully using the in-memory binding, but when the client resumes with a tool call, the task variables are missing the data from the failed write. The workflow proceeds with a "phantom step" whose result exists only in the ephemeral execution and is lost.

**Why it happens:**
`ExecutionContext` is a simple `HashMap<BindingName, Value>` with no error handling on `store_binding`. It always succeeds. `TaskContext::set_variable()` is async and fallible. Developers write the natural sequence: (1) execute step, (2) store in `ExecutionContext` for next step, (3) store in `TaskContext` for durability -- and don't handle the case where (3) fails but (1) and (2) succeeded. The existing `WorkflowPromptHandler` never faced this because there was only one storage path.

**How to avoid:**
Write to durable storage FIRST, then populate the in-memory context. If the durable write fails, the step is considered failed -- do not proceed to the next step with ephemeral-only data. The execution loop should be:

```rust
// Correct order: durable first, then ephemeral
let result = execute_step(&step, &args, &extra).await?;

// 1. Store durably (task variables)
task_ctx.set_variable(&binding_name, result.clone()).await
    .map_err(|e| /* mark step as failed, stop execution */)?;

// 2. Only then store ephemerally (for next-step resolution)
execution_ctx.store_binding(binding_name, result);
```

Additionally, implement a `SyncedExecutionContext` wrapper that writes to both stores atomically (durable first, ephemeral second) and provides rollback semantics (remove from ephemeral if durable fails).

**Warning signs:**
- Step results stored in `ExecutionContext` before `TaskContext`
- No error handling on `task_ctx.set_variable()` calls in the execution loop
- Tests that mock `TaskStore` to never fail -- no failure injection for variable writes
- Client continuation seeing "step X not found" when the server reported it as completed

**Phase to address:**
Phase 2 (Partial Execution Engine). The execution loop must be designed with dual-write semantics from the start.

---

### Pitfall 3: Structured Prompt Reply Too Rigid for Different LLM Clients

**What goes wrong:**
The prompt reply must convey: (a) what steps completed with their results, (b) what steps remain with enough context for the LLM to continue, (c) the task ID for polling. If the reply uses a highly structured JSON format embedded in a conversation message, different LLM clients (Claude, GPT-4, Gemini, open-source models) parse and follow it differently. Claude may follow a numbered step list precisely. GPT-4 may reinterpret the remaining steps and attempt them in a different order. Smaller models may ignore the structure entirely and hallucinate tool calls. The prompt reply becomes a de facto "instruction format" that only works with specific LLM families.

**Why it happens:**
Server developers design the prompt reply for the LLM they test with (usually Claude, given this is an MCP SDK). They assume the LLM will parse structured JSON from a conversation message, follow step ordering, and use the exact tool names and argument structures specified. But MCP is client-agnostic -- the spec makes no assumptions about the LLM powering the client. A reply that says `{"remaining_steps": [{"tool": "deploy", "args": {"config": "$config"}}]}` works if the client parses JSON from messages, but breaks if the client expects natural language instructions.

**How to avoid:**
Use a hybrid approach for the prompt reply: (1) structured data in `_meta` (machine-readable, for smart clients that know how to parse it) AND (2) natural language in the conversation messages (human-readable, for any LLM client). The `_meta` carries the structured step list. The messages carry the same information as conversational text that any LLM can follow.

```
GetPromptResult {
    description: "Deploy service (2 of 4 steps completed)",
    messages: [
        // Completed steps as conversation trace (same as existing WorkflowPromptHandler)
        user("I want to deploy service 'my-api' to us-east-1"),
        assistant("Step 1: Validated config - region: us-east-1 [DONE]"),
        user("Validation result: { valid: true, config: {...} }"),
        assistant("Step 2: Provisioned infrastructure [DONE]"),
        user("Provisioned: { vpc_id: 'vpc-123', subnet: 'sub-456' }"),
        // Remaining steps as natural language guidance
        assistant("Next steps to complete this workflow:
            3. Call 'approve_deployment' to get deployment approval
            4. Call 'deploy_service' with the config and infrastructure details above

            The task ID is 'task-abc-123'. After completing all steps,
            the task will be marked complete."),
    ],
    _meta: {
        "pmcp:task_id": "task-abc-123",
        "pmcp:workflow_progress": {
            "completed": ["validate_config", "provision_infra"],
            "remaining": [
                {"step": "approve_deployment", "tool": "approve_deployment", "args": {}},
                {"step": "deploy_service", "tool": "deploy_service", "args": {"config": "..."}}
            ]
        }
    }
}
```

This way, smart clients can parse `_meta` for machine-readable guidance, while any LLM can follow the natural language instructions in the messages.

**Warning signs:**
- Prompt reply contains ONLY structured JSON, no natural language
- Reply tested with only one LLM client (e.g., only Claude Code)
- Remaining steps lack human-readable descriptions of what to do
- No `_meta` section -- all guidance is in message text (no machine path)

**Phase to address:**
Phase 3 (Structured Prompt Reply). Design the reply format before implementing, and test with at least two different clients (Claude Code + a simple test client that only reads text).

---

### Pitfall 4: Variable Schema Becomes an Implicit API That Can't Evolve

**What goes wrong:**
Task variables store step progress with a schema like `{ "step.validate.result": {...}, "step.deploy.status": "pending", "workflow.current_step": 2 }`. Both server-side step execution and client-side continuation tool calls read and write these variables. The variable key names become an implicit API contract between: (a) the partial execution engine, (b) each tool handler, and (c) the LLM client following the prompt reply. If the schema changes (rename `step.validate.result` to `steps.validate.output`), every component breaks simultaneously with no compile-time safety net. Variables are `HashMap<String, Value>` -- there is zero type safety, no schema validation, and no versioning.

**Why it happens:**
`serde_json::Value` is the type of choice for flexibility ("any JSON value"), but flexibility is the enemy of contracts. Each developer writes tool handlers that assume specific variable key patterns without realizing they are creating a cross-component API. The prompt reply generator reads variables to build the structured guidance. The client's tool calls write variables that the server's continuation logic reads. None of these contracts are enforced anywhere -- they are scattered across code as string literals.

**How to avoid:**
Define a `WorkflowVariableSchema` struct that formally specifies the variable keys, their types, and their semantics. Use this struct for both serialization and deserialization. Variables written by the partial execution engine go through this schema. Variables expected by the prompt reply generator go through this schema. Tool handlers interact with a typed API, not raw string keys.

```rust
/// Standard variable schema for task-backed workflows.
///
/// This schema is the contract between the partial execution engine,
/// tool handlers, and the prompt reply generator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowProgress {
    /// The workflow name that created this task
    pub workflow_name: String,
    /// Goal description for the LLM
    pub goal: String,
    /// Steps that have completed, with their results
    pub completed_steps: Vec<CompletedStep>,
    /// Steps remaining, with their expected tool calls
    pub remaining_steps: Vec<RemainingStep>,
    /// Index of the current step (0-based)
    pub current_step_index: usize,
}
```

Include a `schema_version: u32` field so the variable format can evolve without breaking existing tasks. The schema struct is defined once in `pmcp-tasks` and used by all components.

**Warning signs:**
- Variable keys as string literals in more than one module
- Different components using different key naming conventions (`step.X.result` vs `steps[0].result` vs `x_result`)
- No documentation of what variables a workflow produces
- Client-side code guessing at variable structure from prompt reply text

**Phase to address:**
Phase 1 (Variable Schema Design). Define the schema struct before implementing partial execution, because the schema shapes everything downstream.

---

### Pitfall 5: Partial Execution "Pause Point" Logic Becomes a Second Workflow Engine

**What goes wrong:**
The partial execution engine must decide, for each step, whether it can execute server-side or must be deferred to the client. The simplest rule is "execute if the step has no `DataSource::StepOutput` dependencies on non-yet-executed steps." But this is a simplification. Real pause criteria include: (a) the step needs LLM reasoning (guidance text present), (b) the step needs user input (elicitation), (c) the step's tool is not registered on the server (external tool), (d) the step references a resource that requires client-side fetching, (e) a previous step failed and recovery requires client judgment. As more pause criteria accumulate, the "should I execute this step?" decision tree grows into a second workflow engine embedded inside the first one, with its own state, its own error handling, and its own bugs.

**Why it happens:**
Developers start with a clean "execute all, pause on first unresolvable" loop. Then product requirements add nuance: "skip steps that need user input," "execute steps even if a previous step failed (for non-critical steps)," "allow the workflow author to mark steps as client-only." Each requirement adds a branch to the pause logic, and the branches interact in unexpected ways. The result is a workflow-within-a-workflow that is harder to understand than either system alone.

**How to avoid:**
Make the pause decision a SIMPLE, EXPLICIT property of each `WorkflowStep`, not an inferred decision. Add an execution mode enum to `WorkflowStep`:

```rust
#[derive(Clone, Debug, Default)]
pub enum StepExecution {
    /// Execute server-side during partial execution (default)
    #[default]
    ServerSide,
    /// Always defer to client (step needs LLM reasoning or user input)
    ClientSide,
    /// Execute server-side, but defer to client if server execution fails
    BestEffort,
}
```

The partial execution engine has exactly ONE decision: check `step.execution()`. If `ServerSide`, execute it. If `ClientSide`, stop and add to remaining. If `BestEffort`, try and fall back. No inference, no heuristics, no "does this step have guidance? then it probably needs the client" guessing. The workflow author explicitly declares intent.

The existing `with_guidance()` method on `WorkflowStep` already signals "this step may need client reasoning." But it is currently used for rendering, not for execution decisions. Do not overload its semantics. Keep `guidance` for rendering and `execution` for control flow.

**Warning signs:**
- The pause loop checking for `step.guidance().is_some()` to decide server vs client
- More than 3 conditions in the "should I execute?" decision
- Steps executing server-side that should have been deferred (wrong pause logic)
- The pause decision referencing anything outside the step's own properties (e.g., inspecting task variables to decide whether to pause)

**Phase to address:**
Phase 1 (WorkflowStep extension). Add `StepExecution` before implementing the partial execution loop. Validate with the workflow author, not inferred at runtime.

---

### Pitfall 6: Error During Partial Execution Leaves Task in Inconsistent Half-Completed State

**What goes wrong:**
The partial execution engine runs steps 1, 2, 3 in sequence. Step 2 succeeds. Step 3 fails (tool error, timeout, variable size exceeded). The task now has variables from steps 1 and 2, but step 3's failure is ambiguous: did the tool execute and fail (effect already applied, e.g., database row inserted)? Or did the tool execution error out before any side effects (safe to retry)? The task variables say steps 1-2 completed, but the prompt reply must communicate step 3's failure in a way the client can act on. If step 3's failure is reported as a generic "step failed," the client has no basis for deciding whether to retry step 3 or skip it. If the task status remains `Working`, the client may not even know something went wrong.

**Why it happens:**
The existing `WorkflowPromptHandler` treats any step failure as a terminal error -- it returns an error from `handle()` and the entire prompt fails. But in partial execution, step failure is not necessarily terminal. The workflow should be able to communicate "steps 1-2 done, step 3 failed with error X, remaining steps 4-5 still needed" and let the client decide. This requires a richer error model than "success or failure" -- it needs "partial success with failure details."

**How to avoid:**
Each step in the progress schema should have a status field, not just "completed" or "remaining":

```rust
pub enum StepStatus {
    Completed { result: Value },
    Failed { error: String, retryable: bool },
    Skipped { reason: String },
    Remaining { tool: String, args: Value },
}
```

When a step fails during partial execution:
1. Store the failure in task variables (`step.X.status = "failed"`, `step.X.error = "..."`)
2. Set `retryable` based on whether the tool handler indicates idempotency
3. Continue to the NEXT step only if the failed step was non-critical (opt-in via `WorkflowStep`)
4. By default, stop execution on first failure and report all remaining steps
5. Set task status to `Working` (not `Failed`) -- the task is still in progress, just needs client intervention
6. The prompt reply includes the failure details in both `_meta` and natural language

Never set the task to `Failed` during partial execution unless ALL steps have failed or the workflow is unrecoverable. The task state machine has `Working` and `InputRequired` for exactly this situation -- the task needs client action but is not terminal.

**Warning signs:**
- Step failure causing the entire task to transition to `Failed`
- No `retryable` flag on step failures
- Client receiving a `Failed` task with no information about which steps succeeded
- Steps after a failed step being silently skipped with no record in task variables

**Phase to address:**
Phase 2 (Partial Execution Engine) and Phase 3 (Prompt Reply). Error handling must be designed alongside the execution loop, not bolted on after.

---

### Pitfall 7: Resumption Semantics -- Client Continuation Tools Don't Know About Workflow Context

**What goes wrong:**
After partial execution, the prompt reply tells the client "call `approve_deployment` next." The client (LLM) calls `tools/call approve_deployment` as a regular tool call. But the tool handler for `approve_deployment` doesn't know it is being called as part of a workflow, doesn't know the task ID, and doesn't know to update the workflow progress variables. The tool executes correctly but the workflow state in task variables is stale -- it still shows `approve_deployment` as "remaining." The server has no way to advance the workflow because tool calls and workflow state are disconnected.

**Why it happens:**
MCP tool calls are stateless by design. `tools/call` receives a tool name and arguments -- there is no built-in concept of "this call is part of workflow X, task Y, step Z." The v1.0 task system solved this for task-augmented tools (via `params.task`), but the prompt-driven workflow continuation is different: the client is calling tools directly based on the prompt reply, not via task-augmented requests. There is no mechanism to bind a regular `tools/call` back to a workflow task.

**How to avoid:**
Two options, choose one:

**Option A (recommended): Embed task ID in the prompt reply tool arguments.** The remaining steps in the prompt reply include a `_task_id` argument that the LLM passes through when calling the tool. The server's tool middleware detects `_task_id`, loads the task, and provides the `TaskContext` to the handler. This is the least invasive approach -- it works with any MCP client because the task ID is just another argument.

```json
// In prompt reply:
"remaining": [
    {"tool": "approve_deployment", "args": {"_task_id": "task-abc-123", "config": "..."}}
]
```

**Option B: Use task-augmented tool calls.** The prompt reply instructs the client to use `params.task` with the task ID when calling tools. This requires the client to support MCP Tasks, which most clients don't yet. Not recommended as the primary mechanism.

In either case, the server needs a `WorkflowStepMiddleware` that intercepts tool calls with a `_task_id` argument, loads the task, provides the `TaskContext`, and after the tool completes, updates the workflow progress variables (marking the step as completed, advancing `current_step_index`).

**Warning signs:**
- Prompt reply listing remaining tools without any task ID reference
- Tool handlers that don't update workflow progress variables
- Client calling tools that complete successfully but the task variables are stale
- No middleware that connects a regular `tools/call` back to its workflow task

**Phase to address:**
Phase 3 (Client Continuation Pattern). This is the core of the "bridge" -- connecting prompt-driven guidance back to task state. Design the continuation mechanism before implementing the prompt reply, because the reply's format depends on how continuation works.

---

### Pitfall 8: SequentialWorkflow Validation Does Not Account for Partial Execution Dependencies

**What goes wrong:**
The existing `SequentialWorkflow::validate()` checks that all `DataSource::StepOutput` references point to earlier steps with explicit bindings. This is correct for full execution (all steps run). But in partial execution, some steps are deferred to the client. If step 3 (client-side) depends on step 2's output via `DataSource::StepOutput { step: "step2_result" }`, and step 2 was executed server-side, the dependency is satisfied by task variables. But if step 2 is ALSO client-side, the dependency is between two client-executed steps -- the prompt reply must communicate this dependency so the client executes them in order. The current validation does not distinguish between server-resolved and client-resolved dependencies, and does not flag the case where a client-side step depends on another client-side step's output.

**Why it happens:**
The existing `validate()` method treats all steps equally -- it does not know about `StepExecution::ServerSide` vs `StepExecution::ClientSide`. When partial execution is added, the validation becomes split-brain: some bindings are resolved by the server (available in task variables) and some must be resolved by the client (available only in the LLM's context window). The validation needs to verify that client-side step dependencies can actually be communicated through the prompt reply.

**How to avoid:**
Extend validation to be aware of step execution modes. Add a `validate_for_partial_execution()` method that:
1. Separates steps into server-side and client-side groups
2. Verifies all server-side step dependencies are on other server-side steps (or prompt args)
3. For client-side step dependencies on server-side steps, verifies the server step has a binding (so the result is stored in task variables and available in the prompt reply)
4. For client-side step dependencies on other client-side steps, verifies the dependent step comes AFTER the dependency in the step list and includes the dependency information in the prompt reply

```rust
impl SequentialWorkflow {
    pub fn validate_for_partial_execution(&self) -> Result<PartialExecutionPlan, WorkflowError> {
        // Returns a plan that explicitly states:
        // - Which steps run server-side
        // - Which steps are deferred
        // - Which deferred steps depend on server results (via task variables)
        // - Which deferred steps depend on other deferred steps (via prompt ordering)
    }
}
```

**Warning signs:**
- Existing `validate()` passing for workflows that would fail during partial execution
- Client-side steps referencing bindings that are never stored in task variables
- Prompt reply listing steps in an order that violates data dependencies
- `DataSource::StepOutput` references to steps that were deferred but whose results aren't in the prompt context

**Phase to address:**
Phase 2 (Partial Execution Engine). Validation must be extended before the execution loop is implemented, because the execution plan comes from validation.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Reusing `ExecutionContext` for both full and partial execution | Less code, one execution path | Two fundamentally different state management models (ephemeral vs durable) share code that makes assumptions about one or the other. Adding durable semantics to an ephemeral struct is a category error. | Never -- create a separate `TaskExecutionContext` that wraps `TaskContext` |
| Hardcoding variable key patterns as string literals | Fast to implement, easy to read | Keys drift across modules, no refactoring safety, key typos cause silent bugs. `"step.validate.result"` in the execution engine vs `"steps.validate.result"` in the prompt generator = silent data loss. | Only in tests -- production code must use constants or the typed schema struct |
| Skipping `_meta` in prompt reply (only natural language) | Simpler implementation, works with current LLMs | Smart clients that could parse structured guidance are forced to parse natural language, which is unreliable. Adding `_meta` later requires changing the reply format, breaking existing client integrations. | Never -- include `_meta` from day one, even if minimal |
| Making all steps `ServerSide` by default with no client-side option | Simpler execution loop, backward compatible | Steps that genuinely need LLM reasoning (fuzzy matching, user-facing decisions) are forced server-side and fail because the server has no LLM. Forces awkward workarounds like "skip steps that have guidance." | Acceptable for MVP only if a `ClientSide` option is planned for the next iteration |
| Using `PromptHandler` trait directly for task-aware prompts (no new trait) | Fits existing server registration, no API changes | `PromptHandler::handle()` returns `GetPromptResult` synchronously -- it has no way to signal "I created a task, here's the ID" as a side channel. Task creation becomes invisible to the server framework. | Never -- at minimum, extend `GetPromptResult` with optional `_meta` for task association |

## Integration Gotchas

Common mistakes when connecting the task-prompt bridge to existing systems.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `WorkflowPromptHandler` + `TaskContext` | Passing `TaskContext` via `RequestHandlerExtra::metadata` (a `HashMap<String, String>`) which requires serialization and loses type safety | Add `task_context: Option<TaskContext>` as a proper typed field on a workflow-specific execution context, not smuggled through generic metadata |
| `SequentialWorkflow` step execution in partial mode | Calling the existing `WorkflowPromptHandler::execute_step()` method which assumes all bindings are in-memory and available | Extract step execution into a standalone function that accepts either `ExecutionContext` or `TaskContext` for binding resolution, keeping the resolution mechanism pluggable |
| `PromptHandler` return type for task association | Returning a `GetPromptResult` with no indication that a task was created -- the server framework has no way to auto-include the task ID in the response | Ensure `GetPromptResult._meta` includes `pmcp:task_id` so the client can discover the task association. The existing `_meta: Option<Map<String, Value>>` on `GetPromptResult` is sufficient -- do NOT modify the trait |
| Existing middleware chain during partial execution | Executing tools during `prompts/get` without going through the middleware chain (auth, logging, rate limiting). The existing handler uses `MiddlewareExecutor` but the task-aware handler might bypass it for "simplicity." | Always use the `MiddlewareExecutor` for tool execution, even during partial workflow execution. Tools inside workflows are not special -- they must go through the same auth and logging as direct tool calls. |
| Task store cleanup for workflow tasks | Treating workflow tasks the same as tool-augmented tasks for TTL and cleanup. Workflow tasks may span hours (user thinking time between steps), while tool tasks complete in seconds. | Use workflow-specific TTL defaults (e.g., 4 hours vs 1 hour for tool tasks). Consider making TTL configurable per workflow via `SequentialWorkflow::with_ttl()`. |
| `DataSource::StepOutput` resolution from task variables | Assuming step outputs in task variables have the same shape as in-memory bindings. In-memory bindings store the raw `Value` from the tool result. Task variables may be serialized differently (e.g., string-encoded JSON if the variable schema uses string values). | Define a clear contract: step results stored in task variables use the same `serde_json::Value` representation as in-memory bindings. Test round-trip: in-memory result -> task variable -> resolution produces identical values. |

## Performance Traps

Patterns that work at small scale but fail as usage grows.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-reading all task variables on every step during partial execution | Each step calls `task_ctx.variables()` which fetches the full task record from the store. For a 10-step workflow, that is 10 full reads of an increasingly large variable payload. | Read task variables ONCE at the start of partial execution. Cache in the `TaskExecutionContext`. Only write-through on `set_variable`. | ~5 steps with non-trivial variable payloads on DynamoDB |
| Storing full tool results in task variables | A tool returning a 100KB JSON response stored as a variable. After 5 steps, the task has 500KB of variables, approaching the 1MB limit and slowing every read/write. | Store only the fields needed by subsequent steps, not the entire tool result. Use `DataSource::StepOutput { field: Some("id") }` to extract specific fields and store those. Document a "lean variables" pattern. | ~3-4 steps with large tool responses |
| Prompt reply re-rendering for every `tasks/get` poll | If the client polls `tasks/get` and the server re-generates the prompt reply each time (re-executing the prompt handler to build the message trace), each poll becomes as expensive as the original `prompts/get` call. | The prompt reply is generated ONCE during `prompts/get` and the result is stored in the task variables or task result. Subsequent `tasks/get` returns the stored state without re-rendering. | Any polling frequency under 10 seconds |
| Linear scan of completed steps to build prompt reply | Building the "completed steps" section of the prompt reply by iterating over all task variables and matching key patterns. As variables accumulate, this becomes O(n) string matching. | Use the typed `WorkflowProgress` schema with a `completed_steps: Vec<CompletedStep>` that is directly serialized/deserialized, not reconstructed from flat key scans. | ~20+ variables in a task |

## Security Mistakes

Domain-specific security issues for the task-prompt bridge.

| Mistake | Risk | Prevention |
|---------|------|------------|
| Prompt reply leaking internal tool arguments in natural language | The prompt reply says "Call deploy_service with database_password='hunter2'". The LLM client includes this in its conversation context, potentially leaking it to the user or to other tools. | Never include sensitive argument values in the natural language portion of the prompt reply. Use argument references (`"use the config from step 2"`) not literal values. Store sensitive values in task variables accessed by reference, not inlined in messages. |
| Task ID in prompt reply enables unauthorized task access | The prompt reply includes the task ID in plain text. A malicious user seeing the prompt output could use the task ID to access the task via `tasks/get` without proper authentication. | Task access is always gated by `owner_id` enforcement (already implemented in v1.0). The task ID alone is not sufficient for access. However, ensure the prompt reply makes this clear and does not encourage the client to share task IDs. |
| Client-side tool calls bypassing workflow step ordering | The prompt reply lists steps 3, 4, 5 as remaining. The client calls step 5 first, skipping 3 and 4. If step 5 has side effects (e.g., deploying without approval), the workflow's intended sequencing is bypassed. | The `WorkflowStepMiddleware` should validate that the step being called is the NEXT expected step in the workflow, not an arbitrary step. Reject out-of-order tool calls with a clear error: "Step 'deploy' requires 'approve' to complete first." |
| Workflow progress variables writable by the client | Task variables are a shared scratchpad. A malicious client could write `workflow.current_step_index = 99` to skip all remaining steps, or overwrite `step.validate.result` to inject false data. | Prefix server-managed workflow variables with a reserved namespace (e.g., `_workflow.`) and reject client-side writes to that namespace. Or make workflow progress variables read-only for clients (write-only for the server execution engine). |

## UX Pitfalls

Common user experience mistakes for workflow authors and LLM clients.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Prompt reply says "call tool X" without explaining WHY or WHAT it accomplishes | LLM makes the tool call but does not understand its purpose, leading to poor argument choices or unnecessary calls | Include a description with each remaining step: "Call 'approve_deployment' to get human approval for deploying to production. This step requires user confirmation." |
| No clear signal when all steps are done | Client keeps calling tools after the workflow is complete because the prompt didn't say "when done, the task will be marked complete" | Include explicit completion criteria in the prompt reply: "After completing all steps above, call `tasks/result` with task ID X to verify the workflow completed successfully." |
| Workflow error shows task-internal error codes | Client receives `TaskError::InvalidTransition` which is meaningless to the LLM and user | Map task-internal errors to workflow-level messages: "Cannot call deploy_service because the approval step has not been completed yet." |

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces.

- [ ] **Task-aware prompt handler:** Often missing fallback to full execution when no task store is configured -- verify that `TaskWorkflowPromptHandler` degrades gracefully to `WorkflowPromptHandler` behavior when tasks are not available
- [ ] **Variable schema:** Often missing `schema_version` field -- verify that the variable format can be upgraded without breaking existing in-progress tasks
- [ ] **Prompt reply:** Often missing `_meta` section -- verify that structured guidance is available for machine consumption, not just natural language
- [ ] **Partial execution:** Often missing step failure handling -- verify that a mid-execution step failure produces a valid prompt reply with failure details, not an error
- [ ] **Client continuation:** Often missing task ID propagation -- verify that remaining steps in the prompt reply include the task ID so the client can bind subsequent tool calls to the task
- [ ] **Resumption:** Often missing "where did I leave off" logic -- verify that a client calling a remaining-step tool updates the workflow progress variables and the server can determine the next step
- [ ] **Backward compatibility:** Often missing existing workflow tests run against new code -- verify that the entire existing workflow test suite passes WITHOUT modification
- [ ] **Step ordering:** Often missing client-side step dependency validation -- verify that out-of-order tool calls are rejected with a clear error
- [ ] **TTL:** Often missing workflow-specific TTL -- verify that workflow tasks don't expire during normal user think time (minutes to hours between steps)
- [ ] **Cleanup:** Often missing orphaned workflow task reaping -- verify that workflow tasks stuck in `Working` for longer than TTL are eventually cleaned up

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Existing workflows broken by handler modification | HIGH | Revert to original `WorkflowPromptHandler` (it must be untouched). If already merged and deployed, hotfix by restoring the original handler and creating the task-aware handler as a separate class. This is why the "separate handler" architecture is critical -- the original is always available as a fallback. |
| Dual-write inconsistency (ephemeral has data, durable doesn't) | MEDIUM | For in-progress tasks, read task variables to determine which steps actually persisted. Re-execute from the last durably-persisted step. For completed tasks with missing variable data, mark affected steps as "unknown" and let the client re-do them. |
| Prompt reply format doesn't work with a specific LLM client | LOW | Since both `_meta` (machine) and natural language (human) are present, clients that can't parse `_meta` still work via natural language. Only the `_meta` format needs updating for new clients, which is backward compatible. |
| Variable schema migration needed | MEDIUM | Add a migration function that reads `schema_version` from task variables and upgrades the schema in-place. Run the migration lazily on first access (check version, upgrade if needed, write back). Never delete old schema support -- always support reading old versions. |
| Client calling tools out of order | LOW | The `WorkflowStepMiddleware` rejects the call. The client receives a clear error and can retry with the correct step. No data corruption occurs because the middleware validates before executing. |
| Task expired during user think time | LOW | Client receives a `TaskError::Expired` on the next tool call. Create a new task and re-execute the workflow from scratch. Consider extending default TTL for workflow tasks to minimize this. |

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Handler modification breaking existing workflows | Phase 1: Architecture | `WorkflowPromptHandler` source file has zero diff; all existing workflow tests pass unchanged |
| Dual-write inconsistency | Phase 2: Partial Execution Engine | Integration test: inject `TaskStore::set_variables` failure mid-execution; verify prompt reply shows partial progress correctly |
| Prompt reply too rigid for LLMs | Phase 3: Structured Prompt Reply | Test with two different clients (Claude Code + simple test client); both correctly follow remaining steps |
| Variable schema as implicit API | Phase 1: Variable Schema Design | All variable keys defined in `WorkflowProgress` struct; no string literal keys in production code outside the schema module |
| Pause logic becoming second engine | Phase 1: WorkflowStep extension | `StepExecution` enum on `WorkflowStep`; pause loop is a simple match on `step.execution()` |
| Error during partial execution | Phase 2: Partial Execution Engine | Step failure produces valid prompt reply; task stays in `Working` not `Failed`; failure details in both `_meta` and messages |
| Client continuation disconnected from task | Phase 3: Client Continuation | Tool calls with `_task_id` argument update workflow progress variables; verified by integration test |
| Validation not accounting for partial execution | Phase 2: Validation Extension | `validate_for_partial_execution()` catches dependencies between two client-side steps; test case for this specific scenario |
| Workflow progress variables writable by client | Phase 3: Security | `_workflow.` prefix variables rejected on client write; integration test attempts write and verifies rejection |
| Existing prompt handler return type | Phase 1: Architecture | `GetPromptResult._meta` used for task association; no trait modification needed |

## Sources

- [State Machine Executor Part 2 -- Fault Tolerance](https://blog.adamfurmanek.pl/2025/10/13/state-machine-executor-part-2/) -- serialization pitfalls in paused state machines, backward/forward compatibility
- [Towards Serialization-free State Transfer in Serverless Workflows](https://doi.org/10.1145/3725986) -- performance costs of state serialization in workflow engines
- [QCon SF: Database-Backed Workflow Orchestration](https://www.infoq.com/news/2025/11/database-backed-workflow/) -- challenges in durable workflow state management
- [Using a Rust async function as a polled state machine](https://jeffmcbride.net/blog/2025/05/16/rust-async-functions-as-state-machines/) -- Rust async state machine fundamentals
- [Serde async state machine (Rust Forum)](https://users.rust-lang.org/t/serde-async-state-machine/99648) -- challenges serializing async state
- [Rust Concurrency: Common Async Pitfalls](https://leapcell.medium.com/rust-concurrency-common-async-pitfalls-explained-8f80d90b9a43) -- blocking in async, runtime considerations
- [Future proofing - Rust API Guidelines](https://rust-lang.github.io/api-guidelines/future-proofing.html) -- sealed traits, non_exhaustive, backward compatible changes
- [RFC 1105: API Evolution](https://rust-lang.github.io/rfcs/1105-api-evolution.html) -- what constitutes a breaking change in Rust
- [Effective Rust: Default implementations](https://effective-rust.com/default-impl.html) -- minimizing required trait methods for evolution
- [MCP 2025-11-25 Specification](https://modelcontextprotocol.io/specification/2025-11-25) -- Tasks primitive, prompt semantics
- [MCP's Next Phase: November 2025 Spec](https://medium.com/@dave-patten/mcps-next-phase-inside-the-november-2025-specification-49f298502b03) -- async tasks, prompt-driven workflows
- [A Year of MCP: 2025 Review](https://www.pento.ai/blog/a-year-of-mcp-2025-review) -- MCP ecosystem evolution, client diversity
- [MCP 2025-11-25 Spec Update (WorkOS)](https://workos.com/blog/mcp-2025-11-25-spec-update) -- experimental tasks status
- [MCP Tool Descriptions Are Smelly (arxiv)](https://arxiv.org/html/2602.14878v1) -- tool description quality impacts on LLM behavior
- [LLM Limitations: When Models Make Mistakes](https://learnprompting.org/docs/basics/pitfalls) -- structured prompt interpretation failure modes
- [MIT: Shortcoming makes LLMs less reliable](https://news.mit.edu/2025/shortcoming-makes-llms-less-reliable-1126) -- LLM instruction following reliability
- [Palantir: Best practices for LLM prompt engineering](https://www.palantir.com/docs/foundry/aip/best-practices-prompt-engineering) -- structuring prompts for reliable tool use
- Codebase analysis: `src/server/workflow/prompt_handler.rs` (WorkflowPromptHandler, ExecutionContext)
- Codebase analysis: `src/server/workflow/sequential.rs` (SequentialWorkflow, validation)
- Codebase analysis: `src/server/workflow/workflow_step.rs` (WorkflowStep, DataSource, guidance)
- Codebase analysis: `crates/pmcp-tasks/src/context.rs` (TaskContext, typed variable accessors)
- Codebase analysis: `crates/pmcp-tasks/src/router.rs` (TaskRouterImpl, task-augmented tool calls)
- Codebase analysis: `crates/pmcp-tasks/src/store/mod.rs` (TaskStore trait, atomicity guarantees)
- Codebase analysis: `crates/pmcp-tasks/src/domain/record.rs` (TaskRecord, variable injection)

---
*Pitfalls research for: Task-prompt bridge -- partial execution and client continuation for PMCP SDK v1.1*
*Researched: 2026-02-21*
