# Requirements: MCP Tasks — Task-Prompt Bridge

**Defined:** 2026-02-22
**Core Value:** Tool handlers can manage long-running operations through a durable task lifecycle with shared variable state that persists across tool calls.

## v1.1 Requirements

Requirements for the Task-Prompt Bridge milestone. Each maps to roadmap phases.

### Foundation

- [x] **FNDX-01**: Workflow prompt can create a task when invoked, binding the task to the prompt execution
- [x] **FNDX-02**: ~~StepExecution enum~~ DROPPED — steps execute best-effort at runtime; StepStatus enum tracks outcome (Pending/Completed/Failed/Skipped)
- [x] **FNDX-03**: Typed WorkflowProgress schema struct tracks goal, completed steps, remaining steps, and intermediate outputs in task variables
- [x] **FNDX-04**: TaskRouter trait extended with workflow-specific methods (create_workflow_task, set_task_variables, complete_workflow_task)
- [x] **FNDX-05**: TaskWorkflowPromptHandler composes with (not modifies) existing WorkflowPromptHandler

### Execution

- [x] **EXEC-01**: Server executes server-mode steps sequentially, storing each result in task variables (durable-first write order)
- [x] **EXEC-02**: Execution pauses at client-deferred steps without failing the task (task remains Working)
- [x] **EXEC-03**: Step failure during partial execution keeps task in Working state and records error details in task variables
- [x] **EXEC-04**: At runtime, when a step depends on an output from a failed/skipped step, the engine emits a distinct `UnresolvedDependency` pause reason (reinterpreted from build-time to runtime check)

### Handoff

- [ ] **HAND-01**: Prompt reply includes structured handoff as final assistant message with task ID, completed steps with results, and remaining steps with guidance
- [ ] **HAND-02**: Handoff format is hybrid: JSON block for machine parsing plus natural language for LLM clients
- [ ] **HAND-03**: Each remaining step in handoff includes tool name, expected arguments (with resolved values where available), and guidance text

### Continuation

- [ ] **CONT-01**: Follow-up tool calls can reference the workflow task via _task_id in request _meta
- [ ] **CONT-02**: Task variables are updated with step results when tool calls include _task_id binding
- [ ] **CONT-03**: Client can poll tasks/result to check overall workflow completion status

### Integration

- [ ] **INTG-01**: ServerCoreBuilder provides API to register task-aware workflow prompts
- [ ] **INTG-02**: Existing non-task workflows continue to work unchanged (backward compatibility)
- [ ] **INTG-03**: Working example (62_tasks_workflow.rs) demonstrates complete task-prompt bridge lifecycle
- [ ] **INTG-04**: Integration tests validate create-execute-handoff-continue-complete flow through real ServerCore

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Advanced Workflow

- **ADVW-01**: DataSource::TaskVariable enables steps to read values from task variable store
- **ADVW-02**: Workflow can resume from task state (re-invoke prompt with task ID to continue from last completed step)
- **ADVW-03**: StepExecution user API for runtime step mode customization

## Out of Scope

| Feature | Reason |
|---------|--------|
| Automatic client execution | MCP clients decide when/how to call tools; server cannot drive client |
| Per-step task statuses | Over-engineering; single task status with variable-level step tracking suffices |
| Bidirectional step negotiation | Client-server negotiation about step execution adds complexity without clear value |
| Prompt caching for workflows | Optimization concern, not v1.1 scope |
| Workflow branching/conditionals | Sequential-only for v1.1; branching is a different workflow engine |
| Modifying WorkflowPromptHandler | Composition pattern prevents breaking existing workflows |
| DynamoDB backend | Separate milestone (v2.0); in-memory sufficient for bridge validation |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| FNDX-01 | Phase 4 | Complete |
| FNDX-02 | Phase 4 | Complete |
| FNDX-03 | Phase 4 | Complete |
| FNDX-04 | Phase 4 | Complete |
| FNDX-05 | Phase 4 | Complete |
| EXEC-01 | Phase 5 | Complete |
| EXEC-02 | Phase 5 | Complete |
| EXEC-03 | Phase 5 | Complete |
| EXEC-04 | Phase 5 | Complete |
| HAND-01 | Phase 6 | Pending |
| HAND-02 | Phase 6 | Pending |
| HAND-03 | Phase 6 | Pending |
| CONT-01 | Phase 6 | Pending |
| CONT-02 | Phase 6 | Pending |
| CONT-03 | Phase 6 | Pending |
| INTG-01 | Phase 7 | Pending |
| INTG-02 | Phase 7 | Pending |
| INTG-03 | Phase 7 | Pending |
| INTG-04 | Phase 7 | Pending |

**Coverage:**
- v1.1 requirements: 19 total
- Mapped to phases: 19
- Unmapped: 0

---
*Requirements defined: 2026-02-22*
*Last updated: 2026-02-22 after roadmap creation*
