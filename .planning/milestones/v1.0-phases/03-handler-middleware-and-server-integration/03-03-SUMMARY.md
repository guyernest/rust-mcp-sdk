---
phase: 03-handler-middleware-and-server-integration
plan: 03
subsystem: testing
tags: [integration-tests, example, task-lifecycle, servercore, tokio-spawn]

# Dependency graph
requires:
  - phase: 03-01
    provides: TaskRouter trait, ClientRequest task variants, with_task_store builder method
  - phase: 03-02
    provides: TaskRouterImpl, task routing in ServerCore handle_request_internal
provides:
  - Full lifecycle integration tests through ServerCore::handle_request()
  - Basic tasks example (60_tasks_basic.rs) demonstrating create-poll-complete flow
  - pmcp-tasks as dev-dependency in root Cargo.toml for task examples
affects: [phase-04-dynamodb, phase-05-workflow]

# Tech tracking
tech-stack:
  added: [pmcp-tasks dev-dep in root Cargo.toml]
  patterns: [integration-test-via-ServerCore-handle_request, tokio-spawn-background-simulation]

key-files:
  created:
    - crates/pmcp-tasks/tests/lifecycle_integration.rs
    - examples/60_tasks_basic.rs
  modified:
    - crates/pmcp-tasks/Cargo.toml
    - Cargo.toml

key-decisions:
  - "Integration tests use stateless_mode(true) to skip initialize handshake for simpler test setup"
  - "Tests manipulate store directly via Arc<InMemoryTaskStore> to simulate background completion"
  - "Example uses tokio::spawn + sleep per CONTEXT.md locked decision for background simulation"

patterns-established:
  - "Integration test pattern: build_task_server() returns (ServerCore, Arc<InMemoryTaskStore>) for both request handling and direct store access"
  - "Example pattern: 60_tasks_basic demonstrates full lifecycle with educational println! output"

requirements-completed: [TEST-08, EXMP-01]

# Metrics
duration: 4min
completed: 2026-02-22
---

# Phase 3 Plan 3: Integration Tests and Basic Example Summary

**11 integration tests through ServerCore::handle_request() plus working example demonstrating complete task create-poll-complete lifecycle**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-22T04:12:55Z
- **Completed:** 2026-02-22T04:17:27Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- 11 integration tests covering full lifecycle, list, cancel, auto-task, normal-tool, TTL, METHOD_NOT_FOUND, and tool context variables
- Basic tasks example (60_tasks_basic.rs) compiles and runs end-to-end with clear educational output
- All tests pass through real ServerCore::handle_request() path (not mocked)

## Task Commits

Each task was committed atomically:

1. **Task 1: Write full lifecycle integration tests** - `bf291f5` (test)
2. **Task 2: Create 60_tasks_basic.rs example** - `9013e00` (feat)

## Files Created/Modified
- `crates/pmcp-tasks/tests/lifecycle_integration.rs` - 11 integration tests: full lifecycle, list, cancel, auto-task, normal tool, TTL, errors
- `examples/60_tasks_basic.rs` - Basic task-augmented tool call example demonstrating complete lifecycle
- `crates/pmcp-tasks/Cargo.toml` - Added pmcp as dev-dependency with full features, added async-trait
- `Cargo.toml` - Added pmcp-tasks dev-dependency and [[example]] entry for 60_tasks_basic

## Decisions Made
- Integration tests use `stateless_mode(true)` to skip initialize handshake, simplifying test setup
- Tests hold `Arc<InMemoryTaskStore>` alongside `ServerCore` to directly simulate background task completion
- Example follows CONTEXT.md locked decision: tokio::spawn + sleep for background work simulation
- 11 tests total (exceeding the 5 minimum): added tests for empty list, METHOD_NOT_FOUND, and tool context variables

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 3 complete: all 3 plans executed (TaskRouter trait, TaskRouterImpl + routing, integration tests + example)
- Task system is end-to-end functional with InMemoryTaskStore
- Ready for Phase 4 (DynamoDB backend) and Phase 5 (workflow integration)

## Self-Check: PASSED

All files verified present, all commits verified in git history.

---
*Phase: 03-handler-middleware-and-server-integration*
*Completed: 2026-02-22*
