# Architecture Research

**Domain:** MCP Server Load Testing (CLI-integrated)
**Researched:** 2026-02-26
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
                          cargo pmcp loadtest
                                 |
                                 v
┌─────────────────────────────────────────────────────────────────┐
│                        CLI Layer                                 │
│  ┌──────────────┐  ┌──────────────┐                             │
│  │ Config Parser │  │ CLI Command  │                             │
│  │ (TOML)       │  │ (clap)       │                             │
│  └──────┬───────┘  └──────┬───────┘                             │
│         │                 │                                      │
├─────────┴─────────────────┴──────────────────────────────────────┤
│                     Orchestrator Layer                            │
│  ┌──────────────────────────────────────────────────────┐        │
│  │                  Test Executor                        │        │
│  │  (phases: warmup → ramp-up → sustain → ramp-down)    │        │
│  └──────────┬──────────────────────┬────────────────────┘        │
│             │                      │                             │
├─────────────┴──────────────────────┴─────────────────────────────┤
│                     Engine Layer                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │ VU Scheduler │  │ Metrics      │  │ Live Display │           │
│  │ (tokio tasks)│  │ Collector    │  │ (indicatif)  │           │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘           │
│         │                 │                 │                    │
├─────────┴─────────────────┴─────────────────┴────────────────────┤
│                     Transport Layer                               │
│  ┌──────────────────────────────────────────────────────┐        │
│  │               MCP HTTP Client (reqwest)               │        │
│  │  initialize → tools/list → tools/call (per VU)        │        │
│  └──────────────────────────────────────────────────────┘        │
├──────────────────────────────────────────────────────────────────┤
│                     Output Layer                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                       │
│  │ Terminal  │  │ JSON     │  │ Report   │                       │
│  │ Summary   │  │ File     │  │ Structs  │                       │
│  └──────────┘  └──────────┘  └──────────┘                       │
└──────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Communicates With |
|-----------|----------------|-------------------|
| Config Parser | Deserialize TOML scenario files into typed config structs | CLI Command, Test Executor |
| CLI Command | Parse `cargo pmcp loadtest` args via clap, dispatch to executor | Config Parser, Test Executor |
| Test Executor | Orchestrate load test lifecycle (phases, timing, start/stop) | VU Scheduler, Metrics Collector, Live Display |
| VU Scheduler | Spawn/despawn virtual user tasks on the tokio runtime | MCP HTTP Client, Metrics Collector |
| Metrics Collector | Receive per-request timing samples, compute aggregates (p50/p95/p99, throughput, error rates) | VU Scheduler, Live Display, Report Structs |
| Live Display | Render real-time progress (indicatif bars, rolling stats) during execution | Metrics Collector |
| MCP HTTP Client | Execute MCP JSON-RPC calls (initialize, tools/list, tools/call) over streamable HTTP | Target MCP Server |
| Report Structs | Typed structs for the final report (serde-serializable to JSON and terminal) | Terminal Summary, JSON File |

### Data Flow

```
Scenario TOML
    |
    v
Config Parser ──→ LoadTestConfig { scenarios, users, duration, phases }
    |
    v
Test Executor ──→ creates VU Scheduler + Metrics Collector + Live Display
    |
    v
VU Scheduler ──→ spawns N tokio tasks, each running a VU loop:
    |              ┌──────────────────────────────────────────────┐
    |              │ VU Loop (one per virtual user):              │
    |              │   1. initialize (once)                       │
    |              │   2. tools/list (once)                       │
    |              │   3. loop { pick scenario step, tools/call } │
    |              │   4. record RequestSample to Metrics channel │
    |              └──────────────────────────────────────────────┘
    |
    v
Metrics Collector ←── receives RequestSample { timestamp, latency, status, error? }
    |                  via tokio::sync::mpsc channel (bounded, backpressure-safe)
    |
    ├──→ Live Display (periodic tick: update progress bars, rolling p50/rps)
    |
    v
Test Executor ──→ awaits phase completion, signals VU Scheduler to ramp down
    |
    v
Report Builder ──→ final aggregation: MetricsSnapshot → LoadTestReport
    |
    ├──→ Terminal Summary (formatted table with colored output)
    └──→ JSON File (.pmcp/loadtest-report-{timestamp}.json)
```

## Recommended Project Structure

