# Phase 19: Ship Examples and E2E Tests - Research

**Researched:** 2026-02-26
**Domain:** Rust E2E browser testing (CDP), MCP widget examples, data visualization
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Three examples ship: chess, map, **and new data visualization**
- Data viz example builds on existing Chinook SQLite Explorer template
- Data viz widget renders interactive charts (bar, line, pie) from SQL query results plus a sortable data table
- Uses a lightweight charting library (e.g., Chart.js) -- not hand-rolled SVG
- Chinook database downloaded locally by developer (follow existing curl pattern from other Chinook examples, not embedded via include_bytes)
- Replace Playwright entirely -- remove `tests/playwright/` directory
- Use `chromiumoxide` Rust crate for CDP-based browser testing
- E2E test crate lives at `crates/mcp-e2e-tests/` as a workspace member
- Auto-download Chromium binary (no pre-install requirement)
- Tests run via `cargo test -p mcp-e2e-tests`
- Happy path + key interactions per widget (not comprehensive edge cases)
- One spec file per widget: chess, map, data viz
- Chess: board renders, pieces visible, basic interaction
- Map: map renders, markers/regions visible, interaction with controls
- Data viz: chart canvas/SVG appears after tool call, data table populates with rows, chart type switching works
- No visual regression / screenshot comparison
- Mock MCP bridge (no real server in tests)
- Inject canned responses before widget loads via CDP
- Test server is an embedded Rust HTTP server (axum or similar) spun up in test setup, serving widget files from examples directories
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

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SHIP-01 | Chess MCP App example compiles and runs with `cargo build --features mcp-apps` | Chess example already exists at `examples/mcp-apps-chess/` with `src/main.rs` and `widgets/board.html`. Currently excluded from workspace. Needs workspace inclusion gated on mcp-apps feature. |
| SHIP-02 | Map MCP App example compiles and runs with `cargo build --features mcp-apps` | Map example already exists at `examples/mcp-apps-map/` with `src/main.rs` and `widgets/map.html`. Currently excluded from workspace. Same workspace treatment as chess. |
| SHIP-03 | Playwright test server serves widget files at expected paths | REPLACED: Embedded Rust HTTP server (axum) replaces Node.js Playwright server. Chromiumoxide + axum serve widgets from example directories. |
| SHIP-04 | All chess widget Playwright tests pass | REPLACED: Rust CDP tests via chromiumoxide. Existing Playwright chess tests (`tests/playwright/tests/chess-widget.spec.ts`) provide exact test scenarios to port. |
| SHIP-05 | Map widget Playwright tests written and passing | REPLACED: Rust CDP tests for map widget. Additionally, data viz widget tests added per user decision. |
</phase_requirements>

## Summary

Phase 19 ships three MCP App examples (chess, map, data visualization) and validates them with Rust-native E2E browser tests. Two examples already exist with working server code and widget HTML; the third (data viz) extends the well-established Chinook SQLite pattern. The E2E testing replaces the existing Playwright setup with `chromiumoxide`, a mature Rust CDP client with async tokio support, auto-download via the `fetcher` feature, and `evaluate_on_new_document` for pre-page-load mock injection.

The critical path is: (1) ensure chess/map examples compile within the workspace, (2) create the data viz example following the established MCP App pattern, (3) build the E2E test crate with embedded axum server and chromiumoxide browser driver, (4) port existing Playwright chess tests and write map/dataviz tests. The mock bridge approach uses CDP's `addScriptToEvaluateOnNewDocument` (exposed as `page.evaluate_on_new_document()` in chromiumoxide) to inject `window.mcpBridge` with canned responses before widget scripts execute -- the same strategy used by the existing Playwright `mock-mcp-bridge.ts` fixture.

