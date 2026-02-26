# mcp-e2e-tests

End-to-end browser tests for MCP Apps widgets using chromiumoxide (Chrome DevTools Protocol).

## What's Tested

| Suite | Tests | Widget |
|-------|-------|--------|
| Chess | 10 | Board rendering, piece selection, valid moves, move execution, state persistence, new game, errors |
| Map | 5 | Container render, city search, markers, count display, detail panel |
| DataViz | 5 | Chart render, table populate, column headers, chart type switch, list_tables init |

**Total: 20 E2E tests**

## How It Works

1. **Embedded axum server** serves widget HTML files from example directories
2. **Mock MCP bridge** injected via CDP `evaluate_on_new_document` before page load
3. **chromiumoxide** controls headless Chromium for DOM assertions and JS evaluation
4. **BrowserFetcher** auto-downloads Chromium on first run (cached for subsequent runs)

## Running Tests

```bash
# All E2E tests
just test-e2e

# Individual suites
just test-e2e-chess
just test-e2e-map
just test-e2e-dataviz

# Via cargo directly (must use single thread)
cargo test -p mcp-e2e-tests -- --test-threads=1
```

## Architecture

- `src/lib.rs` — Shared utilities: `launch_browser`, `new_page_with_bridge`, `wait_for_element`
- `src/server.rs` — Embedded axum server serving widget directories on random ports
- `src/bridge.rs` — Mock MCP bridge JavaScript with tool call logging
- `tests/chess.rs` — Chess widget test suite
- `tests/map.rs` — Map widget test suite
- `tests/dataviz.rs` — Data visualization widget test suite
