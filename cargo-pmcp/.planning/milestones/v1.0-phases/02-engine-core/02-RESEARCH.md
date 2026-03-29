# Phase 2: Engine Core - Research

**Researched:** 2026-02-26
**Domain:** Concurrent load test execution engine with live terminal output
**Confidence:** HIGH

## Summary

Phase 2 builds the concurrent execution engine that spawns N virtual users as independent tokio tasks, each performing their own MCP initialize handshake and weighted-random scenario steps, while a central metrics aggregator collects samples via an mpsc channel and publishes snapshots via a watch channel for a live terminal display. The core architecture is: VU tasks -> mpsc channel -> metrics aggregator -> watch channel -> display tick loop. Graceful shutdown uses `tokio_util::sync::CancellationToken` for signaling and `tokio_util::task::TaskTracker` for drain-wait, with double Ctrl+C support (first = graceful drain, second = hard abort via `std::process::exit`).

The standard stack is well-established: `tokio-util` 0.7 for CancellationToken/TaskTracker, `rand` 0.10 (already a dependency) for `WeightedIndex`-based step selection, `indicatif` 0.18 (already a dependency) for the k6-style compact live display, and `colored` 3 (already a dependency) for metric colorization. No new external crates are required beyond `tokio-util`.

**Primary recommendation:** Use the mpsc+watch dual-channel pattern with a single-owner MetricsRecorder in a dedicated aggregator task. Each VU task sends `RequestSample` values through a bounded mpsc channel. The aggregator drains the channel, records into the existing `MetricsRecorder`, and publishes `MetricsSnapshot` values to a watch channel every 2 seconds. The display task subscribes to the watch channel and renders with indicatif.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- On session failure mid-test: respawn with exponential backoff, max 3 attempts before permanent death
- Step selection: weighted random based on configured weights (e.g., 60% tools/call, 30% resources/read, 10% prompts/get)
- Discovery: per-VU (each VU calls tools/list, resources/list, prompts/list independently during its own initialize phase -- no shared cache)
- Dead VUs counted in metrics; active VU count drops as VUs die
- Support both duration-based (--duration 30s) and iteration-based (--iterations 1000) stopping
- If both specified: first limit hit wins (whichever triggers first stops the test)
- Graceful drain on stop: stop sending NEW requests, wait for in-flight to complete (up to timeout), then report
- Ctrl+C handling: first Ctrl+C triggers graceful drain + partial report, second Ctrl+C hard aborts
- k6-style compact display: single updating block that refreshes in-place
- Metrics shown: requests/sec, error count + error rate, active VU count, P95 latency, elapsed time
- Colored output: red for errors, green for healthy metrics (disable with --no-color or when piped)
- Refresh rate: every 2 seconds
- Default: all VUs start at once
- Optional --ramp-up flag for linear stagger (e.g., --ramp-up 30s spreads VU spawning over 30 seconds)
- Duration timer starts when first VU spawns (ramp-up is included in total test time)
- No ramp-down period -- all VUs stop together, graceful drain handles in-flight requests
- Ramp-up metrics excluded from final report (warm-up data)

### Claude's Discretion
- Channel architecture for metrics aggregation (mpsc, broadcast, etc.)
- Tokio task spawning strategy for VUs
- Exponential backoff timing parameters
- indicatif vs custom terminal rendering
- How ramp-up exclusion window is tracked internally

