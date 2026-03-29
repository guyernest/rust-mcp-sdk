---
phase: 02-engine-core
verified: 2026-02-26T00:30:00Z
status: passed
score: 4/4 must-haves verified
re_verification: false
---

# Phase 2: Engine Core Verification Report

**Phase Goal:** Developers can run a concurrent load test with N virtual users against a deployed MCP server and see live progress in the terminal
**Verified:** 2026-02-26
**Status:** PASSED
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | N concurrent virtual users each perform their own MCP initialize handshake and execute tool calls independently with their own session | VERIFIED | `src/loadtest/vu.rs`: `vu_loop()` increments `ActiveVuCounter`, calls `try_initialize()` which creates its own `McpClient` and calls `client.initialize()`. Engine spawns N independent tokio tasks via `TaskTracker::spawn(vu_loop(...))`. Each VU owns its `McpClient` and runs a separate session. |
| 2 | The test runs for a specified duration (seconds) or iteration count, then stops cleanly | VERIFIED | `src/loadtest/engine.rs` run() uses `tokio::select!` racing `tokio::time::sleep(duration)` against `controller_cancel.cancelled()` for first-limit-wins. Iteration limit uses `fetch_add(Relaxed)` with `cancel.cancel()` in `vu_loop_inner`. Graceful drain via `tracker.close()` + `tracker.wait().await`. |
| 3 | Live terminal output shows current requests/second, cumulative error count, active VU count, and elapsed time, updated on a timer (not per-request) | VERIFIED | `src/loadtest/display.rs`: `display_loop()` reacts to `snapshot_rx.changed()`. Watch channel is only sent in the metrics aggregator's 2-second `tick.tick()` arm (not in the per-sample `recv()` arm). `LiveDisplay::format_status()` outputs vus, rps, p95, errors, elapsed. Engine spawns `display_loop` task before controller. |
| 4 | Throughput (requests/second) and error rate are computed correctly from the metrics pipeline | VERIFIED | `src/loadtest/metrics.rs`: `MetricsRecorder` tracks `total_success` and `total_errors`. `error_rate()` = `error_histogram.len() / total_requests()`. RPS is computed in `LiveDisplay::format_status()` as `snap.total_requests as f64 / elapsed_secs`. `MetricsSnapshot` carries `total_requests`, `error_count`, `error_rate`. Property tests verify `error_rate_bounded` (0.0..=1.0) and `metrics_total_matches_sample_count`. |

