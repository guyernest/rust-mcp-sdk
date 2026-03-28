---
phase: 04-load-shaping-and-tool-metrics
verified: 2026-02-27T06:21:10Z
status: passed
score: 32/32 must-haves verified
re_verification:
  previous_status: gaps_found
  previous_score: 31/32
  gaps_closed:
    - "Live terminal shows warning when breaking point detected with actual VU count (not 0): both metrics_aggregator and metrics_aggregator_with_label now accept ActiveVuCounter parameter and call active_vus.get() in detector.observe()"
  gaps_remaining: []
  regressions: []
human_verification:
  - test: "Run a staged load test and observe terminal output when breaking point fires"
    expected: "WARNING line shows the actual VU count at time of detection, not 0"
    why_human: "Cannot programmatically verify terminal output during a live run against a real server"
  - test: "Run a 3-stage load test (target_vus=5, 10, 0 over 30/30/30 seconds)"
    expected: "Live display cycles through [stage 1/3], [stage 2/3], [stage 3/3] as each stage completes"
    why_human: "Engine stage timing is async; cannot simulate the full 90-second run in unit tests"
---

# Phase 4: Load Shaping and Tool Metrics Verification Report

**Phase Goal:** Developers can shape load with ramp-up/ramp-down phases, detect server breaking points automatically, and see per-tool performance breakdowns
**Verified:** 2026-02-27T06:21:10Z
**Status:** passed
**Re-verification:** Yes -- after gap closure (Plan 04-04)

## Goal Achievement

### Observable Truths

#### Plan 04-01: LOAD-04 Must-Haves

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | TOML config with `[[stage]]` blocks parses into Vec<Stage> with target_vus and duration_secs | VERIFIED | `Stage` struct at config.rs:68; `stage: Vec<Stage>` with `#[serde(default)]` at config.rs:96; full TOML parse test at config.rs:504 |
| 2 | Engine ramps VU count linearly over each stage duration using per-VU child cancellation tokens | VERIFIED | `run_staged()` at engine.rs:277; stage scheduler loop at engine.rs:349; `child_token()` at engine.rs:381 |
| 3 | No stages = flat load, all VUs start immediately (backwards compatible) | VERIFIED | `run_flat()` at engine.rs:136; `run()` branches on `has_stages()` at engine.rs:128 |
| 4 | Ramp-down cancels VUs in LIFO order, VU finishes current iteration before exiting | VERIFIED | `vu_tokens.pop().map(|t| t.cancel())` at engine.rs:409; VU loop checks `cancel.is_cancelled()` gracefully |
| 5 | Stage duration is TOTAL time for that stage (including ramp), matching k6 semantics | VERIFIED | Stage scheduler waits `remaining = stage_duration - elapsed_in_stage`; safety_timeout at engine.rs:346 |
| 6 | Live terminal display shows current stage label: `[stage 2/3] vus: XX ...` | VERIFIED | `format_status` with `stage_label: Option<&str>` at display.rs:86; `[{label}]` prefix rendered; test `test_format_status_with_stage_label` passes |
| 7 | VU loop accepts per-VU CancellationToken (child of global) for selective shutdown | VERIFIED | `child_token` at engine.rs:381 pushed to `vu_tokens` and passed to `vu_loop`; vu.rs signature accepts `cancel: CancellationToken` |
| 8 | `settings.virtual_users` is ignored when stages are present | VERIFIED | Engine enters `run_staged()` path which does not use `settings.virtual_users`; warning emitted in `validate()` at config.rs:229 |
| 9 | `settings.duration_secs` acts as safety timeout ceiling when stages are present | VERIFIED | `safety_timeout = effective_duration_secs() + 30` at engine.rs:346 |
| 10 | Config validation rejects stages with zero duration_secs | VERIFIED | Validation at config.rs:231; unit test passes |
| 11 | All existing unit tests continue to pass (no regressions) | VERIFIED | `cargo test -- --test-threads=1` passes 232 total tests; 0 failures |