**Primary recommendation:** Use `chromiumoxide` v0.9.x with `_fetcher-rustls-tokio` + `zip0` features, `axum` 0.8.x for the embedded test server (matching root crate), and `Chart.js` v4.x via CDN for the data viz widget.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| chromiumoxide | 0.9.x | CDP browser automation for E2E tests | Async/tokio-native, auto-download via fetcher, evaluate_on_new_document for mock injection |
| axum | 0.8.x | Embedded HTTP server for serving widgets in tests | Already used in root Cargo.toml (0.8.5); lightweight, tower-compatible |
| tokio | 1.x | Async runtime | Already workspace standard |
| Chart.js | 4.5.x | Client-side charting in data viz widget | Most popular JS charting library, CDN-loadable, zero build step |
| rusqlite | latest | SQLite access for data viz server | Already used in Chinook template pattern |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tower-http | 0.6.x | CORS + static file serving in test server | Middleware for axum test server |
| serde_json | 1.x | JSON serialization for mock responses | Serialize canned bridge responses |
| tokio-test | 0.4 | Test utilities for async tests | Already in project dev-deps |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| chromiumoxide | headless_chrome (1.0.21) | Synchronous API (threads, not tokio); simpler but blocks test threads. chromiumoxide is async-native and integrates better with tokio ecosystem already in use. |
| axum | hyper directly | More boilerplate for routing; axum already in workspace dependency tree. |
| Chart.js | Plotly.js | Heavier (~3.5MB vs ~200KB minified); Chart.js sufficient for bar/line/pie. |
| Chart.js | Lightweight Chart (lightweight-charts) | Focused on financial charts, not general-purpose. Chart.js covers all required chart types. |

**Installation (Cargo.toml for crates/mcp-e2e-tests):**
```toml
[dependencies]
chromiumoxide = { version = "0.9", features = ["_fetcher-rustls-tokio", "zip0"] }
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["cors", "fs"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

## Architecture Patterns

### Recommended Project Structure
```
crates/mcp-e2e-tests/
  Cargo.toml
  src/
    lib.rs              # Shared test utilities (server setup, browser launch, mock bridge)
    server.rs           # Embedded axum HTTP server for widget serving
    bridge.rs           # Mock MCP bridge JS generation
  tests/
    chess.rs            # Chess widget E2E tests
    map.rs              # Map widget E2E tests
    dataviz.rs          # Data viz widget E2E tests

examples/mcp-apps-chess/     # (existing, minor fixes)
examples/mcp-apps-map/       # (existing, minor fixes)
examples/mcp-apps-dataviz/   # (new)
  Cargo.toml
  src/main.rs
  widgets/dashboard.html
  mock-data/              # Canned SQL results for landing page
```

### Pattern 1: Mock Bridge via CDP Script Injection
**What:** Inject `window.mcpBridge` with canned tool responses before widget scripts execute.
**When to use:** Every E2E test -- eliminates need for a real MCP server.
**Example:**
```rust
// Source: chromiumoxide docs, page.evaluate_on_new_document()
use chromiumoxide::cdp::browser_protocol::page::AddScriptToEvaluateOnNewDocumentParams;

