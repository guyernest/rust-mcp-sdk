---
phase: 16-shared-bridge-library
verified: 2026-02-26T04:30:59Z
status: passed
score: 7/7 must-haves verified
re_verification: false
---

# Phase 16: Shared Bridge Library Verification Report

**Phase Goal:** A single canonical bridge library eliminates duplicated JavaScript across widgets and guarantees API consistency between preview, WASM, and production bridge modes
**Verified:** 2026-02-26T04:30:59Z
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria + PLAN frontmatter)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Preview server serves `widget-runtime.mjs` at a stable URL and widgets reference it via a shared library instead of inline bridge code | VERIFIED | `crates/mcp-preview/assets/widget-runtime.mjs` exists (4027 lines, compiled ESM). `index.html` imports via `import { AppBridge } from '/assets/widget-runtime.mjs'` at line 917. Widget iframe loads via `await import('/assets/widget-runtime.mjs')` at line 1536. No inline `window.mcpBridge = {` definition remains (grep returns 0). |
| 2 | TypeScript type definitions ship alongside the bridge library with correct types for App, AppBridge, CallToolParams, CallToolResult, HostContext | VERIFIED | `packages/widget-runtime/dist/index.d.ts` exists. Exports `App`, `AppBridge`, `CallToolParams`, `CallToolResult`, `HostContext`, `AppOptions`, `AppBridgeOptions`, `TransportOptions`, `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcNotification`. 14 occurrences of the required type names confirmed via grep. |
| 3 | Widget author can `import { App } from './widget-runtime.js'` and call `app.callServerTool()` using MCP Apps postMessage JSON-RPC protocol | VERIFIED | `App` class exported from `packages/widget-runtime/src/app.ts`. `callServerTool(params: CallToolParams): Promise<CallToolResult>` sends `tools/call` JSON-RPC request via `PostMessageTransport`. Both present in compiled `dist/index.mjs`. |
| 4 | Backward-compatibility shim exposes `window.mcpBridge.callTool()` backed by the new App class | VERIFIED | `compat.ts` exports `installCompat(app: App)`. Installs `window.mcpBridge.callTool` mapping to `app.callServerTool()` with legacy `{ success, content }` normalization. Deprecation warning fires once. `window.openai` alias also installed. |
| 5 | Host-side AppBridge class handles iframe postMessage dispatch and tool call proxying | VERIFIED | `app-bridge.ts` exports `AppBridge`. Constructor takes `{ iframe, toolCallHandler, origin?, hostContext? }`. `initialize()` creates `PostMessageTransport` targeting `iframe.contentWindow`. Routes `ui/initialize` and `tools/call` to handlers. |
| 6 | Inline bridge injection in proxy and WASM modes is replaced by a single unified `wrapWidgetHtml()` | VERIFIED | `index.html` has a single `wrapWidgetHtml(html)` at line 1524. Separate `wrapWidgetHtmlProxy()` and `wrapWidgetHtmlWasm()` methods are absent. Both modes use the same dynamic `import('/assets/widget-runtime.mjs')` + `installCompat(app)` widget-side loader. |
| 7 | Makefile `build` target builds TypeScript before compiling the Rust preview crate | VERIFIED | `Makefile` has `build-widget-runtime` target (lines 99-115) that runs `npm run build` then copies `dist/index.mjs` to `crates/mcp-preview/assets/widget-runtime.mjs`. `build` and `build-release` targets both list `build-widget-runtime` as a dependency. |

**Score:** 7/7 truths verified

---

## Required Artifacts

### Plan 16-01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `packages/widget-runtime/src/app.ts` | App class with connect(), callServerTool(), lifecycle callbacks | VERIFIED | 289 lines. `class App` with full implementation: `connect()`, `callServerTool()`, `sendMessage()`, `openLink()`, `sendLog()`, `getHostContext()`, `destroy()`. All lifecycle callback setters present. |
| `packages/widget-runtime/src/transport.ts` | PostMessageTransport with JSON-RPC 2.0 | VERIFIED | 278 lines. `class PostMessageTransport` with `send()`, `respond()`, `notify()`, `onNotification()`, `onRequest()`, `destroy()`. Auto-incrementing correlation IDs. Origin validation. PendingRequest map with timeout cleanup. |
| `packages/widget-runtime/src/app-bridge.ts` | Host-side AppBridge for iframe postMessage routing | VERIFIED | 197 lines. `class AppBridge` with `initialize()`, `sendToolInput()`, `sendToolResult()`, `sendHostContextChanged()`, `sendTeardown()`, `destroy()`. Routes `ui/initialize` and `tools/call`. |
| `packages/widget-runtime/src/compat.ts` | Backward-compat shim mapping window.mcpBridge to App | VERIFIED | 131 lines. `installCompat(app: App)` installs `window.mcpBridge` with `callTool`, `getState`, `setState`, `sendMessage`, `openExternal`, `openLink`, `theme`, `locale`, `displayMode`. One-time deprecation warning. `window.openai` alias installed. |
| `packages/widget-runtime/src/types.ts` | TypeScript types aligned with MCP Apps API surface | VERIFIED | `CallToolParams`, `CallToolResult`, `HostContext`, `AppOptions`, `AppBridgeOptions` present alongside preserved legacy types. `TransportOptions` in transport.ts. |
| `packages/widget-runtime/dist/index.mjs` | Compiled ES module bundle | VERIFIED | 4027 lines. Contains `App`, `AppBridge`, `PostMessageTransport`, `installCompat`. 7 references to `AppBridge`, 6 to `PostMessageTransport`, 2 to `installCompat`. |
| `packages/widget-runtime/dist/index.d.ts` | TypeScript declarations | VERIFIED | Exports all required types. `App`, `AppBridge`, `PostMessageTransport`, `installCompat`, `CallToolParams`, `CallToolResult`, `HostContext`, transport types — all present. |

