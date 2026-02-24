# Requirements: MCP Apps Developer Experience

**Defined:** 2026-02-24
**Core Value:** Developers can author, preview, test, and publish MCP Apps with rich UI widgets from the Rust toolchain — from `cargo pmcp new --mcp-apps` through `cargo pmcp preview` to `cargo pmcp manifest`.

## v1.3 Requirements

Requirements for the MCP Apps Developer Experience milestone. Each maps to roadmap phases.

### Preview

- [ ] **PREV-01**: Developer can preview widget in iframe rendered from MCP resource URI via `cargo pmcp preview`
- [ ] **PREV-02**: Widget `window.mcpBridge.callTool()` calls route to real MCP server through preview proxy
- [ ] **PREV-03**: MCP proxy initializes session once and reuses across all subsequent requests
- [ ] **PREV-04**: Preview fetches widget HTML via `resources/read` proxy call to MCP server
- [ ] **PREV-05**: DevTools panel updates in real time when bridge calls are made
- [ ] **PREV-06**: Connection status indicator shows connected/disconnected state
- [ ] **PREV-07**: Resource picker shows multiple UI resources when server exposes more than one

### WASM Bridge

- [ ] **WASM-01**: WASM MCP client loads in preview iframe context and connects to MCP server
- [ ] **WASM-02**: Bridge adapter translates WASM `call_tool()` response to `window.mcpBridge.callTool()` shape
- [ ] **WASM-03**: WASM client handles CORS for cross-origin HTTP transport to local MCP server
- [ ] **WASM-04**: MCP server URL is injected into WASM client from preview server configuration
- [ ] **WASM-05**: Standalone `widget-runtime.js` bundles WASM client as drop-in `window.mcpBridge` polyfill

### Publishing

- [ ] **PUBL-01**: `cargo pmcp manifest` generates ChatGPT-compatible JSON with server URL and tool-to-widget mapping
- [ ] **PUBL-02**: `cargo pmcp landing` generates standalone HTML demo page with mock bridge (no server required)

### Developer Experience

- [ ] **DEVX-01**: Widget HTML files live in `widgets/` directory separate from Rust source code
- [ ] **DEVX-02**: `cargo pmcp new --mcp-apps` scaffolds a complete MCP Apps project with server, widget, and preview.html
- [ ] **DEVX-03**: Shared bridge library (`widget-runtime.js`) eliminates copy-pasted bridge code across widgets
- [ ] **DEVX-04**: Widget preview refreshes on browser reload without server restart
- [ ] **DEVX-05**: Bridge API TypeScript type definitions (`widget-runtime.d.ts`) ship with bridge library
- [ ] **DEVX-06**: Scaffolded project includes README explaining bridge API, stateless pattern, and CSP configuration
- [ ] **DEVX-07**: Scaffolded `main.rs` includes `WidgetCSP` configuration helper with commented examples

### Ship

- [ ] **SHIP-01**: Chess MCP App example compiles and runs with `cargo build --features mcp-apps`
- [ ] **SHIP-02**: Map MCP App example compiles and runs with `cargo build --features mcp-apps`
- [ ] **SHIP-03**: Playwright test server serves widget files at expected paths
- [ ] **SHIP-04**: All chess widget Playwright tests pass
- [ ] **SHIP-05**: Map widget Playwright tests written and passing

## Future Requirements

Deferred to v1.4+. Tracked but not in current roadmap.

### Preview Enhancements

- **PREV-08**: Preview auto-reconnects and refreshes tool list when MCP server restarts
- **PREV-09**: Environment variable simulation (theme, locale, display mode) in preview UI
- **PREV-10**: Fullscreen mode simulating ChatGPT expanded view

### WASM Enhancements

- **WASM-06**: Side-by-side proxy vs WASM comparison toggle in preview UI

### Publishing Enhancements

- **PUBL-03**: `cargo pmcp deploy --mcp-apps` flag with HTTPS validation and MIME verification
- **PUBL-04**: Manifest validation against ChatGPT Apps JSON schema
- **PUBL-05**: Submission checklist printed after manifest generation

### Testing Enhancements

- **SHIP-06**: Playwright HTML report with widget screenshots at each test step
- **SHIP-07**: Integration Playwright test starting preview + real MCP server end-to-end
- **SHIP-08**: Widget accessibility tests via axe-core integration

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| React/Vue/Svelte widget templates | Vanilla HTML/JS sufficient per MCP Apps spec; framework templates balloon scaffolding surface |
| Widget build pipeline (Vite, webpack) | Adds Node.js requirement; MCP Apps spec supports raw HTML; devs can add build step themselves |
| Widget CDN hosting | Widgets served by MCP server via `resources/read`; no separate asset hosting needed |
| Cross-browser testing | Preview targets Chromium only; production hosts use their own rendering |
| Visual regression testing | Widget UIs change frequently; snapshot drift causes false failures |
| Automated ChatGPT submission | Requires OAuth + human review; cannot be automated |
| Publishing to non-ChatGPT stores | Other stores don't exist yet (Claude, Google) |
| HMR for widget HTML | File watcher adds OS-specific complexity; browser refresh sufficient for dev tool |
| WASM client in production widgets | Adds ~200KB+ per widget; ChatGPT injects its own bridge |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| PREV-01 | — | Pending |
| PREV-02 | — | Pending |
| PREV-03 | — | Pending |
| PREV-04 | — | Pending |
| PREV-05 | — | Pending |
| PREV-06 | — | Pending |
| PREV-07 | — | Pending |
| WASM-01 | — | Pending |
| WASM-02 | — | Pending |
| WASM-03 | — | Pending |
| WASM-04 | — | Pending |
| WASM-05 | — | Pending |
| PUBL-01 | — | Pending |
| PUBL-02 | — | Pending |
| DEVX-01 | — | Pending |
| DEVX-02 | — | Pending |
| DEVX-03 | — | Pending |
| DEVX-04 | — | Pending |
| DEVX-05 | — | Pending |
| DEVX-06 | — | Pending |
| DEVX-07 | — | Pending |
| SHIP-01 | — | Pending |
| SHIP-02 | — | Pending |
| SHIP-03 | — | Pending |
| SHIP-04 | — | Pending |
| SHIP-05 | — | Pending |

**Coverage:**
- v1.3 requirements: 26 total
- Mapped to phases: 0
- Unmapped: 26

---
*Requirements defined: 2026-02-24*
*Last updated: 2026-02-24 after initial definition*
