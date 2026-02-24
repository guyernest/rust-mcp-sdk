# Phase 14: Preview Bridge Infrastructure - Context

**Gathered:** 2026-02-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Make `cargo pmcp preview` render widgets from MCP resource URIs in an iframe, with `window.mcpBridge.callTool()` calls proxied to the real MCP server, session reuse across requests, real-time devtools logging, and a resource picker when multiple UI resources exist. Builds on the existing `mcp-preview` crate which already has a preview server, tool panel, widget iframe, bridge injection, devtools panel, and JSON-RPC proxy.

</domain>

<decisions>
## Implementation Decisions

### Widget loading flow
- Auto-load the first UI resource on startup — developer runs `cargo pmcp preview` and immediately sees their widget rendered
- Fetch widget HTML via `resources/read` JSON-RPC proxy call to the MCP server (PREV-04)
- On startup: call `resources/list`, filter to UI resources (HTML MIME types), auto-load the first one into the iframe
- Existing tool panel (left sidebar with tool list, args editor, execute button) stays visible alongside the resource-based widget
- When a widget's bridge `callTool()` returns HTML content, the iframe auto-replaces with the new HTML (current behavior preserved)

### Resource picker design
- Resource picker sits at the top of the left sidebar, above the existing tool list
- Shows name + description for each resource entry
- Only shows UI resources (HTML/widget MIME types) — non-UI resources are filtered out
- When server has only one UI resource: hide the picker, show just a resource name label
- When multiple UI resources exist: show the picker list, clicking switches the iframe to that resource

### DevTools and bridge logging
- Keep all 4 existing devtools tabs: State, Console, Network, Events
- Badge count on the Network tab when new bridge calls happen (non-intrusive notification)
- Each bridge call log entry shows: tool name, arguments sent, response content, and duration in ms — full request/response pair, expandable/collapsible
- Per-tab clear buttons for Console, Network, and Events tabs

### Session lifecycle
- Initialize MCP session once on preview server startup, reuse across all subsequent requests (PREV-03)
- Minimal status display — just connected/disconnected dot in the header, no session ID or duration
- When MCP server is unreachable: inline error message in the widget area with a Retry button
- Reconnect button in the header to re-initialize session and refresh tools/resources without restarting the preview server

### Claude's Discretion
- Exact CSS styling for resource picker entries, badge count, and error states
- Network tab expandable/collapsible entry implementation details
- How the reconnect flow handles in-flight bridge calls
- Session initialization error handling and retry logic internals
- Bridge injection approach for resource-loaded widgets (srcdoc wrapping vs other methods)

</decisions>

<specifics>
## Specific Ideas

- The preview already has a working bridge implementation (`window.mcpBridge` and `window.openai` compatibility) — extend it, don't rewrite it
- The proxy already handles JSON-RPC `initialize`, `tools/list`, `tools/call` — add `resources/list` and `resources/read` to the proxy
- Badge count pattern: similar to browser devtools network tab unread count
- Error state with retry button: keep it simple, inline in the iframe area where the widget would be

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 14-preview-bridge-infrastructure*
*Context gathered: 2026-02-24*
