# Feature Research

**Domain:** MCP server load testing (CLI tool)
**Researched:** 2026-02-26
**Confidence:** MEDIUM (training data for established tools; LOW for mcpdrill specifics)

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist. Missing these = product feels incomplete.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Concurrent virtual users | Every load tester simulates concurrent load; without this it is just sequential benchmarking | MEDIUM | k6/locust/artillery all center on this. Tokio tasks are the natural Rust primitive. |
| Configurable request rate / ramp-up | Users need to control how load increases; step-up, constant, and ramp patterns are universal | MEDIUM | k6 has "stages" (ramp up, hold, ramp down). Locust has spawn rate. Artillery has "phases". |
| Latency percentiles (P50/P95/P99) | Every serious load tester reports percentile latencies, not just averages; averages hide tail latency | LOW | Use HdrHistogram or t-digest. All tools (k6, wrk, vegeta, drill) report percentiles. |
| Throughput metrics (req/s) | Fundamental metric — how many operations per second the server handles | LOW | Simple counter over time window. Every tool reports this. |
| Error rate and classification | Users need to know what percentage of requests fail and why (timeout, 4xx, 5xx, protocol error) | LOW | Categorize by HTTP status, JSON-RPC error codes, and timeout. |
| Duration control | Users set how long the test runs (by time or by request count) | LOW | Standard in all tools. `--duration 60s` or `--iterations 1000`. |
| CLI interface | Load testing is a CLI-first workflow for developers and CI/CD | LOW | Already exists in cargo-pmcp; add `loadtest` subcommand. |
| Machine-readable output (JSON) | CI/CD pipelines consume structured results for pass/fail gating | LOW | k6 outputs JSON, vegeta outputs JSON, artillery outputs JSON. Standard expectation. |
| Human-readable terminal report | Developers read summary in terminal after test; colorized table with key metrics | LOW | wrk and drill both do this well. indicatif already in cargo-pmcp deps. |
| Live progress indicator | Users need to see the test is running and progressing, not just a hanging terminal | LOW | Progress bar showing elapsed time, current RPS, running error count. indicatif handles this. |
| Configuration file support | Complex scenarios should not require 30 CLI flags; config file is expected | LOW | TOML matches existing .pmcp/ conventions. k6 uses JS, artillery uses YAML, drill uses YAML. |
| Timeout configuration | Users must control per-request timeouts; different servers have different latency profiles | LOW | Per-request and global timeout. Every tool has this. |

### Differentiators (Competitive Advantage)

