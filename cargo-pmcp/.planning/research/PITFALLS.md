# Pitfalls Research

**Domain:** MCP server load testing CLI (Rust, tokio, reqwest, streamable HTTP)
**Researched:** 2026-02-26
**Confidence:** HIGH (load testing measurement science is well-established; MCP-specific pitfalls based on protocol analysis)

## Critical Pitfalls

### Pitfall 1: Coordinated Omission

**What goes wrong:**
The load generator waits for each response before sending the next request on that virtual user's channel. When the server slows down, the generator automatically backs off its request rate — so it never actually measures the latency users experience during overload. Reported P99 latencies look 10-100x better than reality. Gil Tene (Azul Systems) documented this as the single most common measurement error in load testing tools, and most naive load generators still get it wrong.

**Why it happens:**
The natural implementation pattern is a loop: send request, await response, record latency, repeat. This closed-loop model means a slow response (say 2 seconds instead of 50ms) delays all subsequent requests by 2 seconds, but only the one slow response is recorded. The 40 requests that *would have arrived* during that 2-second window and experienced queuing delay are never generated, never measured.

**How to avoid:**
- Use open-loop request scheduling: pre-compute a Poisson or constant-rate schedule of send times before the test starts. Each virtual user fires requests at scheduled wall-clock times regardless of whether previous responses have returned.
- Track "intended send time" vs "actual response time" and compute corrected latency as `response_received - intended_send_time`, not `response_received - actual_send_time`.
- Use HdrHistogram (the `hdrhistogram` Rust crate) which is specifically designed for latency recording with coordinated omission correction built in. It supports `record_corrected_value()` with an expected interval parameter.
- Validate the fix: run a test against a server that artificially stalls every 100th request for 1 second. Without correction, P99 looks like ~1s. With correction, P99 should reflect the queuing effect on all requests that were delayed.

**Warning signs:**
- P99 latency is suspiciously close to P95 during load tests that visibly stress the server.
- Measured throughput perfectly matches the configured request rate even when the server is clearly struggling (no backpressure signal).
- Latency histograms show almost no values between P50 and P99 (a "cliff" rather than a smooth tail).

**Phase to address:**
Core measurement engine phase (Phase 1 or 2). This must be baked into the latency recording from day one. Retrofitting coordinated omission correction into an existing measurement system requires rewriting the entire request scheduling and recording pipeline.

---

### Pitfall 2: Reqwest Connection Pool Starvation Under Load

**What goes wrong:**
Reqwest's default `Client` maintains an internal connection pool. When spawning many concurrent virtual users sharing one `Client`, the pool's default limits (typically matching system defaults) throttle actual concurrency. Alternatively, creating one `Client` per virtual user defeats connection reuse and causes massive socket/FD exhaustion. Either way, the load generator becomes the bottleneck, not the server.

**Why it happens:**
Reqwest wraps hyper, which wraps a connection pool per host. The default pool has per-host connection limits. Developers either: (a) share one Client and hit pool limits causing queuing inside the generator itself, inflating measured latency with local wait time, or (b) create N clients and exhaust file descriptors or ephemeral ports.

**How to avoid:**
- Use one `reqwest::Client` shared across all virtual users (it is `Clone` and designed for sharing) but explicitly configure `pool_max_idle_per_host` and `pool_idle_timeout` to match the concurrency target. For 50 virtual users hitting one host, set `pool_max_idle_per_host(60)` to allow headroom.
- Disable connection pooling entirely if MCP servers are expected to handle per-request connections (set `pool_max_idle_per_host(0)`) to test worst-case. Provide this as a configuration option.
- Monitor file descriptor count during the test. On macOS the default `ulimit -n` is 256 which is dangerously low for load testing. Warn the user at startup if the limit is below the concurrency target multiplied by 2.
- Use `tcp_keepalive` and `tcp_nodelay(true)` to avoid Nagle algorithm delays that add phantom latency.

**Warning signs:**
- All virtual users report identical latency patterns (sign of pool serialization).
- "connection closed before message completed" errors at moderate load.
- Measured latency includes a consistent ~100ms base that disappears with fewer virtual users.
- macOS "Too many open files" errors at 30+ virtual users.

**Phase to address:**
HTTP client infrastructure phase. This is a configuration decision made when building the `Client` and should be validated with a test that confirms N concurrent in-flight requests are actually concurrent (e.g., hit a server endpoint that sleeps for 1s; N concurrent requests should complete in ~1s, not N seconds).

---

### Pitfall 3: MCP Session State Means Every Virtual User Needs Full Initialization

