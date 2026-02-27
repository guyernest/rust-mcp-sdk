---
status: testing
phase: 16-shared-bridge-library
source: 16-01-SUMMARY.md, 16-02-SUMMARY.md
started: 2026-02-26T05:00:00Z
updated: 2026-02-26T05:10:00Z
---

## Current Test
<!-- OVERWRITE each test - shows where we are -->

number: 3
name: Preview Widget Loading
expected: |
  Start preview server and load a widget. The widget iframe loads the bridge via `import('/assets/widget-runtime.mjs')` and `installCompat(app)`. No inline bridge injection — the preview uses AppBridge from the shared library on the host side. Console shows no errors.
awaiting: user response

## Tests

### 1. Build Orchestration
expected: Running `make build-widget-runtime` compiles TypeScript and produces packages/widget-runtime/dist/index.mjs, dist/index.d.ts, and copies ESM to crates/mcp-preview/assets/widget-runtime.mjs
result: pass

### 2. TypeScript Declarations
expected: packages/widget-runtime/dist/index.d.ts contains exported types for App, AppBridge, PostMessageTransport, installCompat, CallToolParams, CallToolResult, and HostContext
result: pass

### 3. Preview Widget Loading
expected: Start preview server and load a widget. The widget iframe loads the bridge via `import('/assets/widget-runtime.mjs')` and `installCompat(app)`. No inline bridge injection — the preview uses AppBridge from the shared library on the host side. Console shows no errors.
result: [pending]

### 4. Backward-Compat Bridge
expected: Existing widgets using `window.mcpBridge.callTool('tool_name', {args})` work without modification. The compat shim maps callTool to app.callServerTool() and returns the legacy `{ success, content }` shape.
result: [pending]

### 5. WASM Bridge Toggle
expected: Toggling to WASM bridge mode in preview destroys and recreates AppBridge with a WASM toolCallHandler. Widget continues to work — tool calls route through the WASM client instead of the proxy fetch.
result: [pending]

### 6. Shared Library Served at Stable URL
expected: The preview server serves the compiled ES module at /assets/widget-runtime.mjs. Requesting this URL returns valid JavaScript with export statements for App, AppBridge, etc.
result: [pending]

## Summary

total: 6
passed: 2
issues: 0
pending: 4
skipped: 0

## Gaps

[none yet]