Features that set the product apart. Not required, but valuable.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **MCP protocol awareness** | Understands initialize handshake, tools/list, tool calls as first-class operations — not generic HTTP | MEDIUM | This is THE differentiator. Generic tools treat MCP as opaque HTTP POST. cargo-pmcp understands the protocol semantics: initialize once, then call tools. Scenarios expressed as "call tool X with params Y" not "POST /mcp with body Z". |
| **MCP session lifecycle** | Properly manages MCP sessions — initialize, maintain, close — like a real client | MEDIUM | Real MCP clients initialize once then issue many tool calls per session. Load tester must do the same, not re-initialize per request. Tests session pooling and server statefulness. |
| **Breaking point detection** | Automatically finds the load level where errors spike or latency degrades beyond threshold | MEDIUM | k6 does not do this automatically. Artillery does not. This is step-up load + automatic degradation detection. Huge value for "what's my server's capacity?" question. |
| **Tool-level metrics** | Report latency/throughput per MCP tool, not just per endpoint | LOW | MCP servers expose multiple tools. Developers need to know "get-weather is fast but search-database is slow". Generic HTTP testers report per-URL, not per-tool. |
| **Scenario as MCP workflow** | Define test scenarios as sequences of MCP tool calls with data flow between them | HIGH | "Initialize, list tools, call tool-A, use result in tool-B" — realistic multi-step workflows. This is like k6's scripting but for MCP operations. |
| **JSON report for pmcp.run** | Report format designed for ingestion by pmcp.run managed service from day one | LOW | Stable schema with version field. pmcp.run builds distributed load testing on top of local results format. |
| **Comparison mode** | Compare results between runs (before/after deployment, version A vs B) | MEDIUM | Vegeta has `vegeta report --type=text` for comparison. k6 Cloud has trends. Local regression detection is valuable for CI. |
| **Connection pool sizing** | Test with configurable connection pool sizes to find optimal HTTP connection reuse | LOW | MCP over streamable HTTP benefits from connection pooling. Generic tools assume one connection per VU. |
| **Auth-aware testing** | Built-in support for OAuth/token auth that MCP servers commonly require | MEDIUM | cargo-pmcp already has OAuth flow. Load tester reuses it. Generic tools require manual header configuration. |
| **Server capability validation under load** | Verify MCP server returns correct capabilities even under heavy load, not just that it responds | MEDIUM | Correctness under load is different from just "did it return 200". Validate JSON-RPC responses contain expected fields. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Distributed load generation** | "I need 10,000 concurrent users" | Massive complexity (coordination, clock sync, result aggregation, network setup). This is pmcp.run's domain, not a CLI tool. | Single-machine generates 10-50 VUs. Point users to pmcp.run for distributed. Design JSON report format so pmcp.run can aggregate multiple CLI runs. |
| **GUI / web dashboard** | "I want graphs in my browser" | Scope explosion, maintenance burden, dependency on web framework. Distracts from CLI-first workflow. | Terminal sparklines for live progress. JSON output for external tools (Grafana, custom dashboards). `--open` flag that renders a static HTML report is the maximum scope. |
| **Record-and-replay** | "Record my MCP client session and replay it as load test" | Fragile (sessions contain timestamps, session IDs). Complex to implement correctly. Recorded traffic drifts from reality. | Scenario config files that describe tool calls declaratively. Template variables for dynamic data. |
| **JavaScript/Lua scripting engine** | "I need k6-style scripting for custom logic" | Embedding a scripting runtime (V8, Lua) adds massive dependency, compilation time, and attack surface. This is a Rust CLI, not a scripting platform. | TOML scenario definitions with template variables and sequential step support. Covers 90% of use cases without scripting. For the 10%, users write Rust or use k6 directly. |
| **Every output format** (CSV, XML, InfluxDB, Prometheus, etc.) | "I need to push metrics to my monitoring stack" | Each format is a maintenance burden. Feature creep. | JSON as the single structured format. Provide clear schema docs. Users pipe to `jq` or write trivial adapters. Consider Prometheus exposition format as a single optional addition since it is the monitoring lingua franca. |
| **Protocol support beyond streamable HTTP** | "I also need WebSocket, SSE, stdio testing" | Each transport is a different concurrency model. Multiplies complexity. stdio is inherently single-process. | Streamable HTTP only for v1 (matches MCP protocol direction). Explicitly document this boundary. |
| **Realistic think time / pacing simulation** | "Simulate real user pauses between requests" | Adds complexity to scheduling. Users misunderstand it (it reduces throughput, making results harder to interpret). Load testing is about finding limits, not simulating human behavior. | Constant-rate or ramp-rate model. If users want pacing, they set lower target RPS. |

## Feature Dependencies

```
[MCP Protocol Client (existing)]
    |
    +--requires--> [Concurrent VU Engine]
    |                   |
    |                   +--requires--> [Latency/Throughput Metrics Collection]
    |                   |                   |
    |                   |                   +--enables--> [Terminal Report]
    |                   |                   +--enables--> [JSON Report]
    |                   |                   +--enables--> [Breaking Point Detection]
    |                   |                   +--enables--> [Comparison Mode]
    |                   |
    |                   +--requires--> [Live Progress Display]
    |                   |
    |                   +--enables--> [Ramp-up / Load Phases]
    |
    +--requires--> [MCP Session Lifecycle]
    |                   |
    |                   +--enables--> [Tool-level Metrics]
    |                   +--enables--> [Scenario Workflows]
    |                   +--enables--> [Correctness Validation Under Load]
    |
    +--requires--> [TOML Scenario Config]
    |                   |
    |                   +--enables--> [Scenario Workflows]
    |
    +--enables--> [Auth-aware Testing (reuse existing OAuth)]

[JSON Report]
    +--enables--> [pmcp.run Integration]
    +--enables--> [CI/CD Pass/Fail Gating]
    +--enables--> [Comparison Mode]
```

### Dependency Notes

- **Concurrent VU Engine requires MCP Protocol Client:** Each virtual user is an MCP client. The existing client capabilities in cargo-pmcp are the foundation.
- **Metrics Collection requires Concurrent VU Engine:** Metrics are gathered from the concurrent execution layer.
- **Breaking Point Detection requires Metrics Collection:** It analyzes metric trends to find degradation points.
- **Scenario Workflows require both MCP Session Lifecycle and TOML Config:** Multi-step workflows need proper session management and a way to define steps.
- **Comparison Mode requires JSON Report:** Comparing runs means loading and diffing structured result files.
- **Auth-aware Testing reuses existing OAuth:** cargo-pmcp already has OAuth flow; load tester reuses tokens from it.
- **Tool-level Metrics require MCP Session Lifecycle:** Must understand which tool call maps to which response to report per-tool.

