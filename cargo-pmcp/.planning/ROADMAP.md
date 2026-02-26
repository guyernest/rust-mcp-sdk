# Roadmap: MCP Load Testing (cargo-pmcp loadtest)

## Overview

This roadmap delivers a load testing capability for `cargo pmcp` in four phases, moving from foundational primitives (config, MCP client, metrics) through a working concurrent engine, into CLI integration with structured reports, and finally load shaping with enhanced metrics. Each phase delivers a coherent, testable capability. The structure follows the natural dependency graph: you cannot schedule virtual users without a client and config types, you cannot produce reports without an engine, and you cannot shape load without a working executor.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Foundation** - Config types, MCP-aware HTTP client, and HdrHistogram metrics primitives
- [ ] **Phase 2: Engine Core** - Concurrent VU scheduler, phase-driven executor, and live terminal progress
- [ ] **Phase 3: CLI and Reports** - `cargo pmcp loadtest` command integration with terminal summary and JSON report output
- [ ] **Phase 4: Load Shaping and Tool Metrics** - Ramp-up/ramp-down phases, breaking point detection, and per-tool metric breakdown

## Phase Details

### Phase 1: Foundation
**Goal**: Developers have the building blocks for load generation -- typed TOML config, a stateful MCP HTTP client that initializes sessions correctly, and an accurate latency measurement pipeline with coordinated omission correction
**Depends on**: Nothing (first phase)
**Requirements**: CONF-01, LOAD-03, MCP-01, MCP-03, METR-01
**Success Criteria** (what must be TRUE):
  1. A TOML config file can be parsed into typed Rust structs defining target URL, VU count, duration, timeout, and scenario steps
  2. An MCP HTTP client can perform an initialize handshake against a real MCP server and receive a valid session token
  3. The client correctly classifies JSON-RPC errors (method not found, invalid params) separately from HTTP transport errors (timeout, 5xx)
  4. Latency samples recorded through the metrics pipeline produce accurate P50/P95/P99 percentiles using HdrHistogram with coordinated omission correction
  5. Per-request timeout is enforced so a hanging server does not block the entire test
**Plans**: TBD

Plans:
- [x] 01-01: TOML config types and parsing
- [ ] 01-02: MCP-aware HTTP client with session lifecycle
- [ ] 01-03: HdrHistogram metrics pipeline with coordinated omission correction
- [ ] 01-04: Property tests, fuzz testing, and runnable example

### Phase 2: Engine Core
**Goal**: Developers can run a concurrent load test with N virtual users against a deployed MCP server and see live progress in the terminal
**Depends on**: Phase 1
**Requirements**: LOAD-01, LOAD-02, METR-02, METR-03
**Success Criteria** (what must be TRUE):
  1. N concurrent virtual users each perform their own MCP initialize handshake and execute tool calls independently with their own session
  2. The test runs for a specified duration (seconds) or iteration count, then stops cleanly
  3. Live terminal output shows current requests/second, cumulative error count, active VU count, and elapsed time, updated on a timer (not per-request)
  4. Throughput (requests/second) and error rate are computed correctly from the metrics pipeline
**Plans**: TBD

Plans:
- [ ] 02-01: VU scheduler with tokio tasks and channel-based metrics
- [ ] 02-02: Phase-driven executor with duration/iteration control
- [ ] 02-03: Live terminal display with indicatif

### Phase 3: CLI and Reports
**Goal**: Developers run load tests through the standard `cargo pmcp loadtest` command and get both human-readable terminal output and machine-readable JSON reports
**Depends on**: Phase 2
**Requirements**: CONF-02, CONF-03, METR-04, METR-05
**Success Criteria** (what must be TRUE):
  1. `cargo pmcp loadtest run` executes a load test using scenario config and prints a colorized terminal summary with latency percentiles, throughput, and error breakdown
  2. `cargo pmcp loadtest init` generates a starter `.pmcp/loadtest.toml` file with sensible defaults and comments explaining each field
  3. A JSON report file is written after test completion containing latency percentiles, throughput, error classification, test config, timestamp, and a schema version field
  4. The JSON report structure is stable enough for external tools (CI/CD pipelines, pmcp.run) to parse reliably
**Plans**: TBD

Plans:
- [ ] 03-01: CLI subcommand integration (run and init)
- [ ] 03-02: Terminal summary report with colored output
- [ ] 03-03: JSON report serialization with schema versioning

### Phase 4: Load Shaping and Tool Metrics
**Goal**: Developers can shape load with ramp-up/ramp-down phases, detect server breaking points automatically, and see per-tool performance breakdowns
**Depends on**: Phase 3
**Requirements**: LOAD-04, MCP-02, METR-06
**Success Criteria** (what must be TRUE):
  1. A load test can ramp from 0 to target VU count over a configured duration, hold at peak, and ramp down -- without interrupting running VU sessions at phase boundaries
  2. The load tester can auto-detect the breaking point where error rate spikes or latency degrades beyond configured thresholds
  3. Metrics are reported per MCP tool (latency percentiles, throughput, error count per tool name) in both terminal output and JSON report
**Plans**: TBD

Plans:
- [ ] 04-01: Ramp-up/hold/ramp-down phase execution
- [ ] 04-02: Breaking point detection algorithm
- [ ] 04-03: Per-tool metrics aggregation and reporting

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 0/4 | Not started | - |
| 2. Engine Core | 0/3 | Not started | - |
| 3. CLI and Reports | 0/3 | Not started | - |
| 4. Load Shaping and Tool Metrics | 0/3 | Not started | - |
