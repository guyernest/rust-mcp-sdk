---
phase: 15-wasm-widget-bridge
verified: 2026-02-24T00:00:00Z
status: passed
score: 11/11 must-haves verified
re_verification: false
---

# Phase 15: WASM Widget Bridge Verification Report

**Phase Goal:** Developer can toggle to a WASM bridge mode in preview where an in-browser MCP client connects directly to the server, eliminating the proxy middleman
**Verified:** 2026-02-24
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | WASM client uses unique request IDs for each MCP call (no concurrent call corruption) | VERIFIED | `AtomicU64` field in `WasmClient`, `next_id()` called at all 3 HTTP request sites (connect, list_tools, call_tool). Zero hardcoded `1i64`/`2i64`/`3i64` literals remain. |
| 2 | Preview server can trigger wasm-pack build and cache artifacts | VERIFIED | `WasmBuilder::build()` runs `wasm-pack build --target web` via `tokio::process::Command`, sets `BuildStatus::Ready(pkg_dir)` on success; startup cache check initializes as `Ready` if artifacts exist. |
| 3 | Preview server serves WASM artifacts (.js, .wasm) at /wasm/* with correct MIME types | VERIFIED | Route `/wasm/{filename}` → `handlers::wasm::serve_artifact`; `mime_for_extension()` maps `.wasm` → `application/wasm`, `.js` → `application/javascript`. Path traversal protection present. |
| 4 | WASM build status endpoint reports NotBuilt, Building, Ready, or Failed | VERIFIED | `GET /api/wasm/status` → `build_status()` returns `state.wasm_builder.status()` which formats all four enum variants as JSON strings. |
| 5 | Missing wasm-pack returns clear error message with install instructions | VERIFIED | `wasm_pack_available()` check precedes build; failure sets `Failed("WASM mode requires wasm-pack. Install with: cargo install wasm-pack && rustup target add wasm32-unknown-unknown")`. |
| 6 | Developer can toggle between Proxy and WASM bridge modes in the preview header | VERIFIED | Segmented toggle button (`#bridge-toggle`) in header HTML with two `[data-bridge]` buttons; `setupBridgeToggle()` wires click → `toggleBridgeMode()`; default active is Proxy. |
| 7 | Toggling to WASM triggers a build (if needed) and reloads the widget with WASM bridge injected | VERIFIED | `toggleBridgeMode('wasm')` POSTs to `/api/wasm/build` when `!this.wasmReady`, awaits `{ status: 'ready' }`, then calls `loadResourceWidget(this.activeResourceUri)` which invokes `wrapWidgetHtmlWasm()`. |
| 8 | Widget code calling window.mcpBridge.callTool() works identically in both bridge modes | VERIFIED | Both `wrapWidgetHtmlProxy()` and `wrapWidgetHtmlWasm()` expose `window.mcpBridge.callTool(name, args)` returning `{ success, content, _meta }`. WASM adapter normalizes `CallToolResult { isError, content }` to proxy shape. Both call `preview.logBridgeCall()` with identical arguments. |
| 9 | Bridge calls appear in DevTools Network tab without mode-specific tags | VERIFIED | No `[WASM]` or `[Proxy]` prefixes found in either bridge implementation. Both modes call `preview.logBridgeCall(name, args, result, duration, success)` with the same signature. |
| 10 | Standalone widget-runtime.js loads WASM client and exposes window.mcpBridge outside preview context | VERIFIED | `widget-runtime.js` uses IIFE, reads `data-mcp-url` attribute, resolves WASM URLs relative to script src, dynamically imports WASM module, exposes `window.mcpBridge`. Zero `window.parent.previewRuntime` references. |
| 11 | Default mode is Proxy — WASM requires explicit toggle | VERIFIED | `this.bridgeMode = 'proxy'` initialized in constructor; Proxy button has `class="active"` in initial HTML; WASM toggle is not triggered at startup. |

**Score:** 11/11 truths verified

---

### Required Artifacts

#### Plan 01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `examples/wasm-client/src/lib.rs` | WasmClient with AtomicU64 request ID counter | VERIFIED | `AtomicU64` field declared (line 71), `AtomicU64::new(1)` in `new()` (line 84), `next_id()` helper (line 92), called at 3 HTTP request sites (lines 126, 200, 256). |
| `crates/mcp-preview/src/wasm_builder.rs` | WasmBuilder with build orchestration and caching | VERIFIED | 257-line implementation with `WasmBuilder` struct, `BuildStatus` enum (4 variants), `ensure_built()`, `build()`, `status()`, `artifact_dir()`, `wait_for_build()`, startup cache detection, `find_workspace_root()` helper. |
| `crates/mcp-preview/src/handlers/wasm.rs` | API endpoints for WASM build trigger and artifact serving | VERIFIED | 98 lines; `trigger_build`, `build_status`, `serve_artifact` all implemented with proper MIME types and error handling. |

#### Plan 02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/mcp-preview/assets/index.html` | Proxy/WASM toggle button, wrapWidgetHtmlWasm(), build progress UI | VERIFIED | Toggle HTML at line 797-800; `wrapWidgetHtmlWasm` at line 1588; `wrapWidgetHtmlProxy` at line 1434; `toggleBridgeMode` at line 1756; `bridgeMode = 'proxy'` at line 931; CSS pulse animation; toast error component. |
| `crates/mcp-preview/assets/widget-runtime.js` | Standalone WASM bridge polyfill for use outside preview | VERIFIED | 136-line IIFE; `mcpBridgeReady` event at line 129; `mcpBridgeError` event at line 133; `window.mcpBridge` with all required methods; `window.openai` alias; no preview runtime dependency. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/mcp-preview/src/handlers/wasm.rs` | `crates/mcp-preview/src/wasm_builder.rs` | `AppState.wasm_builder` | WIRED | `state.wasm_builder.ensure_built()` (line 25), `state.wasm_builder.status()` (line 37), `state.wasm_builder.artifact_dir()` (line 55) |
| `crates/mcp-preview/src/server.rs` | `crates/mcp-preview/src/handlers/wasm.rs` | axum route registration | WIRED | `/api/wasm/build` → `handlers::wasm::trigger_build`, `/api/wasm/status` → `handlers::wasm::build_status`, `/wasm/{filename}` → `handlers::wasm::serve_artifact` (lines 94-97) |
| `crates/mcp-preview/assets/index.html` | `/api/wasm/build` | fetch in `toggleBridgeMode()` | WIRED | `fetch('/api/wasm/build', { method: 'POST' })` at line 1765 |
| `crates/mcp-preview/assets/index.html` | `/wasm/mcp_wasm_client.js` | ES module import in `wrapWidgetHtmlWasm()` | WIRED | `import('/wasm/mcp_wasm_client.js')` at line 1604; `init('/wasm/mcp_wasm_client_bg.wasm')` at line 1605 |
| `crates/mcp-preview/assets/widget-runtime.js` | `mcp_wasm_client.js` | dynamic import | WIRED | `import(wasmJsUrl.href)` at line 55, where `wasmJsUrl = new URL('mcp_wasm_client.js', scriptUrl)` (line 45) |
| MCP URL config | `wrapWidgetHtmlWasm()` | `/api/config` → `this.mcpUrl` | WIRED | `get_config` handler exposes `mcp_url` from `PreviewConfig`; `loadConfig()` sets `this.mcpUrl = config.mcp_url` (line 978); injected at line 1608 as `client.connect('${mcpUrl}')` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WASM-01 | 15-01, 15-02 | WASM MCP client loads in preview iframe context and connects to MCP server | SATISFIED | `wrapWidgetHtmlWasm()` imports WASM module at `/wasm/mcp_wasm_client.js`, calls `init()`, creates `WasmClient`, calls `client.connect(mcpUrl)`. MCP URL injected from `/api/config`. |
| WASM-02 | 15-01, 15-02 | Bridge adapter translates WASM `call_tool()` response to `window.mcpBridge.callTool()` shape | SATISFIED | Adapter normalizes `{ isError, content }` to `{ success: !mcpResult.isError, content: mcpResult.content \|\| [], _meta: mcpResult._meta \|\| null }` in both index.html (line 1623-1628) and widget-runtime.js (lines 74-78). |
| WASM-03 | 15-01 | WASM client handles CORS for cross-origin HTTP transport to local MCP server | SATISFIED | `server.rs` applies `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)` globally; WASM MIME types set correctly (`application/wasm`) enabling streaming compilation. |
| WASM-04 | 15-01, 15-02 | MCP server URL is injected into WASM client from preview server configuration | SATISFIED | `PreviewConfig.mcp_url` exposed via `/api/config` endpoint; `loadConfig()` stores it as `this.mcpUrl`; `wrapWidgetHtmlWasm()` interpolates it into the WASM bridge `connect()` call. |
| WASM-05 | 15-02 | Standalone `widget-runtime.js` bundles WASM client as drop-in `window.mcpBridge` polyfill | SATISFIED | `widget-runtime.js` is a self-contained IIFE with `data-mcp-url` configuration, lifecycle events (`mcpBridgeReady`/`mcpBridgeError`), and complete `window.mcpBridge` API. No preview server dependency. |

All 5 required WASM requirements (WASM-01 through WASM-05) are satisfied. No orphaned requirements detected.

---

### Anti-Patterns Found

No blockers or warnings found.

| File | Pattern | Severity | Notes |
|------|---------|----------|-------|
| None | — | — | Zero TODO/FIXME/HACK/PLACEHOLDER comments across all phase files. No empty implementations detected. Both Rust files pass `cargo check` and `cargo clippy -D warnings` clean. |

---

### Human Verification Required

#### 1. WASM Mode End-to-End (WASM-01)

**Test:** Start a local MCP server, run `cargo pmcp preview`, click the "WASM" toggle button, observe build progress, then call a tool via the widget.
**Expected:** Build completes, widget reloads, `window.mcpBridge.callTool()` returns data from the MCP server via direct WASM connection (no proxy hop).
**Why human:** Cannot programmatically verify real WASM-pack build execution, WASM binary initialization in browser, and live MCP connection from an iframe.

#### 2. Response Shape Equivalence (WASM-02)

**Test:** Call the same tool in Proxy mode, then switch to WASM mode and call it again. Inspect the DevTools Network panel in both cases.
**Expected:** Both responses appear identical in the Network log (same `{ success, content }` structure, no mode-specific labels or structural differences).
**Why human:** Requires visual inspection of the DevTools panel at runtime with a live server response.

#### 3. widget-runtime.js Standalone Usage (WASM-05)

**Test:** Create a plain HTML file that includes `<script src="http://localhost:8765/assets/widget-runtime.js" data-mcp-url="http://localhost:3000/mcp"></script>`, open it in a browser, listen for `mcpBridgeReady`, call `window.mcpBridge.callTool()`.
**Expected:** Bridge connects and calls succeed without any preview server context.
**Why human:** Requires real browser execution with a running MCP server and preview server serving the WASM artifacts.

---

## Gaps Summary

No gaps. All 11 must-have truths verified, all 5 artifacts confirmed substantive and wired, all 5 key links confirmed, all 5 WASM requirements satisfied.

The automated checks confirm:
- Zero hardcoded request IDs in WASM client (`grep` count: 0)
- `AtomicU64` present and `next_id()` called at 3 sites in the HTTP path
- `cargo check -p mcp-preview` exits clean
- `cargo clippy -p mcp-preview -- -D warnings` exits clean (zero warnings)
- All 4 phase commits verified in git history: `ca5b29e`, `3368094`, `04d3177`, `fc45b4d`
- No `window.parent.previewRuntime` dependency in `widget-runtime.js`
- No mode tags (`[WASM]`, `[Proxy]`) in bridge call logging

---

_Verified: 2026-02-24_
_Verifier: Claude (gsd-verifier)_
