# Requirements: MCP Load Testing

**Defined:** 2026-02-26
**Core Value:** Give MCP server developers confidence their server meets enterprise scale requirements by showing exactly how it performs under concurrent load

## v1 Requirements

Requirements for initial release. Each maps to roadmap phases.

### Load Generation

- [x] **LOAD-01**: User can run a load test with N concurrent virtual users against a deployed MCP server
- [x] **LOAD-02**: User can set test duration by time (seconds) or iteration count
- [x] **LOAD-03**: User can configure per-request timeout
- [ ] **LOAD-04**: User can define ramp-up/hold/ramp-down phases to gradually increase load

### MCP Protocol

- [x] **MCP-01**: Each virtual user performs its own MCP initialize handshake and maintains its session
- [ ] **MCP-02**: Metrics are reported per MCP tool (latency, throughput, errors per tool)
- [x] **MCP-03**: JSON-RPC errors are classified separately from HTTP errors

### Metrics & Reporting

- [x] **METR-01**: Load test reports latency percentiles (P50/P95/P99) using HdrHistogram
- [x] **METR-02**: Load test reports throughput (requests/second) and error rate with classification
- [x] **METR-03**: Load test shows live terminal progress (current RPS, error count, elapsed time)
- [ ] **METR-04**: Load test produces colorized terminal summary report at completion
- [ ] **METR-05**: Load test outputs JSON report file for CI/CD pipelines
- [ ] **METR-06**: Load test can auto-detect breaking point where performance degrades

### Configuration & CLI

- [x] **CONF-01**: User can define load test scenarios in TOML config file
- [x] **CONF-02**: User can run load tests via `cargo pmcp loadtest` CLI command
- [x] **CONF-03**: User can generate starter loadtest config via `cargo pmcp loadtest init`

## v2 Requirements

Deferred to future release. Tracked but not in current roadmap.

### Integration

- **INTG-01**: Load test JSON report uses versioned schema designed for pmcp.run ingestion
- **INTG-02**: Load test reuses existing cargo-pmcp OAuth tokens for authenticated servers
- **INTG-03**: Load test supports CI/CD threshold assertions (`--assert p99<500ms`)

### Advanced

- **ADVN-01**: User can compare two JSON reports to see performance regression/improvement
- **ADVN-02**: User can define multi-step MCP tool call scenarios with variable extraction
- **ADVN-03**: Load test validates response correctness under load (not just latency)

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Distributed multi-region load generation | pmcp.run's responsibility, not the CLI tool |
| GUI/web dashboard | Scope creep; JSON output feeds external tools like Grafana |
| JavaScript/Lua scripting engine | TOML scenarios cover 90% of use cases; massive dependency for 10% |
| Record-and-replay from test scenarios | Fragile; sessions contain timestamps and IDs that drift |
| SSE/WebSocket transport | Streamable HTTP only for v1; matches MCP protocol direction |
| Static HTML report generation | JSON + jq covers most needs; defer to v2+ if demanded |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| CONF-01 | Phase 1: Foundation | Complete |
| LOAD-03 | Phase 1: Foundation | Complete |
| MCP-01 | Phase 1: Foundation | Complete |
| MCP-03 | Phase 1: Foundation | Complete |
| METR-01 | Phase 1: Foundation | Complete |
| LOAD-01 | Phase 2: Engine Core | Complete |
| LOAD-02 | Phase 2: Engine Core | Complete |
| METR-02 | Phase 2: Engine Core | Complete |
| METR-03 | Phase 2: Engine Core | Complete |
| CONF-02 | Phase 3: CLI and Reports | Complete |
| CONF-03 | Phase 3: CLI and Reports | Complete |
| METR-04 | Phase 3: CLI and Reports | Pending |
| METR-05 | Phase 3: CLI and Reports | Pending |
| LOAD-04 | Phase 4: Load Shaping and Tool Metrics | Pending |
| MCP-02 | Phase 4: Load Shaping and Tool Metrics | Pending |
| METR-06 | Phase 4: Load Shaping and Tool Metrics | Pending |

**Coverage:**
- v1 requirements: 16 total
- Mapped to phases: 16
- Unmapped: 0

---
*Requirements defined: 2026-02-26*
*Last updated: 2026-02-26 after roadmap creation*