### Deferred Ideas (OUT OF SCOPE)
- Shared discovery cache across VUs (decided per-VU for now, could optimize later)
- Ramp-down period (gradual VU removal before stop)
- Configurable retry count (fixed at 3 for v1)
- Configurable refresh rate (fixed at 2s for v1)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LOAD-01 | User can run a load test with N concurrent virtual users against a deployed MCP server | VU scheduler using `tokio_util::task::TaskTracker` to spawn N tasks, each owning an `McpClient` with its own session. CancellationToken for coordinated shutdown. |
| LOAD-02 | User can set test duration by time (seconds) or iteration count | Run controller using `tokio::select!` on duration timer (`tokio::time::sleep`) vs iteration counter (`AtomicU64`). First-limit-wins semantics via CancellationToken. |
| METR-02 | Load test reports throughput (requests/second) and error rate with classification | Metrics aggregator task consuming `RequestSample` via mpsc channel, computing RPS from `total_requests / elapsed_seconds` using the existing `MetricsRecorder`. Error rate already implemented in Phase 1 snapshot. |
| METR-03 | Load test shows live terminal progress (current RPS, error count, elapsed time) | indicatif `MultiProgress` with `ProgressBar::new_spinner()` lines using custom templates. Watch channel delivers `MetricsSnapshot` every 2 seconds. Colored output via `colored` crate with `--no-color` / pipe detection. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio-util | 0.7.18 | CancellationToken + TaskTracker for graceful shutdown | Official Tokio ecosystem. CancellationToken is the standard cooperative cancellation primitive. TaskTracker tracks spawned tasks and waits for drain completion. |
| tokio (already dep) | 1.x | Runtime, mpsc/watch channels, signal handling, timers | Already in Cargo.toml with `features = ["full"]`. Provides `tokio::sync::mpsc`, `tokio::sync::watch`, `tokio::signal::ctrl_c`, `tokio::time::sleep`, `tokio::time::interval`. |
| rand (already dep) | 0.10 | WeightedIndex for weighted random step selection | Already in Cargo.toml. `rand::distr::weighted::WeightedIndex` provides O(log N) weighted sampling. |
| indicatif (already dep) | 0.18 | Live terminal progress display | Already in Cargo.toml. MultiProgress + ProgressBar with custom templates for k6-style compact display. |
| colored (already dep) | 3 | Terminal color output | Already in Cargo.toml. Used for red/green metric coloring. Supports `colored::control::set_override(false)` for `--no-color`. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::sync::atomic | (stdlib) | AtomicU64 for iteration counter, AtomicBool for shutdown flag | Shared counters between VU tasks without channel overhead. AtomicU64 for global iteration count that VUs check after each request. |
| std::io::IsTerminal | (stdlib) | Detect piped output to disable colors | Check `std::io::stderr().is_terminal()` at startup. If piped, disable colored output automatically. |

### New Dependency Required
```toml
tokio-util = { version = "0.7", features = ["rt"] }
```
The `rt` feature enables `CancellationToken` and `TaskTracker`. No other new crates needed.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| tokio mpsc + watch channels | crossbeam channels | crossbeam is sync-only; tokio channels integrate natively with async select! and don't require thread bridging |
| indicatif for display | Raw crossterm/termion | indicatif already handles cursor movement, in-place updates, MultiProgress coordination, and is already a dependency |
| tokio_util::CancellationToken | Manual AtomicBool + Notify | CancellationToken provides parent/child hierarchies, is cancel-safe in select!, and integrates with TaskTracker |
| Hand-rolled backoff | backoff/backon crate | Fixed 3-retry with simple Duration doubling is trivial enough to hand-roll; crate dependency not justified |

## Architecture Patterns

### Recommended Module Structure
```
src/loadtest/
├── config.rs          # (Phase 1 - exists)
├── client.rs          # (Phase 1 - exists)
├── metrics.rs         # (Phase 1 - exists)
├── error.rs           # (Phase 1 - exists)
├── engine.rs          # NEW: LoadTestEngine orchestrator
├── vu.rs              # NEW: VirtualUser task loop
├── display.rs         # NEW: Live terminal display
└── mod.rs             # Updated: export new modules
```

### Pattern 1: Dual-Channel Metrics Pipeline
**What:** VU tasks send `RequestSample` through bounded mpsc -> aggregator task records into `MetricsRecorder` -> publishes `MetricsSnapshot` through watch channel -> display task reads latest snapshot.
**When to use:** Always. This is the core data flow pattern.
**Why:** Single-owner `MetricsRecorder` avoids any locking. mpsc provides backpressure if VUs overwhelm the aggregator. watch channel provides latest-value semantics for the display (no message queue buildup).

