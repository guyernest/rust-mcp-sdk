# Stack Research

**Domain:** HTTP load testing for MCP servers (Rust CLI tool)
**Researched:** 2026-02-26
**Confidence:** MEDIUM (versions verified against existing Cargo.toml lockfile and crates.io training data; live version checks were blocked -- flag individual crate versions for validation)

## Design Principle: Build, Don't Import a Framework

The project already has tokio, reqwest, serde, clap, and indicatif. MCP load testing has domain-specific requirements (JSON-RPC framing, initialize handshake, session lifecycle) that generic load test frameworks like Goose or Drill cannot express. The right approach is to compose focused, single-purpose crates into a custom load engine rather than wrapping or forking an existing load testing framework.

**Rationale:** Goose (the main Rust load test framework) is designed for HTTP/HTML workloads with its own task scheduling, reporting, and CLI. Adapting it to MCP's JSON-RPC-over-HTTP protocol with session semantics would require fighting the framework more than building from scratch. The project already owns the hard parts (tokio runtime, reqwest HTTP client, MCP protocol knowledge). What's missing are the measurement and presentation layers -- which are straightforward library additions.

## Recommended Stack

### Core Technologies (Already in cargo-pmcp)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| tokio | 1.46+ | Async runtime, task spawning for virtual users | Already in use. `tokio::spawn` per virtual user is the natural concurrency model. `JoinSet` for managing VU lifecycle. No new dependency. |
| reqwest | 0.12 | HTTP client for load generation | Already in use. Connection pooling built-in. One `Client` per virtual user with shared connection pool gives realistic load patterns. |
| serde / serde_json | 1.x | JSON-RPC request/response serialization | Already in use. MCP protocol is JSON-RPC; serde handles all serialization. |
| clap | 4.x | CLI argument parsing for `loadtest` subcommand | Already in use. Add `LoadTest` variant to existing subcommand enum. |
| toml | 0.9 | Scenario file parsing | Already in use. Consistent with `.pmcp/` config pattern. |
| indicatif | 0.18 | Progress bars during load test execution | Already in use. Multi-progress bars for per-VU status. |

### New Dependencies (Load Testing Specific)

| Library | Version | Purpose | Why Recommended | Confidence |
|---------|---------|---------|-----------------|------------|
| hdrhistogram | 7.5+ | HDR histogram for latency percentile calculation (P50/P95/P99) | Industry standard for latency measurement. Port of Gil Tene's HdrHistogram. Sub-microsecond recording overhead. Supports merge for combining per-VU histograms. Used by every serious load testing tool (Gatling, wrk2, Locust plugins). No viable alternative in Rust. | HIGH |
| tokio (Instant/Duration) | (already present) | Wall-clock timing for individual requests | `tokio::time::Instant` for measuring request latency. Zero additional dependency. | HIGH |
| chrono | 0.4 (already present) | Timestamps in JSON reports | Already a dependency. RFC3339 timestamps for report metadata. | HIGH |

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| indicatif | 0.18 (already present) | Multi-progress bars for live test status | During load test execution: overall progress, per-VU bars, live RPS counter. `MultiProgress` with `ProgressBar` per virtual user. | HIGH |
| console | 0.16 (already present) | Terminal width detection, styled output | For summary report table formatting. Already used by schema commands. | HIGH |
| colored | 3.x (already present) | Color-coded pass/fail in terminal reports | Green for healthy latencies, yellow for degraded, red for failures. Already in use. | HIGH |
| uuid | 1.x | Unique test run IDs in JSON reports | Generate unique run IDs for report correlation. Small, no-controversy crate. If you want to avoid the dependency, use a simpler scheme (timestamp + random). | MEDIUM |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `just` | Task runner for load test development workflow | Preferred over Makefile per project convention. Add `just loadtest-dev` recipes. |
| `cargo flamegraph` | Profile load generator overhead | Verify the load generator itself isn't the bottleneck. Important for validating measurement accuracy. |

