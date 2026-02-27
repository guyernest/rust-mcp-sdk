---
phase: 16-shared-bridge-library
plan: 02
subsystem: ui
tags: [typescript, esm, appbridge, postmessage, preview-server, makefile, build-orchestration]

# Dependency graph
requires:
  - phase: 16-shared-bridge-library
    plan: 01
    provides: Compiled App, AppBridge, installCompat classes in packages/widget-runtime/dist/index.mjs
provides:
  - Preview server index.html using AppBridge from shared ESM library instead of inline bridge injection
  - Unified widget HTML wrapper using dynamic import of shared library for backward compat
  - Thin standalone widget-runtime.js loader delegating to shared ESM library
  - Makefile build-widget-runtime target compiling TypeScript before Rust embeds assets
affects: [cargo-pmcp, preview-server, widget-authoring]

# Tech tracking
tech-stack:
  added: []
  patterns: [host-side AppBridge with toolCallHandler dispatch, dynamic import() for srcdoc iframes, Makefile TypeScript-before-Rust build orchestration]

key-files:
  created:
    - crates/mcp-preview/assets/widget-runtime.mjs
  modified:
    - crates/mcp-preview/assets/index.html
    - crates/mcp-preview/assets/widget-runtime.js
    - Makefile

key-decisions:
  - "AppBridge toolCallHandler dispatches based on bridgeMode (proxy fetch vs WASM client) on the host side"
  - "Widget iframe uses dynamic import('/assets/widget-runtime.mjs') with App + installCompat for backward compat"
  - "Unified wrapWidgetHtml() replaces separate wrapWidgetHtmlProxy() and wrapWidgetHtmlWasm() methods"
  - "WASM client initialization moved to host-side toggleBridgeMode() instead of widget-side inline code"
  - "Makefile build and build-release targets depend on build-widget-runtime for correct build ordering"

patterns-established:
  - "Host-side bridge dispatch: AppBridge.toolCallHandler is a single function that dispatches to proxy or WASM based on runtime mode"
  - "Dynamic ESM import in srcdoc: use inline <script type='module'> with await import() for srcdoc iframes (null origin workaround)"
  - "Build orchestration: TypeScript compile -> copy to assets -> Rust embed via rust_embed"

requirements-completed: [DEVX-03, DEVX-05]

# Metrics
duration: 6min
completed: 2026-02-26
---

# Phase 16 Plan 02: Preview Server Integration Summary

**Replaced ~250 lines of duplicated inline bridge JavaScript with AppBridge from shared ESM library, unified proxy/WASM wrappers, and added Makefile TypeScript build orchestration**

## Performance

- **Duration:** 6 min
- **Started:** 2026-02-26T04:20:44Z
- **Completed:** 2026-02-26T04:26:40Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Eliminated duplicated inline bridge JavaScript from preview index.html -- both wrapWidgetHtmlProxy() (~120 lines) and wrapWidgetHtmlWasm() (~130 lines) replaced by single unified wrapWidgetHtml() (~25 lines)
- Host side now uses AppBridge from shared library with a toolCallHandler that dispatches to proxy fetch or WASM client based on bridge mode
- Widget iframe loads bridge via `import('/assets/widget-runtime.mjs')` + `installCompat(app)` for backward-compat `window.mcpBridge`
- Standalone widget-runtime.js reduced from 137 to 92 lines -- now a thin loader delegating to shared ESM
- Makefile `build-widget-runtime` target ensures TypeScript compiles before Rust embeds assets

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace inline bridge injection with shared library in preview index.html** - `d6f2534` (feat)
2. **Task 2: Replace standalone widget-runtime.js IIFE and update Makefile build orchestration** - `198a793` (feat)

## Files Created/Modified
- `crates/mcp-preview/assets/widget-runtime.mjs` - Compiled ESM from shared TypeScript library (embedded by rust_embed, served at /assets/widget-runtime.mjs)
- `crates/mcp-preview/assets/index.html` - Preview UI now imports AppBridge from shared library; unified wrapWidgetHtml(); host-side toolCallHandler dispatch
- `crates/mcp-preview/assets/widget-runtime.js` - Thin standalone loader (92 lines) delegating to shared ESM via dynamic import
- `Makefile` - Added build-widget-runtime and clean-widget-runtime targets; build/build-release depend on build-widget-runtime

## Decisions Made
- AppBridge toolCallHandler dispatches based on `bridgeMode` property on the host side -- WASM client is initialized once during toggleBridgeMode() and reused for all subsequent tool calls, rather than being re-initialized per widget load
- Unified wrapWidgetHtml() used for both proxy and WASM modes because the widget-side code is identical (it uses installCompat which goes through postMessage to the host AppBridge regardless of mode)
- WASM client initialization moved from widget-side inline code to host-side toggleBridgeMode() -- cleaner separation of concerns and avoids loading WASM artifacts inside srcdoc iframes
- Kept the `<script type="module">` approach (not regular `<script>`) on the preview outer page to enable static `import` of AppBridge

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Shared bridge library is now fully integrated into the preview server
- Three independent bridge implementations (proxy inline, WASM inline, standalone IIFE) are replaced by the single canonical ES module
- Phase 16 is complete -- ready for Phase 17 (App Scaffolding)

## Self-Check: PASSED

All created/modified files verified present. Both task commits (d6f2534, 198a793) found in git log.

---
*Phase: 16-shared-bridge-library*
*Completed: 2026-02-26*
