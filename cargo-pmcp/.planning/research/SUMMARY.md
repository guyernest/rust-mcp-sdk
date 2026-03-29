# Project Research Summary

**Project:** cargo-pmcp loadtest
**Domain:** MCP server load testing (Rust CLI tool)
**Researched:** 2026-02-26
**Confidence:** MEDIUM-HIGH

## Executive Summary

This project adds a `cargo pmcp loadtest` subcommand to the existing cargo-pmcp CLI tool, giving MCP server developers a native way to measure how their servers perform under concurrent load. The key architectural insight from research is that existing generic load testing tools (k6, wrk, drill, goose) all treat MCP as opaque HTTP and cannot express MCP session semantics, tool-level granularity, or JSON-RPC correctness. The right approach is to compose focused Rust crates — most of which are already present in cargo-pmcp — into a custom load engine rather than wrapping a framework. Only one significant new dependency is needed: `hdrhistogram` for accurate latency percentile computation.

The recommended architecture follows a four-layer pattern (CLI, Orchestrator, Engine, Transport) that mirrors how k6, Gatling, and drill are built internally. Each virtual user is a stateful tokio task that performs its own MCP `initialize` handshake, maintains its own session, and executes tool calls in a loop. A channel-based metrics pipeline (mpsc for samples, watch for snapshots) collects latency data with zero contention between VU tasks and feeds it into an HdrHistogram aggregator that computes accurate P50/P95/P99 values. All of this maps cleanly onto existing cargo-pmcp patterns: TOML config at `.pmcp/loadtest.toml`, progress bars via indicatif, and a JSON report designed for pmcp.run ingestion.

The primary risk is measurement correctness. Load testing has a well-documented failure mode called coordinated omission where the generator's closed-loop request model makes a slow server appear faster than it is — sometimes by 10-100x on P99 latency. This must be addressed in the measurement engine from day one using open-loop scheduling and HdrHistogram's `record_corrected_value()`. A secondary risk is that MCP's stateful session model is easy to get wrong under load: sharing sessions across virtual users, skipping initialization, or mishandling streamable HTTP framing all produce misleading results. These are not hard to prevent but require deliberate design choices that should be baked into the VU lifecycle from the start.

## Key Findings

### Recommended Stack

The stack is almost entirely composed of dependencies already present in cargo-pmcp. The existing tokio runtime provides async task scheduling for virtual users, reqwest handles HTTP with built-in connection pooling, serde/serde_json covers JSON-RPC serialization, indicatif delivers live progress bars, and colored/console handle terminal formatting. The only new dependency is `hdrhistogram = "7.5"` for latency percentile computation. An optional `uuid = { version = "1", features = ["v4"] }` addition generates unique run IDs for JSON reports.

The research explicitly recommends against adopting load testing frameworks like Goose (framework lock-in, HTTP/HTML orientation) or ratatui (full TUI event loop, 15+ transitive deps for progress bars). The "build, don't import a framework" principle applies here because MCP's protocol requirements — initialize handshake, session lifecycle, JSON-RPC error semantics — cannot be expressed through framework abstractions without fighting them.

**Core technologies:**
- `tokio` (existing): VU task spawning via `tokio::spawn`, `JoinSet` for lifecycle management, `mpsc`/`watch` channels for metrics pipeline
- `reqwest` (existing): HTTP client with connection pooling — one `Client` instance shared across VUs with explicit `pool_max_idle_per_host` configuration
- `hdrhistogram` (new, `7.5`): Latency percentile computation with coordinated omission correction via `record_corrected_value()`
- `serde` + `serde_json` (existing): TOML scenario config parsing and JSON report serialization
- `indicatif` (existing): Multi-progress bars for live test status, updated on a timer (not on every request)
- `clap` (existing): Add `Loadtest` variant to existing subcommand enum

### Expected Features

The load testing space is mature. Users coming from k6, locust, or artillery will not tolerate missing table-stakes features. The research is clear: if any of the must-have features are absent, users will simply use k6 with raw HTTP POSTs and accept the loss of MCP protocol awareness.