```
src/
├── commands/
│   └── loadtest/          # CLI command module (new)
│       ├── mod.rs          # LoadtestCommand enum + execute()
│       └── display.rs      # Terminal output formatting
├── loadtest/              # Load testing engine (new)
│   ├── mod.rs             # Public API, re-exports
│   ├── config.rs          # TOML scenario config types
│   ├── executor.rs        # Test lifecycle orchestrator
│   ├── scheduler.rs       # VU spawning and ramp control
│   ├── client.rs          # MCP-aware HTTP client for VUs
│   ├── metrics.rs         # Sample collection and aggregation
│   └── report.rs          # Report structs and JSON serialization
```

### Structure Rationale

- **`commands/loadtest/`:** Follows the existing pattern where each CLI command gets a module under `commands/`. Contains only CLI concerns (arg parsing, terminal output), delegates logic to engine.
- **`loadtest/`:** Separate engine module at the same level as `deployment/`, `secrets/`, etc. This isolates the load testing domain logic from the CLI layer, matching the existing subsystem boundary pattern. This module has zero knowledge of clap or terminal formatting.
- **Separation of `display.rs` from `metrics.rs`:** Metrics collection is an engine concern; how to render those metrics is a CLI concern. This prevents the engine from pulling in `colored`/`indicatif` as hard dependencies.

## Architectural Patterns

### Pattern 1: Channel-Based Metrics Pipeline

**What:** Every virtual user sends `RequestSample` structs through a bounded `tokio::sync::mpsc` channel to a single Metrics Collector task. The collector aggregates samples into rolling windows and periodic snapshots.

**When to use:** Always for this system. The alternative (shared atomic counters or `Arc<Mutex<Vec>>`) either loses sample-level detail or creates lock contention at high throughput.

**Trade-offs:**
- Pro: Zero contention between VU tasks (each sends to channel, no shared state)
- Pro: Natural backpressure via bounded channel (if collector falls behind, VUs slow down, which is the correct behavior)
- Pro: Collector can flush/aggregate without holding any lock
- Con: Slight memory overhead for channel buffer (negligible at 10-50 VUs)

**Example:**
```rust
// VU task sends sample after each request
struct RequestSample {
    timestamp: Instant,
    latency: Duration,
    operation: OperationType,  // Initialize, ToolsList, ToolCall(name)
    status: RequestStatus,     // Success, Error(code), Timeout
    response_bytes: u64,
}

// Metrics collector receives and aggregates
async fn metrics_collector(
    mut rx: mpsc::Receiver<RequestSample>,
    snapshot_tx: watch::Sender<MetricsSnapshot>,
) {
    let mut aggregator = MetricsAggregator::new();
    while let Some(sample) = rx.recv().await {
        aggregator.record(sample);
        if aggregator.should_emit_snapshot() {
            let _ = snapshot_tx.send(aggregator.snapshot());
        }
    }
}
```

### Pattern 2: Phase-Driven Executor

**What:** The test executor drives the load test through explicit phases: warmup, ramp-up, sustain, ramp-down. Each phase has a duration and target VU count. The executor signals the VU scheduler to adjust concurrency at phase boundaries.

**When to use:** Always. This is how k6 (via "stages"), Gatling (via "injection profiles"), and wrk (via duration flags) all structure their execution. Without phases, users cannot distinguish "server is warming up" from "server is failing under load."

**Trade-offs:**
- Pro: Users get meaningful data (sustain-phase metrics are the real results)
- Pro: Ramp-up prevents thundering herd (all VUs hitting server simultaneously)
- Pro: Warmup phase lets JIT/caching effects settle before measurement
- Con: Slightly more config complexity than "just blast N requests"

**Example:**
```rust
struct LoadTestPhase {
    name: String,           // "warmup", "ramp-up", "sustain", "ramp-down"
    duration_secs: u64,
    target_users: u32,
}

// Executor drives phase transitions
async fn run_phases(
    phases: Vec<LoadTestPhase>,
    scheduler: &mut VuScheduler,
    metrics: &MetricsCollector,
) -> Result<LoadTestReport> {
    for phase in &phases {
        scheduler.set_target_users(phase.target_users);
        tokio::time::sleep(Duration::from_secs(phase.duration_secs)).await;
    }
    metrics.finalize()
}
```

### Pattern 3: Stateful MCP Virtual User

**What:** Each virtual user maintains its own MCP session state (session ID from initialize, cached tools list). The VU runs a loop: pick a scenario step, execute it as a JSON-RPC call, record the result. This models real MCP client behavior where a client initializes once and makes many tool calls.

**When to use:** Always for MCP load testing. Unlike generic HTTP load testing (where each request is independent), MCP has session semantics. The initialize handshake must happen once per VU, and subsequent tool calls happen within that session context.

