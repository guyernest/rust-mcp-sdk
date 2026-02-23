# Phase 5: Partial Execution Engine - Context

**Gathered:** 2026-02-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Make TaskWorkflowPromptHandler actually persist step results to task variables as it executes, and pause gracefully when it can't continue. The engine runs steps sequentially, stops at the first blocker, records structured pause reasons, and auto-completes when all steps succeed. The existing WorkflowPromptHandler execution loop is the reference — Phase 5 wires it to the task lifecycle.

</domain>

<decisions>
## Implementation Decisions

### Durable write ordering (batch at end)
- **Batch write after execution completes or pauses** — accumulate step results in memory during execution, write all to task store in one batch at the end
- **Single batch includes both WorkflowProgress (step statuses) and step results** — consistency between progress and results is guaranteed
- **If batch write fails: return prompt result anyway** — graceful degradation, consistent with Phase 4's pattern. Log the error, return _meta with in-memory state. Client gets results even if task store persistence failed.
- **Store raw tool results as-is** — no summarization or truncation. Existing `TaskSecurityConfig.max_variable_size_bytes` handles overflow.

### Pause behavior (stop at first blocker)
- **Sequential execution, stop at first unresolvable step** — matches existing WorkflowPromptHandler break-on-failure pattern. No skip-and-try-remaining.
- **Task stays in Working status** when paused — step statuses in variables tell the story (Completed/Pending/Failed)
- **Structured pause reason** stored in `_workflow.pause_reason` variable — a `PauseReason` enum with specific variants:
  - `unresolvable_params` — step can't resolve its parameters from available context
  - `schema_mismatch` — resolved parameters don't satisfy the tool's input schema
  - `tool_error` — tool execution returned an error
  - `unresolved_dependency` — step depends on output from a failed/skipped step; includes `blocked_step`, `missing_output`, `producing_step`
- **Pause reason includes actionable guidance**: blocked step name + missing parameter + **tool name the client should call** (with `_task_id`). The client continues by calling MCP tools, so the pause must bridge to tool calls.
- **Auto-complete when all steps succeed** — if every step executes without pause, mark task Completed automatically. No client action needed for fully-resolvable workflows.

### Step failure handling (unified with pause)
- **Tool errors are just another pause reason** — same model as unresolvable params. Step marked Failed, remaining steps stay Pending, task stays Working.
- **Error stored in same `_workflow.result.<step_name>` key** — contains either success result OR error. WorkflowStepProgress.status (Completed/Failed) distinguishes them.
- **Current state only, no history** — when a client retries a failed step and it succeeds, the step status changes from Failed to Completed and the error is replaced by the success result. No audit trail.
- **Per-tool retry hint** — specific tools can declare themselves as retryable (e.g., transient network errors). Most tools don't need this. The client/LLM is expected to remedy and retry.

### Dependency validation (EXEC-04 reinterpreted)
- **EXEC-04 reinterpreted as runtime check**, not build-time validation — since steps have no pre-classification, dependency issues surface at runtime when a step can't resolve params because the producing step failed/skipped
- **`unresolved_dependency` is a specific PauseReason variant** — distinct from generic `unresolvable_params`. Includes: blocked_step, missing_output, producing_step, suggested_tool
- **The suggested tool is critical** — client needs to know which MCP tool to call (with `_task_id`) to provide the missing value. This is either the failed step's tool or the next step's tool from the workflow definition.

### Claude's Discretion
- WorkflowProgress and step results batch write implementation details (single `set_task_variables` call vs multiple)
- PauseReason struct field names and exact JSON serialization format
- How auto-complete interacts with the _meta response (completed task vs working task _meta shape)
- Whether per-tool retry hint is a field on WorkflowStep or on the tool definition

</decisions>

<specifics>
## Specific Ideas

- "Tasks are plans, and plans are meant to be changed with reality" — the engine executes what it can and pauses with enough context for the client to continue
- "We don't need to keep the history of the task, only the steps that have information that can help us complete the tasks successfully" — current state only, overwrite on retry success
- "The way that the client can provide such missing variables is by calling MCP tools to the server with the task id as reference" — pause reasons must bridge to actionable tool calls, not just state missing variables
- The existing WorkflowPromptHandler break-on-failure pattern (lines 874-960 in prompt_handler.rs) is the reference implementation — Phase 5 adds task lifecycle around it

</specifics>

<deferred>
## Deferred Ideas

- Step-level retry policies with exponential backoff — over-engineering for v1.1
- Parallel step execution within a workflow — sequential-only for v1.1
- Resume from task state (re-invoke prompt to continue from last step) — v2 requirement (ADVW-02)

</deferred>

---

*Phase: 05-partial-execution-engine*
*Context gathered: 2026-02-22*