#### Plan 04-02: MCP-02 Must-Haves

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | Each RequestSample carries optional `tool_name: Option<String>` | VERIFIED | Field at metrics.rs:78; constructors accept it at metrics.rs |
| 2 | MetricsRecorder maintains per-tool HdrHistograms in HashMap<String, ToolMetrics> | VERIFIED | `per_tool: HashMap<String, ToolMetrics>` at metrics.rs:273; routing in `record()` at metrics.rs:343 |
| 3 | MetricsSnapshot includes `per_tool: Vec<ToolSnapshot>` with full stats | VERIFIED | Field at metrics.rs:193; built and sorted in `snapshot()` at metrics.rs:464 |
| 4 | Terminal summary includes grouped per-tool table after overall metrics | VERIFIED | Per-tool section at summary.rs:161; columns: tool, reqs, rate, err%, p50, p95, p99 |
| 5 | All tools shown in per-tool table (no truncation/limit) | VERIFIED | Iterates all `snap.per_tool` at summary.rs:172; no length limit |
| 6 | JSON report includes `per_tool` object with extended detail | VERIFIED | `per_tool: HashMap<String, ToolReportMetrics>` at report.rs:43; `ToolReportMetrics` and `ToolLatencyMetrics` at report.rs:131 |
| 7 | VU loop extracts tool_name from ScenarioStep and passes to RequestSample | VERIFIED | Extraction at vu.rs:284; passed to `RequestSample::success/error` at vu.rs:292-293 |
| 8 | Per-tool metrics use HdrHistogram pattern with coordinated omission correction | VERIFIED | `ToolMetrics::new()` uses `Histogram::new(3)` with `auto(true)` and `record_correct` at metrics.rs |
| 9 | Schema version bumped to 1.1 | VERIFIED | `const SCHEMA_VERSION: &str = "1.1"` at report.rs:18; assertion in `test_report_schema_version` |

#### Plan 04-03: METR-06 Must-Haves

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | BreakingPointDetector is a struct with VecDeque rolling window of WindowSample entries | VERIFIED | `pub struct BreakingPointDetector` at breaking.rs:87; `window: VecDeque<WindowSample>` at breaking.rs:89 |
| 2 | Breaking point detection is always on by default | VERIFIED | Created unconditionally in both aggregator functions at engine.rs:508, 597 |
| 3 | Detection uses self-calibrating rolling window: splits window in half, compares recent vs baseline | VERIFIED | `half = window_size / 2`; `baseline_slice = &all_samples[..half]`; `recent_slice = &all_samples[half..]` at breaking.rs |
| 4 | Detection triggers when BOTH conditions met (error rate spike OR latency degradation) | VERIFIED | Error rate check and latency check at breaking.rs; thresholds match spec (10%, 2x, 3x) |
| 5 | Detection fires only once | VERIFIED | `if self.detected { return None; }` at breaking.rs:138; `self.detected = true` on first fire |
| 6 | Minimum window fill required before first evaluation | VERIFIED | `if self.window.len() < self.window_size { return None; }` at breaking.rs |
| 7 | Live terminal shows warning when breaking point detected: `WARNING: Breaking point detected at XX VUs (reason: detail)` | VERIFIED | Warning rendered at display.rs:185. VU count now correct: `detector.observe(&snapshot, active_vus.get())` at engine.rs:527 and 616 (gap closed by Plan 04-04) |
| 8 | JSON report includes `breaking_point` object with detected flag, VU count, reason, detail, timestamp | VERIFIED | `BreakingPointReport` struct at report.rs:53 with all fields; wired at report.rs:226. VU count now reflects actual VUs (gap closed by Plan 04-04) |
| 9 | Report-and-continue semantics: test keeps running after detection | VERIFIED | `bp_holder` stores the event but `cancel` is NOT called; test continues normally |
| 10 | Window size is 10 samples | VERIFIED | `DEFAULT_WINDOW_SIZE: usize = 10` at breaking.rs:31; `with_default_window()` at breaking.rs |
| 11 | Property tests verify detector invariants | VERIFIED | 3 property tests at engine_property_tests.rs:161, 190, 217: fires-at-most-once, requires-full-window, low-error-rate-never-triggers-spike |
| 12 | Fuzz target exercises BreakingPointDetector::observe() with arbitrary WindowSample sequences | VERIFIED | `fuzz_breaking_point.rs` at fuzz/fuzz_targets/fuzz_breaking_point.rs; registered in fuzz/Cargo.toml:29 |