async fn inject_mock_bridge(page: &Page, tool_responses: &str) -> Result<()> {
    let script = format!(r#"
        window.mcpBridge = {{
            callTool: async (name, args) => {{
                const responses = {tool_responses};
                const handler = responses[name];
                if (handler) return JSON.parse(JSON.stringify(handler));
                return {{ error: `Unknown tool: ${{name}}` }};
            }},
            getState: () => ({{}}),
            setState: (s) => {{}},
            getHost: () => ({{ type: 'standalone', capabilities: {{ tools: true, resources: true }} }}),
        }};
    "#);

    page.evaluate_on_new_document(
        AddScriptToEvaluateOnNewDocumentParams::new(script)
    ).await?;
    Ok(())
}
```

**Recommendation for Claude's Discretion (mock bridge approach):** Use **Rust-serialized JSON** injected via `evaluate_on_new_document`. This is simpler than JS function serialization: build a `HashMap<String, serde_json::Value>` in Rust, serialize to JSON string, inject as a lookup table. The existing Playwright approach serializes JS functions as strings and uses `new Function()` -- this is fragile. A JSON lookup table is deterministic and debuggable.

### Pattern 2: Embedded Test Server with axum
**What:** Spin up a temporary axum server in test setup to serve widget HTML files from example directories.
**When to use:** Every E2E test -- serves widget files the browser navigates to.
**Example:**
```rust
use axum::{Router, routing::get_service};
use tower_http::services::ServeDir;
use std::net::SocketAddr;

async fn start_test_server(widget_root: &Path) -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .nest_service("/chess", ServeDir::new(widget_root.join("mcp-apps-chess/widgets")))
        .nest_service("/map", ServeDir::new(widget_root.join("mcp-apps-map/widgets")))
        .nest_service("/dataviz", ServeDir::new(widget_root.join("mcp-apps-dataviz/widgets")));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (addr, handle)
}
```

**Key detail:** Bind to port 0 so the OS assigns a free port -- avoids CI port conflicts.

### Pattern 3: Browser Lifecycle Management
**What:** Download chromium once, share browser instance across tests in a module, create new pages per test.
**When to use:** Test setup -- avoids re-downloading and re-launching per test.
**Example:**
```rust
use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions};
use std::sync::OnceLock;

static BROWSER: OnceLock<Browser> = OnceLock::new();

async fn get_or_launch_browser() -> &'static Browser {
    // First call downloads chromium + launches; subsequent calls reuse
    if let Some(browser) = BROWSER.get() {
        return browser;
    }

    let download_path = std::env::temp_dir().join("mcp-e2e-chromium");
    tokio::fs::create_dir_all(&download_path).await.unwrap();

    let fetcher = BrowserFetcher::new(
        BrowserFetcherOptions::builder()
            .with_path(&download_path)
            .build()
            .unwrap(),
    );
    let info = fetcher.fetch().await.unwrap();

    let config = BrowserConfig::builder()
        .chrome_executable(info.executable_path)
        .arg("--headless")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .build()
        .unwrap();

    let (browser, mut handler) = Browser::launch(config).await.unwrap();
    tokio::spawn(async move { while handler.next().await.is_some() {} });

    let _ = BROWSER.set(browser);
    BROWSER.get().unwrap()
}
```

### Pattern 4: Data Viz Widget (Chart.js + Data Table)
**What:** Single-page widget using Chart.js from CDN for charts and vanilla JS for a sortable data table.
**When to use:** The data visualization example.
**Example structure:**
```html
<!-- widgets/dashboard.html -->
<script src="https://cdn.jsdelivr.net/npm/chart.js@4"></script>
<canvas id="chart"></canvas>
<table id="dataTable">...</table>
<select id="chartType">
    <option value="bar">Bar</option>
    <option value="line">Line</option>
    <option value="pie">Pie</option>
</select>
<script>
    // On init: callTool('execute_query', { sql: 'SELECT ...' })
    // Render results as chart + table
    // Chart type switcher destroys and recreates chart
