# Phase 4: Load Shaping and Tool Metrics - Research

**Researched:** 2026-02-26
**Domain:** Multi-stage load shaping, breaking point auto-detection via rolling window, per-MCP-tool metrics collection and reporting
**Confidence:** HIGH

## Summary

Phase 4 adds three capabilities to the existing load test engine: composable multi-stage load profiles (ramp-up/hold/ramp-down), automatic breaking point detection via rolling window analysis, and per-tool metrics reporting. All three build directly on the existing architecture -- the mpsc/watch channel pipeline, the `MetricsRecorder` with HdrHistogram, and the terminal summary/JSON report systems.

The biggest architectural change is replacing the engine's flat "spawn N VUs at once" model with a stage-driven scheduler that spawns and retires VUs according to a `[[stage]]` array in the TOML config. The `LoadTestEngine::run()` method must become a stage-sequencing loop rather than a single spawn-all-then-wait block. Breaking point detection is a passive observer layered on top of the metrics aggregator -- it reads rolling window data and emits warnings without interrupting the test. Per-tool metrics require extending `RequestSample` with an optional tool/resource/prompt identifier string and adding per-identifier HdrHistograms in the recorder.

**Primary recommendation:** Implement stage scheduling as a controller loop in the engine that adjusts VU count at phase boundaries, add a `BreakingPointDetector` struct that consumes `MetricsSnapshot` values from the watch channel and applies threshold-based detection on a rolling window, and extend `MetricsRecorder` with a `HashMap<String, Histogram>` for per-tool-name latency tracking.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Composable stages via `[[stage]]` array blocks in TOML config
- Each stage defines target VU count and duration
- Linear ramp curves only (no stepped or custom curves)
- If no `[[stage]]` blocks defined, flat load (all VUs start immediately) -- backwards compatible with Phase 2 behavior
- VU teardown on ramp-down: VU finishes its current scenario iteration before exiting (no mid-request cancellation)
- Breaking point detection always on by default -- no flag needed to enable
- Report and continue: mark breaking point in output/report but keep running the full test
- Self-calibrating via rolling window: compare recent metrics against a rolling window of earlier measurements (no prior run or first-stage baseline needed)
- Sensible built-in defaults for thresholds (e.g. error rate spike, latency degradation) -- not user-configurable in this phase
- Live terminal warning when breaking point is detected (e.g. "Breaking point detected at 35 VUs (error rate >10%)")
- Terminal: grouped table section after overall summary, one row per tool
- Show all tools (no truncation/limit) -- MCP servers typically have bounded tool sets
- Per-tool metrics: P50/P95/P99 latency, requests/sec, total requests, error count, error rate (same depth as overall)
- JSON report: extended detail beyond terminal -- includes full histogram, min/max/mean, error breakdown by type per tool
- Terminal summary shows overall metrics only (not per-stage breakdown); per-stage data available in JSON
- Live progress line includes current phase label: `[ramp-up 2/3] VUs: XX | req/s: 120 | errors: 0`
- Breaking point detection shows live warning line when triggered
- End summary is aggregate across all stages (overall only in terminal)

