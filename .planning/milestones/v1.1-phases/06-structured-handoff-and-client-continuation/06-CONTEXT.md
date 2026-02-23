# Phase 6: Structured Handoff and Client Continuation - Context

**Gathered:** 2026-02-22
**Status:** Ready for planning

<domain>
## Phase Boundary

After partial execution, the prompt reply tells the LLM client exactly what was done and what to do next, and follow-up tool calls reconnect to the workflow task. This phase covers the handoff message format (HAND-01/02/03) and the client continuation path (CONT-01/02/03). It does NOT add new workflow capabilities, resume-from-task, or builder API changes (those are Phase 7 or v2).

</domain>

<decisions>
## Implementation Decisions

### Handoff message shape
- Primary audience is LLM clients (Claude, GPT) — optimize for LLM comprehension
- The message list follows the MCP prompt spec format: conversation trace of user/assistant messages that mirror what the interaction would look like if the client was actually making the planned calls
- When execution pauses, add an assistant message that naturally continues the conversation: "Step 2 failed because X. To continue, call deploy_service with {region: us-east-1}. Then call notify_team with..."
- _meta on GetPromptResult stays lean for the initial prompt — task_id and task_status. The message list carries the details (step results already appear as tool-call/result message pairs)
- Completed steps are already visible in the conversation trace as tool call announcements + tool result messages — no need to re-summarize them in the handoff

### Remaining step guidance
- Each remaining step in the handoff includes: tool name, pre-resolved argument values (where available), and guidance text
- When args can't be fully resolved (depend on output from a step the client needs to run first), use explicit placeholders: "Call notify_team with {result: <output from deploy_service>}"
- When a step has a guidance field defined, include it in the handoff narrative for reasoning context
- Task ID is only in _meta, NOT mentioned in the narrative text

### Tool-to-task reconnection
- The workflow plan is guidance, not enforcement — the LLM client can call any tool in any order with any arguments. The server does NOT validate or block based on step ordering
- When a tool call includes `_task_id` in `_meta` and the tool matches a remaining workflow step, mark that step as completed and store the result under `_workflow.result.<step_name>` (best-effort matching, no blocking)
- When the tool does NOT match any workflow step, record the result under a generic key like `_workflow.extra.<tool_name>` for observability
- Retries: last result wins (overwrite). If the client calls the same step tool twice, the latest result replaces the previous one
- Progress tracking is updated in task variables after each reconnected tool call

### Completion semantics
- Auto-complete ONLY during initial prompt execution (when all steps succeed in the server-side loop)
- During client continuation, the task stays Working even if all steps are marked completed — explicit completion required
- Client signals completion via existing `tasks/cancel` with a completed result — reuse existing task API infrastructure, no new endpoint
- `tasks/result` polling behavior is Claude's discretion (standard task record with variables already includes `_workflow.*` data)

### Claude's Discretion
- Exact `tasks/result` response formatting (whether to add workflow-specific formatting or rely on standard task variables)
- Internal data structures for step matching during reconnection
- Error response format when `_task_id` references a non-existent or already-completed task
- How to handle edge cases like tool calls after task is already completed

</decisions>

<specifics>
## Specific Ideas

- The message list format mirrors a real conversation: "I want to deploy to us-east-1" → "Here's my plan: 1. validate_config 2. deploy_service 3. notify_team" → tool calls with results → handoff for remaining steps
- "The plans are only plans and the intelligence is the LLM on the MCP client — it can decide to call any tool in any order and with any arguments to complete the task, and the MCP server should not block it in any way"
- Reference the MCP prompt spec for message list format: https://modelcontextprotocol.io/specification/2025-06-18/server/prompts

</specifics>

<deferred>
## Deferred Ideas

- Resume-from-task (re-invoke prompt with task ID to continue from last completed step) — v2 ADVW-02
- DataSource::TaskVariable for reading values from task variable store — v2 ADVW-01
- Builder API wiring for task-aware workflows — Phase 7 INTG-01

</deferred>

---

*Phase: 06-structured-handoff-and-client-continuation*
*Context gathered: 2026-02-22*