### Plan 16-02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/mcp-preview/assets/widget-runtime.mjs` | Compiled ES module served at /assets/widget-runtime.mjs | VERIFIED | File exists (4027 lines). Copied from `packages/widget-runtime/dist/index.mjs` by Makefile `build-widget-runtime` target. |
| `crates/mcp-preview/assets/index.html` | Preview UI using AppBridge from shared library | VERIFIED | Imports `AppBridge` from `/assets/widget-runtime.mjs` at line 917. Creates `new AppBridge(...)` with `toolCallHandler`, `origin`, `hostContext` at line 1503. Unified `wrapWidgetHtml()` uses dynamic `import()` for widget-side bridge loading. |
| `Makefile` | Build orchestration: TypeScript before Rust | VERIFIED | `build-widget-runtime` target at lines 99-115. `build: build-widget-runtime` and `build-release: build-widget-runtime` dependencies established. `clean-widget-runtime` target also present. |

---

## Key Link Verification

### Plan 16-01 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `app.ts` | `transport.ts` | App creates PostMessageTransport on connect() | WIRED | `app.ts` line 8: `import { PostMessageTransport } from './transport'`. Line 82: `this._transport = new PostMessageTransport({...})` inside `connect()`. |
| `app-bridge.ts` | `transport.ts` | AppBridge uses PostMessageTransport host-side | WIRED | `app-bridge.ts` line 9: `import { PostMessageTransport } from './transport'`. Line 72: `this._transport = new PostMessageTransport({targetWindow: contentWindow,...})` inside `initialize()`. |
| `compat.ts` | `app.ts` | Shim imports App type and maps callTool to callServerTool | WIRED | `compat.ts` line 8: `import { App } from './app'`. `installCompat(app: App)` receives App instance. `callTool` maps to `app.callServerTool({...})` at line 71. Note: shim does not create `new App` — it accepts an existing instance, which matches the intended usage pattern (caller creates App, passes to installCompat). |

