---
phase: 01-foundation
plan: 04
subsystem: testing
tags: [proptest, cargo-fuzz, property-testing, fuzz-testing, example]

# Dependency graph
requires:
  - phase: 01-foundation (plans 01-03)
    provides: LoadTestConfig, McpError, MetricsRecorder types for testing
provides:
  - Property-based tests validating config parsing and error classification invariants
  - Cargo-fuzz target for TOML config parsing robustness
  - Runnable example demonstrating all Phase 1 loadtest types
affects: [02-engine-core]

# Tech tracking
tech-stack:
  added: [proptest, libfuzzer-sys]
  patterns: [property-based testing with proptest, cargo-fuzz integration, runnable examples]

key-files:
  created:
    - tests/property_tests.rs
    - fuzz/Cargo.toml
    - fuzz/fuzz_targets/fuzz_config_parse.rs
    - examples/loadtest_demo.rs
  modified:
    - Cargo.toml

key-decisions:
  - "Removed unused arb_scenario_step_with_weight helper to avoid dead_code warning"
  - "Fuzz target uses separate workspace via [workspace] in fuzz/Cargo.toml to avoid interfering with parent workspace"

patterns-established:
  - "Property tests in tests/property_tests.rs using proptest! macro"
  - "Fuzz targets in fuzz/fuzz_targets/ following cargo-fuzz conventions"
  - "Examples in examples/ demonstrating realistic usage of library types"

requirements-completed: [CONF-01, MCP-03, METR-01]

# Metrics
duration: 7min
completed: 2026-02-26
---

# Phase 1 Plan 4: Property Tests, Fuzz Target, and Example Summary

**Proptest-based property tests for config/error invariants, cargo-fuzz target for TOML parsing robustness, and runnable loadtest demo example**

## Performance

- **Duration:** 7 min
- **Started:** 2026-02-26T22:24:58Z
- **Completed:** 2026-02-26T22:32:17Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- 7 property tests covering config roundtrip, validation rejection, timeout conversion, and McpError classification across randomized inputs
- Fuzz target that exercises LoadTestConfig::from_toml() with arbitrary byte sequences, ensuring it never panics
- Runnable example demonstrating config parsing, error classification, and metrics recording in a realistic scenario
- All CLAUDE.md mandatory requirements (fuzz, property, example) satisfied for Phase 1

## Task Commits

Each task was committed atomically:

1. **Task 1: Property tests for config parsing and McpError classification** - `4fb5734` (test)
2. **Task 2: Fuzz target for TOML config parsing and runnable example** - `28f0b81` (feat)

**Workspace fix (Rule 3 deviation):** `edd5e90` (fix: rand/axum API updates in parent workspace)

## Files Created/Modified
- `tests/property_tests.rs` - 7 proptest property tests for config and error invariants
- `fuzz/Cargo.toml` - Fuzz crate manifest with libfuzzer-sys dependency
- `fuzz/fuzz_targets/fuzz_config_parse.rs` - Fuzz target for TOML config parsing
- `examples/loadtest_demo.rs` - Runnable example demonstrating all loadtest module types
- `Cargo.toml` - Added proptest to dev-dependencies

## Decisions Made
- Removed unused `arb_scenario_step_with_weight` strategy function to avoid clippy dead_code warning
- Used `[workspace]` in fuzz/Cargo.toml to prevent fuzz crate from being included in parent workspace compilation

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed workspace dependency compilation failures from updated crate APIs**
- **Found during:** Task 2 verification (cargo test --test property_tests)
- **Issue:** Cargo.lock update from fuzz target compilation pulled in newer versions of axum (0.8.7) and rand (0.10.0) which broke mcp-preview (Message::Text now expects Utf8Bytes) and mcp-tester (rand::Rng::random moved to RngExt trait). Also broke cargo-pmcp/src/secrets/value.rs (thread_rng/gen_range removed).
- **Fix:** Added .into() calls for axum Message::Text, replaced rand::Rng with rand::RngExt, replaced thread_rng()/gen_range() with rng()/random_range()
- **Files modified:** crates/mcp-preview/src/handlers/websocket.rs, crates/mcp-tester/src/oauth.rs, cargo-pmcp/src/secrets/value.rs
- **Verification:** cargo test --test property_tests passes, cargo run --example loadtest_demo runs, cargo check --manifest-path fuzz/Cargo.toml compiles
- **Committed in:** edd5e90 (separate commit in parent workspace)

---

**Total deviations:** 1 auto-fixed (blocking)
**Impact on plan:** Fix necessary for compilation after Cargo.lock update. No scope creep.

## Issues Encountered
None beyond the auto-fixed workspace dependency issue documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 1 Foundation is complete: all 4 plans executed successfully
- Config parsing, error types, MCP client, metrics recorder, property tests, fuzz target, and example all in place
- Ready for Phase 2: Engine Core (load test engine, VU workers, channel-based metrics aggregation)

## Self-Check: PASSED

All 5 files verified present. All 3 commits verified in git log.

---
*Phase: 01-foundation*
*Completed: 2026-02-26*
