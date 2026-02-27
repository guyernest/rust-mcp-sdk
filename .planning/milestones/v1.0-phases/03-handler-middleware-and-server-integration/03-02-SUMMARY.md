---
phase: 03-handler-middleware-and-server-integration
plan: 02
subsystem: server
tags: [task-router, server-core, async-trait, serde-json, owner-resolution]

# Dependency graph
requires:
  - phase: 03-01
    provides: "TaskRouter trait, ClientRequest task variants, builder with_task_store()"
  - phase: 02
    provides: "TaskStore trait, InMemoryTaskStore, TaskSecurityConfig, resolve_owner_id"
  - phase: 01
    provides: "Wire types (Task, CreateTaskResult, params), TaskRecord, TaskError"
provides:
  - "TaskRouterImpl: concrete TaskRouter bridging pmcp trait to TaskStore operations"
  - "Task-augmented tools/call interception in ServerCore before normal tool execution"
  - "All four task endpoints (get/result/list/cancel) routed through TaskRouter in core.rs"
  - "Owner resolution from AuthContext to TaskRouter.resolve_owner in ServerCore"
  - "Tool context (name, arguments, progressToken) stored as task variables for external pickup"
affects: [03-03, 04, 05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Task-augmented call interception pattern: check task field or tool_requires_task before normal tool path"
    - "TaskError -> pmcp::Error conversion using error_code() for proper JSON-RPC codes"
    - "Owner resolution bridge: AuthContext.subject/client_id -> TaskRouter.resolve_owner"

key-files:
  created:
    - "crates/pmcp-tasks/src/router.rs"
  modified:
    - "crates/pmcp-tasks/src/lib.rs"
    - "src/server/core.rs"

key-decisions:
  - "TaskRouterImpl stores tool context (name, args, progressToken) as task variables for external service pickup rather than executing tools"
  - "Task-augmented call interception happens in handle_request_internal BEFORE handle_call_tool to return CreateTaskResult as Value (avoiding CallToolResult type mismatch)"
  - "Tasks not enabled returns -32601 (METHOD_NOT_FOUND) consistent with 03-01 decision for unconfigured task router"

patterns-established:
  - "Router pattern: pmcp-tasks implements pmcp's TaskRouter trait, all params/returns as serde_json::Value to avoid circular dependency"
  - "cfg(not(wasm32)) guards on all task routing code paths in core.rs"

requirements-completed: [INTG-03, INTG-04, INTG-05, INTG-06, INTG-07, INTG-08, INTG-09, INTG-11, INTG-12]

# Metrics
duration: 9min
completed: 2026-02-22
---

# Phase 3 Plan 2: Task Routing Summary

**TaskRouterImpl implementation with ServerCore wiring for all four task endpoints and task-augmented tools/call interception**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-22T04:00:05Z
- **Completed:** 2026-02-22T04:09:51Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments
- Implemented TaskRouterImpl in pmcp-tasks with all 8 TaskRouter trait methods
- Wired task routing in ServerCore for TasksGet, TasksResult, TasksList, TasksCancel
- Task-augmented tools/call intercepted before normal tool execution, returns CreateTaskResult
- Owner resolution bridging AuthContext to resolve_owner_id via TaskRouter
- Tool context (name, arguments, progressToken) stored as task variables for external services
- 18 unit tests in router.rs covering all methods and edge cases
- Zero regressions in all existing tests (155 pmcp-tasks + 19 core.rs)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement TaskRouter in pmcp-tasks crate** - `3d12ab7` (feat)
2. **Task 2: Wire task routing in ServerCore handle_request_internal** - `6e9955b` (feat)

## Files Created/Modified
- `crates/pmcp-tasks/src/router.rs` - TaskRouterImpl implementing pmcp's TaskRouter trait via TaskStore
- `crates/pmcp-tasks/src/lib.rs` - Added pub mod router and pub use TaskRouterImpl re-export
- `src/server/core.rs` - Task routing in handle_request_internal, resolve_task_owner helper, task-augmented CallTool interception

## Decisions Made
- TaskRouterImpl stores tool context (name, args, progressToken) as task variables for external service pickup, consistent with CONTEXT.md locked decision that handlers return immediately
- Task-augmented call interception placed in handle_request_internal before handle_call_tool, returning CreateTaskResult as Value directly via success_response, avoiding the CallToolResult type mismatch
- Error conversion uses TaskError::error_code() (-32602 for client errors, -32603 for internal) mapped to pmcp::Error::invalid_params or pmcp::Error::internal

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All task routing is functional: create, get, list, cancel, result
- Ready for Plan 03: Integration tests exercising the full path through ServerCore with TaskRouter
- TaskRouterImpl can be used by examples and integration tests

## Self-Check: PASSED

All files verified present. All commit hashes verified in git log.

---
*Phase: 03-handler-middleware-and-server-integration*
*Completed: 2026-02-22*