```rust
// Metrics aggregator task
async fn metrics_aggregator(
    mut sample_rx: mpsc::Receiver<RequestSample>,
    snapshot_tx: watch::Sender<MetricsSnapshot>,
    cancel: CancellationToken,
) {
    let mut recorder = MetricsRecorder::new(expected_interval_ms);
    let mut tick = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            // Drain available samples (non-blocking batch)
            Some(sample) = sample_rx.recv() => {
                recorder.record(&sample);
            }
            // Periodic snapshot publish
            _ = tick.tick() => {
                let _ = snapshot_tx.send(recorder.snapshot());
            }
            // Shutdown signal
            _ = cancel.cancelled() => {
                // Drain remaining samples in channel
                while let Ok(sample) = sample_rx.try_recv() {
                    recorder.record(&sample);
                }
                let _ = snapshot_tx.send(recorder.snapshot());
                break;
            }
        }
    }
}
```

### Pattern 2: VU Task Loop with Weighted Step Selection
**What:** Each VU task owns an McpClient, performs initialize + discovery, then loops executing weighted-random scenario steps until cancellation.
**When to use:** Every VU follows this lifecycle.

```rust
use rand::distr::weighted::WeightedIndex;
use rand::prelude::*;

async fn vu_loop(
    vu_id: u32,
    config: Arc<LoadTestConfig>,
    base_url: String,
    sample_tx: mpsc::Sender<RequestSample>,
    cancel: CancellationToken,
    iteration_counter: Arc<AtomicU64>,
    max_iterations: Option<u64>,
) {
    let http = reqwest::Client::new(); // Shared connection pool via Clone
    let timeout = config.settings.timeout_as_duration();
    let mut client = McpClient::new(http, base_url, timeout);

    // Phase: Initialize
    if let Err(e) = client.initialize().await {
        // Record error, attempt respawn logic
        return;
    }

    // Phase: Discovery (per-VU, no shared cache)
    // client.call_tool("tools/list", ...) etc.

    // Phase: Load generation loop
    let weights: Vec<u32> = config.scenario.iter().map(|s| s.weight()).collect();
    let dist = WeightedIndex::new(&weights).unwrap();
    let mut rng = rand::rng();

    loop {
        // Check iteration limit
        if let Some(max) = max_iterations {
            let current = iteration_counter.fetch_add(1, Ordering::Relaxed);
            if current >= max {
                cancel.cancel(); // Signal all VUs to stop
                break;
            }
        }

        // Check cancellation
        if cancel.is_cancelled() {
            break;
        }

        let step_idx = dist.sample(&mut rng);
        let start = Instant::now();
        let result = execute_step(&mut client, &config.scenario[step_idx]).await;
        let duration = start.elapsed();

        let sample = match result {
            Ok(()) => RequestSample::success(operation_type, duration),
            Err(e) => RequestSample::error(operation_type, duration, e),
        };
        let _ = sample_tx.send(sample).await;
    }
}
```

### Pattern 3: Graceful Shutdown with Double Ctrl+C
**What:** First Ctrl+C cancels the CancellationToken (graceful drain). Second Ctrl+C calls `std::process::exit(1)` (hard abort).
**When to use:** Always for the top-level engine.

```rust
async fn handle_ctrl_c(cancel: CancellationToken) {
    // First Ctrl+C: graceful shutdown
    tokio::signal::ctrl_c().await.expect("ctrl_c handler");
    eprintln!("\nReceived Ctrl+C, stopping gracefully...");
    cancel.cancel();

    // Second Ctrl+C: hard abort
    tokio::signal::ctrl_c().await.expect("ctrl_c handler");
    eprintln!("\nReceived second Ctrl+C, aborting immediately.");
    std::process::exit(1);
}
```

### Pattern 4: VU Respawn with Exponential Backoff
**What:** When a VU session fails mid-test, respawn with exponential backoff, max 3 attempts.
**When to use:** On McpError during VU execution (not during initial handshake failure).