### Claude's Discretion
- VU count display format during ramp (current/target vs current only)
- Rolling window size and exact threshold defaults for breaking point detection
- Per-stage metrics depth in JSON report
- Exact terminal formatting of the per-tool table

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LOAD-04 | User can define ramp-up/hold/ramp-down phases to gradually increase load | Composable `[[stage]]` blocks in TOML config. Stage scheduler in engine drives VU spawning/retiring over time. Linear interpolation for ramp curves. CancellationToken per-VU for graceful teardown. Backwards-compatible: no `[[stage]]` means flat load. |
| MCP-02 | Metrics are reported per MCP tool (latency, throughput, errors per tool) | Extend `RequestSample` with optional `tool_name: Option<String>`. Add `per_tool_recorders: HashMap<String, ToolMetrics>` to `MetricsRecorder` where each `ToolMetrics` holds its own success/error HdrHistograms. Terminal renders grouped table; JSON includes extended histogram data. |
| METR-06 | Load test can auto-detect breaking point where performance degrades | `BreakingPointDetector` struct with a rolling window of recent snapshots. Compares recent error rate and P99 latency against the rolling baseline. Emits breaking point event when thresholds are exceeded. Always on, report-and-continue semantics. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| hdrhistogram | 7.5 | Per-tool latency histograms (already in project) | Industry standard for latency percentile tracking; already proven in Phases 1-2 |
| tokio | 1 (full features) | Stage scheduling, VU lifecycle, async timers (already in project) | Existing runtime; stage transitions use `tokio::time::sleep` and `CancellationToken` |
| tokio-util | 0.7 | `CancellationToken` for per-VU graceful shutdown, `TaskTracker` (already in project) | Already used for VU lifecycle management |
| serde | 1 | `[[stage]]` config deserialization (already in project) | Already used for all config parsing |
| toml | 1.0 | TOML config with stage blocks (already in project) | Already used for `LoadTestConfig` |
| colored | 3 | Terminal coloring for per-tool table and breaking point warnings (already in project) | Already used in display and summary modules |
| indicatif | 0.18 | Progress bar with stage labels (already in project) | Already used in `LiveDisplay` |
| chrono | 0.4 | Timestamps for JSON report (already in project) | Already used in `report.rs` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| No new dependencies | - | - | All Phase 4 features are implementable with existing crate dependencies |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Rolling window via `VecDeque<MetricsSnapshot>` | Time-series crate (e.g., `ta-lib`, `streaming-stats`) | VecDeque is trivial, no new dependency needed for simple sliding window. Overkill for 2-3 threshold checks. |
| Per-VU `CancellationToken` for ramp-down | `tokio::sync::Notify` or channel-based signaling | CancellationToken is already the VU lifecycle primitive; adding per-VU tokens for selective shutdown is cleaner than introducing a new signaling mechanism. |
| `HashMap<String, ToolMetrics>` for per-tool tracking | Single merged histogram with labels | Per-tool histograms give independent percentile accuracy. Merged histogram with post-hoc filtering would lose percentile precision. |

**Installation:**
No new dependencies. All crates already in `Cargo.toml`.

## Architecture Patterns

### Current Engine Architecture (Phase 2-3 Baseline)

```
LoadTestEngine::run()
  |-- spawn N VU tasks (all at once, or with ramp-up stagger)
  |-- spawn metrics_aggregator (mpsc -> watch channel)
  |-- spawn display_loop (watch channel -> terminal)
  |-- controller: select! { sleep(duration), cancel, ctrl+c }
  |-- drain: tracker.close() + tracker.wait()
  |-- collect final snapshot from watch channel
```

### Pattern 1: Stage-Driven Scheduler
**What:** Replace the single spawn-all-then-wait controller with a loop that iterates over `[[stage]]` blocks, spawning or retiring VUs to match each stage's target count.

**When to use:** When the config contains `[[stage]]` blocks (new path). When no stages are defined, fall through to existing flat behavior (backwards-compatible path).

**Key design:**
```rust
// Config extension
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Stage {
    /// Target number of virtual users at end of this stage.
    pub target_vus: u32,
    /// Duration of this stage in seconds.
    pub duration_secs: u64,
}

// In LoadTestConfig:
pub struct LoadTestConfig {
    pub settings: Settings,
    pub scenario: Vec<ScenarioStep>,
    #[serde(default)]
    pub stage: Vec<Stage>,  // [[stage]] blocks
}
```