</script>
```

### Anti-Patterns to Avoid
- **Launching a new browser per test:** Chromium startup is ~2s. Share browser, create new pages.
- **Hardcoding port numbers:** Use port 0 for OS-assigned free ports in test server.
- **Including chromium binary in repo:** Use `fetcher` feature for on-demand download. Cache in temp dir.
- **Embedding large JS in Rust strings:** Keep mock bridge JS minimal (JSON lookup table). Complex JS logic belongs in the widget, not the test harness.
- **Using `waitForTimeout` (sleep) in tests:** Use `find_element` / `wait_for_navigation` which wait for actual conditions. The existing Playwright tests use `waitForTimeout(500)` extensively -- the Rust port should replace these with element waits.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Chromium download | Manual curl/wget scripts | `chromiumoxide::fetcher::BrowserFetcher` | Platform detection, version management, caching |
| HTTP file server | Raw hyper handler | `axum` + `tower_http::services::ServeDir` | Routing, MIME types, error handling all built-in |
| Chart rendering | Hand-rolled SVG/Canvas JS | Chart.js via CDN | Animation, tooltips, responsiveness, accessibility |
| CSS selector waiting | Polling loops with sleep | `page.find_element()` / `page.wait_for_navigation()` | Built-in retry + timeout semantics |
| Mock bridge serialization | Complex JS function stringification | JSON lookup table via `serde_json::to_string` | Deterministic, type-safe, no eval() footguns |

**Key insight:** The existing Playwright mock bridge (`tests/playwright/fixtures/mock-mcp-bridge.ts`) uses `new Function()` to reconstruct serialized handler functions -- this is clever but fragile. For Rust CDP tests, a JSON response lookup table is simpler and more reliable since we only need canned static responses, not dynamic handler logic.

## Common Pitfalls

### Pitfall 1: Chromium Download Flakiness in CI
**What goes wrong:** First test run downloads ~150MB Chromium binary; CI timeouts or network flakes cause failures.
**Why it happens:** `BrowserFetcher::fetch()` downloads from Google's CDN; no retry logic built in.
**How to avoid:** Cache the download directory between CI runs (e.g., GitHub Actions cache on `~/.cache/mcp-e2e-chromium/`). Set generous timeout. Add justfile recipe that pre-downloads: `just setup-e2e`.
**Warning signs:** Tests pass locally but fail in CI with timeout or connection errors.

### Pitfall 2: evaluate_on_new_document Timing
**What goes wrong:** Script injected via `evaluate_on_new_document` runs after widget scripts already set up `window.mcpBridge`.
**Why it happens:** If `evaluate_on_new_document` is called AFTER `page.goto()`, it only applies to subsequent navigations.
**How to avoid:** Call `evaluate_on_new_document` BEFORE `page.goto()`. The sequence is: (1) create page, (2) inject script, (3) navigate.
**Warning signs:** Mock bridge not found; widget falls back to "MCP bridge not available" error.

### Pitfall 3: Workspace Compilation Conflicts
**What goes wrong:** Adding chess/map/dataviz to workspace causes feature resolution conflicts or dependency version mismatches.
**Why it happens:** Examples currently use `pmcp = { path = "../..", features = ["full"] }` and are workspace-excluded. Adding them could conflict with workspace-level feature resolution.
**How to avoid:** Keep examples in workspace `exclude` list. The `cargo build --features mcp-apps` requirement refers to the root crate feature flag, not workspace-wide compilation. The examples are standalone binaries compiled separately (via `cd examples/mcp-apps-chess && cargo build`). Justfile recipes handle the correct working directory.
**Warning signs:** Feature unification errors; unexpected dependency conflicts.

### Pitfall 4: Map Widget External Dependencies
**What goes wrong:** Leaflet.js loaded from CDN in map widget; E2E tests fail because headless chromium can't reach CDN.
**Why it happens:** CI environments may have restricted network access.
**How to avoid:** Either (a) accept CDN dependency for E2E tests (most CIs have internet), or (b) serve Leaflet.js locally from the test server. Option (a) is simpler and matches real-world usage. Chart.js has the same consideration.
**Warning signs:** Map renders as empty box; chart canvas stays blank.

### Pitfall 5: Async Test Runtime Conflicts
**What goes wrong:** `#[tokio::test]` with shared browser state causes panics or deadlocks.
**Why it happens:** Default `#[tokio::test]` uses single-threaded runtime. Chromiumoxide handler needs its own task.
**How to avoid:** Use `#[tokio::test(flavor = "multi_thread")]` for all E2E tests. The browser handler spawned via `tokio::spawn` needs the multi-thread executor.
**Warning signs:** Tests hang indefinitely; "cannot spawn" panics.

### Pitfall 6: Port 0 Not Available on All Platforms
**What goes wrong:** Rarely, but `TcpListener::bind("127.0.0.1:0")` can fail.
**Why it happens:** OS port exhaustion in heavy parallel test runs.
**How to avoid:** Run E2E tests with `--test-threads=1` (serial execution). This is already the project standard per CLAUDE.md.
**Warning signs:** "address already in use" errors.