```rust
const MAX_RESPAWN_ATTEMPTS: u32 = 3;
const BASE_BACKOFF_MS: u64 = 500;

async fn respawn_with_backoff(attempt: u32) -> bool {
    if attempt >= MAX_RESPAWN_ATTEMPTS {
        return false; // Permanent death
    }
    let backoff = Duration::from_millis(BASE_BACKOFF_MS * 2u64.pow(attempt));
    // Add jitter: +/- 25%
    let jitter_range = backoff.as_millis() as u64 / 4;
    let jitter = rand::rng().random_range(0..=jitter_range * 2) as i64 - jitter_range as i64;
    let actual = Duration::from_millis((backoff.as_millis() as i64 + jitter).max(0) as u64);
    tokio::time::sleep(actual).await;
    true
}
```

### Pattern 5: Run Controller (Duration vs Iteration)
**What:** A top-level select! that races duration timeout against iteration completion.
**When to use:** Always -- the engine entry point.

```rust
async fn run_controller(
    duration: Option<Duration>,
    max_iterations: Option<u64>,
    cancel: CancellationToken,
    iteration_counter: Arc<AtomicU64>,
) {
    tokio::select! {
        // Duration limit
        _ = async {
            if let Some(d) = duration {
                tokio::time::sleep(d).await;
            } else {
                std::future::pending::<()>().await; // Never completes
            }
        } => {
            cancel.cancel();
        }
        // Iteration limit reached (signaled by VU canceling token)
        _ = cancel.cancelled() => {
            // Already cancelled by a VU hitting the limit
        }
        // Ctrl+C handler
        _ = handle_ctrl_c(cancel.clone()) => {
            // Handled inside handle_ctrl_c
        }
    }
}
```

### Pattern 6: Ramp-Up with Linear Stagger
**What:** Instead of spawning all VUs at once, space them linearly over the ramp-up duration.
**When to use:** When `--ramp-up` is specified.

```rust
async fn spawn_vus_with_rampup(
    vu_count: u32,
    ramp_up: Option<Duration>,
    tracker: &TaskTracker,
    // ... other params
) -> Instant {
    let ramp_up_end;
    let test_start = Instant::now();

    if let Some(ramp_duration) = ramp_up {
        let delay_per_vu = ramp_duration / vu_count;
        for i in 0..vu_count {
            tracker.spawn(vu_task(i, /* ... */));
            if i < vu_count - 1 {
                tokio::time::sleep(delay_per_vu).await;
            }
        }
        ramp_up_end = Instant::now();
    } else {
        for i in 0..vu_count {
            tracker.spawn(vu_task(i, /* ... */));
        }
        ramp_up_end = test_start; // No ramp-up, all metrics count
    }

    ramp_up_end
}
```

### Anti-Patterns to Avoid
- **Shared mutable MetricsRecorder behind Arc<Mutex>:** Never. Use the single-owner + mpsc channel pattern. Mutex contention under load destroys latency accuracy.
- **Unbounded mpsc channel for samples:** Always use bounded. Unbounded can cause OOM under sustained high throughput if the aggregator falls behind.
- **Polling CancellationToken in a tight loop:** Use `cancel.cancelled().await` in `tokio::select!`, not `cancel.is_cancelled()` in a busy loop. The `is_cancelled()` check is only for quick pre-flight checks before starting work.
- **Spawning VUs without TaskTracker:** Without TaskTracker, you cannot reliably wait for all VU tasks to complete during drain. JoinSet works but accumulates return values (memory waste for fire-and-forget tasks).
- **Recording metrics during ramp-up:** Track a `ramp_up_end: Instant` and filter samples with `timestamp < ramp_up_end` when computing final report. Do NOT skip recording entirely -- the aggregator still needs to count them for live display.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cooperative cancellation | AtomicBool + Notify + manual wakeup | `tokio_util::sync::CancellationToken` | Cancel-safe in select!, parent/child hierarchies, integrates with TaskTracker, well-tested |
| Task drain tracking | Manual counter + CondVar | `tokio_util::task::TaskTracker` | Handles edge cases around task panics, drop ordering, ABA problems |
| Weighted random selection | Manual cumulative sum + binary search | `rand::distr::weighted::WeightedIndex` | O(log N) sampling, handles zero weights, edge cases around floating point |
| Terminal cursor management | Raw ANSI escape codes | `indicatif::MultiProgress` | Cross-platform, handles resize, piping detection, thread-safe updates |