**Score:** 4/4 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/loadtest/engine.rs` | LoadTestEngine struct with run() method returning MetricsSnapshot | VERIFIED | 482 lines. Contains `LoadTestEngine`, `LoadTestResult`, `metrics_aggregator`, `handle_ctrl_c`. Builder methods: `new()`, `with_iterations()`, `with_ramp_up()`, `with_no_color()`. `run()` returns `Result<LoadTestResult, LoadTestError>`. |
| `src/loadtest/vu.rs` | VU task loop with weighted step selection, respawn logic, and metrics emission | VERIFIED | 403 lines. Contains `vu_loop`, `vu_loop_inner`, `execute_step`, `respawn_with_backoff`, `try_initialize`, `ActiveVuCounter`, `step_to_operation_type`, `is_session_fatal`. Weighted index via `rand::distr::weighted::WeightedIndex`. |
| `src/loadtest/display.rs` | LiveDisplay struct with display_loop() function consuming watch::Receiver<MetricsSnapshot> | VERIFIED | 263 lines. Contains `LiveDisplay` struct, `display_loop()` function, `format_status()` static method. |
| `src/loadtest/mod.rs` | Updated module declarations for engine, vu, display | VERIFIED | Declares `pub mod client; pub mod config; pub mod display; pub mod engine; pub mod error; pub mod metrics; pub mod vu;` |
| `Cargo.toml` | tokio-util dependency with rt feature | VERIFIED | Line 35: `tokio-util = { version = "0.7", features = ["rt"] }` |
| `tests/engine_property_tests.rs` | Property-based tests for engine invariants and metrics aggregator | VERIFIED | 148 lines. 5 proptest tests: `metrics_total_matches_sample_count`, `error_rate_bounded`, `percentiles_monotonic`, `operation_counts_sum_to_total`, `valid_config_roundtrips`. All pass. |
| `fuzz/fuzz_targets/fuzz_metrics_record.rs` | Fuzz target for metrics pipeline robustness | VERIFIED | Exercises `MetricsRecorder` with arbitrary byte-driven samples. Registered in `fuzz/Cargo.toml`. |
| `examples/engine_demo.rs` | Runnable example demonstrating LoadTestEngine usage | VERIFIED | 74 lines. Uses `LoadTestEngine::new(config, url).with_no_color(false).run().await`. Compiles successfully (`cargo build --example engine_demo`). |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `src/loadtest/engine.rs` | `tokio_util::sync::CancellationToken` | graceful shutdown coordination | WIRED | Line 22: `use tokio_util::sync::CancellationToken;`. Used in `run()`, `metrics_aggregator()`, `handle_ctrl_c()`. |
| `src/loadtest/engine.rs` | `tokio_util::task::TaskTracker` | VU task spawn and drain tracking | WIRED | Line 23: `use tokio_util::task::TaskTracker;`. `tracker.spawn(vu_loop(...))` for each VU, `tracker.close()` + `tracker.wait().await` for drain. |
| `src/loadtest/vu.rs` | `src/loadtest/client.rs` (McpClient) | each VU owns an McpClient | WIRED | Line 8: `use crate::loadtest::client::McpClient;`. `try_initialize()` creates `McpClient::new(...)` and calls `client.initialize()`. `execute_step()` calls `client.call_tool()`, `client.read_resource()`, `client.get_prompt()`. |
| `src/loadtest/vu.rs` | `src/loadtest/metrics.rs` (RequestSample) | sends RequestSample through mpsc channel | WIRED | Line 11: `use crate::loadtest::metrics::{OperationType, RequestSample};`. `sample_tx.send(sample).await` on every request. |
| `src/loadtest/engine.rs` | `src/loadtest/metrics.rs` (MetricsRecorder) | metrics aggregator owns MetricsRecorder, publishes MetricsSnapshot via watch | WIRED | Lines 15-16: imports `MetricsRecorder`, `MetricsSnapshot`, `RequestSample`. `metrics_aggregator()` owns two `MetricsRecorder` instances (`live` + `report`) and publishes via `snapshot_tx.send(live.snapshot())` on 2-second tick. |
| `src/loadtest/display.rs` | `indicatif::MultiProgress` | k6-style live terminal rendering | WIRED | Line 11: `use indicatif::{MultiProgress, ProgressBar, ProgressStyle};`. `LiveDisplay::new()` creates `MultiProgress::new()` and adds a spinner `ProgressBar`. |
| `src/loadtest/display.rs` | `colored::Colorize` | metric colorization (red for errors, green for healthy) | WIRED | Line 10: `use colored::Colorize;`. `format_status()` uses `.green()`, `.red()`, `.yellow()` on metric strings. |
| `src/loadtest/display.rs` | `tokio::sync::watch::Receiver` | receives MetricsSnapshot updates every 2 seconds | WIRED | Line 14: `use tokio::sync::watch;`. `display_loop()` takes `mut snapshot_rx: watch::Receiver<MetricsSnapshot>` and calls `snapshot_rx.changed()` and `snapshot_rx.borrow_and_update()`. |
| `src/loadtest/engine.rs` | `src/loadtest/display.rs` (display_loop) | spawns display task with snapshot_rx and active_vus | WIRED | Line 13: `use crate::loadtest::display::display_loop;`. `run()` spawns `tokio::spawn(display_loop(display_rx, display_vus, target_vus, display_cancel, no_color, test_start))`. |
| `tests/engine_property_tests.rs` | `src/loadtest/engine.rs` (LoadTestEngine) | tests LoadTestEngine and metrics aggregator | WIRED | `use cargo_pmcp::loadtest::metrics::{MetricsRecorder, OperationType, RequestSample};` and `use cargo_pmcp::loadtest::config::LoadTestConfig;`. Property tests run and pass (`cargo test --test engine_property_tests`). |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| LOAD-01 | 02-01, 02-03 | User can run a load test with N concurrent virtual users against a deployed MCP server | SATISFIED | `LoadTestEngine::run()` spawns N VU tasks via `TaskTracker`. `vu_loop()` owns individual `McpClient` and session. Smoke test `test_engine_run_with_no_color_does_not_panic` validates engine handles concurrent VUs. |
| LOAD-02 | 02-01, 02-03 | User can set test duration by time (seconds) or iteration count | SATISFIED | `run()` `tokio::select!` races `tokio::time::sleep(duration_secs)` vs `cancel.cancelled()`. `with_iterations(n)` builder sets `max_iterations`. VU loop uses `fetch_add` with `cancel.cancel()` on limit hit. First-limit-wins semantics in place. |
| METR-02 | 02-01, 02-03 | Load test reports throughput (requests/second) and error rate with classification | SATISFIED | `MetricsSnapshot` carries `total_requests`, `error_count`, `error_rate`. RPS computed in `format_status()`. `error_rate()` computed in `MetricsRecorder`. Property test `error_rate_bounded` verifies bounds. McpError variants (JsonRpc, Http, Timeout, Connection) provide error classification. |
| METR-03 | 02-02 | Load test shows live terminal progress (current RPS, error count, elapsed time) | SATISFIED | `display_loop()` wakes on watch channel changes. Watch channel sends only in the 2-second tick arm of `metrics_aggregator`. `format_status()` formats VUs, RPS, P95, error count/rate, elapsed. Spawned via `tokio::spawn(display_loop(...))` inside `run()`. |

---

### Timer-Based Display Verification

The display update mechanism is confirmed timer-driven, not per-request. The evidence trail:

1. `metrics_aggregator()` in `engine.rs` has a `biased;` `tokio::select!` with three arms:
   - `tick.tick()` (2-second interval): records buffered samples via `try_recv()` then calls `snapshot_tx.send(live.snapshot())` — this is the ONLY place where the watch channel is updated during the test run.
   - `sample_rx.recv()`: records the sample into `live` and `report` recorders BUT does NOT call `snapshot_tx.send()`.
   - `cancel.cancelled()`: drains remaining samples and sends final snapshot.
2. `display_loop()` in `display.rs` wakes only via `snapshot_rx.changed()` — which fires when `snapshot_tx.send()` is called.
3. Therefore: display updates fire exactly at 2-second tick intervals (plus one final update at test end), not on every request.

---

### Anti-Patterns Scan

Files scanned: `src/loadtest/engine.rs`, `src/loadtest/vu.rs`, `src/loadtest/display.rs`, `src/loadtest/mod.rs`, `tests/engine_property_tests.rs`, `examples/engine_demo.rs`

| File | Pattern | Severity | Finding |
|------|---------|----------|---------|
| All files | TODO/FIXME/PLACEHOLDER | None | No TODO, FIXME, XXX, HACK, or PLACEHOLDER comments found. |
| `engine.rs` | Empty implementation | None | All methods have substantive implementations. `run()` is 128 lines of real logic. |
| `vu.rs` | Empty implementation | None | `vu_loop_inner()` has 67 lines of real load generation logic. |
| `display.rs` | Stub handler | None | `display_loop()` is 42 lines wired to real watch channel and indicatif rendering. |
| `engine.rs` | `snapshot_receiver()` unimplemented | None — already resolved | The plan mentioned a placeholder `snapshot_receiver()` method with `unimplemented!()`. This was NOT implemented in the final code. Plan 02-02 correctly integrated display into `run()` directly, making the placeholder unnecessary. No `unimplemented!()` in the codebase. |

---

### Human Verification Required

#### 1. Visual Terminal Output Rendering

**Test:** Run `cargo run --example engine_demo -- http://your-mcp-server/mcp` against a live MCP server with 2 VUs for 5 seconds.
**Expected:** Terminal shows a spinning indicator with the status line updating in-place every 2 seconds showing `vus: 2/2 | rps: X.X | p95: Xms | errors: 0 (0.0%) | elapsed: Xs`. Line updates without scrolling.
**Why human:** Color output, spinner animation, and in-place update behavior require visual confirmation. Unit tests verify the data content of `format_status()` but cannot verify the indicatif spinner renders correctly in a live terminal.

