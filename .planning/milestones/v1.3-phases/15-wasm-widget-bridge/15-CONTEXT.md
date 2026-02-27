# Phase 15: WASM Widget Bridge - Context

**Gathered:** 2026-02-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Add an in-browser WASM MCP client as an alternative bridge mode in preview. When toggled, the WASM client connects directly to the MCP server from inside the iframe — no proxy middleman. Bundle the WASM client as a standalone `widget-runtime.js` that exposes the same `window.mcpBridge` API as the proxy bridge. Builds on Phase 14's preview UI and bridge protocol.

</domain>

<decisions>
## Implementation Decisions

### Bridge mode toggle
- Toggle button in the preview UI header: "Proxy" / "WASM" — developer clicks to swap modes
- Default mode is Proxy (simpler, faster startup)
- Toggling triggers a full widget reload — iframe is destroyed and recreated with the selected bridge injected
- The toggle button itself shows which mode is active (highlighted state) — no separate label needed
- Strategic context: Rust-first with PMCP SDK is the primary use case; wasm-pack and the wasm32 target are acceptable prerequisites. However, architect so the WASM binary could be pre-built and distributed separately later (for non-Rust developers using dev/test tools)

### WASM delivery and build
- Auto-build on toggle: when developer toggles to WASM mode, preview server runs wasm-pack build if needed, caches the output
- WASM artifacts cached (Claude's discretion on location — `target/wasm-bridge/` or equivalent)
- First toggle may be slow (WASM compilation); subsequent toggles use cached artifacts
- Developer needs wasm-pack and the wasm32 target installed (acceptable for Rust-first use case)

### widget-runtime.js shape
- API surface matches Phase 14 proxy bridge exactly — drop-in replacement: callTool, getState, setState, sendMessage, openExternal, theme/locale/displayMode getters
- Auto-connect on script load — reads server URL and connects immediately, no manual init code needed
- Bundle approach at Claude's discretion (single file with inlined WASM vs JS + separate .wasm fetch)
- Server URL specification at Claude's discretion (data attribute vs init call — pick simplest for widget authors)

### Transport selection
- HTTP (Streamable HTTP) transport for the WASM bridge — matches production MCP server patterns
- CORS handling at Claude's discretion (investigate existing SDK CORS support in StreamableHttpServer)
- Non-CORS fallback at Claude's discretion (Rust-first strategy means PMCP servers have CORS by default)
- Bridge calls in devtools look identical regardless of mode — transparent, no [WASM]/[Proxy] tags

### Claude's Discretion
- WASM artifact cache location (target/wasm-bridge/ or user-global cache)
- widget-runtime.js bundle approach (inlined WASM vs separate .wasm file)
- Server URL injection method for standalone widget-runtime.js
- CORS handling strategy (SDK built-in vs preview proxy fallback)
- Non-CORS server fallback approach
- wasm-pack build configuration and flags
- Error handling when wasm-pack is not installed

</decisions>

<specifics>
## Specific Ideas

- Existing `examples/wasm-client/` has a working `WasmClient` with HTTP and WebSocket transports — use it as the foundation, don't rewrite
- The `WasmClient` already handles `connect()`, `list_tools()`, `call_tool()`, and session management
- `wasm-bindgen` and `serde-wasm-bindgen` are already dependencies
- Widget authors should be able to include `widget-runtime.js` with a single `<script>` tag and have `window.mcpBridge` ready immediately
- The dual-audience insight: Rust developers (wasm-pack OK) now, pre-built binary distribution for other frameworks later

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 15-wasm-widget-bridge*
*Context gathered: 2026-02-24*