**Stage execution loop (inside engine run):**
```rust
// Pseudocode for stage scheduler
for (i, stage) in stages.iter().enumerate() {
    let stage_duration = Duration::from_secs(stage.duration_secs);
    let current_vus = active_vus.get();
    let target = stage.target_vus;

    // Update display with stage label
    stage_label_tx.send(format!("stage {}/{}", i + 1, stages.len()));

    if target > current_vus {
        // Ramp up: spawn (target - current) VUs with linear stagger
        let to_spawn = target - current_vus;
        let delay_per_vu = stage_duration / to_spawn;
        for j in 0..to_spawn {
            spawn_vu(/* ... */);
            sleep(delay_per_vu).await;
        }
    } else if target < current_vus {
        // Ramp down: signal (current - target) VUs to stop
        let to_remove = current_vus - target;
        for _ in 0..to_remove {
            vu_cancel_tokens.pop().map(|t| t.cancel());
        }
        // VUs finish current iteration then exit
    }
    // Hold at target for remaining stage duration
    // (if ramp consumed some time, hold for remainder)
}
```

**Critical detail -- per-VU cancellation for ramp-down:**
The existing engine uses a single shared `CancellationToken` for all VUs. For ramp-down, we need selective VU shutdown. Solution: each VU gets its own `CancellationToken` that is a *child* of the global token. Cancelling the child stops only that VU; cancelling the parent stops all VUs.

```rust
// Per-VU cancellation
let global_cancel = CancellationToken::new();
let mut vu_tokens: Vec<CancellationToken> = Vec::new();

// When spawning a VU:
let vu_cancel = global_cancel.child_token();
vu_tokens.push(vu_cancel.clone());
tracker.spawn(vu_loop(vu_id, ..., vu_cancel));

// When ramping down:
// Cancel the LAST spawned VU tokens first (LIFO order)
if let Some(token) = vu_tokens.pop() {
    token.cancel();  // VU finishes current iteration, then exits
}
```

`CancellationToken::child_token()` is a built-in method in `tokio_util::sync::CancellationToken`. When the parent is cancelled, all children are cancelled too. When a child is cancelled individually, only that child's subtree is affected. This is exactly the primitive needed for selective VU teardown during ramp-down while preserving the global Ctrl+C shutdown path.

**Confidence:** HIGH -- `CancellationToken::child_token()` is documented in tokio-util and already used by the project.


### Pattern 2: Rolling Window Breaking Point Detection
**What:** A stateless detector that compares recent metrics against a sliding window of historical measurements to identify the point where server performance degrades.

**When to use:** Always (always-on by default per user decision). Runs as a passive observer alongside the display loop.

**Key design:**
```rust
pub struct BreakingPointDetector {
    /// Rolling window of recent snapshots (newest at back).
    window: VecDeque<WindowSample>,
    /// Maximum window size (number of samples to keep).
    window_size: usize,
    /// Whether a breaking point has already been detected.
    detected: bool,
    /// The VU count at which breaking point was detected.
    breaking_point_vus: Option<u32>,
}

struct WindowSample {
    error_rate: f64,
    p99_ms: u64,
    active_vus: u32,
    timestamp: Instant,
}

impl BreakingPointDetector {
    /// Feed a new snapshot. Returns Some(BreakingPoint) if newly detected.
    pub fn observe(
        &mut self,
        snap: &MetricsSnapshot,
        active_vus: u32,
    ) -> Option<BreakingPoint> {
        // Add to window, drop oldest if full
        // Compare recent N samples against older N samples
        // Detect: error_rate spike > threshold OR p99 spike > threshold
        // Return event only on first detection (not repeated)
    }
}
```