**Trade-offs:**
- Pro: Accurately models real MCP client behavior (session-per-client)
- Pro: Can test session statefulness and concurrency handling on the server
- Pro: Initialize and tools/list overhead is measured separately from tool call latency
- Con: Slightly more complex than stateless HTTP blasting (but stateless blasting would produce misleading results for MCP)

**Example:**
```rust
async fn vu_loop(
    vu_id: u32,
    config: &ScenarioConfig,
    client: &McpClient,
    metrics_tx: mpsc::Sender<RequestSample>,
    cancel: CancellationToken,
) {
    // Phase 1: Initialize session
    let session = client.initialize().await;
    metrics_tx.send(RequestSample::from_initialize(&session)).await;

    // Phase 2: Discover tools
    let tools = client.tools_list().await;
    metrics_tx.send(RequestSample::from_tools_list(&tools)).await;

    // Phase 3: Execute scenario steps in loop
    while !cancel.is_cancelled() {
        for step in &config.steps {
            let result = client.tool_call(&step.tool, &step.arguments).await;
            metrics_tx.send(RequestSample::from_tool_call(&result)).await;
        }
    }
}
```

## How Established Tools Architect Their Engines

### k6 (Grafana)
k6 uses a VU-centric model. Each VU runs a JavaScript scenario function in a loop. The engine has three core layers: (1) a **VU Scheduler** that manages the lifecycle of goroutines (one per VU), handling ramp-up/ramp-down via "executors" (constant-vus, ramping-vus, constant-arrival-rate, etc.), (2) a **Metrics Engine** that collects tagged metric samples from VUs via channels, aggregates them into periodic "sub-metrics" thresholds, and (3) **Output Extensions** that consume aggregated metrics (stdout summary, JSON, cloud, InfluxDB). The key architectural insight from k6 is that the executor abstraction (how load is shaped over time) is decoupled from the scenario (what each VU does). For our use case, we adopt the VU-centric model and channel-based metrics but do not need the executor plugin system -- a simple phase list suffices for v1.

### drill (Rust)
drill is a Rust HTTP load testing tool using tokio for async I/O. It reads YAML scenario files ("benchmark files") containing a sequence of HTTP requests. The architecture is flat: parse config, spawn N tokio tasks each running the request sequence in a loop, collect timing data, print a summary. drill's simplicity is instructive -- it demonstrates that a Rust load tester does not need complex abstractions. Each task independently makes HTTP requests using reqwest, records latencies into a shared structure, and the main task aggregates after completion. drill's limitation is the lack of real-time reporting and phase control, which we will add.

### wrk / wrk2
wrk uses an event-loop-per-thread model with epoll/kqueue. Each thread runs C-level non-blocking socket I/O with a configurable number of connections. wrk2 adds a key innovation: **coordinated omission correction**, where the tool detects when a slow response delays the next request (making results appear better than reality) and corrects for it by recording "expected send time" vs "actual send time." This is relevant for our design because MCP tool calls are sequential within a VU (not pipelined), which means coordinated omission is less of a concern -- but we should still record the intended schedule vs actual timing for accuracy.

### Gatling
Gatling (Scala/JVM) uses an actor-model architecture with Akka. Scenarios are defined as DSLs that compile into execution plans. The injection profile (how users are added over time) is separated from the scenario (what each user does). Gatling's reporting is its strength: it produces detailed HTML reports with response time distributions, percentile charts, and request/second graphs. For our JSON report format, Gatling's report structure (with global stats, per-request stats, and time-series data) is the model to follow.

### Architecture Decision: What We Take From Each

| Tool | Lesson Applied | Lesson Skipped |
|------|---------------|----------------|
| k6 | VU-centric model, channel-based metrics, phase/stage execution | JavaScript runtime, executor plugin system, cloud integration |
| drill | Rust/tokio simplicity, reqwest for HTTP, TOML-like config | Lack of real-time reporting, no phase control |
| wrk2 | Coordinated omission awareness, latency distribution focus | C-level socket I/O (we use reqwest), thread-per-core model |
| Gatling | Report structure (global + per-request + time-series), injection profiles | Scala/Akka, HTML reports, DSL-based scenarios |

## Integration with Existing cargo-pmcp

### CLI Integration Point

The load test command integrates into cargo-pmcp following the exact same pattern as `TestCommand`:

```rust
// In main.rs Commands enum:
/// Load test MCP servers under concurrent load
Loadtest {
    #[command(subcommand)]
    command: commands::loadtest::LoadtestCommand,
}

// In execute_command():
Commands::Loadtest { command } => {
    command.execute()?;
}
```