## Code Examples

### Complete Test Structure (Chess)
```rust
// tests/chess.rs
use mcp_e2e_tests::{inject_mock_bridge, start_test_server, get_or_launch_browser};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn chess_board_renders_64_squares() {
    let (addr, _server) = start_test_server().await;
    let browser = get_or_launch_browser().await;
    let page = browser.new_page("about:blank").await.unwrap();

    // Inject mock bridge with chess_new_game response
    let mock_responses = json!({
        "chess_new_game": {
            "board": [/* initial board state */],
            "turn": "white",
            "history": [],
            "status": "inprogress"
        }
    });
    inject_mock_bridge(&page, &mock_responses).await.unwrap();

    // Navigate to chess widget
    page.goto(format!("http://{}/chess/board.html", addr)).await.unwrap();
    page.wait_for_navigation().await.unwrap();

    // Assert 64 squares rendered
    let squares = page.find_elements(".square").await.unwrap();
    assert_eq!(squares.len(), 64);
}
```

### Data Viz Server Pattern (Chinook)
```rust
// examples/mcp-apps-dataviz/src/main.rs (simplified)
fn execute_query_handler(input: ExecuteQueryInput, _extra: RequestHandlerExtra) -> Result<Value> {
    let db = open_db()?;
    let mut stmt = db.prepare(&input.sql)?;
    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let rows: Vec<Vec<Value>> = /* iterate rows, collect as JSON */;
    Ok(json!({ "columns": columns, "rows": rows }))
}
```