#### Plan 04-04: Gap Closure Must-Haves

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | Both `metrics_aggregator` and `metrics_aggregator_with_label` accept an `ActiveVuCounter` parameter | VERIFIED | engine.rs:504: `active_vus: ActiveVuCounter`; engine.rs:593: `active_vus: ActiveVuCounter` |
| 2 | `detector.observe(&snapshot, active_vus.get())` called instead of hardcoded 0 in both aggregators | VERIFIED | engine.rs:527: `detector.observe(&snapshot, active_vus.get())`; engine.rs:616: same |
| 3 | `run_flat()` passes its `active_vus` clone to `metrics_aggregator` | VERIFIED | engine.rs:217: `active_vus.clone()` as final argument to `metrics_aggregator` spawn |
| 4 | `run_staged()` passes its `active_vus` clone to `metrics_aggregator_with_label` | VERIFIED | engine.rs:326: `active_vus.clone()` as final argument to `metrics_aggregator_with_label` spawn |
| 5 | BreakingPoint.vus reflects the actual active VU count at time of detection, not 0 | VERIFIED | Wiring confirmed at both call sites; format string at engine.rs:528-531 uses `bp.vus` |
| 6 | All existing tests pass (cargo test -- --test-threads=1) with zero failures | VERIFIED | 232 tests: 114 lib + 101 lib + 8 integration + 7 property + 2 doctests -- all pass, 0 failures |
| 7 | Zero clippy warnings (cargo clippy -- -D warnings) | VERIFIED | `cargo clippy -- -D warnings` exits cleanly; `#[allow(clippy::too_many_arguments)]` applied to both 8-param aggregator functions |

**Score:** 32/32 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|---------|--------|---------|
| `src/loadtest/config.rs` | Stage struct, stage field, helpers, validation | VERIFIED | Stage at line 68; `pub stage: Vec<Stage>` at line 96; helpers at 187, 192, 200; validate() extended at 229 |
| `src/loadtest/engine.rs` | Stage-driven scheduler, DisplayState with stage_label and breaking_point, ActiveVuCounter threaded | VERIFIED | DisplayState at line 44; run_flat/run_staged branching; stage scheduler; active_vus.get() at 527, 616 |
| `src/loadtest/vu.rs` | tool_name extracted from ScenarioStep, passed to RequestSample | VERIFIED | Extraction at line 284; passed to constructors at 292-293 |
| `src/loadtest/display.rs` | DisplayState-based watch channel, stage label rendered, breaking_point warning | VERIFIED | `display_loop` accepts `watch::Receiver<DisplayState>`; stage_label rendered; bp warning at 185 |
| `src/loadtest/metrics.rs` | ToolMetrics, ToolSnapshot, per_tool in recorder and snapshot | VERIFIED | ToolMetrics; ToolSnapshot at 133; per_tool HashMap at 273; per_tool Vec at 193 |
| `src/loadtest/summary.rs` | Per-tool metrics table section | VERIFIED | Section at line 161; header, column formatting, color coding |
| `src/loadtest/report.rs` | ToolReportMetrics, BreakingPointReport, per_tool and breaking_point in LoadTestReport | VERIFIED | per_tool at 43; breaking_point at 45; BreakingPointReport at 53; ToolReportMetrics at 131; schema 1.1 at 18 |
| `src/loadtest/breaking.rs` | BreakingPointDetector, WindowSample, BreakingPoint, observe() | VERIFIED | pub struct at line 87; observe() at 122; unit tests pass |
| `src/loadtest/mod.rs` | `pub mod breaking;` added | VERIFIED | Line 6 |
| `tests/engine_property_tests.rs` | Property tests for BreakingPointDetector invariants | VERIFIED | 3 proptest blocks at lines 161, 190, 217 |
| `fuzz/fuzz_targets/fuzz_breaking_point.rs` | Fuzz target for BreakingPointDetector::observe() | VERIFIED | `fuzz_target!` present; registered in fuzz/Cargo.toml:29 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `config.rs` | `engine.rs` | Stage struct parsed from TOML drives stage scheduler loop | VERIFIED | `config.stage.iter()` at engine.rs:353; `config.stage.len()` at 292 |
| `engine.rs` | `vu.rs` | Engine passes per-VU child_token() to vu_loop for selective ramp-down | VERIFIED | `cancel.child_token()` at engine.rs:381; pushed to `vu_tokens` and passed to `vu_loop` at 390 |
| `engine.rs` | `display.rs` | Engine sends DisplayState with stage_label through watch channel | VERIFIED | `watch::channel(initial_display_state)` at engine.rs:151, 298; display_loop consumes at display.rs |
| `breaking.rs` | `engine.rs` | Engine calls detector.observe() each snapshot tick with live VU count | VERIFIED | `detector.observe(&snapshot, active_vus.get())` at engine.rs:527 and 616 -- VU count is now live |
| `engine.rs` | `display.rs` | DisplayState.breaking_point carries warning message to display loop | VERIFIED | `DisplayState.breaking_point` set at engine.rs:532; consumed at display.rs:185 |
| `engine.rs` | `report.rs` | LoadTestResult carries breaking point to report builder | VERIFIED | `breaking_point: Option<BreakingPoint>` at engine.rs:480; consumed in `LoadTestReport::from_result` at report.rs:226 |
| `engine.rs (run_flat)` | `engine.rs (metrics_aggregator)` | active_vus.clone() passed as parameter | VERIFIED | engine.rs:217: `active_vus.clone()` as final argument |
| `engine.rs (run_staged)` | `engine.rs (metrics_aggregator_with_label)` | active_vus.clone() passed as parameter | VERIFIED | engine.rs:326: `active_vus.clone()` as final argument |
| `vu.rs` | `metrics.rs` | VU loop sets tool_name on RequestSample; MetricsRecorder routes to per-tool histograms | VERIFIED | tool_name extracted at vu.rs:284; `per_tool.entry(name).or_insert_with(ToolMetrics::new)` at metrics.rs:343 |
| `metrics.rs` | `summary.rs` | ToolSnapshot in MetricsSnapshot consumed by per-tool table renderer | VERIFIED | `snap.per_tool` iterated at summary.rs:172 |
| `metrics.rs` | `report.rs` | ToolSnapshot converted to ToolReportMetrics for JSON serialization | VERIFIED | `snap.per_tool.iter().map(...)` at report.rs:200 |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| LOAD-04 | 04-01-PLAN.md | User can define ramp-up/hold/ramp-down phases to gradually increase load | SATISFIED | Stage struct, stage scheduler, per-VU child_token, LIFO ramp-down all implemented and tested |
| MCP-02 | 04-02-PLAN.md | Metrics are reported per MCP tool (latency, throughput, errors per tool) | SATISFIED | ToolMetrics, ToolSnapshot, per-tool table in summary, per_tool in JSON report all present and tested |
| METR-06 | 04-03-PLAN.md, 04-04-PLAN.md | Load test can auto-detect breaking point where performance degrades | SATISFIED | BreakingPointDetector fires correctly; VU count in warning and report now reflects actual live VUs; gap fully closed |

