# Phase 16: Shared Bridge Library - Context

**Gathered:** 2026-02-25
**Status:** Ready for planning

<domain>
## Phase Boundary

A single canonical bridge library eliminates duplicated JavaScript across widgets and guarantees API consistency between preview, WASM, and production bridge modes. Ships with TypeScript type definitions. Aligns with the official MCP Apps specification (postMessage-based JSON-RPC protocol).

</domain>

<decisions>
## Implementation Decisions

### API surface contract
- Align with the official MCP Apps postMessage/JSON-RPC protocol (`ui/initialize`, `tools/call`, etc.) — not a custom `window.mcpBridge` API
- Preview server serves the `@modelcontextprotocol/ext-apps` JS library (or equivalent) at a stable URL for convenience, but widgets can also bring their own bundled copy
- Focus on the subset needed for interactive views (chess board, maps) — not the full MCP Apps surface
- Early days for MCP Apps — stick to the official spec, don't over-extend

### Bridge mode detection
- Host provides the postMessage interface uniformly across all modes — widget code is identical whether running in preview, WASM standalone, or production (Claude Desktop, VS Code, etc.)
- WASM standalone mode becomes a "mini host" implementation: it creates the iframe, handles postMessage, and proxies tool calls through the WASM MCP client
- Graceful degradation with console warnings when a host doesn't support a particular capability — widget should not crash

### Distribution & loading
- Widget-runtime.js is an ES module (`<script type="module">`)
- Built from TypeScript source — compiled to JS as part of the build process (enterprise/security focus — type-safe source of truth)
- Bundling approach and URL path: Claude's discretion
- `cargo pmcp app` — new subcommand scaffolds a widget/app project with the bridge library, TypeScript types, and best practices baked in

### TypeScript types
- Align type definitions with `@modelcontextprotocol/ext-apps` type signatures for consistent widget author experience
- Type only the methods we actually implement — no phantom types for unimplemented capabilities
- TypeScript source location within repo: Claude's discretion

### Claude's Discretion
- Whether to migrate existing chess/map examples to the official protocol in this phase or separately
- Preview server's approach to hosting the iframe (AppBridge pattern vs inline injection)
- Bundling strategy (single file vs modular imports)
- URL path for serving the bridge library
- TypeScript source directory location within the repo

</decisions>

<specifics>
## Specific Ideas

- Official MCP Apps spec provided as reference: apps use postMessage JSON-RPC protocol, `App` class from `@modelcontextprotocol/ext-apps` is the convenience wrapper
- The spec's AppBridge module handles host-side rendering, message passing, tool call proxying, and security policy enforcement
- `cargo pmcp app` should scaffold a best-practices widget project (similar to how `cargo pmcp test`, `cargo pmcp landing`, `cargo pmcp deploy` work for other aspects)
- The skeleton should be something a developer or their AI assistant can complete — bake in best practices

</specifics>

<deferred>
## Deferred Ideas

- Full MCP Apps protocol coverage (context updates, streaming tool inputs, permissions negotiation) — future phase once the base bridge is stable
- Publishing widget-runtime as a standalone npm package — consider after the API stabilizes

</deferred>

---

*Phase: 16-shared-bridge-library*
*Context gathered: 2026-02-25*
