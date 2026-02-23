# Phase 4: Foundation Types and Contracts - Context

**Gathered:** 2026-02-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Define the type contracts and composition boundary for the task-prompt bridge — WorkflowProgress schema, TaskRouter trait extension, _meta on GetPromptResult, and the opt-in mechanism for task-aware workflows. The existing WorkflowPromptHandler must not be modified.

</domain>

<decisions>
## Implementation Decisions

### Variable schema (hybrid approach)
- **Structured progress object** under `_workflow.progress` key — contains full step definitions: `{goal, steps: [{name, tool, status, execution_mode}], schema_version}`
- **Flat result keys** under `_workflow.result.<step_name>` — one key per completed step with its tool result
- **Prefix convention**: `_workflow.` (single underscore, matches `_meta` convention)
- **Result storage**: Default to raw tool result JSON, but allow WorkflowStep to specify a `result_summary_fn` for large outputs (configurable per step)

### Step execution mode (no StepExecution enum)
- **Steps are a plan, not a fixed assignment.** The server tries best-effort execution at runtime — no pre-classification of steps as server/client
- **Pause is runtime-emergent**: server runs until it can't resolve parameters, can't satisfy tool schema, or tool execution fails
- **This matches existing WorkflowPromptHandler behavior** (lines 874-960 in prompt_handler.rs) — the `break` statements already implement graceful handoff
- **Tool execution errors are pause points, not failures**: task stays Working, error recorded in variables, client decides what to do
- **FNDX-02 (StepExecution enum) is dropped** — the runtime pause mechanism replaces it

### Handoff format (_meta on GetPromptResult)
- **Message list stays as-is** — this is the MCP-standard prompt reply format showing the conversation trace (what happened). It works well and shouldn't change.
- **Task data goes in `_meta`** on GetPromptResult — complementary experimental layer. Requires adding `_meta: Option<serde_json::Map<String, Value>>` to `GetPromptResult` (currently missing)
- **Prompt reply `_meta` is simplified** — task_id, task_status, and a brief step plan. The message list already shows what happened, so _meta is just a task-aware client's shortcut.
- **Tool call `_meta` is more complete** — when a follow-up tool call includes `_task_id`, the response _meta should give fuller task context to help the MCP client understand progress.
- **_meta should be mostly consistent** across prompt results and tool call results — same structure, varying completeness.

### Composition model (per-workflow opt-in)
- **Task store on builder enables the capability**: `.with_task_store()` makes task backing available
- **Each workflow opts in individually**: not all workflows need task backing, gradual adoption
- **Opt-in mechanism**: Claude's Discretion (either `.with_task_support()` on SequentialWorkflow, or `.task_workflow()` on ServerCoreBuilder — Claude picks what fits the existing builder pattern best)
- **Composition approach**: Claude's Discretion — wrap (delegation) vs independent + shared helpers, choose based on what the codebase patterns support best
- **Backward compatibility is absolute**: existing workflows registered via `prompt_workflow()` must work identically whether or not a task store is configured

### Claude's Discretion
- Opt-in mechanism placement (workflow builder vs server builder method)
- Composition implementation (delegation vs independent with shared helpers)
- Exact _meta JSON structure (field names, nesting)
- Result summary function signature and defaults

</decisions>

<specifics>
## Specific Ideas

- "Tasks are plans, and plans are meant to be changed with reality" — the step list is best-effort, not a contract. Server executes what it can, client continues the rest.
- "The workflow as prompt should remain the same as it is working well" — the message list format is proven MCP. Tasks are complementary, not replacement.
- "The _meta should be mostly consistent across calls" — prompt results and tool call results use similar _meta structure for task state.
- Cross-server task sharing (future): same task ID readable by multiple MCP servers in a pmcp.run deployment via shared TaskStore + same OAuth sub owner binding.

</specifics>

<deferred>
## Deferred Ideas

- Cross-server task sharing on pmcp.run — captured in PROJECT.md Future requirements
- Client-initiated task plans — MCP client creates a task with a plan and shares it with the server for reference
- DataSource::TaskVariable for reading from task variable store — v2 requirement (ADVW-01)

</deferred>

---

*Phase: 04-foundation-types-and-contracts*
*Context gathered: 2026-02-22*