No orphaned requirements found. REQUIREMENTS.md maps LOAD-04, MCP-02, METR-06 exclusively to Phase 4 -- all marked Complete.

### Anti-Patterns Found

None. The previously identified anti-pattern (`detector.observe(&snapshot, 0)` hardcoded 0) has been resolved. No TODO/FIXME comments, no stub implementations, no empty returns found in any of the 12 modified files.

### Human Verification Required

#### 1. Breaking Point VU Count in Terminal Warning

**Test:** Run a staged load test with a server that degrades under load. Wait for breaking point detection to fire.
**Expected:** Terminal shows `WARNING: Breaking point detected at XX VUs` where XX is the actual active VU count at time of detection, not 0.
**Why human:** Cannot programmatically verify terminal output against a live server during a real timed run. The wiring is correct (confirmed via code inspection) but the displayed value can only be confirmed by observing a live run.

#### 2. Stage Display Progression

**Test:** Run a 3-stage load test (`target_vus=5, 10, 0` over 30/30/30 seconds).
**Expected:** Live display cycles through `[stage 1/3]`, `[stage 2/3]`, `[stage 3/3]` as each stage completes.
**Why human:** Engine stage timing is async; cannot simulate the full 90-second run in unit tests.

### Gaps Summary

**No gaps remain.** The single gap from the initial verification has been fully closed by Plan 04-04:

**Closed gap:** Both `metrics_aggregator` (engine.rs:496) and `metrics_aggregator_with_label` (engine.rs:585) now accept an `active_vus: ActiveVuCounter` parameter. The call to `detector.observe()` at engine.rs:527 and engine.rs:616 now passes `active_vus.get()` instead of the previously hardcoded `0`. The call sites in `run_flat()` (engine.rs:217) and `run_staged()` (engine.rs:326) each pass `active_vus.clone()`. Both aggregator functions carry `#[allow(clippy::too_many_arguments)]` to accommodate the 8-parameter signature while maintaining zero clippy warnings.

All 32 must-have truths are now VERIFIED. The phase goal -- "Developers can shape load with ramp-up/ramp-down phases, detect server breaking points automatically, and see per-tool performance breakdowns" -- is fully achieved.

---

_Verified: 2026-02-27T06:21:10Z_
_Verifier: Claude (gsd-verifier)_
