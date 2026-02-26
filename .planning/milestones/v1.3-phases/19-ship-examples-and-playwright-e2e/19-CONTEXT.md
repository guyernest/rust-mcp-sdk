# Phase 19: Ship Examples and E2E Tests - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Finalize three MCP App examples (chess, map, data viz) so they compile and run, then build a Rust-based E2E test suite using CDP to prove the complete widget pipeline works. Replaces the original Playwright approach with a native Rust test crate using `chromiumoxide`.

</domain>

<decisions>
## Implementation Decisions

### Example app lineup
- Three examples ship: chess, map, **and new data visualization**
- Data viz example builds on existing Chinook SQLite Explorer template
- Data viz widget renders interactive charts (bar, line, pie) from SQL query results plus a sortable data table
- Uses a lightweight charting library (e.g., Chart.js) — not hand-rolled SVG
- Chinook database downloaded locally by developer (follow existing curl pattern from other Chinook examples, not embedded via include_bytes)

### E2E test framework
- Replace Playwright entirely — remove `tests/playwright/` directory
- Use `chromiumoxide` Rust crate for CDP-based browser testing
- E2E test crate lives at `crates/mcp-e2e-tests/` as a workspace member
- Auto-download Chromium binary (no pre-install requirement)
- Tests run via `cargo test -p mcp-e2e-tests`

### Test coverage
- Happy path + key interactions per widget (not comprehensive edge cases)
- One spec file per widget: chess, map, data viz
- Chess: board renders, pieces visible, basic interaction
- Map: map renders, markers/regions visible, interaction with controls
- Data viz: chart canvas/SVG appears after tool call, data table populates with rows, chart type switching works
- No visual regression / screenshot comparison

### Mock bridge strategy
- Mock MCP bridge (no real server in tests)
- Inject canned responses before widget loads via CDP
- Test server is an embedded Rust HTTP server (axum or similar) spun up in test setup, serving widget files from examples directories

### Build & run ergonomics
- `cargo build --features mcp-apps` compiles all three examples
- Justfile recipes: `just run-chess`, `just run-map`, `just run-dataviz` for individual examples
- `just test-e2e` runs all widget tests; `just test-e2e-chess`, `just test-e2e-map`, `just test-e2e-dataviz` for individual
- Running an example prints URL to console (no auto-open browser)

### Claude's Discretion
- Mock bridge implementation approach (JS injection vs Rust-serialized JSON)
- Embedded HTTP server choice (axum, warp, or other lightweight option)
- Chart.js vs alternative lightweight charting library
- Exact test assertions and element selectors
- Test parallelism strategy

</decisions>

<specifics>
## Specific Ideas

- Data viz is a natural extension of the existing Chinook SQLite template — developers already know the database from `cargo pmcp new`
- Having three diverse examples (game, geographic, data analytics) demonstrates the variety of the MCP Apps concept
- Rust CDP approach keeps the entire toolchain in Rust — easier CI integration and potential for load testing reuse
- Follow existing Chinook pattern where developer curls the database locally

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 19-ship-examples-and-playwright-e2e*
*Context gathered: 2026-02-26*