**What goes wrong:**
MCP is a stateful protocol. Each client session begins with an `initialize` handshake exchanging capabilities, followed by an `initialized` notification. Developers treat load testing like HTTP benchmarking (fire requests at endpoints) and skip or share initialization across virtual users. The server either rejects unauthenticated tool calls, returns wrong capability-dependent behavior, or crashes because it receives tool calls on sessions that were never initialized.

**Why it happens:**
HTTP load testing tools like wrk, hey, or k6 test stateless endpoints. Developers copy this mental model. MCP's streamable HTTP transport uses session tokens returned during initialization — sharing a session token across virtual users means the server sees one logical client sending impossibly concurrent requests, which is not what production looks like and may trigger server-side rate limiting or session corruption.

**How to avoid:**
- Each virtual user must perform its own `initialize` -> `initialized` handshake and maintain its own session ID/token for all subsequent requests.
- Measure initialization separately from steady-state tool calls. Report init latency as a distinct metric. Enterprise deployments care about cold-start time independently from tool call latency.
- Build a virtual user lifecycle: `connect -> initialize -> initialized -> [warm-up tool calls] -> [measured tool calls] -> shutdown`. The warm-up phase prevents JIT/cache effects from contaminating measurements.
- Include an explicit `--include-init` flag so users can choose whether initialization overhead is included in the latency report or excluded.

**Warning signs:**
- Server returns "method not found" or "not initialized" errors during the load test.
- All virtual users report identical session metadata (they are sharing one session).
- Server-side metrics show 1 active session instead of N.

**Phase to address:**
Virtual user lifecycle phase (early). The session management abstraction must exist before any load generation logic is built on top of it.

---

### Pitfall 4: Measuring Generator Overhead Instead of Server Latency

**What goes wrong:**
The load generator's own processing time (JSON serialization of MCP requests, response parsing, histogram recording, progress bar updates) is included in the latency measurement. At high request rates (thousands/sec), the generator becomes CPU-bound and its own overhead dominates the measurement. Reported latencies reflect tokio task scheduling delays, not server response time.

**Why it happens:**
The typical pattern `let start = Instant::now(); let resp = client.post(...).await?; let elapsed = start.elapsed();` includes everything between the two timestamps: HTTP client queuing, TLS handshake (if not reused), generator-side serialization, and response deserialization. On a busy tokio runtime, `start` may be recorded well before the request is actually sent because the task was descheduled.

**How to avoid:**
- Record timestamps as close to the network boundary as possible. In practice with reqwest this means measuring from just before `.send()` to just after the response headers arrive (not after body parsing). Use reqwest's built-in timing if available, or measure at the hyper layer.
- Run latency recording on a dedicated tokio task (or even a dedicated OS thread) that is not competing with request-sending tasks for CPU time. Use a channel (tokio::sync::mpsc) to send (timestamp, latency) pairs from sender tasks to a recorder task.
- Profile the generator itself under load. If `tokio::task::yield_now()` latency exceeds 1ms, the runtime is overloaded and measurements are contaminated. Consider using `tokio_metrics` crate to monitor task poll times.
- Keep progress bar updates off the hot path. Indicatif updates should happen on a separate timer (every 100ms), not on every request completion.

**Warning signs:**
- P50 latency increases linearly with virtual user count even when server CPU is idle.
- Generator machine CPU is at 100% during the test.
- Latency measurements show suspicious "steps" at powers-of-2 (tokio runtime scheduling artifacts).
- Disabling JSON response parsing reduces measured latency significantly.

**Phase to address:**
Measurement engine phase. Architect the separation between "hot path" (request sending) and "recording path" (latency collection, reporting) from the beginning.

---

### Pitfall 5: Ignoring MCP Tool Call Dependencies and Realistic Scenarios

**What goes wrong:**
The load test fires tool calls in isolation (e.g., 1000 calls to `search_users`), but production usage involves sequential chains: `list_databases` -> `query_table` -> `format_results`. Testing isolated calls misses the latency of dependent sequences and does not exercise server-side caching, connection, or state that accumulates across a session's lifetime.

**Why it happens:**
Isolated tool call testing is simpler to implement and reason about. Scenario-based testing requires a DSL for expressing dependencies, variable extraction from responses (use result of call A as input to call B), and more complex virtual user state machines.

**How to avoid:**
- Design the TOML scenario format to support sequential steps within a virtual user, with variable extraction (e.g., `$prev.result.database_id`). This is the same pattern used by k6, Gatling, and JMeter scenario scripting.
- Support both modes explicitly: "throughput mode" (blast isolated calls) and "scenario mode" (realistic user journeys). Report them as fundamentally different test types — they answer different questions.
- In scenario mode, measure end-to-end journey latency (entire chain) as the primary metric, with per-step breakdowns as secondary.
- Allow think time between steps (configurable pause to simulate human decision time). Without think time, scenario mode degrades into throughput mode and overwhelms the server unrealistically.

