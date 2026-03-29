---
phase: 01-foundation
plan: 03
subsystem: metrics
tags: [hdrhistogram, coordinated-omission, latency, percentiles, metrics-pipeline]

# Dependency graph
requires:
  - phase: 01-foundation/01-01
    provides: "loadtest module structure, OperationType enum stub, McpError type, hdrhistogram dependency"
provides:
  - "MetricsRecorder with HdrHistogram-backed latency recording"
  - "Coordinated omission correction via record_correct()"
  - "Separate success/error histogram buckets"
  - "Per-operation-type request counting"
  - "MetricsSnapshot for point-in-time capture"
  - "RequestSample convenience constructors"
affects: [01-foundation/01-04, 02-engine-core]

# Tech tracking
tech-stack:
  added: [hdrhistogram (record_correct API)]
  patterns: [single-owner metrics recorder, separate success/error histograms, coordinated omission correction at record time]

key-files:
  created: []
  modified: [src/loadtest/metrics.rs]

key-decisions:
  - "Histogram::new(3) with auto-resize instead of new_with_bounds -- avoids silent recording failures on outlier values"
  - "success_count()/error_count() return histogram len (includes synthetic fills) for accurate percentile denominators"
  - "operation_counts use logical counts (one per record() call) not histogram entries for business-level counting"

patterns-established:
  - "TDD RED-GREEN-REFACTOR: write failing tests first, implement to pass, clean up with clippy/docs"
  - "Single-owner metrics: no Arc<Mutex>, designed for mpsc channel aggregation in Phase 2"
  - "Coordinated omission correction at record time via record_correct() -- never retrofittable"

requirements-completed: [METR-01]

# Metrics
duration: 5min
completed: 2026-02-26
---

# Phase 1 Plan 3: HdrHistogram Metrics Pipeline Summary

**HdrHistogram-based MetricsRecorder with coordinated omission correction, separate success/error buckets, and per-operation-type tracking**

## Performance

- **Duration:** 5 min
- **Started:** 2026-02-26T22:15:33Z
- **Completed:** 2026-02-26T22:20:33Z
- **Tasks:** 1 (TDD: 3 commits for RED/GREEN/REFACTOR)
- **Files modified:** 1

## Accomplishments
- MetricsRecorder with HdrHistogram recording and accurate P50/P95/P99 percentile extraction
- Coordinated omission correction baked in from day one via record_correct() -- not retrofittable
- Separate success and error histogram buckets preventing error latency pollution
- Per-operation-type counting (ToolsCall, ResourcesRead, PromptsGet, Initialize, etc.)
- MetricsSnapshot for thread-safe point-in-time capture
- 12 comprehensive tests proving percentile accuracy and coordinated omission correction

## Task Commits

Each task was committed atomically (TDD phases):

1. **Task 1 RED: Failing tests** - `4450a37` (test)
2. **Task 1 GREEN: Full implementation** - `cba0636` (feat)
3. **Task 1 REFACTOR: Clippy fixes + rustdoc** - `ef646dc` (refactor)

## Files Created/Modified
- `src/loadtest/metrics.rs` - MetricsRecorder, RequestSample, MetricsSnapshot, OperationType with full HdrHistogram integration

## Decisions Made
- Used `Histogram::new(3)` with `auto(true)` instead of `new_with_bounds()` to avoid silent recording failures on outlier values that exceed the max bound
- `success_count()` and `error_count()` return histogram len (which includes coordinated omission synthetic fills) rather than logical counters, giving accurate denominators for percentile calculations
- `operation_count()` uses separate logical HashMap counters (not histogram len) so business-level per-operation counts are not inflated by correction

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing clippy warnings in sibling crate `mcp-tester` prevent `cargo clippy -D warnings` from passing at workspace level. These are out of scope -- verified cargo-pmcp lib code has zero warnings by filtering output to our files.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- MetricsRecorder API is ready for Phase 2's mpsc channel aggregation pattern
- Single-owner design means worker tasks will send RequestSample via channels to a dedicated recorder thread
- Plan 01-04 (MCP client) can use RequestSample to wrap timing measurements

## Self-Check: PASSED

- FOUND: src/loadtest/metrics.rs
- FOUND: 4450a37 (RED commit)
- FOUND: cba0636 (GREEN commit)
- FOUND: ef646dc (REFACTOR commit)
- FOUND: 01-03-SUMMARY.md

---
*Phase: 01-foundation*
*Completed: 2026-02-26*