**Key insight:** The concurrency primitives (CancellationToken, TaskTracker) are the most critical items to NOT hand-roll. They have subtle edge cases around cancellation ordering, memory reclamation, and panic handling that are extremely hard to get right in custom implementations.

## Common Pitfalls

### Pitfall 1: Coordinated Omission in the Metrics Channel
**What goes wrong:** If the mpsc channel is full (backpressure), VUs block on `send().await`, which inflates their measured latency because the timer was started before the send. The recorded duration includes channel wait time, not just server response time.
**Why it happens:** Bounded channels provide backpressure, which is good for memory, but bad if the timing boundary is wrong.
**How to avoid:** Size the mpsc channel generously (e.g., 10,000) so it never fills under normal operation. VUs should capture `Instant::now()` BEFORE `send_request()` and compute duration BEFORE sending to the channel. The sample already contains the pre-computed duration, so channel backpressure doesn't affect accuracy.
**Warning signs:** P99 latency in metrics is much higher than actual server response time.

### Pitfall 2: Aggregator Task Starvation in select!
**What goes wrong:** If the mpsc channel receives messages faster than the aggregator can process them, the `tick.tick()` branch in `tokio::select!` never fires because `sample_rx.recv()` always wins.
**Why it happens:** `tokio::select!` is biased by default toward the first matching branch.
**How to avoid:** Use `biased;` in select! to explicitly control priority, OR drain messages in batches using `try_recv()` in a loop after each `recv()` returns, then let the tick fire naturally. Better: use `tokio::select!` with the tick branch first and a `biased;` directive.
**Warning signs:** Live display never updates even though the test is running.

### Pitfall 3: VU Respawn Storm
**What goes wrong:** If the MCP server is down, all VUs fail simultaneously, all attempt exponential backoff respawn at similar times, creating thundering herd.
**Why it happens:** All VUs started at the same time, so their failures and retry timers are synchronized.
**How to avoid:** Add jitter to the exponential backoff (+/- 25% of the backoff duration). The per-VU backoff naturally desynchronizes after the first retry due to different failure timing.
**Warning signs:** Periodic spikes of N simultaneous connection attempts followed by N simultaneous failures.

### Pitfall 4: Watch Channel Lagging
**What goes wrong:** No practical issue -- watch channels don't lag. But a common mistake is using `broadcast` channel instead, which DOES have lag and will drop messages for slow receivers.
**Why it happens:** Confusion between watch (latest-value) and broadcast (queue-per-receiver) semantics.
**How to avoid:** Use `tokio::sync::watch` for the snapshot, not `broadcast`. The display only cares about the latest snapshot, not every intermediate value.
**Warning signs:** "lagged" errors in the display task.

### Pitfall 5: Iteration Counter Race at Boundary
**What goes wrong:** Multiple VUs read the iteration counter, all see it below the limit, all proceed, resulting in more iterations than requested.
**Why it happens:** Check-then-act race on the AtomicU64.
**How to avoid:** Use `fetch_add(1, Ordering::Relaxed)` and check the RETURNED value (pre-increment). If the returned value >= max, this VU should stop and cancel. Minor overshoot (by up to N-1 iterations where N is VU count) is acceptable and expected.
**Warning signs:** Total iterations significantly exceeding the configured limit.

### Pitfall 6: Ramp-Up Metrics Pollution
**What goes wrong:** Ramp-up period metrics (where the server is warming up and not all VUs are active) are included in the final report, making throughput and latency numbers misleading.
**Why it happens:** No filtering of warm-up samples.
**How to avoid:** Track `ramp_up_end: Instant`. In the final report, create a fresh `MetricsRecorder` and replay only samples with `timestamp >= ramp_up_end`. For the live display, show ALL samples (including ramp-up) since the user wants to see real-time progress. The ramp-up exclusion applies ONLY to the final summary.
**Warning signs:** RPS in final report is lower than observed steady-state RPS during the test.

## Code Examples