**Warning signs:**
- Load test shows excellent performance but production users report slowness (the test was measuring the wrong thing).
- Server performs well under isolated calls but crashes under scenario chains (state accumulation not tested).
- Tool calls that depend on initialization state (like listing resources after capabilities exchange) fail unpredictably.

**Phase to address:**
Scenario engine phase. The TOML format and variable extraction system should be designed early (even if scenario mode is implemented after throughput mode) so the format is not retrofitted.

---

### Pitfall 6: Clock and Timer Resolution on macOS vs Linux

**What goes wrong:**
`std::time::Instant` resolution varies by platform. On macOS, `mach_absolute_time` has nanosecond resolution but can have microsecond-level jitter under load. On Linux, `clock_gettime(CLOCK_MONOTONIC)` is generally rock-solid. More critically, when the load test runs inside a container or VM, clock sources may be virtualized with significantly worse resolution (10-100us). Sub-millisecond latency measurements become meaningless noise.

**Why it happens:**
Developers test on macOS (this project's primary dev platform per the environment info), see reasonable numbers, and ship. Users on Linux containers get different characteristics. Or vice versa: CI runs on Linux containers with poor clock sources and flaky tests result.

**How to avoid:**
- Use `std::time::Instant` (not `SystemTime`) for all latency measurements — it is monotonic and not subject to NTP adjustments. This is the correct choice and is well-documented.
- At startup, run a clock resolution self-test: measure the minimum distinguishable interval by calling `Instant::now()` in a tight loop and finding the smallest nonzero delta. If resolution is > 1ms, warn that sub-millisecond latency measurements are unreliable and suggest increasing the reporting granularity.
- Report latencies in microseconds, not nanoseconds. Nanosecond precision from an HTTP load test is meaningless given network jitter.
- In the JSON report, include a `clock_resolution_us` field so pmcp.run can flag reports from environments with poor timer resolution.

**Warning signs:**
- Latency histograms show many values clustered at exact multiples of a fixed interval (timer quantization).
- P50 latency on macOS differs significantly from the same test on Linux for the same server.
- Tests show sub-microsecond latencies (physically impossible for HTTP; indicates a measurement bug).

**Phase to address:**
Measurement engine phase. Include clock resolution self-check as part of the pre-test validation step.

---

### Pitfall 7: Streamable HTTP Transport Framing Mishandled Under Load

**What goes wrong:**
MCP streamable HTTP uses HTTP POST for client-to-server messages and optionally returns server-to-client messages in the response body (or via a separate GET-based SSE stream for notifications). Under load, response framing gets tricky: chunked transfer encoding means the response body may arrive in fragments. If the load generator reads the full body before parsing JSON-RPC, it adds buffering latency. If it tries to stream-parse, partial JSON frames cause parse errors.

**Why it happens:**
At low concurrency, HTTP responses arrive complete. Under load, chunked responses arrive in fragments due to TCP segmentation, server-side buffering, or reverse proxy behavior. The response parsing code that works perfectly at 1 RPS breaks at 100 RPS.

**How to avoid:**
- Use reqwest's `.json::<T>()` for simple request-response patterns (it handles buffering correctly). Do not manually read chunks and parse.
- For endpoints that may return streaming responses (SSE-style), use reqwest's `.bytes_stream()` and a proper JSON-RPC frame parser that handles partial reads with an internal buffer.
- Set explicit `Content-Length` expectations where possible. MCP streamable HTTP responses should have `Content-Type: application/json` for single responses. If the response is `text/event-stream`, switch to SSE parsing mode.
- Test the parser explicitly with artificially fragmented responses (inject a middleware that splits every response into 2-byte chunks).

**Warning signs:**
- Sporadic "unexpected EOF" or "invalid JSON" errors that increase with concurrency.
- Error rate is 0% at 5 VUs but 2-5% at 50 VUs with no server-side errors logged.
- Errors cluster at the end of long-running tests (buffer accumulation).

**Phase to address:**
Transport layer phase. Build robust response parsing before layering load generation on top. The mcp-tester crate's transport code may be reusable but must be validated under concurrent use.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Skip coordinated omission correction | Simpler request loop, faster to ship | All latency percentiles are wrong, eroding trust in the tool | Never — this is the tool's core value proposition |
| Hardcode reqwest Client defaults | No configuration needed | Pool starvation at 20+ VUs, confusing for users | Early prototype only, must be configurable before any release |
| Single-threaded tokio runtime | Simpler to reason about, no Send bounds | Generator becomes CPU-bound bottleneck at moderate load | Never for the load generation runtime (fine for the reporting/CLI thread) |
| Record latency with `Instant::now()` around entire request+parse | Simple, accurate-looking | Includes deserialization overhead, not true server latency | MVP only, with a documented known-limitation |
| Share MCP sessions across virtual users | Faster startup, less initialization overhead | Tests a scenario that never occurs in production; server may corrupt shared state | Never — defeats the purpose of realistic load testing |
| Synchronous progress bar updates on hot path | Real-time feedback on every request | Generator throughput limited by terminal write speed | Never — batch updates on a timer |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| reqwest + tokio runtime | Using `block_on` inside async context for "convenience" | Pure async throughout; single `Runtime::new()` at CLI entrypoint, everything else is `.await` |
| HdrHistogram recording | Recording from multiple tasks without synchronization | Use `hdrhistogram::sync::Recorder` (thread-safe writer) or channel-based collection to a single recording task |
| indicatif progress bars + tokio | Creating bars in async tasks (bars are not Send in some versions) | Create `MultiProgress` on the main thread, pass `ProgressBar` handles (which are Send) to async tasks |
| MCP session tokens | Treating session tokens as simple strings, no expiry handling | Parse Mcp-Session header from initialize response, carry it on all subsequent requests, handle 401/token-expired with re-initialization |
| JSON report output | Writing report incrementally during the test | Buffer all metrics in memory, write report atomically after test completion to avoid partial/corrupt files if test is interrupted |
| TOML scenario config | Flat list of tool calls | Hierarchical: scenario -> user_journeys -> steps, with variable references between steps |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Allocating new `String`/`Vec` for every JSON-RPC request body | Latency floor rises with VU count, high allocator contention | Pre-serialize request templates, use `bytes::Bytes` for zero-copy where possible | >200 req/s with complex tool call arguments |
| Logging every request at INFO level | tokio runtime starved by tracing subscriber I/O | Use `tracing` with `RUST_LOG=warn` default; only log errors. Buffer log writes. | >100 req/s with default log level |
| Collecting all response bodies in a Vec for post-test analysis | OOM on long-running tests | Stream metrics to HdrHistogram during test; only store error details | >10,000 total requests or responses >1KB |
| Nagle's algorithm on macOS | +40ms latency floor on small requests | Set `tcp_nodelay(true)` on the reqwest Client builder | Always on macOS, intermittent on Linux |
| Spawning one tokio::spawn per request | Task spawn overhead dominates at high rate | Use a fixed pool of long-lived virtual user tasks, each running a request loop | >500 req/s per VU |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Storing MCP server auth tokens in the TOML scenario file in plaintext | Credentials leak into version control | Reference tokens via environment variables (`$env.MCP_AUTH_TOKEN`) or the existing `cargo pmcp secret` system; never inline |
| Sending load test traffic to production endpoints by default | Accidental DDoS of production infrastructure | Require explicit `--target` flag with no default; warn if target URL matches known production patterns (e.g., no localhost, includes "prod") |
| No rate limiting on the load generator itself | Unintentional amplification if misconfigured (e.g., 10000 VUs instead of 10) | Hard cap at 100 VUs for v1 with `--i-know-what-im-doing` flag for higher; require explicit confirmation above 50 |
| Trust server TLS certificates without validation in test mode | Mask real TLS issues that production clients would hit | Default to strict TLS validation; provide `--insecure` flag for self-signed certs with a warning |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Only showing final summary after test completes | User has no idea if test is working for minutes; kills and restarts repeatedly | Show live progress: current RPS, running P50/P99, error count, elapsed time via indicatif multi-progress bars |
| Reporting raw numbers without context | "P99: 847ms" means nothing without knowing if that's good or bad | Include threshold judgments: configurable SLO targets (e.g., P99 < 500ms) with PASS/FAIL in the report |
| Error messages showing raw reqwest/hyper errors | "hyper::Error(IncompleteMessage)" is meaningless to MCP developers | Map transport errors to MCP-context messages: "Server closed connection during tool call response — this often indicates the server crashed or timed out" |
| No way to reproduce a specific test run | "It was slow yesterday" with no way to compare | Include full test configuration, timestamp, and random seed in the JSON report; support `--seed` for deterministic request ordering |

## "Looks Done But Isn't" Checklist

- [ ] **Latency measurement:** Often missing coordinated omission correction — verify by testing against a server with artificial stalls and checking that P99 reflects queuing delay, not just the stall duration
- [ ] **Virtual user lifecycle:** Often missing proper MCP shutdown — verify each VU sends a clean shutdown and the server does not leak sessions
- [ ] **Error classification:** Often just counts errors — verify errors are classified (connection refused, timeout, HTTP 4xx, HTTP 5xx, JSON parse error, MCP error response) with separate counters
- [ ] **Report format stability:** Often evolves with features — verify the JSON schema is versioned (include `"schema_version": "1.0"`) so pmcp.run can handle format changes
- [ ] **Warm-up period:** Often absent — verify the first N seconds of measurements are discarded (configurable) to exclude connection establishment, TLS handshake, and server JIT overhead
- [ ] **Cooldown/drain:** Often abrupt — verify virtual users drain gracefully (finish in-flight requests) rather than dropping connections mid-request, which contaminates error metrics
- [ ] **Throughput calculation:** Often uses wall-clock / total-requests — verify it accounts for ramp-up and ramp-down periods (only count the steady-state window)
- [ ] **macOS file descriptor limit:** Often untested — verify the tool checks `ulimit -n` at startup and warns if it is below 2x the VU count

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Coordinated omission baked into measurement | HIGH | Rewrite request scheduling to open-loop model; switch to HdrHistogram with corrected recording; all previous benchmark data is invalid |
| Connection pool misconfiguration | LOW | Change Client builder configuration; re-run tests |
| Shared MCP sessions | MEDIUM | Refactor virtual user to own its session lifecycle; requires per-VU state management |
| Generator overhead in measurements | MEDIUM | Add channel-based measurement collection; separate hot path from recording path; requires re-architecture of the measurement pipeline |
| Missing scenario dependencies | MEDIUM | Extend TOML format with step sequencing and variable extraction; existing isolated-call tests remain valid |
| Clock resolution issues | LOW | Add resolution self-check at startup; adjust reporting granularity |
| Streamable HTTP parsing failures | MEDIUM | Replace manual parsing with robust buffered reader; add fragmentation tests |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Coordinated omission | Measurement engine (Phase 1-2) | Test against stalling server; P99 must reflect queuing delay |
| Connection pool starvation | HTTP client infrastructure (Phase 1) | Confirm N concurrent requests to a 1s-sleep endpoint complete in ~1s |
| MCP session initialization | Virtual user lifecycle (Phase 1-2) | Server-side logs show N distinct sessions for N virtual users |
| Generator overhead contamination | Measurement engine (Phase 2) | Generator CPU < 50% at target load; latency does not scale with VU count when server is idle |
| Tool call dependencies | Scenario engine (Phase 3+) | Multi-step scenario extracts variable from step 1, uses in step 2 successfully |
| Clock resolution | Pre-test validation (Phase 1) | Self-check runs at startup; warning emitted on poor resolution |
| Streamable HTTP framing | Transport layer (Phase 1) | Tests pass with artificially fragmented HTTP responses |
| Accidental production DDoS | CLI safety (Phase 1) | No default target; confirmation prompt above 50 VUs |
| Latency without context | Reporting (Phase 3) | Report includes SLO pass/fail judgments, not just raw numbers |
| Session leaks on shutdown | Virtual user lifecycle (Phase 2) | Server shows 0 active sessions after test completion |

## Sources

- Gil Tene, "How NOT to Measure Latency" (2013, Azul Systems) — the definitive reference on coordinated omission. Every claim about CO in this document derives from Tene's work and subsequent validation by the performance engineering community.
- HdrHistogram project (hdrhistogram.org / hdrhistogram crate on crates.io) — purpose-built for latency recording with CO correction support.
- reqwest documentation (docs.rs/reqwest) — connection pool configuration, `pool_max_idle_per_host`, `tcp_nodelay`.
- MCP specification (modelcontextprotocol.io) — session lifecycle, initialize/initialized handshake, streamable HTTP transport, Mcp-Session header.
- tokio documentation (docs.rs/tokio) — runtime configuration, task scheduling, `tokio_metrics` for task poll time monitoring.
- Gatling, k6, and JMeter documentation — scenario scripting patterns, variable extraction, think time configuration. These represent the state of the art in load testing scenario design.
- Confidence note: All critical pitfalls (CO, connection pooling, session state) are HIGH confidence based on well-established load testing engineering principles. MCP-specific pitfalls are MEDIUM-HIGH confidence based on protocol specification analysis. Platform-specific pitfalls (macOS clock, fd limits) are HIGH confidence based on OS-level documentation.

---
*Pitfalls research for: MCP server load testing CLI in Rust*
*Researched: 2026-02-26*