**Must have (table stakes):**
- Concurrent virtual users — every load tester provides this; without it this is just sequential benchmarking
- Latency percentiles (P50/P95/P99) — averages hide tail latency; all mature tools report percentiles
- Throughput metrics (req/s) — fundamental capacity metric
- Error rate with classification — separate HTTP 4xx, 5xx, timeout, and JSON-RPC errors
- Duration control (`--duration 60s` or `--iterations 1000`) — universal in all tools
- TOML scenario config — complex tests should not require 30 CLI flags
- Live terminal progress — show the test is running with real-time RPS and error count
- Terminal summary report — colorized table at test completion
- JSON report output — structured results for CI/CD and pmcp.run

**Should have (competitive):**
- MCP protocol awareness — THE differentiator: proper session lifecycle, tool-level metrics, JSON-RPC error semantics
- Ramp-up phases — step load from 0 to target VU count to avoid thundering herd
- Breaking point detection — automatically find the load level where error rate or latency spikes
- Tool-level metrics breakdown — per-tool P50/P99/RPS for multi-tool servers
- Comparison mode — diff two JSON reports for before/after deployment testing
- CI/CD threshold assertions (`--assert p99<500ms`) — automated pass/fail gating
- Auth-aware testing — reuse existing cargo-pmcp OAuth token flow

**Defer (v2+):**
- Scenario workflows with variable extraction — multi-step tool call chains with data dependencies
- Correctness validation under load — verify response contents, not just HTTP status
- Static HTML report generation — JSON + jq covers most needs
- Distributed load generation — this is pmcp.run's domain, not the CLI's