#### 2. Ctrl+C Graceful Drain Behavior

**Test:** Start a load test with a long duration (60s), then press Ctrl+C once.
**Expected:** Test stops sending NEW requests, waits for in-flight requests to complete (up to timeout), then prints results. Pressing Ctrl+C a second time should immediately exit with code 1.
**Why human:** Signal handling behavior requires interactive terminal testing. The `handle_ctrl_c()` function in engine.rs implements this logic but it cannot be exercised in unit tests.

---

### Test Results Summary

All automated tests pass:

| Test Suite | Tests | Result |
|------------|-------|--------|
| `cargo test --lib loadtest::vu` | 10 | All passed |
| `cargo test --lib loadtest::engine` | 5 | All passed |
| `cargo test --lib loadtest::display` | 4 | All passed |
| `cargo test --test engine_property_tests` | 5 | All passed |
| `cargo build --example engine_demo` | — | Compiles successfully |
| `cargo check` | — | No errors |

Total: 24 tests, all passing.

---

### Gaps Summary

None. All 4 observable truths are verified. All 8 required artifacts exist and are substantive. All 10 key links are wired. All 4 phase requirements (LOAD-01, LOAD-02, METR-02, METR-03) are satisfied.

The phase goal is achieved: developers can run a concurrent load test with N virtual users against a deployed MCP server and see live progress in the terminal.

---

_Verified: 2026-02-26_
_Verifier: Claude (gsd-verifier)_