## Cargo.toml Additions

```toml
# Load testing (add to existing [dependencies])
hdrhistogram = "7.5"

# Optional: unique run IDs for reports
uuid = { version = "1", features = ["v4"] }
```

That's it. Two new dependencies. Everything else is already present.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Latency histograms | hdrhistogram 7.5 | Manual Vec<Duration> + sort | hdrhistogram uses constant memory regardless of sample count, supports configurable precision, handles merge across threads, and computes percentiles in O(1). Rolling your own is slower, uses more memory, and gets the math wrong at the tails. |
| Latency histograms | hdrhistogram 7.5 | quantiles crate | quantiles (t-digest, etc.) gives approximate percentiles. HdrHistogram gives exact percentiles within configurable precision. For load testing, exact percentiles matter -- you need to know your real P99, not an approximation. |
| HTTP client | reqwest 0.12 (existing) | hyper 1.x directly | reqwest already wraps hyper with connection pooling, TLS, and cookie support. Going lower adds complexity without benefit for 10-50 VU scale. |
| HTTP client | reqwest 0.12 (existing) | ureq (blocking) | ureq is sync-only. Load testing needs async concurrency. |
| Concurrency model | tokio::spawn per VU | rayon thread pool | Load testing is I/O-bound (waiting on HTTP responses), not CPU-bound. tokio's cooperative scheduling is the right model. rayon is for CPU parallelism. |
| Concurrency model | tokio::spawn per VU | async-channel worker pool | Over-engineering for 10-50 VUs. Spawn a task per VU, each with its own request loop. Simpler, debuggable, and sufficient at this scale. |
| Terminal UI | indicatif 0.18 (existing) | ratatui 0.29+ | ratatui is a full terminal UI framework (charts, layouts, event loops). Massive overkill for progress bars + summary table. Would add ~15 transitive dependencies and require restructuring the CLI around a TUI event loop. indicatif does exactly what's needed: multi-progress bars with live stats. |
| Terminal UI | indicatif 0.18 (existing) | crossterm + manual rendering | crossterm is what ratatui uses underneath. Using it directly means reimplementing progress bars. indicatif already exists in the project. |
| Load test framework | Custom (compose crates) | goose 0.17+ | Goose is a full framework with its own CLI, task scheduling, metrics, HTML reports, and Gaggle distributed mode. It assumes HTTP/HTML workloads. MCP's JSON-RPC protocol, session initialization handshake, and tool-call semantics don't map to Goose's "Transaction" model without heavy wrapping. The framework overhead exceeds the implementation cost of the ~500 lines of custom load engine code needed. |
| Load test framework | Custom (compose crates) | drill 0.8+ | Drill is YAML-driven and oriented toward simple HTTP benchmarking. No programmatic API, no histogram support, and no way to express MCP session lifecycle. |
| JSON reporting | serde_json (existing) | Custom format | serde_json + `#[derive(Serialize)]` on report structs is the obvious choice. Already in the project. |
| Report format | JSON file | HTML report | JSON is machine-readable (CI/CD, pmcp.run). HTML is a presentation concern that pmcp.run can handle. CLI should produce data, not visualization. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| goose | Framework lock-in. Forces Goose's task model, CLI flags, and reporting pipeline. MCP protocol doesn't fit its HTTP/HTML assumptions. Adds ~30 transitive dependencies. | Custom load engine with tokio::spawn + reqwest + hdrhistogram |
| ratatui | Full TUI framework with event loop, layout system, and widget tree. Adds ~15 dependencies and forces architectural restructuring for a feature that needs progress bars and a summary table. | indicatif 0.18 (already present) for progress bars; println + colored for summary |
| criterion (for load testing) | criterion is a microbenchmark harness. It measures function execution time with statistical rigor, not HTTP endpoint performance under concurrent load. Different tool for a different problem. | hdrhistogram for latency recording; custom timing with tokio::time::Instant |
| wrk/wrk2 bindings | wrk is a C tool. Calling it via subprocess loses all MCP protocol awareness. Cannot do JSON-RPC framing, session initialization, or tool-call-specific measurement. | Native Rust implementation with reqwest |
| hyper (as direct HTTP client) | Lower-level than reqwest without meaningful benefit at 10-50 VU scale. Would need to manually handle connection pooling, TLS, redirects. | reqwest 0.12 (already present) |
| async-std | Alternative async runtime. Project is tokio-based. Mixing runtimes causes executor conflicts and doubles the dependency tree. | tokio (already present) |

