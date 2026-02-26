---
phase: 16-shared-bridge-library
plan: 01
subsystem: ui
tags: [typescript, postmessage, json-rpc, mcp-apps, esm, bridge]

# Dependency graph
requires:
  - phase: 15-wasm-widget-bridge
    provides: Existing widget-runtime package with WidgetRuntime class and React hooks
provides:
  - App class for widget-side MCP Apps protocol communication
  - PostMessageTransport for JSON-RPC 2.0 over postMessage
  - AppBridge for host-side iframe bridge management and tool call proxying
  - installCompat backward-compatibility shim for window.mcpBridge
  - TypeScript type definitions aligned with MCP Apps spec
  - Compiled ES module (dist/index.mjs) and CJS (dist/index.js) with declarations
affects: [16-02-PLAN, preview-server, wasm-bridge, cargo-pmcp]

# Tech tracking
tech-stack:
  added: []
  patterns: [postMessage JSON-RPC 2.0 protocol, correlation ID request tracking, origin validation]

key-files:
  created:
    - packages/widget-runtime/src/app.ts
    - packages/widget-runtime/src/transport.ts
    - packages/widget-runtime/src/app-bridge.ts
    - packages/widget-runtime/src/compat.ts
  modified:
    - packages/widget-runtime/src/types.ts
    - packages/widget-runtime/src/index.ts
    - packages/widget-runtime/package.json

key-decisions:
  - "App class uses document.referrer for target origin resolution instead of wildcard postMessage"
  - "PostMessageTransport distinguishes requests, responses, and notifications via presence of id and method fields"
  - "Backward-compat shim normalizes CallToolResult to legacy { success, content } shape"
  - "AppBridge auto-responds with JSON-RPC method-not-found for unknown methods"

patterns-established:
  - "JSON-RPC 2.0 correlation: auto-incrementing integer IDs with pending map and timeout cleanup"
  - "Origin validation: reject messages from unexpected origins before processing"
  - "Graceful degradation: 2s connect timeout with console warning, widget continues in standalone mode"

requirements-completed: [DEVX-03, DEVX-05]

# Metrics
duration: 4min
completed: 2026-02-26
---

# Phase 16 Plan 01: Shared Bridge Library Summary

**MCP Apps protocol-aligned TypeScript bridge with App, PostMessageTransport, AppBridge classes and backward-compat shim compiled to ESM/CJS bundle**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-26T04:13:59Z
- **Completed:** 2026-02-26T04:18:17Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments
- Built App class (widget-side) with connect(), callServerTool(), lifecycle callbacks, and graceful degradation
- Built PostMessageTransport with JSON-RPC 2.0 request/response correlation, origin validation, and timeout handling
- Built AppBridge (host-side) for iframe communication with tool call routing and notification dispatch
- Built backward-compat shim mapping window.mcpBridge and window.openai to App instance
- Added CallToolParams, CallToolResult, HostContext, AppOptions, AppBridgeOptions types aligned with MCP Apps spec
- Compiled to dist/index.mjs (ESM) + dist/index.js (CJS) + dist/index.d.ts (declarations)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create App, PostMessageTransport, AppBridge, and compat shim TypeScript modules** - `5523d4b` (feat)
2. **Task 2: Update package.json, index.ts exports, and build ESM + DTS output** - `c3290ef` (feat)

## Files Created/Modified
- `packages/widget-runtime/src/transport.ts` - PostMessageTransport class with JSON-RPC 2.0 send/respond/notify
- `packages/widget-runtime/src/app.ts` - App class for widget-side MCP Apps protocol client
- `packages/widget-runtime/src/app-bridge.ts` - AppBridge class for host-side iframe bridge management
- `packages/widget-runtime/src/compat.ts` - Backward-compatibility shim (window.mcpBridge, window.openai)
- `packages/widget-runtime/src/types.ts` - Added MCP Apps protocol types (CallToolParams, CallToolResult, HostContext, etc.)
- `packages/widget-runtime/src/index.ts` - Added exports for new classes and types
- `packages/widget-runtime/package.json` - Updated build script, added sideEffects: false, build:copy script

## Decisions Made
- App class resolves target origin from `document.referrer` falling back to `window.location.origin` rather than using wildcard `'*'` -- avoids the CVE-class vulnerability noted in STATE.md blockers
- PostMessageTransport uses auto-incrementing integer IDs (not UUIDs) for correlation -- simpler, matches JSON-RPC 2.0 convention
- Backward-compat shim normalizes CallToolResult `{ content, isError }` to legacy `{ success, content }` shape -- existing widgets expecting the old format continue working
- AppBridge auto-responds with JSON-RPC `-32601` (method not found) for unhandled methods rather than silently dropping
- 2-second connect timeout for ui/initialize handshake -- allows standalone development without a host while keeping responsiveness

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed TypeScript strict mode cast errors in compat.ts**
- **Found during:** Task 1 (compat.ts creation)
- **Issue:** `(window as Record<string, unknown>)` rejected by strict TypeScript -- Window type doesn't overlap Record<string, unknown>
- **Fix:** Changed to double cast `(window as unknown as Record<string, unknown>)` for both mcpBridge and openai assignments
- **Files modified:** packages/widget-runtime/src/compat.ts
- **Verification:** `npx tsc --noEmit` passes with zero errors
- **Committed in:** 5523d4b (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minimal -- standard TypeScript strict mode fix. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Bridge library compiled and ready for integration
- Plan 16-02 can now integrate App/AppBridge into preview server and WASM standalone modes
- All existing exports (WidgetRuntime, hooks, utils, types) remain backward-compatible

---
*Phase: 16-shared-bridge-library*
*Completed: 2026-02-26*
