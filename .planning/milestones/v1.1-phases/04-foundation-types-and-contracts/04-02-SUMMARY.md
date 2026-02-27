---
phase: 04-foundation-types-and-contracts
plan: 02
subsystem: workflow
tags: [composition, delegation, task-router, prompt-handler, opt-in]

# Dependency graph
requires:
  - phase: 04-foundation-types-and-contracts
    provides: "WorkflowProgress types, TaskRouter workflow methods, GetPromptResult._meta"
provides:
  - "TaskWorkflowPromptHandler composing with WorkflowPromptHandler via delegation"
  - "SequentialWorkflow.with_task_support(true) opt-in mechanism"
  - "Builder wiring that wraps opted-in workflows in TaskWorkflowPromptHandler"
  - "Step-status inference from message trace (infer_step_statuses)"
  - "Minimal example demonstrating task workflow opt-in API"
affects: [05-execution-engine, 06-handoff-and-continuation, 07-integration-and-testing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Composition over modification: TaskWorkflowPromptHandler wraps WorkflowPromptHandler"
    - "Graceful degradation: task creation failure logs warning, returns inner result without _meta"
    - "Opt-in via builder pattern: .with_task_support(true) on SequentialWorkflow"
    - "Build-time fail-fast: task support enabled without task router produces error"

key-files:
  created:
    - "src/server/workflow/task_prompt_handler.rs"
    - "examples/62_task_workflow_opt_in.rs"
  modified:
    - "src/server/workflow/mod.rs"
    - "src/server/workflow/sequential.rs"
    - "src/server/builder.rs"

key-decisions:
  - "Example numbered 62 instead of 61 (plan specified 61 but 61_observability_middleware.rs already exists)"
  - "Step-status inference uses assistant/user message pair counting, skipping first 2 header messages"
  - "Owner resolution delegates to TaskRouter.resolve_owner matching ServerCore pattern"

patterns-established:
  - "Task-aware prompt composition: new handler wraps existing handler, no modifications to inner"
  - "Opt-in field with builder method: field defaults false, .with_task_support(true) sets it"
  - "Build-time validation: task_support=true + no task_router = error at builder.prompt_workflow()"

requirements-completed: [FNDX-01, FNDX-05]

# Metrics
duration: 9min
completed: 2026-02-22
---

# Phase 4 Plan 2: TaskWorkflowPromptHandler Composition and Opt-In Summary

**TaskWorkflowPromptHandler composing with WorkflowPromptHandler via delegation, with SequentialWorkflow.with_task_support(true) opt-in and builder wiring**

## Performance

- **Duration:** 9 min
- **Started:** 2026-02-22T21:44:43Z
- **Completed:** 2026-02-22T21:53:59Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- TaskWorkflowPromptHandler struct composes with WorkflowPromptHandler via delegation (FNDX-05)
- Creates task on workflow invocation, enriches GetPromptResult with _meta containing task state
- SequentialWorkflow.with_task_support(true) opt-in with builder wiring
- Build-time error when task support enabled but no task router configured
- 16 new tests (9 for task_prompt_handler, 4 for sequential, 3 for builder)
- Minimal example (62_task_workflow_opt_in.rs) demonstrates and runs the API

## Task Commits

Each task was committed atomically:

1. **Task 1: TaskWorkflowPromptHandler with delegation composition** - `ee1c509` (feat)
2. **Task 2: Task support opt-in, builder wiring, and minimal example** - `bced472` (feat)

## Files Created/Modified
- `src/server/workflow/task_prompt_handler.rs` - TaskWorkflowPromptHandler struct with delegation, helpers, 9 unit tests
- `src/server/workflow/mod.rs` - Added task_prompt_handler module and re-export
- `src/server/workflow/sequential.rs` - task_support field, with_task_support() builder method, has_task_support() accessor, 4 tests
- `src/server/builder.rs` - prompt_workflow() wraps opted-in workflows in TaskWorkflowPromptHandler, 3 new builder tests
- `examples/62_task_workflow_opt_in.rs` - Minimal example demonstrating opt-in API through builder

## Decisions Made
- **Example numbering**: Used 62 instead of 61 because `61_observability_middleware.rs` already exists
- **Step-status inference**: Counts assistant/user message pairs after skipping first 2 header messages (user-intent + assistant-plan). This is heuristic but matches WorkflowPromptHandler's message structure.
- **Owner resolution**: Delegates to TaskRouter.resolve_owner using the same pattern as ServerCore.resolve_task_owner for consistency
- **Graceful degradation**: If task creation fails, logs warning and returns inner handler's result without _meta (workflow still works)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Example numbered 62 instead of 61**
- **Found during:** Task 2 (Create minimal example)
- **Issue:** Plan specified `examples/61_task_workflow_opt_in.rs` but `61_observability_middleware.rs` already exists
- **Fix:** Created as `examples/62_task_workflow_opt_in.rs`
- **Files modified:** examples/62_task_workflow_opt_in.rs
- **Verification:** `cargo check --example 62_task_workflow_opt_in` passes
- **Committed in:** bced472 (Task 2 commit)

**2. [Rule 1 - Bug] Fixed unwrap_err() on non-Debug type in builder test**
- **Found during:** Task 2 (Builder tests)
- **Issue:** `Result.unwrap_err()` requires T: Debug, but ServerCoreBuilder has `#[allow(missing_debug_implementations)]`
- **Fix:** Used match expression instead of `unwrap_err()`
- **Files modified:** src/server/builder.rs
- **Verification:** `cargo test --package pmcp --lib` passes with all 686 tests
- **Committed in:** bced472 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug fix)
**Impact on plan:** Minor mechanical fixes. No scope creep.

## Issues Encountered
- Pre-existing doctest failures in `preset.rs` and `middleware_presets.rs` (missing `streamable-http` feature). Not related to our changes.
- Pre-existing property test failure in `pmcp-tasks::property_tests::fresh_task_record_is_not_expired` (TTL overflow). Documented in 04-01-SUMMARY.md.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- TaskWorkflowPromptHandler ready for execution engine integration (Phase 5)
- SequentialWorkflow opt-in mechanism available for workflow authors
- Builder wiring complete -- workflows with task support are automatically wrapped
- WorkflowPromptHandler remains unmodified (zero behavioral change for existing workflows)
- All 686 lib tests pass (16 new, 670 existing unchanged)

## Self-Check: PASSED

All files exist, all commits verified:
- ee1c509: Task 1 (TaskWorkflowPromptHandler)
- bced472: Task 2 (opt-in, builder wiring, example)

---
*Phase: 04-foundation-types-and-contracts*
*Completed: 2026-02-22*
