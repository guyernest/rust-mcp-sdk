---
phase: 06-structured-handoff-and-client-continuation
plan: 01
subsystem: workflow
tags: [handoff, prompt, workflow, mcp, task-prompt-handler]

# Dependency graph
requires:
  - phase: 05-partial-execution-engine
    provides: "TaskWorkflowPromptHandler with active step loop, PauseReason, StepStatus, ExecutionContext"
provides:
  - "build_handoff_message method for narrating paused workflow state"
  - "build_placeholder_args helper for unresolvable step arguments"
  - "Handoff message integration in handle() when pause_reason is present"
affects: [06-structured-handoff-and-client-continuation, 07-integration-wiring]

# Tech tracking
tech-stack:
  added: []
  patterns: [hybrid-handoff-format, placeholder-argument-syntax]

key-files:
  created: []
  modified:
    - src/server/workflow/task_prompt_handler.rs

key-decisions:
  - "build_placeholder_args is a static method (does not need &self since it only reads step metadata and args)"
  - "Retryable failed steps appear as first item in remaining steps list, followed by pending steps"
  - "DataSource::Field variant mentioned in plan does not exist; adapted to DataSource::StepOutput with field: Some(f)"

patterns-established:
  - "Handoff message format: Section 1 (what happened) + Section 2 (remaining steps with tool/args/guidance)"
  - "Placeholder syntax: <output from {binding}>, <field '{f}' from {binding}>, <prompt arg {name}>"

requirements-completed: [HAND-01, HAND-02, HAND-03]

# Metrics
duration: 3min
completed: 2026-02-23
---

# Phase 6 Plan 1: Structured Handoff Message Generation Summary

**Handoff message generation with PauseReason narration, placeholder argument resolution, and guidance text for paused workflow prompts**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-23T05:11:53Z
- **Completed:** 2026-02-23T05:15:22Z
- **Tasks:** 1
- **Files modified:** 3 (1 primary + 2 formatting cleanup)

## Accomplishments
- Added build_handoff_message that produces a final assistant message narrating what happened and what steps remain, covering all 4 PauseReason variants
- Added build_placeholder_args for argument maps with placeholder syntax when step outputs cannot be resolved
- Integrated handoff message into handle() so it appears as the last assistant message before _meta when execution pauses
- Retryable failed steps are included as the first item in the remaining steps list
- Task ID is never mentioned in narrative text (only in _meta) per locked decision
- 7 comprehensive unit tests covering all variants, no-task-id invariant, placeholders, and guidance

## Task Commits

Each task was committed atomically:

1. **Task 1: Handoff message generation with argument resolution and placeholder fallback** - `033f34d` (feat)

**Plan metadata:** [pending final commit]

## Files Created/Modified
- `src/server/workflow/task_prompt_handler.rs` - Added build_handoff_message, build_placeholder_args, handle() integration, 7 new tests
- `src/server/builder.rs` - Formatting cleanup via cargo fmt
- `src/server/workflow/prompt_handler.rs` - Formatting cleanup via cargo fmt

## Decisions Made
- Made build_placeholder_args a static method since it only needs step metadata and args, not the full handler instance
- Retryable failed steps appear first in the remaining steps list (before pending steps) to guide the client to retry immediately
- Adapted plan's DataSource::Field { source, field } to actual DataSource::StepOutput { step, field: Some(f) } since the Field variant does not exist in the codebase

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] DataSource::Field variant does not exist**
- **Found during:** Task 1 (build_placeholder_args implementation)
- **Issue:** Plan referenced DataSource::Field { source, field } but the actual enum only has PromptArg, StepOutput (with optional field), and Constant
- **Fix:** Used DataSource::StepOutput { step: binding, field: Some(f) } which provides equivalent functionality
- **Files modified:** src/server/workflow/task_prompt_handler.rs
- **Verification:** Tests pass, placeholder_args_step_output test validates field extraction placeholders
- **Committed in:** 033f34d (Task 1 commit)

**2. [Rule 1 - Bug] Unreachable wildcard pattern warning**
- **Found during:** Task 1 (initial compilation)
- **Issue:** Wildcard `_` arm in match on DataSource was unreachable because all variants are covered within the same crate (despite #[non_exhaustive])
- **Fix:** Removed the unreachable wildcard arm to satisfy zero-warnings clippy gate
- **Files modified:** src/server/workflow/task_prompt_handler.rs
- **Verification:** cargo clippy -- -D warnings passes with zero warnings
- **Committed in:** 033f34d (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both auto-fixes necessary for correctness. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Handoff message generation complete, ready for client continuation (Plan 2: tool-to-task reconnection)
- The hybrid format is now functional: _meta JSON for programmatic clients + narrative assistant message for LLM clients

## Self-Check: PASSED

- FOUND: src/server/workflow/task_prompt_handler.rs
- FOUND: commit 033f34d
- FOUND: 06-01-SUMMARY.md

---
*Phase: 06-structured-handoff-and-client-continuation*
*Completed: 2026-02-23*