The `LoadtestCommand` enum follows the existing subcommand pattern:

```rust
pub enum LoadtestCommand {
    /// Run a load test scenario
    Run {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        scenario: Option<PathBuf>,
        #[arg(long, default_value = "10")]
        users: u32,
        #[arg(long, default_value = "30")]
        duration: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Generate a load test scenario from server capabilities
    Init {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}
```

### Reuse Existing Dependencies

| Dependency | Existing Use | Load Test Use |
|-----------|-------------|---------------|
| `tokio` (full, rt-multi-thread) | Per-command runtime | VU task spawning, timers, channels |
| `reqwest` (json, rustls-tls) | pmcp.run API, schema export | MCP JSON-RPC HTTP client for VUs |
| `serde` + `serde_json` | Config deserialization, schema | Scenario config, report serialization |
| `toml` | `.pmcp/deploy.toml` | `.pmcp/loadtest.toml` scenario config |
| `indicatif` | Deployment progress bars | Live load test progress (VU count, RPS, p50) |
| `colored` | Terminal output formatting | Report summary colorization |
| `chrono` | Token expiry handling | Report timestamps |
| `anyhow` | Error handling | Error handling |

**New dependencies needed:** None for v1. `hdrhistogram` would be ideal for accurate percentile computation (used by wrk2, k6, and Gatling), but a simple sorted-vec approach works for 10-50 VU scale. If precision matters later, `hdrhistogram` (a well-maintained Rust crate) can be added as a single focused dependency.

### Config Placement

Load test config lives at `.pmcp/loadtest.toml`, consistent with existing `.pmcp/deploy.toml`:

```toml
[target]
url = "https://my-server.example.com/mcp"
transport = "streamable-http"

[load]
users = 20
duration_secs = 60
ramp_up_secs = 10

[[scenarios]]
name = "heavy-tool-usage"
weight = 80  # 80% of VUs run this scenario

[[scenarios.steps]]
method = "tools/call"
tool = "query_database"
arguments = { query = "SELECT * FROM users LIMIT 10" }

[[scenarios]]
name = "discovery-only"
weight = 20

[[scenarios.steps]]
method = "tools/list"
```

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1-10 VUs | Single tokio runtime, unbounded metrics channel, simple vec-based percentile calc |
| 10-50 VUs | Bounded mpsc channel (1024 buffer), periodic snapshot emission (every 1s), HdrHistogram for percentiles |
| 50+ VUs (future) | This is pmcp.run's domain. CLI stays at single-machine scale. If needed: connection pooling in reqwest, separate metrics aggregator task, consider `tokio::sync::broadcast` for multi-consumer snapshots |

### Scaling Priorities

1. **First bottleneck:** reqwest connection pool exhaustion. At 50 VUs making concurrent requests, the default reqwest pool may queue connections. Fix: configure `reqwest::Client` with explicit `pool_max_idle_per_host(users * 2)` so each VU has headroom.
2. **Second bottleneck:** Metrics channel backpressure. If the collector cannot keep up with high-frequency samples, the bounded channel causes VU tasks to await. Fix: batch samples (send Vec<RequestSample> every 100ms instead of per-request) or increase channel buffer. At 50 VUs this is unlikely to be a real problem.

## Anti-Patterns

### Anti-Pattern 1: Shared Mutable Metrics State

**What people do:** Use `Arc<Mutex<Vec<Sample>>>` or `Arc<RwLock<HashMap>>` for metrics collection, where every VU task locks, writes, and unlocks.
**Why it's wrong:** Lock contention scales linearly with VU count. At 50 VUs making requests every 100ms, that is 500 lock acquisitions per second competing for one mutex. This adds artificial latency to the VU tasks themselves, which corrupts the latency measurements (you are measuring your own lock contention, not server latency).
**Do this instead:** Use `mpsc::channel` (one sender clone per VU, one receiver in the collector). Zero contention, natural backpressure, and the collector can aggregate without any lock.

### Anti-Pattern 2: Measuring Wall Clock Instead of Request Latency

**What people do:** Record `Instant::now()` before sending the request and `Instant::elapsed()` after receiving the response, but include DNS resolution, TLS handshake, and connection establishment in every measurement.
**Why it's wrong:** The first request to a host includes DNS + TLS (~50-200ms). Subsequent requests reuse the connection. If you include setup overhead in all samples, your p99 is dominated by cold-start connection establishment, not server processing time.
**Do this instead:** Warm up each VU's connection during the warmup phase (one throwaway request). Use reqwest's connection pooling so subsequent requests measure only server response time. Report connection establishment latency separately if needed.