### Justfile Recipes
```just
# E2E tests
test-e2e:
    cargo test -p mcp-e2e-tests -- --test-threads=1

test-e2e-chess:
    cargo test -p mcp-e2e-tests chess -- --test-threads=1

test-e2e-map:
    cargo test -p mcp-e2e-tests map -- --test-threads=1

test-e2e-dataviz:
    cargo test -p mcp-e2e-tests dataviz -- --test-threads=1

# Run examples
run-chess:
    cd examples/mcp-apps-chess && cargo run

run-map:
    cd examples/mcp-apps-map && cargo run

run-dataviz:
    cd examples/mcp-apps-dataviz && cargo run

# Pre-download chromium for CI
setup-e2e:
    cargo test -p mcp-e2e-tests --no-run
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Playwright (Node.js) for widget E2E | chromiumoxide (Rust) for CDP testing | User decision, Phase 19 | Eliminates Node.js dependency; entire pipeline is Rust |
| `tests/playwright/` directory | `crates/mcp-e2e-tests/` workspace member | User decision, Phase 19 | Cleaner workspace integration; `cargo test` runs E2E |
| Two examples (chess, map) | Three examples (chess, map, dataviz) | User decision, Phase 19 | Better demonstrates breadth of MCP Apps concept |
| Makefile for build tasks | Justfile for build tasks | User instruction (CLAUDE.md) | Simpler syntax, cross-platform |

**Deprecated/outdated:**
- `tests/playwright/` directory: Will be removed. Contains `serve.js` (Node HTTP server), `chess-widget.spec.ts`, `mock-mcp-bridge.ts`. All test logic is ported to Rust.
- `examples/mcp-apps-chess/widget/` and `examples/mcp-apps-map/widget/` (singular): Legacy directories alongside `widgets/` (plural). The canonical location is `widgets/` (plural) per Phase 17 convention.

## Existing Code Inventory

### What Already Exists (Reuse)
| Item | Location | Status |
|------|----------|--------|
| Chess server + widget | `examples/mcp-apps-chess/` | Complete, compiles standalone |
| Map server + widget | `examples/mcp-apps-map/` | Complete, compiles standalone |
| Playwright chess tests | `tests/playwright/tests/chess-widget.spec.ts` | 10 test cases to port |
| Mock bridge pattern | `tests/playwright/fixtures/mock-mcp-bridge.ts` | Architecture to replicate in Rust |
| Chinook DB pattern | `cargo-pmcp/src/templates/sqlite_explorer.rs` | SQL tool handlers to reference |
| Widget serving pattern | `crates/mcp-preview/src/server.rs` | axum server pattern to reference |
| MCP App template | `cargo-pmcp/src/templates/mcp_app.rs` | Cargo.toml and main.rs patterns |

### What Needs Creating
| Item | Notes |
|------|-------|
| `examples/mcp-apps-dataviz/` | New example: Chinook SQL + Chart.js + data table |
| `crates/mcp-e2e-tests/` | New crate: E2E test infrastructure + 3 test files |
| `justfile` | New file at workspace root (no justfile exists currently) |

### What Needs Modifying
| Item | Change |
|------|--------|
| Root `Cargo.toml` | Add `crates/mcp-e2e-tests` to workspace members |
| `tests/playwright/` | Remove entire directory |
| `examples/mcp-apps-chess/widget/` | Remove legacy singular directory (keep `widgets/`) |
| `examples/mcp-apps-map/widget/` | Remove legacy singular directory (keep `widgets/`) |

## Open Questions

1. **Chromiumoxide v0.9.x exact feature flag naming**
   - What we know: Features follow pattern `_fetcher-{tls}-{runtime}` (e.g., `_fetcher-rustls-tokio`) plus `zip0` or `zip8`
   - What's unclear: The underscore prefix in feature names is unusual; needs verification against actual Cargo.toml
   - Recommendation: Verify feature names against crates.io/chromiumoxide at implementation time. If exact names differ, the pattern is well-documented in the README.

2. **axum version alignment**
   - What we know: Root crate uses axum 0.8.5; mcp-preview uses axum 0.7. The E2E test crate is independent.
   - What's unclear: Whether workspace feature unification will cause issues with two axum versions
   - Recommendation: Use axum 0.8.x in the E2E crate (matching root). The mcp-preview crate's 0.7 dependency is separate and should not conflict since it's already in the workspace.

3. **Chinook DB in E2E tests**
   - What we know: The data viz example requires the Chinook SQLite database. E2E tests mock the bridge (no real server), so the DB is not needed for tests.
   - What's unclear: Whether `just run-dataviz` should auto-download the DB or require manual curl
   - Recommendation: Follow existing pattern (manual curl documented in README). Add a `just setup-dataviz` recipe that downloads it.

## Sources

### Primary (HIGH confidence)
- [chromiumoxide GitHub](https://github.com/mattsse/chromiumoxide) - README with BrowserFetcher example, feature flags, API overview
- [chromiumoxide docs.rs](https://docs.rs/chromiumoxide/latest/chromiumoxide/page/struct.Page.html) - Page struct methods including evaluate_on_new_document, find_element, evaluate
- Existing codebase: `examples/mcp-apps-chess/`, `examples/mcp-apps-map/`, `tests/playwright/`, `crates/mcp-preview/` -- direct code inspection

### Secondary (MEDIUM confidence)
- [Chart.js docs](https://www.chartjs.org/docs/latest/) - v4.5.1 CDN usage, chart types
- [headless_chrome comparison](https://github.com/rust-headless-chrome/rust-headless-chrome) - Alternative considered; sync API, v1.0.21

### Tertiary (LOW confidence)
- chromiumoxide fetcher feature flag exact naming -- needs verification at implementation time

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- chromiumoxide is the established async Rust CDP crate; axum already in project; Chart.js is industry standard
- Architecture: HIGH -- patterns directly reference existing code (mcp-preview server, Playwright mock bridge) and verified chromiumoxide API
- Pitfalls: HIGH -- timing issues, CI caching, async runtime requirements are well-documented in chromiumoxide ecosystem

**Research date:** 2026-02-26
**Valid until:** 2026-03-26 (stable ecosystem, 30-day validity)
