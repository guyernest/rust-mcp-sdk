---
phase: 15-wasm-widget-bridge
plan: 02
subsystem: preview
tags: [wasm, javascript, bridge-toggle, widget-runtime, iframe, srcdoc, polyfill]

# Dependency graph
requires:
  - phase: 15-wasm-widget-bridge
    provides: WasmBuilder, WASM API endpoints (/api/wasm/build, /api/wasm/status, /wasm/*), AtomicU64 request IDs
provides:
  - Proxy/WASM segmented toggle button in preview header with build progress UI
  - wrapWidgetHtmlWasm() WASM bridge adapter with response normalization matching proxy shape
  - Standalone widget-runtime.js polyfill for WASM bridge usage outside preview context
  - Bridge mode dispatch (wrapWidgetHtml delegates to proxy or WASM variant)
affects: [16-widget-testing, widget-authors, standalone-deployment]

# Tech tracking
tech-stack:
  added: [ES module dynamic import for WASM client, CustomEvent for lifecycle hooks]
  patterns: [Bridge adapter pattern normalizing WASM CallToolResult to proxy shape, data-attribute configuration for script tags, segmented toggle with build state animation]

key-files:
  created:
    - crates/mcp-preview/assets/widget-runtime.js
  modified:
    - crates/mcp-preview/assets/index.html

key-decisions:
  - "WASM bridge adapter normalizes CallToolResult { content, isError } to proxy shape { success, content, _meta } -- widget code is bridge-mode-agnostic"
  - "widget-runtime.js resolves WASM artifact URLs relative to its own script location for deployment portability"
  - "Bridge toggle defaults to Proxy; WASM requires explicit opt-in via toggle click"

patterns-established:
  - "Bridge mode dispatch: wrapWidgetHtml() delegates to wrapWidgetHtmlProxy() or wrapWidgetHtmlWasm() based on this.bridgeMode"
  - "Standalone polyfill pattern: IIFE with data-attribute config, lifecycle events (mcpBridgeReady/mcpBridgeError), window.__mcpState for shared state"
  - "Build progress UX: disabled toggle with CSS pulse animation during WASM build, error toast with auto-dismiss"

requirements-completed: [WASM-01, WASM-02, WASM-04, WASM-05]

# Metrics
duration: 3min
completed: 2026-02-25
---

# Phase 15 Plan 02: WASM Bridge Frontend Toggle and Widget Runtime Summary

**Proxy/WASM segmented toggle in preview header with response-normalized WASM bridge adapter and standalone widget-runtime.js polyfill**

## Performance

- **Duration:** 3 min
- **Started:** 2026-02-25T01:37:57Z
- **Completed:** 2026-02-25T01:41:21Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added Proxy/WASM segmented toggle button in the preview header with color-coded active states (blue for Proxy, purple for WASM)
- Implemented wrapWidgetHtmlWasm() that imports WASM client as ES module, initializes it, connects to MCP server, and normalizes CallToolResult to match proxy bridge shape exactly
- Created standalone widget-runtime.js that provides window.mcpBridge outside preview context via data-mcp-url attribute configuration
- Both bridge modes log identically to DevTools Network tab with no mode-specific tags

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Proxy/WASM toggle and WASM bridge injection** - `04d3177` (feat)
2. **Task 2: Create standalone widget-runtime.js** - `fc45b4d` (feat)

## Files Created/Modified
- `crates/mcp-preview/assets/index.html` - Added bridge toggle HTML/CSS, bridgeMode/wasmReady properties, toggleBridgeMode(), wrapWidgetHtmlWasm(), renamed original to wrapWidgetHtmlProxy(), WASM status check on startup
- `crates/mcp-preview/assets/widget-runtime.js` - Standalone WASM bridge polyfill with data-mcp-url config, mcpBridgeReady/mcpBridgeError lifecycle events, window.openai ChatGPT compat

## Decisions Made
- WASM bridge adapter normalizes `CallToolResult { content, isError }` to `{ success, content, _meta }` matching proxy bridge -- widget code needs no awareness of which bridge is active
- widget-runtime.js resolves WASM artifact URLs relative to its own script `src` attribute using `new URL('mcp_wasm_client.js', scriptUrl)` for deployment portability
- Build error feedback uses a fixed-position toast with 5-second auto-dismiss rather than inline error to avoid layout shift
- During WASM build, both toggle buttons are disabled to prevent race conditions from rapid toggling

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 15 (WASM Widget Bridge) is fully complete: backend infrastructure (Plan 01) + frontend toggle and standalone polyfill (Plan 02)
- Widget authors can now use either proxy bridge (default, zero build step) or WASM bridge (direct connection) with identical APIs
- Standalone widget-runtime.js enables WASM bridge usage outside the preview server context
- Ready for Phase 16 (widget testing and validation)

## Self-Check: PASSED

All 2 created/modified files verified on disk. Both task commits (04d3177, fc45b4d) verified in git history.

---
*Phase: 15-wasm-widget-bridge*
*Completed: 2026-02-25*