**Explicit anti-features to avoid:**
- Distributed load generation in the CLI (complexity explosion; pmcp.run's job)
- GUI/web dashboard (scope creep; JSON output feeds external tools)
- JavaScript/Lua scripting engine (TOML scenarios cover 90% of use cases)
- Record-and-replay (fragile; sessions contain timestamps and IDs)

### Architecture Approach

The load engine follows a four-layer separation: CLI layer (clap parsing, terminal output) -> Orchestrator layer (phase-driven test executor) -> Engine layer (VU scheduler, metrics collector, live display) -> Transport layer (MCP-aware HTTP client). The CLI and engine are in separate module directories (`commands/loadtest/` and `loadtest/`) matching the existing subsystem boundary pattern. The engine has zero knowledge of clap or terminal rendering; it returns a `LoadTestReport` struct that the CLI formats.

The three non-negotiable architecture patterns are: (1) channel-based metrics pipeline using `mpsc::Sender<RequestSample>` per VU and a single aggregator task — never shared mutable state; (2) phase-driven executor that transitions through warmup, ramp-up, sustain, and ramp-down, adjusting VU count at boundaries without interrupting running VUs; and (3) stateful MCP virtual users that initialize once per VU, cache session state, and execute tool calls within that session, modeling real MCP client behavior.

**Major components:**
1. `loadtest/config.rs` — TOML scenario types: target URL, VU count, phases, scenario steps; pure data, no async
2. `loadtest/client.rs` — MCP-aware HTTP client: initialize handshake, tools/list, tools/call, session token management
3. `loadtest/metrics.rs` — HdrHistogram wrapper, RequestSample aggregation, periodic snapshot emission via watch channel
4. `loadtest/scheduler.rs` — VU task spawning and ramp control via CancellationTokens
5. `loadtest/executor.rs` — Phase orchestration: drives scheduler through phases, signals completion
6. `loadtest/report.rs` — `#[derive(Serialize)]` report structs with schema version field for pmcp.run
7. `commands/loadtest/mod.rs` — CLI integration: clap args, dispatches to executor
8. `commands/loadtest/display.rs` — Terminal output: indicatif progress bars, colored summary table

**Build order (dictated by dependency graph):**
Phase 1: `config.rs`, `report.rs`, `client.rs` (pure data, no dependencies between them)
Phase 2: `metrics.rs`, `scheduler.rs` (depend on Phase 1 types)
Phase 3: `executor.rs` (depends on Phase 2)
Phase 4: CLI integration (pure glue, depends on Phase 3)

### Critical Pitfalls

1. **Coordinated omission** — The closed-loop request model (send, await response, send next) makes a slow server appear faster by 10-100x at P99. Avoid by using open-loop scheduling: pre-compute a request schedule, fire at wall-clock times regardless of response arrival, and use `hdrhistogram::record_corrected_value()`. This must be baked in from day one; retrofitting costs a full rewrite of the measurement pipeline.

2. **MCP session state violations** — Sharing session tokens across virtual users, or skipping the initialize handshake, produces results that do not model real MCP client behavior. Each VU must perform its own `initialize` -> `initialized` sequence, carry its own session token on all subsequent requests, and handle re-initialization if the token expires (401).

3. **reqwest connection pool starvation** — The default pool limits throttle concurrency, causing the generator to queue internally and inflate measured latency with its own wait time. Configure `pool_max_idle_per_host(users * 2)` and set `tcp_nodelay(true)`. On macOS, check `ulimit -n` at startup and warn if it is below `VU count * 2`.

4. **Generator overhead in measurements** — Recording `Instant::now()` around the entire request-plus-parse cycle includes tokio scheduling jitter and deserialization time. Separate the hot path (request sending) from the recording path via the mpsc channel. Keep indicatif updates on a 100ms timer, never on every request completion.

5. **Streamable HTTP framing failures under load** — Response parsing that works at 1 RPS breaks at 100 RPS due to chunked transfer encoding and TCP segmentation. Use `reqwest::Response::json::<T>()` for standard responses and a buffered SSE parser for streaming. Test explicitly with artificially fragmented responses before layering load generation on top.

**Secondary pitfalls to address early:**
- Clock resolution self-test at startup (macOS jitter under load can make sub-millisecond measurements unreliable)
- No default `--target` URL; require explicit flag with warning if URL matches production patterns
- Hard cap at 100 VUs for v1 with confirmation prompt above 50
- Never inline auth tokens in TOML; reference via environment variables or `cargo pmcp secret`

## Implications for Roadmap

Based on the dependency graph from ARCHITECTURE.md and the pitfall-to-phase mapping from PITFALLS.md, the natural phase structure is:

### Phase 1: Foundation — Config, Transport, and Measurement Primitives

**Rationale:** Everything else depends on these three. Config types define the shape of scenarios. The MCP HTTP client is the transport layer all VUs share. Getting measurement right (coordinated omission, clock resolution, connection pool configuration) from the beginning prevents an expensive rewrite later. These three components have no dependencies on each other and can be built in parallel.

**Delivers:** A validated MCP client that can initialize a session and call tools, typed TOML config structs, and a RequestSample recording pipeline with HdrHistogram that correctly handles coordinated omission.

**Implements from FEATURES.md:** Foundation for all table-stakes features; TOML scenario config (table stakes); timeout configuration.

**Avoids from PITFALLS.md:** Coordinated omission (address in metrics.rs from day one), connection pool starvation (configure reqwest Client here), streamable HTTP framing failures (validate in client.rs), clock resolution issues (add startup self-check here).

**Research flag:** Standard patterns, skip research-phase. Reqwest, hdrhistogram, and tokio channels are well-documented. MCP client code can be adapted from existing cargo-pmcp internals.

### Phase 2: Engine Core — VU Scheduler, Executor, and Live Display

**Rationale:** With foundation types in place, the engine that drives virtual users can be built. The stateful VU lifecycle (initialize once, execute tool calls in a loop) is the critical correctness requirement. The phase-driven executor and indicatif live display are tightly coupled to the scheduler and should be built together.

**Delivers:** A working load engine that spawns N concurrent VUs, each with their own MCP session, drives them through a configurable duration test, and displays live progress. A basic terminal summary report at completion.

**Implements from FEATURES.md:** Concurrent virtual users (P1), MCP session lifecycle (P1), live terminal progress (P1), terminal summary report (P1), duration control (P1), throughput metrics (P1), latency percentiles (P1), error rate with classification (P1).

**Implements from ARCHITECTURE.md:** `scheduler.rs` with CancellationTokens, `executor.rs` phase driver, `commands/loadtest/display.rs` with indicatif MultiProgress.

**Avoids from PITFALLS.md:** MCP session state violations (each VU owns its session), generator overhead in measurements (separate hot path from recording via mpsc channel), synchronous progress bar updates (timer-based, not per-request).

**Research flag:** Standard patterns, skip research-phase. VU-centric model with channel-based metrics is well-established (k6, drill). Indicatif MultiProgress is already used in the project.

### Phase 3: Reports and CLI Integration

**Rationale:** With the engine working, wire it into the cargo-pmcp CLI and produce structured output. JSON report format should be designed with pmcp.run's expected schema from day one, including schema version field. CI/CD pass/fail gating is a low-cost addition at this stage.

**Delivers:** `cargo pmcp loadtest run` and `cargo pmcp loadtest init` commands fully integrated into the CLI. JSON report written to `.pmcp/loadtest-report-{timestamp}.json`. Terminal summary with colorized metrics table. The loadtest init command generates a starter `.pmcp/loadtest.toml` from server capabilities.

**Implements from FEATURES.md:** CLI interface (P1), machine-readable JSON output (P1), configuration file support (P1), human-readable terminal report (P1).

**Implements from ARCHITECTURE.md:** `commands/loadtest/mod.rs`, `commands/loadtest/display.rs`, `loadtest/report.rs` with schema versioning.

**Avoids from PITFALLS.md:** Incremental report writes (write atomically after test completion), missing schema version (include `"schema_version": "1.0"` from day one), missing reproducibility (include full config and timestamp in report).

**Research flag:** Standard patterns, skip research-phase. Follows existing cargo-pmcp command patterns exactly.

### Phase 4: Load Shaping and Enhanced Metrics

**Rationale:** Once the basic engine is validated with real users, add load shaping (ramp-up, breaking point detection) and enhanced metrics (tool-level breakdown). These features require the foundation to be solid first — ramp-up requires the phase-driven executor to be working correctly, and breaking point detection requires reliable metrics collection.

**Delivers:** Ramp-up/ramp-down phases (step load avoids thundering herd), automatic breaking point detection, per-tool latency and throughput breakdown, comparison mode for before/after testing.

**Implements from FEATURES.md:** Ramp-up phases (P2), breaking point detection (P2), tool-level metrics (P2), comparison mode (P2).

**Avoids from PITFALLS.md:** Sequential-within-phase execution anti-pattern (phase transitions must adjust running VU count smoothly, not pause-restart), throughput calculation error (only count steady-state window for RPS).

**Research flag:** Breaking point detection algorithm needs design work during planning — define the degradation criteria (error rate threshold, latency threshold, or both). Standard ramp-up patterns are well-documented.

### Phase 5: Auth and CI/CD Integration

**Rationale:** Auth-aware testing reuses existing cargo-pmcp OAuth infrastructure; this is low-complexity once the core is working. CI/CD threshold assertions (`--assert p99<500ms`) are high-value additions at this stage. These are deliberately deferred until the core is stable to avoid building CI integration on a shifting foundation.

**Delivers:** Load tests that authenticate using existing cargo-pmcp OAuth token flow. Assertion flags for CI/CD pass/fail gating. Exit codes that convey test result (0 = all assertions pass, non-zero = failure).

**Implements from FEATURES.md:** Auth-aware testing (P2), CI/CD threshold assertions (P2).

**Avoids from PITFALLS.md:** Plaintext auth tokens in TOML (reference via environment variables or `cargo pmcp secret` system).

**Research flag:** Standard patterns, skip research-phase. OAuth reuse is straightforward; assertion flag pattern is well-established in k6 and artillery.

### Phase Ordering Rationale

- Phases 1-2 are strictly ordered by dependency: you cannot build a VU scheduler without config types and an MCP client; you cannot build the executor without the scheduler.
- Phase 3 (CLI integration) could technically be done earlier but is deliberately deferred until the engine is working — wiring an unfinished engine into the CLI creates integration noise.
- Phases 4-5 are independent of each other and could be reordered based on user demand; the ordering above prioritizes load shaping (Phase 4) because it is a more fundamental capability than CI/CD integration.
- The build order from ARCHITECTURE.md reinforces this: `config.rs` -> `client.rs` -> `metrics.rs` + `scheduler.rs` -> `executor.rs` -> CLI integration.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 4 (Breaking Point Detection):** The degradation detection algorithm needs a concrete design — what constitutes "breaking" (error rate crossing a threshold, P99 exceeding a limit, or throughput plateau)? Research existing approaches (k6 thresholds, Gatling assertions) and define the TOML schema for specifying thresholds before implementation.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** hdrhistogram, reqwest, tokio channels are all well-documented with stable APIs. MCP client initialization pattern is understood from the existing codebase.
- **Phase 2 (Engine Core):** VU-centric load engine with channel-based metrics is a canonical pattern documented by k6, drill, and Gatling. indicatif is already in use.
- **Phase 3 (CLI Integration):** Follows exact same pattern as existing cargo-pmcp commands (TestCommand, DeployCommand). No novel patterns.
- **Phase 5 (Auth/CI/CD):** OAuth reuse is straightforward; assertion flags match established patterns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Nearly all dependencies are already in the project and verified against the Cargo.toml lockfile. Only hdrhistogram is new — it is a well-established crate (used by vector.dev, linkerd-proxy). Version numbers should be confirmed against crates.io at implementation time. |
| Features | MEDIUM-HIGH | Table-stakes features derived from k6/locust/artillery (HIGH confidence stable tools). Differentiator features derived from MCP protocol analysis (HIGH confidence). mcpdrill competitor analysis is LOW confidence — the repository was not directly inspectable. |
| Architecture | HIGH | Derived from direct reading of existing cargo-pmcp source code plus well-documented patterns from k6, drill, wrk2, and Gatling. Channel-based metrics and VU-centric model are canonical for the domain. |
| Pitfalls | HIGH | Coordinated omission is Gil Tene's well-documented result. Connection pool and MCP session pitfalls follow directly from reqwest and MCP spec documentation. Platform-specific pitfalls (macOS fd limits, clock resolution) are OS-level facts. |

**Overall confidence:** HIGH for the core approach. The main uncertainty is mcpdrill's current feature set, which does not affect the build decision but affects competitive positioning claims.

### Gaps to Address

- **mcpdrill current state:** The only MCP-specific competitor could not be inspected directly. Before writing marketing copy or positioning, fetch and review `https://github.com/bc-dunia/mcpdrill` README to verify the "Node-based, lacks PMCP integration" characterization. This does not affect implementation decisions.
- **HdrHistogram version pinning:** Research used `7.5` based on training data. Confirm the latest compatible version on crates.io before adding to Cargo.toml.
- **Breaking point detection algorithm:** The TOML schema and detection logic for Phase 4 need concrete design. Recommend a planning spike at Phase 4 kickoff using k6 thresholds and Gatling assertions as reference models.
- **pmcp.run JSON report schema:** The report format is described as "designed for pmcp.run" but the exact expected schema fields were not available. Coordinate with pmcp.run requirements before finalizing `report.rs` structs in Phase 3.

## Sources

### Primary (HIGH confidence)
- Existing cargo-pmcp source code (`main.rs`, `commands/test/`, `commands/deploy/`, `Cargo.toml`) — verified all "already present" dependency claims
- MCP specification (modelcontextprotocol.io) — session lifecycle, initialize/initialized handshake, streamable HTTP transport, Mcp-Session header
- k6 architecture and documentation — VU-centric model, channel-based metrics, executor abstraction, stages for load shaping
- hdrhistogram crate (docs.rs/hdrhistogram) — coordinated omission correction via `record_corrected_value()`, histogram merge for aggregation
- Gil Tene, "How NOT to Measure Latency" (Azul Systems, 2013) — definitive reference on coordinated omission

### Secondary (MEDIUM confidence)
- reqwest documentation (docs.rs/reqwest) — connection pool configuration, `pool_max_idle_per_host`, `tcp_nodelay`
- tokio documentation (docs.rs/tokio) — runtime configuration, mpsc/watch channels, `tokio_metrics` for task monitoring
- drill (Rust load tester on crates.io) — TOML-like config pattern, tokio-based request loop simplicity
- wrk/wrk2 — coordinated omission correction in practice, HdrHistogram for latency recording
- Gatling (Scala/Akka) — report structure model (global stats + per-request stats + time-series), injection profile pattern

### Tertiary (LOW confidence)
- mcpdrill (`https://github.com/bc-dunia/mcpdrill`) — referenced in PROJECT.md as "Node-based, lacks PMCP integration"; repository could not be directly inspected; feature characterization may be outdated

---
*Research completed: 2026-02-26*
*Ready for roadmap: yes*
