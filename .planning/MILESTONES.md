# Milestones

## v1.0 MCP Tasks Foundation (Shipped: 2026-02-22)

**Phases completed:** 3 phases, 9 plans
**Lines of code:** ~11,500 Rust LOC (7,621 source + 3,888 tests/examples)
**Timeline:** 2026-02-21 → 2026-02-22

**Delivered:** Complete MCP Tasks support for the PMCP SDK — from spec-compliant protocol types through in-memory storage with security enforcement to full server integration with task-augmented tool calls, lifecycle polling, and working examples.

**Key accomplishments:**
1. Complete MCP 2025-11-25 Tasks wire types with spec-compliant serialization (10 protocol types, state machine with validated transitions)
2. In-memory task store with DashMap concurrency, owner isolation, and configurable security limits (max tasks, TTL, anonymous access)
3. TaskContext ergonomic wrapper with typed variable accessors and atomic completion
4. Server integration — task-augmented tool calls intercepted and routed through TaskRouter trait, avoiding circular crate dependencies
5. Full lifecycle integration tests (11 tests) proving create-poll-complete-result flow end-to-end through real ServerCore
6. Working example (`60_tasks_basic.rs`) demonstrating the complete task lifecycle with background execution simulation

**Requirements:** 51/51 satisfied (TYPE-01..10, STOR-01..07, HNDL-01..06, SEC-01..08, INTG-01..12, TEST-01..04/06..08, EXMP-01)

---


## v1.1 Task-Prompt Bridge (Shipped: 2026-02-23)

**Phases completed:** 5 phases, 10 plans
**Code changes:** +10,697 / -553 across 77 files
**Timeline:** 2026-02-22 → 2026-02-23

**Delivered:** Task-prompt bridge for the PMCP SDK — workflow prompts create tasks, execute server-resolvable steps, return structured handoff with remaining step guidance, and support client continuation via `_task_id` binding.

**Key accomplishments:**
1. Task-aware workflow composition via `TaskWorkflowPromptHandler` that wraps `WorkflowPromptHandler` with zero modification to existing behavior
2. Active execution engine that creates tasks, runs server-resolvable steps sequentially, and pauses at client-deferred steps with typed `PauseReason` diagnostics
3. Hybrid handoff format with `_meta` JSON for machine parsing plus natural language narrative, including resolved arguments and remaining step guidance
4. Client continuation via `_task_id` in `_meta` with fire-and-forget step recording and cancel-with-result completion
5. End-to-end integration validation through `ServerCore::handle_request` plus lifecycle example (`62_task_workflow_lifecycle.rs`)
6. Quality polish closing all audit findings: accurate `SchemaMismatch` diagnostics, complete `PauseReason` coverage, zero clippy warnings, safe TTL overflow handling

**Requirements:** 19/19 satisfied (FNDX-01..05, EXEC-01..04, HAND-01..03, CONT-01..03, INTG-01..04)

---