### Plan 16-02 Key Links

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/mcp-preview/assets/index.html` | `widget-runtime.mjs` | `import { AppBridge } from '/assets/widget-runtime.mjs'` | WIRED | Line 917: static import of `AppBridge`. Line 1536: dynamic `import('/assets/widget-runtime.mjs')` in widget wrapper. Both wired. |
| `crates/mcp-preview/assets/index.html` | `/api/tools/call` | AppBridge toolCallHandler proxies to preview API | WIRED | `createToolCallHandler()` at line 1420. Proxy mode path at line 1443: `fetch('/api/tools/call', { method: 'POST', ... })`. Result returned to AppBridge as `CallToolResult`. |
| `packages/widget-runtime/dist/index.mjs` | `crates/mcp-preview/assets/widget-runtime.mjs` | Makefile copy step | WIRED | Makefile line 104: `cp dist/index.mjs ../../crates/mcp-preview/assets/widget-runtime.mjs` inside `build-widget-runtime` target. |

---

## Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| DEVX-03 | 16-01-PLAN, 16-02-PLAN | Shared bridge library (`widget-runtime.js`) eliminates copy-pasted bridge code across widgets | SATISFIED | Three separate bridge implementations (proxy inline ~120 lines, WASM inline ~130 lines, standalone IIFE 137 lines) replaced by single `packages/widget-runtime/` TypeScript source compiled to `dist/index.mjs`. Preview uses `AppBridge` from shared library. Standalone `widget-runtime.js` reduced to 92-line thin loader. `window.mcpBridge = {}` inline definition count in index.html: 0. |
| DEVX-05 | 16-01-PLAN, 16-02-PLAN | Bridge API TypeScript type definitions (`widget-runtime.d.ts`) ship with bridge library | SATISFIED | `packages/widget-runtime/dist/index.d.ts` and `dist/index.d.mts` exist. Export `App`, `AppBridge`, `PostMessageTransport`, `installCompat`, `CallToolParams`, `CallToolResult`, `HostContext`, `AppOptions`, `AppBridgeOptions`, `TransportOptions`, `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcNotification`, plus all legacy types. |

**Orphaned requirements check:** REQUIREMENTS.md maps only DEVX-03 and DEVX-05 to Phase 16. Both are claimed by both plan files. No orphaned requirements.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `app-bridge.ts` | 182, 189 | `return {}` in switch cases | INFO | Valid JSON-RPC acknowledgment returns for optional protocol methods (`ui/sendMessage`, `ui/openLink`). Not a stub — the methods have real side effects (logging, `window.open()`). No impact. |
| `compat.ts` | 77 | `return {}` in `getState()` | INFO | Documented no-op: state management not implemented in new App class. Shim is forward-compat placeholder per plan. No blocker — existing widgets relying on `getState` would receive empty object as documented. |

No blocker or warning-level anti-patterns found.

---

## Human Verification Required

### 1. Widget backward compatibility (chess and map widgets)

**Test:** Run `cargo pmcp preview` with the chess or map example widget. Load the widget in the iframe. Open browser DevTools console. Verify `window.mcpBridge.callTool('some_tool', {})` works and returns a result.
**Expected:** The deprecation warning fires once in console. The tool call routes through `App.callServerTool()` → postMessage → `AppBridge.toolCallHandler` → `fetch('/api/tools/call')` → MCP server response.
**Why human:** End-to-end iframe postMessage flow requires a running browser and MCP server. Origin validation is strict — srcdoc iframes and their origins need runtime confirmation.

### 2. WASM bridge mode toggle

**Test:** In the preview UI, click the WASM toggle button. Verify the bridge switches from proxy fetch to WASM client tool calls. Make a tool call and observe the DevTools Network tab shows WASM mode.
**Expected:** `toggleBridgeMode('wasm')` destroys the current AppBridge, initializes the WASM client, creates a new AppBridge with the WASM `call_tool()` as the toolCallHandler. Network tab badge updates.
**Why human:** WASM initialization requires loading `.wasm` binary and the WASM MCP client JS module at runtime. Cannot verify the WASM initialization sequence programmatically.

### 3. srcdoc iframe ESM import resolution

**Test:** Open the preview with a widget loaded. In browser DevTools, verify no "Failed to fetch" or "Not allowed to load local resource" errors appear for `/assets/widget-runtime.mjs` loaded from the srcdoc iframe.
**Expected:** The `await import('/assets/widget-runtime.mjs')` dynamic import inside the srcdoc script tag resolves against the parent page's base URL, loading the shared library successfully.
**Why human:** The srcdoc + dynamic import() origin resolution is browser-dependent. Chrome and Firefox may behave differently for `null` origin iframes. Runtime browser test needed.

---

## Commit Verification

All 4 task commits from SUMMARYs verified in git log:

| Commit | Description | Verified |
|--------|-------------|---------|
| `5523d4b` | feat(16-01): add App, PostMessageTransport, AppBridge, and compat shim modules | Present |
| `c3290ef` | feat(16-01): update index.ts exports and package.json build config | Present |
| `d6f2534` | feat(16-02): replace inline bridge injection with shared AppBridge library | Present |
| `198a793` | feat(16-02): replace standalone IIFE with thin loader and add Makefile build orchestration | Present |

---

## Summary

Phase 16 goal is achieved. The codebase contains a complete, substantive, and wired canonical bridge library:

**What was built (verified against actual files):**
- `packages/widget-runtime/src/` now contains `app.ts`, `transport.ts`, `app-bridge.ts`, `compat.ts` — all substantive implementations, not stubs
- `dist/index.mjs` (4027 lines) is a real compiled ESM bundle containing `App`, `AppBridge`, `PostMessageTransport`, `installCompat`
- `dist/index.d.ts` ships complete TypeScript declarations for all public API surface

**Duplication eliminated (verified):**
- `index.html` has zero inline `window.mcpBridge = {` definitions
- Single `wrapWidgetHtml()` replaces separate proxy and WASM variants
- `widget-runtime.js` reduced from 137 to 92 lines (thin loader pattern, not duplicated bridge)

**Build pipeline (verified):**
- Makefile `build-widget-runtime` compiles TypeScript and copies to preview assets
- `build` and `build-release` targets depend on it

**API consistency (verified):**
- All three bridge consumers (preview host-side, preview widget-side, standalone loader) pull from the same compiled `widget-runtime.mjs`
- TypeScript types available for all consumers via `dist/index.d.ts`

Three items require human browser testing to confirm runtime behavior (iframe origin resolution, WASM toggle, and backward-compat widget flow).

---

_Verified: 2026-02-26T04:30:59Z_
_Verifier: Claude (gsd-verifier)_
