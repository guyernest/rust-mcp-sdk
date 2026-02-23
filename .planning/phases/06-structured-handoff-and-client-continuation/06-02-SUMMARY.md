---
phase: 06-structured-handoff-and-client-continuation
plan: 02
subsystem: tasks
tags: [workflow, continuation, task-variables, fire-and-forget, cancel-with-result]

# Dependency graph
requires:
  - phase: 05-partial-execution-engine
    provides: "WorkflowProgress, StepStatus, WORKFLOW_PROGRESS_KEY, WORKFLOW_RESULT_PREFIX, PauseReason types"
  - phase: 06-structured-handoff-and-client-continuation
    plan: 01
    provides: "Handoff format with _meta containing workflow progress and task_id"
provides:
  - "_task_id field on RequestMeta for tool-to-task reconnection"
  - "handle_workflow_continuation on TaskRouter trait and TaskRouterImpl"
  - "Fire-and-forget intercept in ServerCore CallTool path"
  - "WORKFLOW_EXTRA_PREFIX and workflow_extra_key for unmatched tool observability"
  - "Cancel-with-result completion path on tasks/cancel endpoint"
affects: [07-end-to-end-integration]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Fire-and-forget recording: tool result returned regardless of continuation success"
    - "Step matching: first pending/failed step with matching tool name wins"
    - "Cancel-as-completion: tasks/cancel with result field transitions to Completed"

key-files:
  created: []
  modified:
    - src/types/protocol.rs
    - src/server/core.rs
    - src/server/tasks.rs
    - crates/pmcp-tasks/src/router.rs
    - crates/pmcp-tasks/src/types/params.rs
    - crates/pmcp-tasks/src/types/workflow.rs
    - crates/pmcp-tasks/tests/protocol_types.rs

key-decisions:
  - "Fire-and-forget pattern: tool call always succeeds, continuation recording logged on failure"
  - "First-match-wins for step matching: scans steps in order, first pending/failed with matching tool name is selected"
  - "Completed steps not re-matchable: retrying a completed step routes to _workflow.extra instead"
  - "Cancel-with-result uses existing complete_with_result store method for atomic status transition"
  - "Pause reason cleared on any continuation call since client is making progress"

patterns-established:
  - "Fire-and-forget intercept: extract context before handler, act on result without blocking response"
  - "WORKFLOW_EXTRA_PREFIX for observability of unmatched tool calls"
  - "Cancel-as-completion: dual-purpose endpoint via optional result field"

requirements-completed: [CONT-01, CONT-02, CONT-03]

# Metrics
duration: 7min
completed: 2026-02-23
---

# Phase 6 Plan 02: Client Continuation Summary

**Tool-to-task reconnection with _task_id on RequestMeta, fire-and-forget step matching in ServerCore, and cancel-with-result workflow completion**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-23T05:11:58Z
- **Completed:** 2026-02-23T05:19:32Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Added `_task_id` field to `RequestMeta` enabling clients to reconnect tool calls to workflow tasks
- Implemented fire-and-forget intercept in `ServerCore` that records tool results against workflows without blocking the tool response
- Built step-matching logic in `TaskRouterImpl::handle_workflow_continuation` that matches tool names to pending/failed steps and updates progress
- Extended `tasks/cancel` with optional result field for workflow completion (transitions to Completed, not Cancelled)
- Added `WORKFLOW_EXTRA_PREFIX` and `workflow_extra_key` for observability of unmatched tool calls

## Task Commits

Each task was committed atomically:

1. **Task 1: RequestMeta _task_id field, TaskRouter continuation trait method, and WORKFLOW_EXTRA_PREFIX constant** - `16d1a4e` (feat)
2. **Task 2: ServerCore intercept, TaskRouterImpl continuation implementation, and cancel-with-result** - `d745155` (feat)

