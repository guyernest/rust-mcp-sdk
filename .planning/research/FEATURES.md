# Feature Research: Task-Prompt Bridge for PMCP SDK v1.1

**Domain:** Protocol SDK -- task-aware workflow prompts with partial execution and client continuation
**Researched:** 2026-02-21
**Confidence:** MEDIUM (MCP spec defines tasks and prompts independently; bridge semantics are PMCP innovation, not spec-mandated. A2A's input_required pattern and durable execution research provide validated analogues.)

## Context

v1.0 shipped the complete MCP Tasks lifecycle (create, poll, complete, cancel, list), task variables as shared state, owner isolation, and the SequentialWorkflow system with server-side execution. This research covers **only the v1.1 features**: bridging workflows (prompts) with tasks so workflows can pause mid-execution, track progress in task variables, and return structured guidance for client continuation.

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any task-prompt bridge must have. Without these, the bridge concept fails to deliver on its promise. A user who reads "task-aware workflow prompts" and gets anything less will consider the feature broken.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Task-aware prompt handler** (WorkflowPromptHandler creates a task on invocation) | The foundational bridge. A workflow prompt that claims task support must actually create a task when invoked. Without this, there is no bridge -- just a workflow and a separate task. Users expect `prompts/get` on a task-aware workflow to return a task ID they can poll. | MEDIUM | Extends existing `WorkflowPromptHandler`. On `handle()`, create task via store, bind step execution to task, return task ID in `_meta` of GetPromptResult. Must integrate with existing `SequentialWorkflow` without breaking non-task workflows. Requires `Arc<dyn TaskStore>` access from prompt handler. |
| **Partial server-side execution** (run steps until unresolvable, then stop) | The existing WorkflowPromptHandler already does this -- it breaks on parameter resolution failure or schema unsatisfied. The task-aware version must do the same BUT persist progress in the task. Users expect: steps the server can do are done, steps it cannot are left for the client. | MEDIUM | Current `prompt_handler.rs` (lines 874-961) already breaks on unresolvable steps. New behavior: before breaking, write completed step bindings to task variables and update task status. The ExecutionContext's in-memory bindings become durable via task variables. Key change: `ExecutionContext` wraps `TaskContext` when task-backed. |
| **Step state tracking in task variables** (standard schema for workflow progress) | Users polling `tasks/get` need to see what happened. Task variables must contain a predictable schema showing which steps completed, what they produced, and what remains. Without this, the task is an opaque blob -- the client cannot reason about progress. Both A2A (artifacts accumulate per-turn) and Temporal (workflow state is queryable) provide this. | MEDIUM | Define a standard variable schema: `_workflow.goal`, `_workflow.steps` (array of step definitions), `_workflow.completed` (array of completed step names with results), `_workflow.remaining` (array of remaining step names), `_workflow.current_step_index`. Prefix with `_workflow.` to avoid collision with user-defined variables. Schema must be JSON-serializable and readable by any MCP client. |
| **Structured prompt reply with step guidance** (GetPromptResult includes completed results + remaining steps + task ID) | The prompt reply is the client's instruction manual. It must tell the LLM: (1) what was already done (completed step results), (2) what needs to happen next (remaining steps with tool names and expected arguments), and (3) how to track progress (task ID). Without structure, the LLM gets raw text and must guess what to do. | MEDIUM | The existing prompt handler builds a conversation trace (user intent, plan, tool calls, results). For partial execution, extend with: a summary assistant message listing completed vs remaining steps, tool call guidance for the first remaining step (tool name, argument sources, expected binding), and the task ID in `_meta`. This is the "handoff document" from server to client. |
| **Client continuation via tool calls + task polling** | After receiving the structured prompt reply, the client must be able to: (1) call the next tool directly using guidance from the prompt reply, (2) pass the task ID in `_meta` so tool results bind to the task, (3) poll `tasks/get` to see updated progress. Without this, the partial execution is a dead end. | LOW | Mostly a protocol-level concern, not new code. Existing task-augmented `tools/call` already works. The new requirement is documentation of the continuation protocol and ensuring tool handlers can read/write the same task variables the workflow uses. Task ID propagation via `_meta.io.modelcontextprotocol/related-task` is already implemented. |
| **Task status lifecycle integration** (workflow states map to task states) | A task-backed workflow must set appropriate task statuses: `working` during execution, `completed` when all steps finish server-side, and the task must NOT be marked terminal when partial execution hands off to client (client still needs to continue). Users expect task status to reflect workflow progress faithfully. | LOW | During partial execution: task stays `working` (client is expected to continue). After all steps complete server-side: task transitions to `completed` with the final result. On error: task transitions to `failed`. On cancellation (via `extra.is_cancelled()`): task transitions to `cancelled`. Clear mapping, small code change. |

### Differentiators (Competitive Advantage)

Features that make PMCP's task-prompt bridge uniquely valuable. No other MCP SDK has a workflow-as-prompt system, let alone one backed by durable tasks. These features compound the existing differentiators.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Automatic step result persistence to task variables** | Every completed step's binding is automatically written to task variables. If step "validate" produces `{"config": {...}}`, it appears in `task.variables["validate"]` immediately. The client can poll `tasks/get` mid-workflow and see accumulated results. No manual `set_variable()` calls needed for basic data flow. This is the "server memory" promise made real. | MEDIUM | When a step with `.bind("name")` completes, the prompt handler writes the result to `task_context.set_variable("name", result)`. This mirrors the existing `execution_context.store_binding()` but makes it durable. Dual-write: in-memory (for same-request step chaining) + task store (for persistence). |
| **Structured handoff message format** | The prompt reply's final assistant message is a machine-readable handoff: a JSON-structured block (embedded in text for LLM consumption, parseable for programmatic clients) listing task ID, completed steps with summaries, and next-step guidance with tool name, expected arguments, and data sources. This goes beyond "here's what I did" to "here's exactly how to continue." | HIGH | Design a handoff format that works for both LLMs (natural language with structure) and programmatic clients (parseable JSON within the message). The format must include: `taskId`, `completedSteps[]` (name, tool, status, binding), `nextStep` (name, tool, arguments with sources), `remainingSteps[]` (names). Balance between human-readable and machine-parseable. |
| **DataSource resolution from task variables** | New `DataSource::TaskVariable { key, field }` variant that resolves step arguments from task variables instead of in-memory bindings. This enables a client-completed step to feed data to subsequent server steps in a resumed workflow. Cross-session data flow becomes possible: session 1 creates task + does steps 1-2, session 2 resumes and steps 3-4 read from task variables. | HIGH | Extends the existing `DataSource` enum with a new variant. Resolution in `resolve_tool_parameters()` pulls from `TaskContext` instead of `ExecutionContext`. Must handle the case where a task variable was set by a previous session (not in current execution context). Backward-compatible: existing `StepOutput` still works for same-session chains. |
| **Guidance-aware step classification** | Steps with `.with_guidance()` are classified as "client steps" (need LLM reasoning), steps without guidance and with fully resolvable parameters are "server steps" (can execute deterministically). The prompt handler pre-classifies steps and reports this in the handoff, so the client knows upfront which steps it must handle. | LOW | Classification is already implicit in the current code (steps that fail parameter resolution are handed off). Making it explicit via a `StepExecutability` enum (ServerExecutable, ClientRequired, Conditional) and including it in the handoff message. Enables smarter client strategies (skip polling if all remaining steps are client-required). |
| **Workflow resume from task state** | A new API `WorkflowPromptHandler::resume(task_id, extra)` that reconstructs execution state from task variables and continues from where the workflow paused. Enables a second `prompts/get` call to pick up a partially-executed workflow. Neither A2A nor any MCP SDK supports this -- they require the client to manually call individual tools. | HIGH | Read `_workflow.completed` and `_workflow.current_step_index` from task variables. Reconstruct `ExecutionContext` bindings from stored step results in task variables. Resume step loop from `current_step_index`. Must handle: steps completed by client (results in task variables but not from server execution), steps that were skipped, and steps whose prerequisites changed. This is the most complex feature in the milestone. |
| **Example demonstrating full task-prompt bridge** (`61_tasks_workflow.rs`) | A working example that shows: prompt creates task, server runs 2 of 4 steps, returns structured handoff, simulated client continues steps 3-4 using guidance, polls task for final result. Proves the bridge works end-to-end. No other MCP SDK has an equivalent. | MEDIUM | Must demonstrate: workflow definition with 4 steps (2 server-executable, 2 client-required), task creation on prompt invocation, partial execution with task variable persistence, structured handoff message, client-side tool calls with task ID propagation, final task completion. Complex example but high-value -- this IS the feature demonstration. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem natural for a task-prompt bridge but create problems in the MCP context.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Automatic client-side step execution** | "The SDK should handle remaining steps automatically on the client side" | MCP clients are LLM applications. The SDK provides prompts (guidance), not execution. Automatic execution would require the SDK to embed an LLM orchestrator, which is the host application's job. The protocol separation is intentional: server provides context, client decides actions. | Provide structured handoff with explicit tool call guidance. Let the host LLM decide how to execute remaining steps. Document the continuation protocol. |
| **Step-level task status transitions** (working per step) | "Each step should have its own task status, like `working` for step 1, `input_required` for step 2" | MCP tasks have ONE status. The spec does not support per-step status within a task. Adding sub-statuses creates a state machine explosion (5 states x N steps). Task variables are the right place for per-step state. | Track per-step state in task variables: `_workflow.completed`, `_workflow.remaining`. Task-level status stays simple: `working` (in progress), `completed` (all done), `failed` (error). |
| **Bidirectional step negotiation** (server asks client mid-execution) | "During workflow execution, the server should be able to ask the client questions before continuing to the next step" | This requires sampling or elicitation during `prompts/get`, which is a completely different protocol flow. Prompts are single-request-response. Mixing prompts with server-initiated requests creates circular dependencies and violates the MCP protocol flow. A2A handles this with multi-turn messages; MCP does not. | The partial execution pattern IS the solution: server runs what it can, returns a structured handoff, client acts on it. If the server needs client input, it stops at that step and includes guidance. The next round (tool call or re-invocation) provides the input. |
| **Task-backed prompt caching** (resume prompt from cache) | "If I call `prompts/get` again with the same arguments, return the cached task result instead of re-executing" | Prompts are expected to be side-effect-free in most MCP clients. Caching a task reference would make repeated `prompts/get` calls return stale or unexpected results. The task ID is in the original response -- the client should use it for continuation, not re-invoke the prompt. | Return task ID in the prompt response. Document that clients should use `tasks/get` and direct tool calls for continuation, not repeat `prompts/get`. Provide `resume()` API for explicit re-entry with a task ID. |
| **Workflow branching based on step results** | "If step 1 returns X, run step 2a; if step 1 returns Y, run step 2b" | SequentialWorkflow is sequential by design. Adding branching makes it a DAG engine, which massively increases complexity in validation, execution, and especially in the handoff message format (which branch to communicate?). This is a workflow engine concern, not an SDK concern. | Use step guidance to describe conditional logic: "Based on the validation result above, choose the appropriate next action." The LLM client handles branching naturally. For programmatic branching, use separate workflows or custom PromptHandler implementations. |
| **Persistent workflow definitions in task store** | "Store the workflow definition alongside the task so any server instance can resume it" | Workflow definitions are code (Rust structs). Serializing them requires a DSL or schema language, adding massive complexity. Workflows are registered at server startup -- they are always available on any server instance running the same code. | Store only state (task variables) in the task store, not definitions. The workflow definition lives in the server code. Any server instance running the same version can resume from task variables because the step definitions are identical. Document that workflow schema changes require migration. |

## Feature Dependencies

```
[EXISTING: SequentialWorkflow + WorkflowPromptHandler]
    |
    +--extended-by--> [Task-Aware Prompt Handler]
    |                       |
    |                       +--requires--> [EXISTING: TaskStore + TaskContext]
    |                       |
    |                       +--requires--> [EXISTING: InMemoryTaskStore]
    |                       |
    |                       +--enables--> [Partial Server-Side Execution with Persistence]
    |                       |                   |
    |                       |                   +--requires--> [Step State Tracking in Task Variables]
    |                       |                   |                   |
    |                       |                   |                   +--defines--> [Standard Variable Schema (_workflow.*)]
    |                       |                   |
    |                       |                   +--enables--> [Structured Handoff Message]
    |                       |                   |                   |
    |                       |                   |                   +--enables--> [Client Continuation Protocol]
    |                       |                   |
    |                       |                   +--enables--> [Task Status Lifecycle Integration]
    |                       |
    |                       +--enables--> [Automatic Step Result Persistence]
    |                                           |
    |                                           +--enables--> [DataSource::TaskVariable]
    |                                                               |
    |                                                               +--enables--> [Workflow Resume from Task State]

[Step State Tracking] + [Structured Handoff Message]
    |
    +--enables--> [Guidance-Aware Step Classification]

[All of the above]
    |
    +--enables--> [Example: 61_tasks_workflow.rs]
```

### Dependency Notes

- **Task-Aware Prompt Handler extends WorkflowPromptHandler, not replaces.** Non-task workflows must continue working unchanged. The bridge is opt-in via `.with_task_support()` on SequentialWorkflow or a new `TaskWorkflowPromptHandler` that wraps `WorkflowPromptHandler`.
- **Step State Tracking in Task Variables is the critical foundation.** Without a standard schema, the handoff message has nothing to communicate and the resume API has nothing to reconstruct from. Design the schema first.
- **Structured Handoff Message depends on Step State Tracking.** The handoff reads from `_workflow.completed` and `_workflow.remaining` to build the guidance.
- **DataSource::TaskVariable depends on Automatic Step Result Persistence.** The new data source variant is only useful if step results are actually in task variables. Build persistence first, then the data source.
- **Workflow Resume is the last and hardest feature.** It depends on everything else: task-aware handler (creates the task), step state tracking (knows what was done), automatic persistence (has the data), and DataSource::TaskVariable (can read it back). Build last, validate thoroughly.
- **Client Continuation Protocol is mostly documentation, not code.** The tools/call + task polling already work. The new contribution is documenting the handoff contract and ensuring _meta propagation.

## MVP Definition

### Launch With (v1.1.0 -- Task-Prompt Bridge)

Minimum feature set to validate that task-backed workflows with partial execution work end-to-end.

- [ ] **Task-aware prompt handler** -- WorkflowPromptHandler creates a task when invoked with task support enabled. Returns task ID in GetPromptResult `_meta`.
- [ ] **Partial execution with task variable persistence** -- Steps that execute server-side write bindings to task variables. Step loop behavior unchanged (break on unresolvable params), but now progress is durable.
- [ ] **Step state tracking in task variables** -- Standard `_workflow.*` schema: goal, steps, completed, remaining, current_step_index. Written on each step completion and at handoff point.
- [ ] **Structured prompt reply (handoff message)** -- Final assistant message in GetPromptResult includes: task ID, completed step summaries, next step guidance with tool name and argument sources, remaining step list.
- [ ] **Task status lifecycle integration** -- Task stays `working` during execution and at handoff. Transitions to `completed` only when all steps finish server-side. Transitions to `failed` on error.
- [ ] **Automatic step result persistence** -- Step bindings automatically written to task variables alongside in-memory context. Dual-write ensures both same-request chaining and durability.
- [ ] **Example: 61_tasks_workflow.rs** -- End-to-end demonstration of 4-step workflow with 2 server-executed steps, handoff, and simulated client continuation.

### Add After Validation (v1.1.x)

Features to add once the basic bridge is working and tested.

- [ ] **DataSource::TaskVariable** -- Trigger: users need cross-session data flow where a client-completed step feeds subsequent server steps.
- [ ] **Guidance-aware step classification** -- Trigger: clients want upfront knowledge of which steps require LLM reasoning vs which are server-executable.
- [ ] **Workflow resume from task state** -- Trigger: users need to re-invoke a workflow prompt to continue from where it left off across sessions.
- [ ] **Structured handoff as parseable JSON** -- Trigger: programmatic clients (not LLMs) need to parse the handoff message programmatically rather than from natural language.

### Future Consideration (v1.2+)

Features to defer until the task-prompt bridge is validated in production.

- [ ] **Nested workflows** (task-backed workflows that invoke sub-workflows) -- Wait for demand.
- [ ] **Conditional step execution within workflows** -- Wait for branching demand; currently handled by LLM reasoning.
- [ ] **Progress notifications per step** -- Wait for SSE transport adoption; polling is sufficient initially.
- [ ] **Client-side SDK helpers for continuation** -- Wait for client SDK ecosystem maturity.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority | Depends On |
|---------|------------|---------------------|----------|------------|
| Task-aware prompt handler | HIGH | MEDIUM | P1 | Existing TaskStore, WorkflowPromptHandler |
| Partial execution with persistence | HIGH | MEDIUM | P1 | Task-aware prompt handler |
| Step state tracking (_workflow.* schema) | HIGH | MEDIUM | P1 | Task-aware prompt handler |
| Structured handoff message | HIGH | MEDIUM | P1 | Step state tracking |
| Task status lifecycle integration | HIGH | LOW | P1 | Task-aware prompt handler |
| Automatic step result persistence | HIGH | LOW | P1 | Task-aware prompt handler |
| Example 61_tasks_workflow.rs | HIGH | MEDIUM | P1 | All P1 features |
| DataSource::TaskVariable | MEDIUM | HIGH | P2 | Automatic step result persistence |
| Guidance-aware step classification | MEDIUM | LOW | P2 | Step state tracking |
| Workflow resume from task state | HIGH | HIGH | P2 | DataSource::TaskVariable, step state tracking |
| Structured handoff as parseable JSON | MEDIUM | LOW | P2 | Structured handoff message |

**Priority key:**
- P1: Must have for v1.1.0 launch (validates the task-prompt bridge concept)
- P2: Should have, add when core bridge is working (enables advanced patterns)

## Competitor Feature Analysis

| Feature | MCP Spec (base) | A2A Protocol | Temporal Workflows | PMCP v1.1 Approach |
|---------|-----------------|--------------|--------------------|--------------------|
| **Prompt + task binding** | Prompts and tasks are independent primitives. No spec-defined bridge. | Tasks are the unit of agent interaction; no separate "prompt" concept. Messages within a task serve as guidance. | Workflows ARE the task. No separate prompt layer. | Prompt creates and binds to a task. Prompt reply includes task ID. Unique in MCP ecosystem. |
| **Partial execution** | Not addressed. Prompts return full result synchronously. | Tasks accumulate messages/artifacts over multiple turns. Server can return `input-required` to pause. | Workflows checkpoint after each activity. Resume from checkpoint on failure. | Server executes resolvable steps, persists progress to task variables, returns structured handoff. Closest to A2A's multi-turn pattern but using MCP primitives. |
| **Step progress tracking** | Not addressed for prompts. Tasks have `statusMessage` but no structured progress. | Task has `history` (message log) and `artifacts` (accumulated outputs). Client polls or subscribes for updates. | Full event history with each activity's input/output/status. Queryable via workflow queries. | Task variables with `_workflow.*` schema. Simpler than Temporal's event history, more structured than A2A's message log. Designed for LLM consumption. |
| **Client continuation guidance** | Not addressed. Client must infer what to do from prompt messages. | Agent returns messages with next-action hints. No standardized format. | N/A (Temporal handles continuation internally). | Structured handoff message with explicit tool name, argument sources, and remaining step list. Most prescriptive guidance of any system. |
| **Cross-session resume** | Not possible. Prompts are stateless. | Tasks persist across sessions. Client can send follow-up messages. | Core feature. Workflows resume from any checkpoint on any worker. | Workflow resume via task variables. Less robust than Temporal (no event replay), but works within MCP's simpler model. |
| **Variable schema for progress** | No standard. `_meta` is freeform. | No standard variable schema. Artifacts are typed but custom per agent. | Workflow state is custom per workflow. No universal schema. | `_workflow.*` prefix convention: `goal`, `steps`, `completed`, `remaining`, `current_step_index`. PMCP-defined standard. |

### Key Takeaways

1. **PMCP is creating a new pattern.** No existing protocol or framework combines prompts (templates for LLM guidance) with durable tasks (stateful progress) and structured handoff (continuation instructions). This is genuinely novel.

2. **A2A's multi-turn task model is the closest analogue.** A2A tasks accumulate messages and artifacts over multiple `message/send` calls. PMCP's approach is similar but uses prompts for the initial structured guidance and task variables for state, which is simpler and more LLM-friendly.

3. **Keep it simpler than Temporal.** Temporal's event replay and checkpoint system is orders of magnitude more complex. PMCP's "read state from task variables, resume from step index" approach trades some robustness for massive simplicity. This is the right tradeoff for an SDK.

4. **The structured handoff is the unique value.** Every other system leaves continuation to the client's intelligence. PMCP explicitly tells the client "call tool X with arguments Y and Z" -- this is what makes the bridge practical for LLM clients that benefit from explicit guidance.

## Sources

### Authoritative (HIGH confidence)
- [MCP Tasks Specification (2025-11-25)](https://modelcontextprotocol.io/specification/2025-11-25) -- Task states, polling, result retrieval
- [SEP-1686: Tasks Proposal](https://github.com/modelcontextprotocol/modelcontextprotocol/issues/1686) -- Task lifecycle, input_required semantics

### Verified (MEDIUM confidence)
- [WorkOS: MCP Async Tasks Guide](https://workos.com/blog/mcp-async-tasks-ai-agent-workflows) -- Client continuation patterns, polling best practices, input_required semantics
- [MCP Prompts: Building Workflow Automation](http://blog.modelcontextprotocol.io/posts/2025-07-29-prompts-for-automation/) -- Prompt-driven automation patterns
- [A2A Protocol: Life of a Task](https://a2a-protocol.org/latest/topics/life-of-a-task/) -- Multi-turn task lifecycle, artifact accumulation, input-required patterns
- [A2A Protocol Specification](https://a2a-protocol.org/latest/specification/) -- Task state machine comparison
- [Restate: Building a Durable Execution Engine](https://www.restate.dev/blog/building-a-modern-durable-execution-engine-from-first-principles) -- Step checkpoint patterns

### Additional Context (LOW confidence -- architectural patterns)
- [Agents At Work: 2026 Playbook](https://promptengineering.org/agents-at-work-the-2026-playbook-for-building-reliable-agentic-workflows/) -- Pause/resume architecture, structured outputs
- [2026 Guide to Agentic Workflows](https://www.vellum.ai/blog/agentic-workflows-emerging-architectures-and-design-patterns) -- Workflow control flow patterns
- [Mastra Workflows](https://mastra.ai/docs/workflows/overview) -- Step state tracking, variable passing patterns
- [AWS Step Functions Variable Passing](https://docs.aws.amazon.com/step-functions/latest/dg/workflow-variables.html) -- Variable schema patterns for step data flow

---
*Feature research for: Task-Prompt Bridge (PMCP SDK v1.1)*
*Researched: 2026-02-21*
