---
phase: 04-load-shaping-and-tool-metrics
plan: 02
subsystem: metrics
tags: [hdrhistogram, per-tool, terminal-summary, json-report, schema-versioning]

requires:
  - phase: 04-load-shaping-and-tool-metrics
    provides: "Load shaping stages, MetricsRecorder, MetricsSnapshot, RequestSample pipeline"
provides:
  - "ToolMetrics and ToolSnapshot types for per-tool HdrHistogram tracking"
  - "Per-tool terminal table in summary output (tool, reqs, rate, err%, P50, P95, P99)"
  - "Per-tool JSON report object with extended latency and error detail"
  - "Schema version 1.1 (backwards compatible additive fields)"
affects: [04-03-breaking-point-detection]

tech-stack:
  added: []
  patterns: ["per-tool HdrHistogram routing in single-threaded recorder", "tool_name extraction from ScenarioStep variant fields"]

key-files:
  created: []
  modified:
    - "src/loadtest/metrics.rs"
    - "src/loadtest/vu.rs"
    - "src/loadtest/summary.rs"
    - "src/loadtest/report.rs"
    - "src/loadtest/engine.rs"
    - "src/loadtest/display.rs"
    - "tests/engine_property_tests.rs"
    - "fuzz/fuzz_targets/fuzz_metrics_record.rs"
    - "examples/loadtest_demo.rs"

key-decisions:
  - "Per-tool percentiles from success histogram only (primary latency view), min/max/mean across both"
  - "Tool display name is the raw tool_name (not prefixed with operation type) -- tool names, URIs, and prompt names are inherently distinct"
  - "Schema version 1.1 (additive per_tool field, backwards compatible with 1.0 consumers)"
  - "Per-tool recording happens inside single-threaded MetricsRecorder via tool_name on RequestSample, not shared state in VU tasks"

patterns-established:
  - "tool_name: Option<String> on RequestSample for optional per-tool metrics routing"
  - "ToolMetrics private struct in metrics.rs with same HdrHistogram config as main recorder"
  - "ToolSnapshot public struct sorted alphabetically for deterministic output"

requirements-completed: [MCP-02]

duration: 8min
completed: 2026-02-27
---

# Phase 4 Plan 2: Per-Tool Metrics Summary

**Per-tool HdrHistogram tracking with terminal table and JSON report extension at schema version 1.1**

## Performance

- **Duration:** 8 min
- **Started:** 2026-02-27T05:37:26Z
- **Completed:** 2026-02-27T05:46:03Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments
- Extended RequestSample with tool_name field and per-tool HdrHistogram routing in MetricsRecorder
- Added k6-style per-tool terminal table showing tool name, request count, rate, error%, P50/P95/P99 with color coding
- Added ToolReportMetrics in JSON report with full latency stats and per-tool error breakdown
- Bumped schema version to 1.1 with backwards-compatible additive per_tool field
- All 103 lib tests pass, zero clippy warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Add per-tool metrics types and recording** - `d848697` (feat)
2. **Task 2: Per-tool terminal table and JSON report extension** - `dd2476b` (feat)

## Files Created/Modified
- `src/loadtest/metrics.rs` - ToolMetrics, ToolSnapshot structs; per_tool HashMap in MetricsRecorder; per_tool Vec in MetricsSnapshot
- `src/loadtest/vu.rs` - tool_name extraction from ScenarioStep and passing to RequestSample constructors
- `src/loadtest/summary.rs` - Per-tool metrics table section with color-coded columns
- `src/loadtest/report.rs` - ToolReportMetrics, ToolLatencyMetrics structs; per_tool in LoadTestReport; schema 1.1
- `src/loadtest/engine.rs` - Updated RequestSample call sites for new signature
- `src/loadtest/display.rs` - Updated MetricsSnapshot construction with per_tool field
- `tests/engine_property_tests.rs` - Updated RequestSample call sites
- `fuzz/fuzz_targets/fuzz_metrics_record.rs` - Updated RequestSample call sites
- `examples/loadtest_demo.rs` - Updated RequestSample call sites

## Decisions Made
- Per-tool percentiles from success histogram only (error latencies are in separate histograms); min/max/mean combine both histograms for overall tool view
- Tool display name in terminal table is the raw tool_name without operation type prefix -- tool names, URIs, and prompt names are inherently distinct
- Schema version bumped from 1.0 to 1.1 (additive per_tool field, backwards compatible for existing 1.0 consumers)
- Per-tool recording happens inside single-threaded MetricsRecorder via tool_name on RequestSample -- no shared mutable state in VU tasks

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Per-tool metrics pipeline complete from VU sample creation through terminal and JSON output
- Ready for Phase 4 Plan 3: breaking point detection algorithm
- All 9 must-have truths from plan satisfied

---
*Phase: 04-load-shaping-and-tool-metrics*
*Completed: 2026-02-27*