### Anti-Pattern 3: Sequential-Within-Phase Execution

**What people do:** Run all warmup requests, then all ramp-up requests, then all sustain requests -- blocking between phases.
**Why it's wrong:** Real load is continuous. A "sustain" phase should transition smoothly from ramp-up, not pause-reset-restart. Pausing between phases gives the server recovery time that real traffic would not.
**Do this instead:** Phase transitions adjust the target VU count on a scheduler that adds/removes VU tasks gradually. VUs that are already running continue uninterrupted through phase boundaries. Only newly added VUs need initialization.

### Anti-Pattern 4: Treating MCP Like Stateless HTTP

**What people do:** Fire independent HTTP POST requests at the MCP endpoint without session initialization, treating it like a REST API benchmark.
**Why it's wrong:** MCP servers expect an initialize handshake first. Skipping it means every request either fails (server rejects unauthenticated session) or triggers server-side session creation overhead that real clients would not cause repeatedly. The benchmark measures the wrong thing.
**Do this instead:** Each VU initializes once, caches the session, and makes tool calls within that session. This models real MCP client behavior (Claude Code, Cursor, etc.) which initialize once and make many calls.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Target MCP Server | HTTP POST (JSON-RPC over streamable HTTP) | reqwest client with connection pooling; single `Client` instance shared across VUs |
| pmcp.run (future) | JSON report upload via existing OAuth/GraphQL client | Report format designed for pmcp.run consumption from day one |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| CLI (commands/loadtest) <-> Engine (loadtest/) | Direct function calls; CLI owns tokio runtime, passes config structs to engine | Engine returns `LoadTestReport` struct; CLI formats for terminal and writes JSON |
| Engine (executor) <-> VU Scheduler | `VuScheduler::set_target_users(n)` adjusts spawned task count | Scheduler owns CancellationTokens for graceful VU shutdown |
| VU tasks <-> Metrics Collector | `mpsc::Sender<RequestSample>` (bounded channel, 1024 buffer) | One sender clone per VU; single receiver in collector task |
| Metrics Collector <-> Live Display | `watch::Receiver<MetricsSnapshot>` (latest-value channel) | Display task polls on tick interval (1s); always gets latest snapshot |
| Engine <-> Report | `MetricsCollector::finalize() -> LoadTestReport` | Synchronous after all VU tasks complete; final aggregation |

## Suggested Build Order

The dependency graph between components dictates build order:

```
Phase 1: Foundation (no dependencies between these)
  ├── config.rs       (TOML scenario types -- everything depends on this)
  ├── report.rs       (Report structs -- output types needed by metrics)
  └── client.rs       (MCP HTTP client -- VUs depend on this)

Phase 2: Engine Core (depends on Phase 1)
  ├── metrics.rs      (depends on report.rs for sample/snapshot types)
  └── scheduler.rs    (depends on client.rs for VU execution)

Phase 3: Orchestration (depends on Phase 2)
  └── executor.rs     (depends on metrics.rs + scheduler.rs)

Phase 4: CLI Integration (depends on Phase 3)
  ├── commands/loadtest/mod.rs    (depends on executor.rs + config.rs)
  └── commands/loadtest/display.rs (depends on report.rs + metrics.rs)
```

**Rationale:** Config types and report structs are pure data -- no async, no I/O, no dependencies. Build and test these first. The MCP client can be built and tested against a real server independently. Metrics and scheduler are the engine core that wires everything together. The executor is the top-level orchestrator. CLI integration is last because it is pure glue.

## Sources

- k6 architecture: well-documented open source (Go), VU-centric model with goroutines, channel-based metrics, executor abstraction for load shaping. Grafana maintains it. (HIGH confidence -- direct knowledge of codebase)
- drill: Rust load testing tool on crates.io, simple tokio-based architecture, YAML scenarios. (HIGH confidence -- Rust ecosystem tool with public source)
- wrk/wrk2: C-based HTTP benchmark tools, event-loop-per-thread model, HdrHistogram for latency recording, coordinated omission correction in wrk2. (HIGH confidence -- widely referenced in load testing literature)
- Gatling: Scala/Akka load testing tool, actor model, injection profiles, HTML report generation. (HIGH confidence -- established tool with extensive documentation)
- Existing cargo-pmcp architecture: derived from reading the actual source code of main.rs, commands/test/mod.rs, deployment/config.rs, and the codebase ARCHITECTURE.md. (HIGH confidence -- primary source)

---
*Architecture research for: MCP Server Load Testing*
*Researched: 2026-02-26*
