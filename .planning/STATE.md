# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-24)

**Core value:** Developers can author, preview, test, and publish MCP Apps with rich UI widgets entirely from the Rust toolchain.
**Current focus:** Milestone v1.3 -- MCP Apps Developer Experience (Phase 19 in progress)

## Current Position

Milestone: v1.3 MCP Apps Developer Experience
Phase: 19 of 19 (Ship Examples & E2E Tests)
Plan: 1 of 2 complete
Status: In Progress
Last activity: 2026-02-26 -- Completed 19-01 (Ship examples and justfile)

Progress: [===================================] 97% (18.5/19 phases across all milestones; 6.5/7 in v1.3)

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |

## Performance Metrics

**Velocity:**
- Total plans completed: 40 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12)
- v1.3 plans completed: 12

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

Recent decisions affecting current work:
- Bridge-first approach: Phase 14 (preview bridge) is the load-bearing dependency -- nothing downstream works until widgets render with live tool calls
- Extract shared library after proving: Build two independent bridge implementations (proxy + WASM) before extracting widget-runtime.js to ensure the abstraction is correct
- Used RwLock<Option<SessionInfo>> instead of OnceCell for resettable session support (reconnect button requires reset capability)
- UI resource filtering done in handler layer, not proxy layer (proxy returns all resources; handler filters for HTML MIME types)
- Used native HTML details/summary for expandable bridge call entries (no JS toggle library needed)
- Resource picker shows label for single resource, clickable list for multiple (avoids unnecessary dropdown)
- Badge count auto-clears when Network tab is selected
- Used tokio::sync::RwLock for WasmBuilder build status (async-safe across await points)
- Workspace root detection walks up from cwd looking for [workspace] in Cargo.toml
- Cache check at startup: existing WASM artifacts initialize WasmBuilder as Ready without rebuild
- WASM bridge adapter normalizes CallToolResult { content, isError } to proxy shape { success, content, _meta } -- widget code is bridge-mode-agnostic
- widget-runtime.js resolves WASM artifact URLs relative to script src for deployment portability
- Bridge toggle defaults to Proxy; WASM requires explicit opt-in
- App class resolves target origin from document.referrer (not wildcard '*') for postMessage security
- PostMessageTransport uses auto-incrementing integer IDs for JSON-RPC correlation
- Backward-compat shim normalizes CallToolResult to legacy { success, content } shape
- AppBridge responds with JSON-RPC -32601 for unknown methods
- AppBridge toolCallHandler dispatches based on bridgeMode (proxy fetch vs WASM client) on the host side
- Widget iframe uses dynamic import('/assets/widget-runtime.mjs') with App + installCompat for backward compat
- Unified wrapWidgetHtml() replaces separate proxy/WASM wrappers -- widget-side code is identical regardless of bridge mode
- WASM client initialization moved to host-side toggleBridgeMode() for cleaner separation
- Makefile build/build-release targets depend on build-widget-runtime for correct TypeScript-before-Rust ordering
- WidgetDir reads from disk on every call (no caching) for zero-config hot-reload
- Bridge script auto-injected as type=module before </head> or after <body>
- Widget URI convention: widgets/board.html maps to ui://app/board
- Preview server implements own inject_bridge_script (mirrors WidgetDir) since mcp-preview crate does not depend on pmcp
- Examples use CARGO_MANIFEST_DIR to resolve widgets/ path at compile time
- WidgetCSP commented examples use actual API (.connect/.resources/.redirect) not the nonexistent .default_src/.script_src
- App subcommand namespace: cargo pmcp app new leaves room for future app build, app test
- One-shot scaffolding with error-if-exists matching cargo new semantics
- detect_project takes explicit Path parameter for testability (not cwd)
- WidgetInfo.html field included for future packaging pipeline (marked allow(dead_code))
- name_for_model replaces hyphens and spaces with underscores for ChatGPT compatibility
- server_url trailing slash stripped before /openapi.json path construction
- Mock bridge uses type=module script with window.mcpBridge matching live bridge API
- srcdoc escaping only escapes & and " (minimum for attribute context)
- load_mock_data takes explicit Path parameter for testability (matching detect_project pattern)
- Build command shares single detect_project call for both manifest and landing generation
- Data viz example uses rusqlite with bundled feature for zero-config SQLite compilation
- Contractual element IDs (chart, chartType, dataTable, queryInput, runQueryBtn, loading) for E2E test stability
- Examples remain in workspace exclude list (standalone builds per RESEARCH.md Pitfall 3)
- Justfile replaces ad-hoc scripts per CLAUDE.md convention

### Pending Todos

None.

### Blockers/Concerns

- ~~McpProxy re-initializes MCP session on every request (no session stickiness)~~ -- FIXED in 14-01
- postMessage wildcard origin ('*') in bridge code is a CVE-class vulnerability -- must fix in Phase 14
- ~~WASM client uses hardcoded request IDs causing concurrent call corruption~~ -- FIXED in 15-01
- Bridge contract divergence between preview mock and ChatGPT Skybridge -- must address across Phases 14-16

## Session Continuity

Last session: 2026-02-26
Stopped at: Completed 19-01-PLAN.md (Ship examples and justfile)
Resume: Continue with 19-02 (E2E test crate with chromiumoxide).