## Files Created/Modified
- `src/types/protocol.rs` - Added `_task_id: Option<String>` to RequestMeta with `serde(rename = "_task_id")` for correct JSON serialization
- `src/server/tasks.rs` - Added `handle_workflow_continuation` default no-op method to TaskRouter trait
- `src/server/core.rs` - Added fire-and-forget _task_id intercept in the normal tool call path (CallTool branch)
- `crates/pmcp-tasks/src/router.rs` - Implemented `handle_workflow_continuation` with step matching, progress updates, and pause reason clearing; updated `handle_tasks_cancel` to branch on result presence
- `crates/pmcp-tasks/src/types/params.rs` - Extended `TaskCancelParams` with optional `result: Option<Value>` field
- `crates/pmcp-tasks/src/types/workflow.rs` - Added `WORKFLOW_EXTRA_PREFIX` constant and `workflow_extra_key` helper function
- `crates/pmcp-tasks/tests/protocol_types.rs` - Fixed `TaskCancelParams` construction for new result field

## Decisions Made
- **Fire-and-forget pattern:** The continuation recording never fails the tool call. If recording fails, a `tracing::warn!` is logged but the tool result is returned to the client normally. This ensures the client experience is unaffected by recording failures.
- **First-match-wins step matching:** Steps are scanned in order; the first step with status "pending" or "failed" whose tool name matches the called tool is selected. This is deterministic and handles retries (failed steps can be re-matched).
- **Completed steps not re-matchable:** Once a step is marked "completed" in progress, calling the same tool again routes to `_workflow.extra.<tool_name>` rather than overwriting the step result. This preserves the step completion record.
- **Cancel-as-completion:** Rather than adding a new `tasks/complete` endpoint, we extend `tasks/cancel` with an optional `result` field. When present, it transitions the task to `Completed` status. This minimizes API surface changes.
- **Pause reason clearing:** Every continuation call clears `_workflow.pause_reason` by setting it to `Value::Null`, since the client making tool calls implies it is making progress.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed TaskRecord field access pattern**
- **Found during:** Task 2 (TaskRouterImpl continuation implementation)
- **Issue:** Plan referenced `record.status` but TaskRecord stores status at `record.task.status`
- **Fix:** Changed to `record.task.status` in both the status check and error message
- **Files modified:** crates/pmcp-tasks/src/router.rs
- **Verification:** Compilation succeeded, all tests pass
- **Committed in:** d745155 (Task 2 commit)

**2. [Rule 3 - Blocking] Fixed missing result field in integration test**
- **Found during:** Task 2 (TaskCancelParams extension)
- **Issue:** Integration test `protocol_types.rs` constructed `TaskCancelParams` without the new `result` field, causing compilation failure
- **Fix:** Added `result: None` to existing test construction
- **Files modified:** crates/pmcp-tasks/tests/protocol_types.rs
- **Verification:** All 36 integration tests pass
- **Committed in:** d745155 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** Both auto-fixes were necessary for compilation. No scope creep.

## Issues Encountered
- Pre-existing clippy warning in `task_prompt_handler.rs` (unreachable pattern) -- out of scope, not fixed
- Pre-existing rustfmt issues in files not modified by this plan -- out of scope, only formatted files touched by this plan
- Pre-existing property test failure (`fresh_task_record_is_not_expired` with TTL overflow) -- out of scope, regression file already tracked

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Tool-to-task reconnection is complete: clients can send `_task_id` in `_meta` to record tool results against workflow tasks
- Cancel-with-result provides workflow completion: clients can signal explicit workflow completion via `tasks/cancel` with result
- Ready for Phase 7 (end-to-end integration testing) which should exercise the full handoff -> continuation -> completion flow
- The pre-existing clippy warning in `task_prompt_handler.rs` should be addressed before final release

## Self-Check: PASSED

All modified files exist on disk. Both task commits (16d1a4e, d745155) verified in git log.

---
*Phase: 06-structured-handoff-and-client-continuation*
*Completed: 2026-02-23*
