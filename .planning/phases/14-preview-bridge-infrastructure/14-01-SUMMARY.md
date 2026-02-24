---
phase: 14-preview-bridge-infrastructure
plan: 01
subsystem: api
tags: [axum, reqwest, json-rpc, mcp-proxy, session-management, resources]

# Dependency graph
requires: []
provides:
  - "McpProxy with session-once initialization via RwLock double-checked locking"
  - "list_resources() and read_resource() proxy methods for MCP resources"
  - "GET /api/resources endpoint with UI HTML MIME type filtering"
  - "GET /api/resources/read endpoint for resource content"
  - "POST /api/reconnect endpoint for session reset and re-initialization"
  - "GET /api/status endpoint for connection state"
affects: [14-02, 15-wasm-bridge, 16-bridge-contract]

# Tech tracking
tech-stack:
  added: []
  patterns: [session-once-rwlock, double-checked-locking, fire-and-forget-notification]

key-files:
  created: []
  modified:
    - crates/mcp-preview/src/proxy.rs
    - crates/mcp-preview/src/handlers/api.rs
    - crates/mcp-preview/src/server.rs
    - crates/mcp-preview/src/handlers/websocket.rs

key-decisions:
  - "Used RwLock<Option<SessionInfo>> instead of OnceCell for resettable session support"
  - "Named resource content struct ResourceContentItem to avoid collision with existing tool ContentItem"
  - "UI resource filtering done in handler layer, not proxy layer (proxy returns all resources)"

patterns-established:
  - "Session-once initialization: RwLock<Option<SessionInfo>> with double-checked locking in ensure_initialized()"
  - "Mcp-Session-Id header capture and forwarding on all subsequent requests"
  - "notifications/initialized fire-and-forget after session handshake"

requirements-completed: [PREV-02, PREV-03, PREV-04]

# Metrics
duration: 2min
completed: 2026-02-24
---

# Phase 14 Plan 01: Preview Bridge Infrastructure Summary

**Session-persistent MCP proxy with RwLock double-checked locking, resource list/read methods, and four new API endpoints for resources, reconnect, and status**

## Performance

- **Duration:** 2 min
- **Started:** 2026-02-24T22:04:25Z
- **Completed:** 2026-02-24T22:06:50Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments
- Replaced per-request MCP session initialization with session-once RwLock pattern using double-checked locking
- Added list_resources() and read_resource() proxy methods for MCP resources/list and resources/read JSON-RPC calls
- Registered four new API endpoints: GET /api/resources (UI-filtered), GET /api/resources/read, POST /api/reconnect, GET /api/status
- Captured and forwarded Mcp-Session-Id header for stateful MCP server compatibility
- Sent notifications/initialized after session handshake per MCP protocol requirement

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor McpProxy for session persistence and add resource methods** - `d4e67a6` (feat)
2. **Task 2: Add resource and reconnect API routes** - `17901f1` (feat)

## Files Created/Modified
- `crates/mcp-preview/src/proxy.rs` - McpProxy with session persistence, resource methods, notification support
- `crates/mcp-preview/src/handlers/api.rs` - Four new API handlers (list_resources, read_resource, reconnect, status)
- `crates/mcp-preview/src/server.rs` - Route registration for new API endpoints
- `crates/mcp-preview/src/handlers/websocket.rs` - Fixed pre-existing clippy useless_conversion warnings

## Decisions Made
- Used `RwLock<Option<SessionInfo>>` instead of `tokio::sync::OnceCell` because `OnceCell` cannot be reset (needed for reconnect button)
- Named the resource content type `ResourceContentItem` to avoid collision with existing `ContentItem` (used for tool responses)
- UI resource filtering (HTML MIME type matching) is done in the handler layer, not the proxy layer -- proxy returns all resources, handler filters for UI

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing clippy warnings in websocket.rs**
- **Found during:** Task 1 (clippy verification)
- **Issue:** Two `useless_conversion` clippy errors in `websocket.rs` (`.into()` on values already of type `String`) blocked `-D warnings` from passing
- **Fix:** Removed unnecessary `.into()` calls on lines 70 and 99
- **Files modified:** `crates/mcp-preview/src/handlers/websocket.rs`
- **Verification:** `cargo clippy -p mcp-preview -- -D warnings` passes clean
- **Committed in:** d4e67a6 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Pre-existing clippy issue in unrelated file blocked quality gate. Fix was trivial (remove 2 `.into()` calls). No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Backend API layer complete -- frontend (14-02) can now call /api/resources, /api/resources/read, /api/reconnect, /api/status
- Resource picker, auto-load, and enhanced DevTools work in Plan 02 can build directly on these endpoints
- McpProxy session persistence eliminates the per-request initialization blocker noted in STATE.md

## Self-Check: PASSED

All files verified present. All commit hashes verified in git log.

---
*Phase: 14-preview-bridge-infrastructure*
*Completed: 2026-02-24*
