---
phase: 19-ship-examples-and-playwright-e2e
plan: 02
subsystem: testing
tags: [chromiumoxide, cdp, e2e, axum, browser-testing, headless]

# Dependency graph
requires:
  - phase: 19-01
    provides: "Widget HTML files (chess/map/dataviz), mcp-e2e-tests placeholder crate, justfile recipes"
provides:
  - "20 passing E2E browser tests for chess (10), map (5), and dataviz (5) widgets"
  - "Embedded axum test server serving widget files from example directories"
  - "Mock MCP bridge injection via CDP evaluate_on_new_document"
  - "Browser lifecycle management with auto-download Chromium via BrowserFetcher"
  - "Reusable test utilities: wait_for_element, wait_for_js_condition, get_tool_call_log"
affects: []

# Tech tracking
tech-stack:
  added: [chromiumoxide 0.9.1, chromiumoxide_fetcher, tower-http 0.6]
  patterns: [CDP mock bridge injection, embedded test server, polling-based element wait]

key-files:
  created:
    - crates/mcp-e2e-tests/src/server.rs
    - crates/mcp-e2e-tests/src/bridge.rs
    - crates/mcp-e2e-tests/tests/chess.rs
    - crates/mcp-e2e-tests/tests/map.rs
    - crates/mcp-e2e-tests/tests/dataviz.rs
  modified:
    - crates/mcp-e2e-tests/Cargo.toml
    - crates/mcp-e2e-tests/src/lib.rs

key-decisions:
  - "chromiumoxide fetcher/rustls/zip0 features for auto-download Chromium with rustls TLS"
  - "Mock bridge uses IIFE wrapper with callTool logging via __toolCallLog array"
  - "Map city detail test calls getCityDetails() directly instead of clicking city item to avoid Leaflet map pan tile loading blocking CDP"
  - "Each test creates fresh browser + server (no shared state across tests) since test binaries are separate processes"
  - "wait_for_js_condition polls with 100ms interval using page.evaluate for element-based waits"

patterns-established:
  - "CDP mock injection: evaluate_on_new_document before page.goto() for canned tool responses"
  - "Embedded axum test server: bind to 127.0.0.1:0 for OS-assigned port, serve widget dirs"
  - "Polling wait pattern: loop with evaluate + sleep for element/condition waits"

requirements-completed: [SHIP-03, SHIP-04, SHIP-05]

# Metrics
duration: 39min
completed: 2026-02-26
---

# Phase 19 Plan 02: E2E Test Crate Summary

**Rust-native CDP browser tests via chromiumoxide with auto-download Chromium, embedded axum widget server, and mock bridge injection -- 20 tests across 3 widgets**

## Performance

- **Duration:** 39 min
- **Started:** 2026-02-26T19:21:38Z
- **Completed:** 2026-02-26T20:01:36Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Built complete E2E test infrastructure: browser lifecycle, embedded HTTP server, mock bridge injection
- Ported all 10 chess widget Playwright tests to Rust CDP tests (board render, status, piece selection, valid moves, move execution, status update, state persistence, new game, error handling)
- Created 5 map widget E2E tests (container render, city search, markers, count display, detail panel)
- Created 5 dataviz widget E2E tests (chart render, table populate, column headers, chart type switch, list_tables init)
- All 20 tests pass with `cargo test -p mcp-e2e-tests -- --test-threads=1`

## Task Commits

Each task was committed atomically:

1. **Task 1: Build E2E test infrastructure** - `e785910` (feat)
2. **Task 2: Write chess, map, and dataviz E2E test suites** - `716dac2` (feat)

## Files Created/Modified
- `crates/mcp-e2e-tests/Cargo.toml` - Full dependencies: chromiumoxide, axum, tower-http, serde, futures
- `crates/mcp-e2e-tests/src/lib.rs` - Shared utilities: launch_browser, new_page_with_bridge, wait_for_element, wait_for_js_condition
- `crates/mcp-e2e-tests/src/server.rs` - Embedded axum server serving chess/map/dataviz widget directories
- `crates/mcp-e2e-tests/src/bridge.rs` - Mock MCP bridge JS injection via CDP evaluate_on_new_document
- `crates/mcp-e2e-tests/tests/chess.rs` - 10 chess widget E2E tests
- `crates/mcp-e2e-tests/tests/map.rs` - 5 map widget E2E tests
- `crates/mcp-e2e-tests/tests/dataviz.rs` - 5 dataviz widget E2E tests

## Decisions Made
- Used `fetcher`, `rustls`, `zip0` features for chromiumoxide (not `_fetcher-rustls-tokio` which does not exist in v0.9.1)
- Map city detail test bypasses city item click and calls `getCityDetails()` directly -- Leaflet's `marker.openPopup()` triggers map pan and tile loading from openstreetmap.org CDN, which blocks CDP evaluate calls in headless mode for 60+ seconds
- Each test creates its own browser + server to avoid cross-test state leakage (the fetcher caches Chromium on disk so only the first test downloads it)
- Used `#[tokio::test(flavor = "multi_thread")]` for all tests as required by chromiumoxide's handler task

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed chromiumoxide feature flags**
- **Found during:** Task 1 (Cargo.toml setup)
- **Issue:** Plan specified `_fetcher-rustls-tokio` feature which does not exist in chromiumoxide 0.9.1
- **Fix:** Used correct features: `fetcher`, `rustls`, `zip0` (verified via cargo metadata)
- **Files modified:** crates/mcp-e2e-tests/Cargo.toml
- **Verification:** `cargo check -p mcp-e2e-tests` compiles successfully

**2. [Rule 1 - Bug] Fixed map city detail test CDP timeout**
- **Found during:** Task 2 (map tests)
- **Issue:** Clicking a city item triggers `marker.openPopup()` which pans the Leaflet map, loading tiles from openstreetmap.org that block CDP evaluate calls for 60+ seconds in headless mode
- **Fix:** Test calls `getCityDetails('tokyo')` directly instead of clicking the city item, and uses mock response without `suggested_view` to avoid `map.setView()` tile loading
- **Files modified:** crates/mcp-e2e-tests/tests/map.rs
- **Verification:** Test passes in isolation and as part of full suite

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for correctness. No scope creep.

## Issues Encountered
- chromiumoxide's `page.evaluate()` blocks for the full CDP timeout (30-60s) when Leaflet tile loading is in progress. Resolved by avoiding map pan operations in tests that need subsequent CDP evaluate calls.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 19 is the final phase. All examples ship with E2E tests.
- Justfile recipes `just test-e2e`, `just test-e2e-chess`, `just test-e2e-map`, `just test-e2e-dataviz` work correctly.
- Chromium is auto-downloaded on first test run; subsequent runs use cached binary.

---
*Phase: 19-ship-examples-and-playwright-e2e*
*Completed: 2026-02-26*
