---
status: testing
phase: 15-wasm-widget-bridge
source: [15-01-SUMMARY.md, 15-02-SUMMARY.md]
started: 2026-02-25T01:50:00Z
updated: 2026-02-25T01:50:00Z
---

## Current Test

number: 1
name: Preview server starts with WASM routes registered
expected: |
  Run `cargo run -p mcp-preview` (or however you normally start the preview server).
  The server should start without errors. Visiting the preview UI in a browser should load successfully.
awaiting: user response

## Tests

### 1. Preview server starts with WASM routes registered
expected: Run the preview server. It should start without errors and the preview UI loads in the browser.
result: [pending]

### 2. Proxy/WASM toggle button visible in preview header
expected: The preview UI header shows a segmented toggle with two options: "Proxy" (active, blue background) and "WASM" (inactive). Proxy is the default highlighted state.
result: [pending]

### 3. WASM build status endpoint responds
expected: Visiting /api/wasm/status in the browser (or curl) returns JSON like `{"status": "not_built"}` (or "ready" if previously built). No errors.
result: [pending]

### 4. Clicking WASM toggle triggers build with progress UI
expected: Clicking the "WASM" segment triggers a build. During the build, both toggle buttons are disabled and the WASM segment shows a pulsing animation. After the build completes, the widget reloads. If wasm-pack is not installed, an error toast appears with install instructions: "cargo install wasm-pack && rustup target add wasm32-unknown-unknown".
result: [pending]

### 5. WASM bridge callTool works with same response shape as proxy
expected: In WASM mode, calling window.mcpBridge.callTool('some_tool', {}) from a widget returns the same `{ success, content, _meta }` shape as proxy mode. Both modes work identically from the widget's perspective.
result: [pending]

### 6. DevTools network logging has no mode-specific tags
expected: In the preview DevTools panel, bridge call logs appear identical in both Proxy and WASM modes. There are no "[WASM]" or "[Proxy]" prefixes or tags distinguishing the modes in the log output.
result: [pending]

### 7. widget-runtime.js served and accessible
expected: Visiting /assets/widget-runtime.js in the browser returns JavaScript content. The file contains `data-mcp-url`, `mcpBridgeReady`, and `window.mcpBridge` references. It has no dependency on `window.parent.previewRuntime`.
result: [pending]

### 8. WASM artifacts served with correct MIME types
expected: After a successful WASM build, requesting /wasm/mcp_wasm_client.js returns Content-Type `application/javascript` and /wasm/mcp_wasm_client_bg.wasm returns Content-Type `application/wasm`.
result: [pending]

## Summary

total: 8
passed: 0
issues: 0
pending: 8
skipped: 0

## Gaps

[none yet]
