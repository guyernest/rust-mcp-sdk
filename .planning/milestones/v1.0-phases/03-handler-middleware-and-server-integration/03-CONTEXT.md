# Phase 3: Handler, Middleware, and Server Integration - Context

**Gathered:** 2026-02-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the pmcp-tasks system into the PMCP server — capability advertisement, tools/call interception for task creation, routing the four task endpoints (get/result/list/cancel), and a basic example. This phase delivers the end-to-end integration so a PMCP server can support the full task lifecycle.

**Critical architectural context:** The MCP server is a **thin stateless Lambda** — it does NOT run tasks itself. The server creates tasks in the TaskStore, triggers external execution (Step Functions, SQS, etc.), and returns immediately. External services update the task status in the shared TaskStore. The MCP server acts as a bridge between client polling and the TaskStore, never blocking for the duration of a task.

</domain>

<decisions>
## Implementation Decisions

### Tool handler opt-in and task creation patterns
- Phase 3 implements TWO task creation patterns: (1) client-initiated (client sends `task` field in tools/call), (2) long-running tools (tools that declare `taskSupport: required` in ToolInfo)
- Other patterns (server-initiated via prompts, task_start/task_complete convenience tools, composite operations) deferred to Phase 5
- No tool-level task annotations — tasks are NOT just "background execution" of a single tool. Tasks represent long-running, multi-step interactions that may span multiple tool calls
- TaskContext injected via RequestHandlerExtra (Option<TaskContext>) — handler checks if task context is present
- Handler's responsibility: (1) create task in store, (2) trigger external job execution, (3) store job reference (execution ARN, job ID) in task variables, (4) return immediately. Handler does NOT wait for completion.

### Task endpoint routing
- Built-in middleware handles all four task endpoints (tasks/get, tasks/result, tasks/list, tasks/cancel) automatically
- Developer does NOT write any task routing code — enabling tasks on the server is enough
- Status change notifications (notifications/tasks/status): Claude's discretion on automatic vs explicit triggering
- Execution model: Handler triggers external service, returns immediately. No tokio::spawn for long-running work inside the server. Simulated background (tokio::spawn + sleep) acceptable only in examples for demonstration purposes.

### Capability advertisement
- Explicit builder config: developer calls .with_task_store() on the server builder to enable tasks
- Store required, security defaults: just .with_task_store(store) is enough. TaskSecurityConfig has sensible defaults.
- All four endpoints (get, result, list, cancel) always-on when tasks are enabled — no individual control
- Placement in initialize response: Claude's discretion (follow the MCP 2025-11-25 spec)

### Example and developer UX
- 60_tasks_basic.rs: minimal viable example — simplest possible task-enabled server with one tool demonstrating create-poll-complete lifecycle
- Uses InMemoryTaskStore (self-contained, no external dependencies)
- Simulates background execution with tokio::spawn + sleep for demonstration (real Lambda pattern deferred to Phase 5 examples)
- Boilerplate: Claude's discretion on exact line count, but should be minimal given the builder pattern decisions above

### Claude's Discretion
- Status change notification triggering (automatic vs explicit)
- Task capability placement in initialize response (per spec)
- Exact builder API boilerplate for enabling tasks
- Execution model details for the example

</decisions>

<specifics>
## Specific Ideas

- "The MCP server is a thin layer, running on an AWS Lambda in a stateless mode. If a long running job needs to run, it should run outside of the lambda as blocking it is a waste of money."
- "The MCP server can trigger a state machine execution in AWS Step Functions, or a different job that is managed by a different service."
- "The task store is used for the stateful aspect of the task execution, where the MCP server can query the task store to the job-id or execution-id that it needs to query to report back."
- "Tasks are not necessarily background execution — they represent long-running, multi-tool-call interactions"
- Task creation triggers include: client-initiated, server-initiated via prompts, task mode tools, long-running tools, composite operations (like code mode validate+execute)

</specifics>

<deferred>
## Deferred Ideas

- Server-initiated task creation via prompts/workflows — Phase 5
- Task mode convenience tools (task_start/task_complete) — Phase 5
- Composite operation task patterns (code mode validate→execute) — Phase 5
- Real Lambda + Step Functions integration example — Phase 5
- Individual control over list/cancel capabilities — not currently needed

</deferred>

---

*Phase: 03-handler-middleware-and-server-integration*
*Context gathered: 2026-02-22*
