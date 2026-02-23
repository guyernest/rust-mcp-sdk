# Roadmap: MCP Tasks for PMCP SDK

## Milestones

- ✅ **v1.0 MCP Tasks Foundation** — Phases 1-3 (shipped 2026-02-22)
- 🚧 **v1.1 Task-Prompt Bridge** — Phases 4-7 (in progress)

## Phases

<details>
<summary>v1.0 MCP Tasks Foundation (Phases 1-3) — SHIPPED 2026-02-22</summary>

- [x] Phase 1: Foundation Types and Store Contract (3/3 plans) — completed 2026-02-21
- [x] Phase 2: In-Memory Backend and Owner Security (3/3 plans) — completed 2026-02-22
- [x] Phase 3: Handler, Middleware, and Server Integration (3/3 plans) — completed 2026-02-22

See: `.planning/milestones/v1.0-ROADMAP.md` for full phase details

</details>

### v1.1 Task-Prompt Bridge (In Progress)

**Milestone Goal:** A workflow prompt can create a task, execute steps server-side, store progress in task variables, and return structured guidance so the LLM client knows what's done and what to do next.

- [x] **Phase 4: Foundation Types and Contracts** - Schema, trait extension, step execution mode, and handler composition boundary — completed 2026-02-22
- [x] **Phase 5: Partial Execution Engine** - Task creation, durable step sync, pause on client-deferred steps, failure handling — completed 2026-02-23
- [x] **Phase 6: Structured Handoff and Client Continuation** - Hybrid prompt reply format and tool-call-to-task reconnection (completed 2026-02-23)
- [ ] **Phase 7: Integration and End-to-End Validation** - Builder wiring, backward compatibility, working example, integration tests

## Phase Details

### Phase 4: Foundation Types and Contracts
**Goal**: All contracts for the task-prompt bridge are defined, compilable, and non-breaking — the isolation boundary between existing workflows and task-aware workflows is proven
**Depends on**: Phase 3 (v1.0 server integration)
**Requirements**: FNDX-01, FNDX-02, FNDX-03, FNDX-04, FNDX-05
**Success Criteria** (what must be TRUE):
  1. `WorkflowProgress` struct with typed fields (`goal`, `steps: Vec<WorkflowStepProgress>`, `schema_version`) serializes to/from task variable JSON without data loss (proptest + unit tests)
  2. `TaskRouter` trait has 3 new methods (`create_workflow_task`, `set_task_variables`, `complete_workflow_task`) with default error implementations — all existing `TaskRouterImpl` code compiles without modification
  3. ~~`StepExecution` enum~~ DROPPED — steps execute best-effort at runtime; `StepStatus` enum (Pending/Completed/Failed/Skipped) tracks outcome per step
  4. `TaskWorkflowPromptHandler` struct exists, composes with `WorkflowPromptHandler` via delegation, and the original `WorkflowPromptHandler` has zero behavioral changes (only `_meta: None` field additions)
  5. All existing workflow tests pass without modification (backward compatibility proven)
**Plans:** 2 plans
Plans:
- [x] 04-01-PLAN.md — Foundation types (WorkflowProgress, TaskRouter extension, GetPromptResult _meta)
- [x] 04-02-PLAN.md — TaskWorkflowPromptHandler composition and opt-in mechanism

### Phase 5: Partial Execution Engine
**Goal**: Task-aware workflows create a task on invocation and execute server-mode steps with durable progress tracking, pausing cleanly at client-deferred steps
**Depends on**: Phase 4
**Requirements**: EXEC-01, EXEC-02, EXEC-03, EXEC-04
**Success Criteria** (what must be TRUE):
  1. When a task-aware workflow prompt is invoked, a task is created and step results are batch-written to task variables after execution completes or pauses
  2. Execution pauses at the first unresolvable step without failing the task — the task remains in `Working` status and task variables reflect all completed steps
  3. When a step fails during execution, the task stays `Working`, the error details are recorded in task variables, and the step is marked as failed with a per-tool `retryable` hint
  4. At runtime, when a step depends on an output that wasn't produced (producing step failed/skipped), the engine emits a distinct `UnresolvedDependency` pause reason with the blocked step, missing output, and suggested tool
**Plans:** 2 plans
Plans:
- [x] 05-01-PLAN.md — PauseReason types, retryable field, pub(crate) visibility on WorkflowPromptHandler internals
- [x] 05-02-PLAN.md — Active execution engine with step loop, batch write, auto-complete

### Phase 6: Structured Handoff and Client Continuation
**Goal**: After partial execution, the prompt reply tells the LLM client exactly what was done and what to do next, and follow-up tool calls reconnect to the workflow task
**Depends on**: Phase 5
**Requirements**: HAND-01, HAND-02, HAND-03, CONT-01, CONT-02, CONT-03
**Success Criteria** (what must be TRUE):
  1. The prompt reply includes a final assistant message containing the task ID, a summary of each completed step with its result, and a list of remaining steps with tool name, expected arguments (with resolved values where available), and guidance text
  2. The handoff format is hybrid: a `_meta` JSON block for machine parsing and natural language narrative for LLM clients that cannot parse structured data
  3. A follow-up `tools/call` request with `_task_id` in `_meta` updates the workflow's task variables with the step result and advances the workflow progress
  4. A client can poll `tasks/result` at any time to check overall workflow completion status and see all step results accumulated in task variables
**Plans:** 2/2 plans complete
Plans:
- [ ] 06-01-PLAN.md — Handoff message generation with argument resolution and placeholder fallback
- [ ] 06-02-PLAN.md — Tool-to-task reconnection, continuation recording, and cancel-with-result completion

### Phase 7: Integration and End-to-End Validation
**Goal**: The task-prompt bridge is wired into `ServerCoreBuilder` with a clean API, validated end-to-end, and demonstrated with a working example
**Depends on**: Phase 6
**Requirements**: INTG-01, INTG-02, INTG-03, INTG-04
**Success Criteria** (what must be TRUE):
  1. `ServerCoreBuilder` provides an API to register task-aware workflow prompts that requires no more than `with_task_support(true)` on the workflow plus an already-configured task store
  2. Existing non-task workflows registered via `prompt_workflow()` continue to work identically to v1.0 — no behavioral changes when task support is not enabled
  3. Example `62_tasks_workflow.rs` demonstrates the complete lifecycle: workflow prompt creates task, server executes resolvable steps, returns structured handoff, simulated client calls remaining tools with `_task_id`, final `tasks/result` poll shows completion
  4. Integration tests validate the full create-execute-handoff-continue-complete flow through a real `ServerCore` instance with `InMemoryTaskStore`
**Plans:** 2 plans
Plans:
- [ ] 07-01-PLAN.md — Bug fix (task_id extraction) + integration tests (builder API, backward compatibility, full lifecycle)
- [ ] 07-02-PLAN.md — Lifecycle example (62_task_workflow_lifecycle.rs) replacing opt-in example

## Progress

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Types and Store Contract | v1.0 | 3/3 | Complete | 2026-02-21 |
| 2. In-Memory Backend and Owner Security | v1.0 | 3/3 | Complete | 2026-02-22 |
| 3. Handler, Middleware, and Server Integration | v1.0 | 3/3 | Complete | 2026-02-22 |
| 4. Foundation Types and Contracts | v1.1 | 2/2 | Complete | 2026-02-22 |
| 5. Partial Execution Engine | v1.1 | 2/2 | Complete | 2026-02-23 |
| 6. Structured Handoff and Client Continuation | v1.1 | Complete    | 2026-02-23 | - |
| 7. Integration and End-to-End Validation | v1.1 | 0/2 | Not started | - |