### LoadTestEngine Public API
```rust
/// Top-level load test engine configuration and entry point.
pub struct LoadTestEngine {
    config: LoadTestConfig,
    base_url: String,
    max_iterations: Option<u64>,
    ramp_up: Option<Duration>,
    no_color: bool,
}

impl LoadTestEngine {
    pub fn new(config: LoadTestConfig, base_url: String) -> Self { /* ... */ }
    pub fn with_iterations(mut self, n: u64) -> Self { /* ... */ }
    pub fn with_ramp_up(mut self, duration: Duration) -> Self { /* ... */ }
    pub fn with_no_color(mut self, no_color: bool) -> Self { /* ... */ }

    /// Run the load test. Returns the final MetricsSnapshot.
    pub async fn run(&self) -> Result<MetricsSnapshot, LoadTestError> { /* ... */ }
}
```

### indicatif k6-Style Display
```rust
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use colored::Colorize;

fn create_display(no_color: bool) -> (MultiProgress, ProgressBar) {
    if no_color {
        colored::control::set_override(false);
    }

    let mp = MultiProgress::new();
    let status_bar = mp.add(ProgressBar::new_spinner());
    status_bar.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {wide_msg}")
            .unwrap()
    );
    status_bar.enable_steady_tick(std::time::Duration::from_millis(100));
    (mp, status_bar)
}

fn format_live_status(snap: &MetricsSnapshot, elapsed: Duration, active_vus: u32) -> String {
    let rps = if elapsed.as_secs() > 0 {
        snap.total_requests as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };

    format!(
        "  vus: {}  |  rps: {:.1}  |  p95: {}ms  |  errors: {} ({:.1}%)  |  elapsed: {}s",
        active_vus.to_string().green(),
        rps,
        snap.p95,
        if snap.error_count > 0 {
            snap.error_count.to_string().red().to_string()
        } else {
            snap.error_count.to_string()
        },
        snap.error_rate * 100.0,
        elapsed.as_secs(),
    )
}
```

### Ramp-Up Exclusion Window Tracking
```rust
/// Samples collected during ramp-up are tagged with timestamps.
/// The aggregator records all samples (for live display) but the
/// engine filters by ramp_up_end when computing the final report.
struct RampUpWindow {
    ramp_up_end: Instant,
    /// Buffer of all samples during ramp-up (replayed into fresh recorder for final report)
    all_samples: Vec<RequestSample>,  // Only needed if we want exact replay
}

// Simpler approach: just use two recorders
struct DualRecorder {
    live: MetricsRecorder,    // All samples (for display)
    report: MetricsRecorder,  // Post-ramp-up only (for final report)
    ramp_up_end: Instant,
}

impl DualRecorder {
    fn record(&mut self, sample: &RequestSample) {
        self.live.record(sample);
        if sample.timestamp >= self.ramp_up_end {
            self.report.record(sample);
        }
    }
}
```