## Stack Patterns by Variant

**If scale target increases beyond 50 VUs (future pmcp.run distributed mode):**
- Keep the same core stack
- Add `tokio::sync::Semaphore` to cap concurrent connections per machine
- Add histogram serialization/merge for combining results across machines (hdrhistogram supports this natively)
- Consider reqwest connection pool tuning (`pool_max_idle_per_host`)

**If real-time streaming results are needed (future pmcp.run integration):**
- Add `tokio::sync::broadcast` channel for live metric streaming
- Consumer can be a WebSocket endpoint or SSE stream
- hdrhistogram supports interval snapshots for time-windowed reporting

**If SSE/WebSocket transport support is added later:**
- reqwest handles SSE via `response.bytes_stream()`
- For WebSocket: add `tokio-tungstenite` (~2 dependencies)
- Latency measurement approach stays the same (Instant::now around request cycle)

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| hdrhistogram 7.5 | tokio 1.x, serde 1.x | Optional `serialization` feature for serde support. Default features are sufficient for in-memory histograms. |
| indicatif 0.18 | console 0.16 | indicatif uses console internally. Both are already in the project at compatible versions. |
| reqwest 0.12 | tokio 1.x, hyper 1.x | Uses hyper 1.x internally. Connection pooling configured via `ClientBuilder`. |
| uuid 1.x | serde 1.x | `v4` feature for random UUIDs. `serde` feature for automatic serialization. |

## Architecture Note for Downstream Consumer

The load test engine should be structured as:

```
src/commands/loadtest/
  mod.rs          -- CLI subcommand definition
  config.rs       -- TOML scenario parsing
  engine.rs       -- VU spawning, timing, coordination
  metrics.rs      -- hdrhistogram wrapper, metric collection
  report.rs       -- JSON report generation
  display.rs      -- Terminal output (indicatif + colored)
```

Key types:
- `Histogram` from hdrhistogram -- one per VU, merged at end
- `MetricsCollector` -- thread-safe aggregator using `Arc<Mutex<>>` or channel-based collection
- `VirtualUser` -- async task that loops: build request, time it, record to histogram
- `LoadTestReport` -- `#[derive(Serialize)]` struct matching pmcp.run's expected schema

## Sources

- hdrhistogram crate: Rust port of Gil Tene's HdrHistogram (HIGH confidence -- well-established, used in production by multiple Rust projects including vector.dev and linkerd-proxy)
- Goose load test framework: https://github.com/tag1consulting/goose (HIGH confidence -- reviewed API surface; confirmed HTTP/HTML orientation doesn't fit MCP)
- Existing cargo-pmcp Cargo.toml: Verified all "already present" claims against actual dependency list (HIGH confidence)
- ratatui: TUI framework successor to tui-rs (MEDIUM confidence -- version 0.29+ is training-data based; confirm exact latest version before use)
- indicatif MultiProgress API: Supports multiple concurrent progress bars with independent update (HIGH confidence -- already used in the project)
- reqwest connection pooling: Default pool with configurable `pool_max_idle_per_host` (HIGH confidence -- documented in reqwest API docs)

---
*Stack research for: MCP server load testing capability in cargo-pmcp*
*Researched: 2026-02-26*
