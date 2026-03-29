---
phase: 02-engine-core
plan: 01
subsystem: loadtest
tags: [tokio, async, mpsc, watch, cancellation-token, task-tracker, hdrhistogram, weighted-random]

# Dependency graph
requires:
  - phase: 01-foundation
    provides: McpClient, MetricsRecorder, LoadTestConfig, McpError, RequestSample
provides:
  - LoadTestEngine orchestrator with run() returning LoadTestResult
  - VU task loop with weighted-random step selection and respawn logic
  - Metrics aggregator with dual-recorder pattern (live + report)
  - ActiveVuCounter for live VU tracking
  - Graceful shutdown via CancellationToken + TaskTracker
  - Ctrl+C handler with two-phase shutdown (graceful then hard abort)
affects: [02-02-live-display, 02-03-integration-test, 03-cli-reports]

# Tech tracking
tech-stack:
  added: [tokio-util 0.7 (rt feature)]
  patterns: [CancellationToken for graceful shutdown, TaskTracker for VU drain, biased select for display starvation prevention, dual-recorder for ramp-up exclusion, StdRng for Send-safe weighted random]

key-files:
  created:
    - src/loadtest/engine.rs
    - src/loadtest/vu.rs
  modified:
    - src/loadtest/error.rs
    - src/loadtest/mod.rs
    - Cargo.toml

key-decisions:
  - "StdRng::from_rng instead of ThreadRng -- ThreadRng is not Send, required for TaskTracker::spawn"
  - "Dual-recorder pattern: live recorder for display, report recorder excludes ramp-up for final snapshot"
  - "biased select in aggregator: tick branch first to prevent display starvation under high mpsc throughput"
  - "mpsc buffer = vu_count * 100 -- generous to avoid backpressure, per research"
  - "run() is self-contained -- Plan 02-02 will modify to spawn display task with watch receiver"

patterns-established:
  - "CancellationToken + TaskTracker: standard graceful shutdown pattern for all concurrent tasks"
  - "mpsc(RequestSample) -> aggregator -> watch(MetricsSnapshot): one-way metrics pipeline"
  - "try_initialize with backoff: reusable respawn logic for session recovery"

requirements-completed: [LOAD-01, LOAD-02, METR-02]

# Metrics
duration: 8min
completed: 2026-02-26
---

# Phase 2 Plan 1: Engine Core Summary

**Concurrent VU engine with CancellationToken/TaskTracker shutdown, dual-recorder metrics aggregation, and weighted-random step selection via StdRng**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-26T23:29:32Z
- **Completed:** 2026-02-26T23:37:32Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- VU task loop with weighted-random step selection, exponential backoff respawn (500ms base, x2, +/-25% jitter, max 3 attempts)
- LoadTestEngine orchestrator with builder pattern, duration/iteration first-limit-wins run control
- Metrics aggregator with biased select!, dual-recorder pattern excluding ramp-up from final report
- Full graceful shutdown: CancellationToken propagation + TaskTracker drain-wait + two-phase Ctrl+C
- 14 new unit tests (10 VU + 4 engine), all passing

## Task Commits

Each task was committed atomically:

1. **Task 1: Add tokio-util and create vu.rs with VU task loop** - `adee6af` (feat)
2. **Task 2: Create engine.rs with LoadTestEngine orchestrator** - `a3b0d58` (feat)

## Files Created/Modified
- `src/loadtest/vu.rs` - VU task loop, ActiveVuCounter, execute_step, respawn_with_backoff, step_to_operation_type
- `src/loadtest/engine.rs` - LoadTestEngine, LoadTestResult, metrics_aggregator, handle_ctrl_c
- `src/loadtest/error.rs` - Added Engine variant to LoadTestError
- `src/loadtest/mod.rs` - Added engine and vu module declarations
- `Cargo.toml` - Added tokio-util = { version = "0.7", features = ["rt"] }

## Decisions Made
- Used StdRng::from_rng instead of ThreadRng because ThreadRng (Rc-based) is not Send, which is required by TaskTracker::spawn
- Dual-recorder pattern in metrics aggregator: live recorder for display snapshots, report recorder excludes ramp-up samples for final result
- biased select! in aggregator ensures tick branch gets priority, preventing display starvation under high throughput
- run() is fully self-contained for now; Plan 02-02 will refactor to spawn a display task using the internal watch receiver

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] ThreadRng not Send -- switched to StdRng**
- **Found during:** Task 2 (engine.rs compilation)
- **Issue:** rand::rng() returns ThreadRng which uses Rc<UnsafeCell> internally, not Send. TaskTracker::spawn requires Send futures.
- **Fix:** Changed to rand::rngs::StdRng::from_rng(&mut rand::rng()) which is Send-safe
- **Files modified:** src/loadtest/vu.rs
- **Verification:** cargo check succeeds, all tests pass
- **Committed in:** a3b0d58 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary for compilation. StdRng has identical quality to ThreadRng for load testing purposes. No scope creep.

## Issues Encountered
- Pre-existing clippy errors in bin target (main.rs) and sibling crate mcp-tester -- out of scope per deviation scope boundary rule. Lib target has zero clippy warnings.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Engine is fully functional -- ready for Plan 02-02 (live display with indicatif)
- watch channel inside run() provides MetricsSnapshot feed for display task integration
- ActiveVuCounter exposed for live VU count display
- Plan 02-02 will need to refactor run() to spawn display task before controller starts

---
*Phase: 02-engine-core*
*Completed: 2026-02-26*