### Active VU Tracking
```rust
use std::sync::atomic::{AtomicU32, Ordering};

/// Shared active VU counter. VUs increment on start, decrement on exit.
/// Display reads this atomically for the live VU count.
struct ActiveVuCounter(Arc<AtomicU32>);

impl ActiveVuCounter {
    fn new() -> Self { Self(Arc::new(AtomicU32::new(0))) }
    fn increment(&self) { self.0.fetch_add(1, Ordering::Relaxed); }
    fn decrement(&self) { self.0.fetch_sub(1, Ordering::Relaxed); }
    fn get(&self) -> u32 { self.0.load(Ordering::Relaxed) }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual task tracking with JoinSet | `TaskTracker` from tokio-util 0.7.10+ | 2023 | TaskTracker doesn't accumulate return values (no memory leak), integrates with CancellationToken |
| `rand::distributions::WeightedIndex` | `rand::distr::weighted::WeightedIndex` | rand 0.9/0.10 (2024-2025) | Module path changed from `distributions` to `distr` in rand 0.9+ |
| AtomicBool + Notify for cancellation | `CancellationToken` from tokio-util | tokio-util 0.7.3+ | Cancel-safe, hierarchical, well-tested |
| `atty` crate for terminal detection | `std::io::IsTerminal` trait | Rust 1.70 (2023) | Standard library now provides this, `atty` is deprecated |

**Deprecated/outdated:**
- `rand::distributions::WeightedIndex`: Use `rand::distr::weighted::WeightedIndex` in rand 0.10
- `atty::is(Stream::Stderr)`: Use `std::io::stderr().is_terminal()` from stdlib

## Open Questions

1. **mpsc Channel Buffer Size**
   - What we know: Bounded channel prevents OOM. Block size is 32 messages on 64-bit.
   - What's unclear: Optimal buffer size for N=100+ VUs with high throughput.
   - Recommendation: Use `N_VUS * 100` as buffer size (e.g., 1000 for 10 VUs). This gives each VU ~100 in-flight samples before backpressure. If testing reveals channel contention, increase to `N_VUS * 500`. The exact value is not critical because VU latency measurement happens before the channel send.

2. **Ramp-Up Exclusion Implementation**
   - What we know: User wants ramp-up metrics excluded from final report. Live display should show all metrics.
   - What's unclear: Whether to use dual-recorder approach or single-recorder with post-hoc filtering.
   - Recommendation: Use the dual-recorder approach (one for live, one for report). It's simpler than buffering and replaying samples, and the memory overhead of two HdrHistograms is negligible (~2KB each).

3. **RequestSample Needs to Be Send**
   - What we know: `RequestSample` contains `Instant` (which is `Send`) and `McpError` (needs to be `Send`).
   - What's unclear: Whether `McpError`'s `Clone` derive handles this.
   - Recommendation: `McpError` is already `Clone` (Phase 1). Verify it is also `Send + Sync`. Since it only contains primitive types (`i32`, `u16`, `String`), it should be. Add a compile-time assertion: `const _: () = { fn assert_send<T: Send>() {} fn check() { assert_send::<RequestSample>(); } };`

## Sources

### Primary (HIGH confidence)
- [Tokio Graceful Shutdown Guide](https://tokio.rs/tokio/topics/shutdown) - CancellationToken + TaskTracker patterns
- [tokio_util::sync::CancellationToken docs](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html) - API reference for CancellationToken
- [tokio_util::task::TaskTracker docs](https://docs.rs/tokio-util/latest/tokio_util/task/task_tracker/struct.TaskTracker.html) - API reference for TaskTracker, spawn/close/wait semantics
- [tokio::sync::mpsc docs](https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html) - Bounded channel API, backpressure behavior, cancel safety
- [rand::distr::weighted::WeightedIndex docs](https://docs.rs/rand/latest/rand/distr/weighted/struct.WeightedIndex.html) - Weighted random sampling API for rand 0.10
- [indicatif docs](https://docs.rs/indicatif/latest/indicatif/) - MultiProgress, ProgressBar, ProgressStyle API
- [tokio::signal::ctrl_c docs](https://docs.rs/tokio/latest/tokio/signal/fn.ctrl_c.html) - Signal handling API

### Secondary (MEDIUM confidence)
- [Tokio Select Tutorial](https://tokio.rs/tokio/tutorial/select) - select! macro patterns verified against official docs
- [Tokio Channels Tutorial](https://tokio.rs/tokio/tutorial/channels) - mpsc and watch channel usage patterns
- [tokio-util feature flags](https://lib.rs/crates/tokio-util/features) - Confirmed `rt` feature needed for TaskTracker

### Tertiary (LOW confidence)
- k6 terminal output format: Based on general knowledge of k6 CLI output style. Exact format details not verified against k6 source code, but the user has explicitly described the desired format in CONTEXT.md.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries are official Tokio ecosystem or already in project dependencies. Versions verified against docs.rs.
- Architecture: HIGH - mpsc+watch dual-channel pattern is the canonical Tokio metrics aggregation pattern, documented in official Tokio guides.
- Pitfalls: HIGH - Coordinated omission, select! fairness, and iteration counter races are well-documented concurrency pitfalls with established solutions.

**Research date:** 2026-02-26
**Valid until:** 2026-03-26 (stable ecosystem, no major changes expected)