## MVP Definition

### Launch With (v1)

Minimum viable product -- what is needed to validate the concept.

- [x] `cargo pmcp loadtest` CLI command -- entry point for the feature
- [ ] TOML scenario config -- define target URL, tool calls, VU count, duration
- [ ] Concurrent virtual user engine -- spawn N tokio tasks, each running MCP client
- [ ] MCP session lifecycle -- initialize once per VU, then execute tool calls
- [ ] Latency percentiles (P50/P95/P99) -- core metric users need
- [ ] Throughput (requests/second) -- core metric users need
- [ ] Error rate with classification -- know what failed and why
- [ ] Live terminal progress -- show test is running with real-time stats
- [ ] Terminal summary report -- colorized table at test completion
- [ ] JSON report output -- structured results for CI/CD and pmcp.run

### Add After Validation (v1.x)

Features to add once core is working.

- [ ] Ramp-up phases (step load) -- trigger: users ask "how do I gradually increase load?"
- [ ] Breaking point detection -- trigger: users ask "what's my server's max capacity?"
- [ ] Tool-level metrics breakdown -- trigger: users have multi-tool servers and need per-tool insight
- [ ] Comparison mode (diff two JSON reports) -- trigger: users run before/after deployments
- [ ] Auth-aware testing (reuse OAuth tokens) -- trigger: users test auth-protected servers
- [ ] CI/CD threshold gating (--assert p99<500ms) -- trigger: users want automated pass/fail

### Future Consideration (v2+)

Features to defer until product-market fit is established.

- [ ] Scenario workflows (multi-step tool call chains) -- complex, needs real user demand
- [ ] Correctness validation under load -- needs schema for expected responses
- [ ] Connection pool tuning options -- advanced, niche
- [ ] Static HTML report generation -- nice to have, JSON + jq covers most needs
- [ ] Prometheus metrics exposition -- only if monitoring integration is demanded

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Concurrent VU engine | HIGH | MEDIUM | P1 |
| MCP session lifecycle | HIGH | MEDIUM | P1 |
| Latency percentiles | HIGH | LOW | P1 |
| Throughput metrics | HIGH | LOW | P1 |
| Error classification | HIGH | LOW | P1 |
| TOML scenario config | HIGH | LOW | P1 |
| Live progress display | MEDIUM | LOW | P1 |
| Terminal summary report | HIGH | LOW | P1 |
| JSON report output | HIGH | LOW | P1 |
| Ramp-up / load phases | MEDIUM | MEDIUM | P2 |
| Breaking point detection | HIGH | MEDIUM | P2 |
| Tool-level metrics | MEDIUM | LOW | P2 |
| Comparison mode | MEDIUM | MEDIUM | P2 |
| Auth-aware testing | MEDIUM | LOW | P2 |
| CI/CD threshold assertions | HIGH | LOW | P2 |
| Scenario workflows | MEDIUM | HIGH | P3 |
| Correctness validation | MEDIUM | MEDIUM | P3 |
| HTML report | LOW | MEDIUM | P3 |

**Priority key:**
- P1: Must have for launch
- P2: Should have, add when possible
- P3: Nice to have, future consideration

## Competitor Feature Analysis

### Tool Survey

