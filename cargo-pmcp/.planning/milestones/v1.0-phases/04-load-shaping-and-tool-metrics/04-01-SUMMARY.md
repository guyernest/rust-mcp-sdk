---
phase: 04-load-shaping-and-tool-metrics
plan: 01
subsystem: loadtest
tags: [stages, load-shaping, ramp-up, ramp-down, cancellation-token, toml, k6]

requires:
  - phase: 02-engine-core
    provides: "LoadTestEngine with VU spawning, metrics aggregation, watch channel display"
  - phase: 03-cli-and-reports
    provides: "CLI loadtest run command, terminal summary, JSON reports"
provides:
  - "Stage struct with target_vus and duration_secs for TOML [[stage]] blocks"
  - "Stage-driven scheduler loop with per-VU child_token() cancellation"
  - "DisplayState wrapper carrying MetricsSnapshot + stage_label through watch channel"
  - "Stage-aware live terminal display with [stage N/M] prefix"
  - "Backwards-compatible flat load path preserved when no stages defined"
affects: [04-02, 04-03, load-shaping, breaking-point-detection]

tech-stack:
  added: []
  patterns:
    - "child_token() per-VU cancellation for selective ramp-down"
    - "LIFO VU shutdown order (last spawned, first killed)"
    - "Arc<Mutex<Option<String>>> for shared stage label between scheduler and aggregator"
    - "DisplayState wrapper pattern for enriching watch channel payload"

key-files:
  created: []
  modified:
    - src/loadtest/config.rs
    - src/loadtest/engine.rs
    - src/loadtest/display.rs
    - src/commands/loadtest/run.rs
    - tests/property_tests.rs

key-decisions:
  - "DisplayState wraps MetricsSnapshot + stage_label rather than replacing the watch channel type directly"
  - "Stage scheduler uses Arc<Mutex> for stage label sharing (lightweight, lock held briefly)"
  - "ramp_up_end = test_start for staged mode (all stage data counts in report since ramp is part of defined test shape)"
  - "Safety timeout = effective_duration_secs + 30s as absolute ceiling"
  - "Separate metrics_aggregator_with_label for staged mode to read dynamic label from shared mutex"

patterns-established:
  - "run_flat() vs run_staged() branching in engine.run() for mode isolation"
  - "Per-VU CancellationToken via child_token() for selective shutdown"
  - "LIFO ramp-down: pop from vu_tokens Vec to cancel most-recently-spawned VU first"

requirements-completed: [LOAD-04]

duration: 7min
completed: 2026-02-27
---

# Phase 4 Plan 1: Load Shaping Stages Summary

**Composable [[stage]] blocks in TOML config with stage-driven VU scheduler using per-VU child_token() cancellation for linear ramp-up/ramp-down load profiles**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-27T05:26:38Z
- **Completed:** 2026-02-27T05:33:38Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Stage config type with TOML [[stage]] array-of-tables parsing and validation
- Stage-driven scheduler loop that ramps VU count linearly through stages with per-VU cancellation
- Live terminal display shows [stage N/M] prefix during staged execution
- Full backwards compatibility: configs without stages behave identically to Phase 3

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Stage config type and extend LoadTestConfig** - `e3d0c81` (feat)
2. **Task 2: Implement stage-driven scheduler with per-VU cancellation** - `7b4ec40` (feat)

## Files Created/Modified
- `src/loadtest/config.rs` - Stage struct, stage field on LoadTestConfig, validate() extension, helpers (has_stages, total_stage_duration, effective_duration_secs)
- `src/loadtest/engine.rs` - DisplayState struct, run_flat()/run_staged() branching, stage scheduler loop, metrics_aggregator_with_label
- `src/loadtest/display.rs` - DisplayState-based watch channel, format_status with stage_label parameter, display_loop accepts DisplayState
- `src/commands/loadtest/run.rs` - apply_overrides skips --vus when stages present with warning
- `src/loadtest/summary.rs` - Updated LoadTestConfig literal with stage field
- `src/loadtest/report.rs` - Updated LoadTestConfig literal with stage field
- `tests/property_tests.rs` - Updated LoadTestConfig literals with stage field

## Decisions Made
- DisplayState wraps MetricsSnapshot + stage_label rather than replacing watch channel type -- cleaner separation, display reads both from single struct
- Stage scheduler uses Arc<Mutex<Option<String>>> for stage label sharing between scheduler task and aggregator task -- mutex held only briefly for clone, no contention risk
- ramp_up_end = test_start for staged mode because all stage data is part of the defined test shape (user explicitly defined the ramp as part of their profile)
- Safety timeout is effective_duration_secs + 30s to prevent runaway tests if stage timing drifts
- Separate metrics_aggregator_with_label function for staged mode reads dynamic label from shared mutex (vs flat mode which uses static None)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Stage config parsing and engine scheduler fully functional for Plan 04-02 (per-tool metrics) and 04-03 (breaking point detection)
- DisplayState wrapper pattern established for future enrichments
- All 207 tests pass with zero clippy warnings

---
*Phase: 04-load-shaping-and-tool-metrics*
*Completed: 2026-02-27*
