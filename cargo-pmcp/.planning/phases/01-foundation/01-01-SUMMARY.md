---
phase: 01-foundation
plan: 01
subsystem: config
tags: [toml, serde, thiserror, hdrhistogram, tdd]

# Dependency graph
requires: []
provides:
  - "LoadTestConfig, Settings, ScenarioStep types with Deserialize"
  - "LoadTestError enum with ConfigParse, ConfigValidation, ConfigIo variants"
  - "McpError enum with JsonRpc, Http, Timeout, Connection variants and classification methods"
  - "McpClient stub (compilable, expanded in 01-02)"
  - "MetricsRecorder, OperationType stubs (compilable, expanded in 01-03)"
  - "Library crate target (cargo_pmcp::loadtest::) for fuzz, tests, examples"
affects: [01-02, 01-03, 01-04, 02-01, 03-01]

# Tech tracking
tech-stack:
  added: [hdrhistogram 7.5]
  patterns: [serde tagged enum for MCP operation types, thiserror error enums, lib+bin crate layout]

key-files:
  created:
    - src/lib.rs
    - src/loadtest/mod.rs
    - src/loadtest/config.rs
    - src/loadtest/error.rs
    - src/loadtest/client.rs
    - src/loadtest/metrics.rs
  modified:
    - Cargo.toml

key-decisions:
  - "No url field in Settings -- target server URL comes from --url CLI flag per user decision"
  - "ScenarioStep uses serde internally tagged enum with type field for TOML ergonomics"
  - "Library crate (cargo_pmcp::) added to Cargo.toml for external imports by fuzz/tests/examples"
  - "expected_interval_ms defaults to 100ms for coordinated omission correction"

patterns-established:
  - "Serde tagged enum pattern: ScenarioStep with #[serde(tag = 'type')] for TOML [[scenario]] arrays"
  - "Error classification: LoadTestError for config, McpError for protocol/transport with category accessor"
  - "Dual crate layout: [lib] src/lib.rs + [[bin]] src/main.rs in same package"

requirements-completed: [CONF-01, LOAD-03, MCP-03]

# Metrics
duration: 5min
completed: 2026-02-26
---

# Phase 1 Plan 01: TOML Config Types and Loadtest Module Bootstrap Summary

**Typed TOML config with weighted MCP operation mix (tools/call, resources/read, prompts/get), McpError classification enum, and loadtest module scaffolding with compilable stubs for client and metrics**

## Performance

- **Duration:** 4m 30s
- **Started:** 2026-02-26T22:07:50Z
- **Completed:** 2026-02-26T22:12:20Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- LoadTestConfig parses TOML with [settings] and [[scenario]] sections into typed Rust structs supporting weighted mixes of tools/call, resources/read, and prompts/get
- McpError enum classifies JSON-RPC, HTTP, timeout, and connection errors with category accessor and reqwest mapping
- Dual lib+bin crate layout enables fuzz targets, integration tests, and examples to import `cargo_pmcp::loadtest::*`
- All 17 tests passing (10 config + 7 error classification)

## Task Commits

Each task was committed atomically:

1. **Task 1: Bootstrap loadtest module structure** - `13185e9` (feat)
2. **Task 2: TDD config types RED** - `557aac8` (test)
3. **Task 2: TDD config types GREEN** - `fbadb36` (feat)

_Note: TDD Task 2 has separate RED and GREEN commits. REFACTOR skipped -- no cleanup needed._

## Files Created/Modified
- `Cargo.toml` - Added [lib] and [[bin]] targets, hdrhistogram dependency
- `src/lib.rs` - Library root with `pub mod loadtest`
- `src/loadtest/mod.rs` - Module declarations for config, error, client, metrics
- `src/loadtest/config.rs` - LoadTestConfig, Settings, ScenarioStep with serde Deserialize + validation + 10 inline tests
- `src/loadtest/error.rs` - LoadTestError and McpError enums with classification methods + 7 inline tests
- `src/loadtest/client.rs` - McpClient stub (expanded in Plan 01-02)
- `src/loadtest/metrics.rs` - OperationType, RequestSample, MetricsSnapshot, MetricsRecorder stubs (expanded in Plan 01-03)

## Decisions Made
- Used serde internally tagged enum (`#[serde(tag = "type")]`) for ScenarioStep to enable natural TOML `type = "tools/call"` syntax
- No url field in Settings struct -- target server URL comes from --url CLI flag per user decision from CONTEXT.md
- expected_interval_ms defaults to 100ms (10 req/s per VU baseline) for coordinated omission correction
- Created dual lib+bin crate targets so cargo_pmcp:: can be imported by fuzz targets, property tests, and examples

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Config types ready for Plans 01-02 (client) and 01-03 (metrics) to build upon
- McpError enum ready for client error classification in Plan 01-02
- OperationType enum ready for metrics pipeline in Plan 01-03
- Library crate target ready for Plan 01-04 (fuzz, property tests, example)

## Self-Check: PASSED

All 6 created files verified on disk. All 3 commit hashes verified in git log.

---
*Phase: 01-foundation*
*Completed: 2026-02-26*
