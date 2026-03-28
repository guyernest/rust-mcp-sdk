# MCP Load Testing for cargo-pmcp

## What This Is

A load testing CLI for `cargo pmcp` that lets MCP server developers stress-test their deployed servers under realistic concurrent load. Developers define test scenarios in TOML config, run `cargo pmcp loadtest run`, and get k6-style live terminal progress, colorized summary reports, and schema-versioned JSON output for CI/CD pipelines. Includes stage-driven load shaping (ramp-up/hold/ramp-down), automatic breaking point detection, and per-tool performance breakdown.

## Core Value

Give MCP server developers confidence their server meets enterprise scale requirements by showing exactly how it performs under concurrent load — throughput, latency percentiles, and breaking points.

## Requirements

### Validated

<!-- Shipped and confirmed valuable. -->

- ✓ CLI command routing via clap with subcommand pattern — existing
- ✓ Streamable HTTP transport support via reqwest/tokio — existing
- ✓ MCP JSON-RPC client capabilities (initialize, tools/list, tool calls) — existing
- ✓ TOML-based configuration system (.pmcp/) — existing
- ✓ Deployment target awareness (knows where server is deployed) — existing
- ✓ Terminal output with colored/indicatif progress — existing
- ✓ MCP server testing framework (mcp-tester crate) — existing
- ✓ OAuth/auth flow for pmcp.run API calls — existing
- ✓ Load test scenario definition in TOML config — v1.0
- ✓ Concurrent virtual user simulation (10-50 users from single machine) — v1.0
- ✓ Streamable HTTP transport for load generation — v1.0
- ✓ Live terminal progress during load test execution — v1.0
- ✓ Latency measurement (P50/P95/P99 response times) — v1.0
- ✓ Throughput measurement (requests/second, tool calls/second) — v1.0
- ✓ Error rate tracking and classification — v1.0
- ✓ Breaking point detection (load where errors spike or latency degrades) — v1.0
- ✓ Terminal summary report after test completion — v1.0
- ✓ JSON report file for CI/CD and pmcp.run ingestion — v1.0
- ✓ `cargo pmcp loadtest` CLI command integration — v1.0
- ✓ Stage-driven load shaping with ramp-up/hold/ramp-down — v1.0
- ✓ Per-tool metrics breakdown in terminal and JSON output — v1.0

### Active

<!-- Next milestone scope. -->

- [ ] JSON report schema designed for pmcp.run ingestion (INTG-01)
- [ ] Reuse existing cargo-pmcp OAuth tokens for authenticated servers (INTG-02)
- [ ] CI/CD threshold assertions (`--assert p99<500ms`) (INTG-03)
- [ ] Compare two JSON reports for performance regression/improvement (ADVN-01)
- [ ] Multi-step MCP tool call scenarios with variable extraction (ADVN-02)
- [ ] Response correctness validation under load (ADVN-03)

### Out of Scope

- Distributed multi-region execution — pmcp.run's responsibility, not the CLI
- GUI/web dashboard — terminal + JSON output only
- JavaScript/Lua scripting engine — TOML scenarios cover 90% of use cases
- Record-and-replay from existing test scenarios — fragile, sessions drift
- SSE/WebSocket transport — streamable HTTP only, matches MCP protocol direction
- Load generation beyond single machine (1000+ users) — v1 targets tens of concurrent users

## Context

Shipped v1.0 with 5,749 LOC Rust across 14 plans in 4 phases (1.28 hours execution).
Tech stack: Rust, reqwest, tokio, serde, HdrHistogram, indicatif, colored, chrono.
The loadtest module lives at `src/loadtest/` with submodules: config, client, metrics, engine, display, report, vu, stages, breaking_point, per_tool.
JSON report schema at v1.1 (backwards compatible from v1.0).
All 16 v1 requirements satisfied. 3 info-level tech debt items (orphaned public exports, one doc inaccuracy).
v2 requirements focus on pmcp.run integration, CI/CD assertions, and advanced scenario capabilities.

## Constraints

- **Tech stack**: Rust, integrated with cargo-pmcp CLI (clap subcommands, anyhow errors, tokio runtime)
- **Transport**: Streamable HTTP only — matches the modern MCP transport direction
- **Scale**: Single-machine execution targeting 10-50 concurrent virtual users
- **Dependencies**: Reuses existing crate dependencies (reqwest, tokio, serde, indicatif)
- **Config format**: TOML matching existing .pmcp/ configuration patterns
- **Report format**: JSON report schema v1.1, stable for pmcp.run consumption

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Streamable HTTP only (no SSE) | Aligns with MCP protocol direction, simpler implementation | ✓ Good — clean HTTP client, session-based |
| TOML scenario config (not YAML) | Consistent with existing .pmcp/ config ecosystem | ✓ Good — natural [[scenario]] array syntax |
| Separate from mcp-tester | Load testing has different concerns than functional testing | ✓ Good — clean module boundary |
| JSON report as first-class output | Enables pmcp.run integration and CI/CD pipelines | ✓ Good — schema v1.1, extensible |
| Single-machine for v1 | Distributed load is pmcp.run's domain, CLI does local | ✓ Good — focused scope |
| HdrHistogram with coordinated omission | Accurate percentiles under load, baked in from Phase 1 | ✓ Good — not retrofittable |
| Channel-based metrics (mpsc+watch) | No shared mutable state, clean async boundaries | ✓ Good — zero contention |
| Dual lib+bin crate layout | Enables fuzz/test/example imports of loadtest types | ✓ Good — clean ergonomics |
| Serde tagged enum for ScenarioStep | Natural TOML `type="tools/call"` syntax | ✓ Good — ergonomic config |
| k6-style terminal output | Familiar UX for developers who use k6/wrk | ✓ Good — immediate recognition |
| Rolling window breaking point detection | Self-calibrating, no upfront thresholds required | ✓ Good — works across scale |
| Per-VU child_token cancellation | Clean ramp-down without interrupting active requests | ✓ Good — LIFO shutdown |

---
*Last updated: 2026-02-27 after v1.0 milestone*