**Threshold defaults (Claude's discretion):**
- **Error rate spike:** Recent error rate > 10% AND recent error rate > 2x the baseline window average
- **Latency degradation:** Recent P99 > 3x the baseline window P99 average
- **Window size:** 10 samples (at 2-second snapshot interval = 20 seconds of history)
- **Comparison:** Split window in half -- older half is baseline, newer half is recent. This makes it self-calibrating: no prior run needed.

**Rationale for defaults:**
- 10% absolute error rate threshold prevents false positives on low-count startup noise
- 2x relative spike catches degradation even at higher baseline error rates
- 3x P99 multiplier accounts for normal latency variance while catching real degradation
- 20-second window balances responsiveness with noise filtering

### Pattern 3: Per-Tool Metrics Recording
**What:** Extend the metrics pipeline to track latency histograms per tool name (not just per operation type).

**When to use:** Always. Every `RequestSample` from a `tools/call` operation already knows the tool name from the scenario step. Resources know their URI, prompts know their name.

**Key design:**
```rust
// Extend RequestSample with tool identifier
pub struct RequestSample {
    pub operation: OperationType,
    pub duration: Duration,
    pub result: Result<(), McpError>,
    pub timestamp: Instant,
    /// Tool/resource/prompt name for per-tool metrics.
    /// E.g., "calculate" for tools/call, "file:///data" for resources/read.
    pub tool_name: Option<String>,
}

// Per-tool metrics in the recorder
pub struct ToolMetrics {
    success_histogram: Histogram<u64>,
    error_histogram: Histogram<u64>,
    total_success: u64,
    total_errors: u64,
    error_category_counts: HashMap<String, u64>,
}

// In MetricsRecorder:
pub struct MetricsRecorder {
    // ... existing fields ...
    per_tool: HashMap<String, ToolMetrics>,
}
```

**Where the tool name originates:**
In `vu.rs`, the `execute_step` function matches on `ScenarioStep` and already has access to the tool name, URI, and prompt name. The tool name is passed into `RequestSample` construction:

```rust
// In execute_step or vu_loop, when building the sample:
let tool_name = match step {
    ScenarioStep::ToolCall { tool, .. } => Some(tool.clone()),
    ScenarioStep::ResourceRead { uri, .. } => Some(uri.clone()),
    ScenarioStep::PromptGet { prompt, .. } => Some(prompt.clone()),
};
```

**String cloning concern:** Tool names are short strings (typically <50 chars) and are cloned once per request sample. At 1000 req/s this is ~50KB/s of allocations -- negligible. If profiling shows concern, `Arc<str>` could be used, but premature optimization is not warranted.

### Pattern 4: Stage-Aware Live Display
**What:** Extend the display loop to show the current stage label and breaking point warnings.

**Key design:**
The display already reads from a `watch::Receiver<MetricsSnapshot>`. For stage labels, add a second watch channel or embed stage info in the snapshot. Simpler approach: embed stage label in a new `DisplayState` struct sent via watch:

```rust
pub struct DisplayState {
    pub snapshot: MetricsSnapshot,
    pub stage_label: Option<String>,  // e.g., "ramp-up 2/3"
    pub breaking_point: Option<String>,  // warning message
}
```

Alternatively, keep the watch channel for snapshots and use a separate `watch::Sender<StageInfo>` for stage metadata. Both work; a single combined `DisplayState` is simpler since both are consumed by the same display loop.

### Recommended Module Changes
```
src/loadtest/
  config.rs       # ADD: Stage struct, stage field in LoadTestConfig
  metrics.rs      # ADD: ToolMetrics, per_tool HashMap, ToolSnapshot
  engine.rs       # MODIFY: stage scheduler loop, per-VU cancel tokens
  display.rs      # MODIFY: stage label in progress line, breaking point warning
  summary.rs      # ADD: per-tool table section after overall metrics
  report.rs       # ADD: per-tool extended metrics, per-stage data, breaking_point field
  vu.rs           # MODIFY: pass tool_name to RequestSample, accept per-VU cancel token
  breaking.rs     # NEW: BreakingPointDetector struct
```

### Anti-Patterns to Avoid
- **Shared mutable state for per-tool metrics:** Do NOT use `Arc<Mutex<HashMap<String, Histogram>>>` shared across VU tasks. The existing mpsc channel pattern is correct -- VUs send samples, the single aggregator thread records them. Per-tool tracking happens inside the aggregator, not in VU tasks.
- **Blocking VU tasks during ramp-down:** Do NOT `abort()` VU tasks. Use cancellation tokens so VUs finish their current iteration cleanly. The user decision explicitly states "VU finishes its current scenario iteration before exiting."
- **Per-stage timer in VU loop:** Do NOT add stage awareness to VU tasks. VUs are stateless workers that loop until cancelled. Stage scheduling is the engine's responsibility. VUs should not know what stage they belong to.
- **Recomputing percentiles from raw samples:** Do NOT store raw latency values and compute percentiles at report time. HdrHistogram is designed for streaming computation -- record at ingest time, extract percentiles at report time. This is already the pattern and must continue.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-VU selective shutdown | Manual boolean flags or channel-based signaling | `CancellationToken::child_token()` from tokio-util | Built-in parent-child cancellation hierarchy; already in the project's dependency tree |
| Latency percentiles per tool | Custom sorted-array or reservoir sampling | `hdrhistogram::Histogram` per tool | HdrHistogram handles memory-efficient percentile tracking with coordinated omission correction; rolling your own would lose CO correction |
| Rolling window data structure | Custom ring buffer with manual index tracking | `std::collections::VecDeque` with `push_back()` / `pop_front()` | Standard library, O(1) push/pop, exactly the right abstraction for a sliding window |
| Linear interpolation for VU ramp | Custom math with floating point edge cases | Integer division: `stage_duration / vus_to_add` for uniform spacing | Simple integer math avoids floating point drift; tokio::time::sleep handles the actual delay |

**Key insight:** All Phase 4 features compose on top of existing primitives. No new crates needed. The main complexity is in the engine's stage scheduler loop and the per-VU cancellation wiring.

## Common Pitfalls

### Pitfall 1: VU Count Mismatch at Stage Boundaries
**What goes wrong:** If stage 1 targets 10 VUs and stage 2 targets 20 VUs, but some stage-1 VUs have died (respawn limit exceeded), the engine thinks it has 10 active VUs but actually has 7. Stage 2 then spawns 10 more (targeting 20 from perceived 10), ending up with 17 instead of 20.
**Why it happens:** The `active_vus` counter reflects actual running VUs, not the target. Dead VUs decrement the counter.
**How to avoid:** Always compute `to_spawn = stage.target_vus - active_vus.get()` using the *actual* live count, not the previous stage's target. This naturally handles dead VUs by spawning replacements.
**Warning signs:** Mismatch between displayed VU count and expected target in live output.

### Pitfall 2: Breaking Point False Positives During Ramp-Up
**What goes wrong:** During ramp-up from 0 to 50 VUs, the first few VUs might see high latency (server cold start, connection pool warming) that triggers false breaking point detection.
**Why it happens:** The rolling window hasn't accumulated enough baseline data; early high-latency samples dominate.
**How to avoid:** Require a minimum window fill before enabling detection (e.g., at least `window_size` samples before first evaluation). Alternatively, ignore the first N seconds of data.
**Warning signs:** Breaking point reported at very low VU counts (< 5).

### Pitfall 3: Stage Duration Consumed by Ramp
**What goes wrong:** If a stage says "target 50 VUs, duration 30s" and ramping from 10 to 50 takes 20 seconds (delay_per_vu * 40 VUs), only 10 seconds remain at peak. User expects 30 seconds at peak.
**Why it happens:** Ambiguous semantics: does "duration" mean total stage time or time-at-target?
**How to avoid:** Define stage duration as the TOTAL stage time (including ramp). The ramp happens within that window. Document clearly. This matches k6's behavior: each stage's duration is the total time for that stage, and VU count ramps linearly over that duration.
**Warning signs:** Users report tests ending earlier than expected.

### Pitfall 4: Per-Tool Histogram Memory with Many Tools
**What goes wrong:** If a server exposes 100+ tools and all are in the scenario, each gets its own success + error histogram pair. With HdrHistogram's default configuration, each histogram is ~10-20KB. 100 tools x 2 histograms = ~4MB.
**Why it happens:** The user decision says "show all tools (no truncation/limit)" because MCP servers typically have bounded tool sets. But edge cases exist.
**How to avoid:** This is not actually a problem for the stated use case. MCP servers typically have 5-20 tools. At 20 tools, memory is ~800KB -- trivial. If future evidence shows this matters, add lazy histogram initialization (only create when first sample arrives).
**Warning signs:** Memory usage spikes in extremely large-tool-count tests (unlikely for MCP).

### Pitfall 5: Watch Channel Backpressure During Fast Stage Transitions
**What goes wrong:** If stages transition rapidly (e.g., 5-second stages), the display loop may not process all watch channel updates before the next stage label change.
**Why it happens:** The watch channel only retains the latest value; intermediate values are dropped. This is actually correct behavior (display always shows most recent state), but stage transitions might flash by without being visible.
**How to avoid:** This is acceptable behavior. The watch channel's "latest value" semantics are correct for live display. Stage transitions are also logged in the JSON report per-stage data. No action needed.

## Code Examples

### Example 1: TOML Config with Stages
```toml
[settings]
virtual_users = 10    # ignored when [[stage]] is present
duration_secs = 120   # total test timeout (safety limit)
timeout_ms = 5000

[[stage]]
target_vus = 10
duration_secs = 15    # ramp from 0 to 10 over 15s

[[stage]]
target_vus = 50
duration_secs = 30    # ramp from 10 to 50 over 30s

[[stage]]
target_vus = 50
duration_secs = 60    # hold at 50 for 60s

[[stage]]
target_vus = 0
duration_secs = 15    # ramp down from 50 to 0 over 15s

[[scenario]]
type = "tools/call"
weight = 70
tool = "calculate"
arguments = { expression = "2+2" }

[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"
```

### Example 2: Per-Tool Snapshot in MetricsSnapshot
```rust
/// Per-tool metrics snapshot.
#[derive(Debug, Clone)]
pub struct ToolSnapshot {
    pub name: String,
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub min: u64,
    pub max: u64,
    pub mean: f64,
    pub total_requests: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub error_categories: HashMap<String, u64>,
}

// Extend MetricsSnapshot:
pub struct MetricsSnapshot {
    // ... existing fields ...
    /// Per-tool metrics (key is tool name string).
    pub per_tool: Vec<ToolSnapshot>,
}
```

### Example 3: Breaking Point Detector Integration
```rust
// In the engine's display/aggregator loop:
let mut detector = BreakingPointDetector::new(10); // 10-sample window

// Each time a new snapshot is published:
if let Some(bp) = detector.observe(&snapshot, active_vus.get()) {
    // Update display with warning
    eprintln!(
        "  {} Breaking point detected at {} VUs ({}: {})",
        "WARNING:".yellow().bold(),
        bp.vus,
        bp.reason,
        bp.detail,
    );
    // Store for JSON report
    breaking_point_event = Some(bp);
}
```

### Example 4: Per-Tool Terminal Table
```text
  per-tool metrics:

  tool                              reqs     rate    err%     p50     p95     p99
  ─────────────────────────────────────────────────────────────────────────────────
  tools/call: calculate             680    11.3/s    2.1%    42ms   120ms   350ms
  tools/call: search                120     2.0/s    8.3%    85ms   250ms   800ms
  resources/read: file:///data      200     3.3/s    0.0%    12ms    35ms    65ms
```

### Example 5: JSON Report Extension
```json
{
  "schema_version": "1.1",
  "breaking_point": {
    "detected": true,
    "vus": 35,
    "reason": "error_rate_spike",
    "detail": "Error rate 12.3% exceeds threshold (>10% and >2x baseline 3.1%)",
    "timestamp": "2026-02-26T14:23:45Z"
  },
  "stages": [
    {
      "index": 0,
      "target_vus": 10,
      "duration_secs": 15,
      "metrics": { /* per-stage ReportMetrics */ }
    }
  ],
  "per_tool": {
    "calculate": {
      "total_requests": 680,
      "success_count": 666,
      "error_count": 14,
      "error_rate": 0.021,
      "latency": {
        "p50_ms": 42,
        "p95_ms": 120,
        "p99_ms": 350,
        "min_ms": 5,
        "max_ms": 1200,
        "mean_ms": 68.3
      },
      "errors": { "timeout": 10, "jsonrpc": 4 }
    }
  }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single ramp-up Duration on engine | Composable `[[stage]]` array | Phase 4 | Replaces `with_ramp_up()` builder method with stage-driven scheduling |
| Per-operation-type metrics only | Per-tool-name granular metrics | Phase 4 | Extends OperationType tracking with tool-name-level histograms |
| No degradation detection | Rolling window breaking point auto-detect | Phase 4 | New capability, not replacing anything |
| Schema version 1.0 | Schema version 1.1 (additive fields) | Phase 4 | Backwards compatible: new fields are additive (breaking_point, stages, per_tool) |

**Deprecated/outdated:**
- `LoadTestEngine::with_ramp_up(Duration)`: Will be superseded by `[[stage]]` config blocks. If stages are present, the old ramp_up field is ignored. Could be removed or kept as a simple single-stage shorthand.

## Open Questions

1. **`settings.duration_secs` semantics when stages are present**
   - What we know: When stages are defined, the total test duration is the sum of stage durations. The `duration_secs` in settings becomes a safety timeout.
   - What's unclear: Should `duration_secs` be required when stages are present? Should it be auto-calculated?
   - Recommendation: Make `duration_secs` optional when stages are present (sum of stage durations is the implicit total). If both are specified, `duration_secs` acts as a safety ceiling. If neither stages nor duration is present, error.

2. **`settings.virtual_users` semantics when stages are present**
   - What we know: Stages define their own VU targets. The `virtual_users` setting is redundant.
   - What's unclear: Should it be an error to specify both, or silently ignored?
   - Recommendation: Ignore `virtual_users` when stages are present. Log a warning if both are specified. This preserves backwards compatibility (old configs still work) while stages take precedence.

3. **Report schema version bump**
   - What we know: New JSON fields (breaking_point, stages, per_tool) are additive.
   - What's unclear: Is this a 1.0 -> 1.1 minor bump or 2.0 major bump?
   - Recommendation: Bump to 1.1. All new fields are optional/additive. Existing 1.0 parsers should ignore unknown fields. This follows semver for data schemas.

4. **Breaking point detector placement**
   - What we know: It needs to observe snapshots periodically.
   - What's unclear: Should it run inside the metrics_aggregator, the display_loop, or as a separate task?
   - Recommendation: Run it inside a combined display+detection loop (or as part of the display task). It reads from the same watch channel as the display. This avoids a third task and keeps detection co-located with the output that shows the warning.

## Sources

### Primary (HIGH confidence)
- **Existing codebase** (`/Users/guy/Development/mcp/sdk/rust-mcp-sdk/cargo-pmcp/src/loadtest/`) -- all architecture patterns derived from reading the actual implementation across 8 source files
- **tokio-util CancellationToken** -- `child_token()` method is a first-class API in the crate already in use
- **hdrhistogram 7.5** -- per-tool histogram pattern follows the same `Histogram::<u64>::new(3)` with `auto(true)` established in Phase 1

### Secondary (MEDIUM confidence)
- **k6 stages pattern** -- the `[[stage]]` composable design is modeled after k6's `stages` option, which is the industry standard for load shaping in CLI tools. Users familiar with k6 will recognize the pattern. Verified against k6 documentation.

### Tertiary (LOW confidence)
- **Breaking point detection thresholds** -- the specific default values (10% error rate, 2x spike, 3x P99) are engineering judgment, not sourced from external authority. They should be validated empirically during implementation and adjusted if they produce too many false positives or miss real breaking points.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all patterns build on existing codebase
- Architecture: HIGH -- stage scheduler, per-tool metrics, and breaking point detector are straightforward extensions of the existing mpsc/watch/histogram pipeline
- Pitfalls: HIGH -- identified from direct codebase reading and k6 community patterns
- Breaking point thresholds: MEDIUM -- defaults are engineering judgment, need empirical validation

**Research date:** 2026-02-26
**Valid until:** 2026-03-26 (30 days -- stable domain, no fast-moving dependencies)