| Feature | k6 | wrk | drill | vegeta | locust | artillery | mcpdrill (LOW confidence) |
|---------|-----|-----|-------|--------|--------|-----------|--------------------------|
| **Language** | Go (scripts in JS) | C | Rust | Go | Python | Node.js (YAML config) | Node.js |
| **Concurrent users** | Yes (VUs) | Yes (threads+connections) | Yes (concurrency) | Yes (rate-based) | Yes (users) | Yes (VUs) | Yes |
| **Ramp-up phases** | Yes (stages) | No | No | No (constant rate) | Yes (spawn rate) | Yes (phases) | Unknown |
| **Latency percentiles** | P50/P90/P95/P99 | P50/P75/P90/P99 | Percentiles | P50/P95/P99 | P50/P95 | P50/P95/P99 | Likely yes |
| **Throughput (rps)** | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| **Error tracking** | Yes | Basic | Yes | Yes | Yes | Yes | Unknown |
| **Config format** | JavaScript files | CLI flags | YAML | CLI + stdin | Python code | YAML | Unknown |
| **JSON output** | Yes | No | No | Yes | No (CSV) | Yes | Unknown |
| **Live progress** | Yes (terminal) | After completion only | Basic | No (streaming) | Web UI | Console | Unknown |
| **Scripting** | Full JS | No | No | No | Full Python | JS/YAML hooks | No |
| **Protocol awareness** | HTTP/gRPC/WS | HTTP only | HTTP only | HTTP only | HTTP (plugins) | HTTP/WS/Socket.io | MCP-specific |
| **Breaking point detection** | No (manual) | No | No | No | No | No | Unknown |
| **CI/CD integration** | Thresholds | Exit codes | Exit codes | Exit codes | Exit codes | Thresholds | Unknown |
| **Distributed** | k6 Cloud | No | No | No | Built-in | Artillery Cloud | No |
| **MCP awareness** | None | None | None | None | None | None | Yes (primary purpose) |

### mcpdrill Analysis (LOW confidence -- based on PROJECT.md reference and training data)

mcpdrill appears to be the only existing MCP-specific load testing tool. Based on the PROJECT.md context ("Node-based, lacks PMCP integration and Rust performance"):

- **What it likely does:** Sends concurrent MCP tool calls to an MCP server endpoint, measures latency and throughput.
- **What it likely lacks:** PMCP SDK integration, Rust-level performance for high-concurrency local generation, TOML config matching PMCP conventions, report format designed for managed service ingestion.
- **Why cargo-pmcp has an advantage:** Native integration with existing MCP client code, reuse of OAuth flow, TOML config consistency, Rust performance (tokio async), and designed for pmcp.run service integration.

**NOTE:** mcpdrill feature details are LOW confidence. The GitHub repository (https://github.com/bc-dunia/mcpdrill) should be verified directly. I was unable to fetch its README due to tool restrictions.

### What Makes Generic Tools Insufficient for MCP

1. **No MCP session semantics.** Generic tools treat each request independently. MCP requires initialize handshake, session maintenance, and proper lifecycle management. Testing without this generates unrealistic load.
2. **No tool-level granularity.** Generic tools report per-URL metrics. MCP servers expose multiple tools on a single endpoint. Developers need per-tool metrics.
3. **No JSON-RPC understanding.** A 200 OK with a JSON-RPC error inside is a failure. Generic tools count it as success.
4. **No capability negotiation.** MCP clients negotiate capabilities during initialization. Load testers must do this correctly or the server may reject subsequent requests.
5. **No scenario modeling.** Real MCP usage involves calling tools in sequences with data dependencies. Generic tools just hammer a single endpoint.

## Key Observations

### What is Actually Table Stakes in 2026

The load testing space is mature. Users coming from k6, locust, or artillery expect:
- Configurable concurrency with ramp-up
- Percentile latency reporting (not just averages)
- Machine-readable output for CI/CD
- Configuration files (not just CLI flags)
- Real-time feedback during test execution

Missing any of these and users will say "I'll just use k6 with raw HTTP POSTs."

### Where the Real Differentiation Lives

The only defensible differentiation for cargo-pmcp loadtest is **MCP protocol awareness**:
- Understanding MCP session lifecycle (initialize, tool calls, close)
- Reporting metrics at the MCP tool level, not HTTP endpoint level
- Treating JSON-RPC errors as real errors (not HTTP 200 success)
- Scenario definitions expressed in MCP terms ("call tool X") not HTTP terms ("POST body Y")
- Integration with the PMCP ecosystem (config, auth, pmcp.run)

Everything else (percentiles, ramp-up, JSON output) is commodity. Build it well, but do not over-invest in differentiating there.

## Sources

- k6 documentation and feature set: training data (HIGH confidence -- well-established, stable tool)
- wrk, drill, vegeta features: training data (HIGH confidence -- stable, mature tools)
- locust feature set: training data (HIGH confidence -- widely documented)
- artillery feature set: training data (HIGH confidence -- widely documented)
- mcpdrill: PROJECT.md reference + training data (LOW confidence -- could not verify current repo state)
- MCP protocol semantics: codebase knowledge from cargo-pmcp (HIGH confidence)
- Load testing best practices: training data (MEDIUM confidence -- patterns are stable but specifics may have evolved)

---
*Feature research for: MCP server load testing (cargo-pmcp loadtest)*
*Researched: 2026-02-26*
