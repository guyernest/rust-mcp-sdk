# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-24)

**Core value:** Developers can author, preview, test, and publish MCP Apps with rich UI widgets entirely from the Rust toolchain.
**Current focus:** Milestone v1.3 -- MCP Apps Developer Experience (Phase 14 complete, ready for Phase 15)

## Current Position

Milestone: v1.3 MCP Apps Developer Experience
Phase: 14 of 19 (Preview Bridge Infrastructure) -- COMPLETE
Plan: 2 of 2 complete
Status: Phase Complete
Last activity: 2026-02-24 -- Completed 14-02 (preview UI: resource picker, auto-load, DevTools enhancements, connection status)

Progress: [==========================.........] 74% (14/19 phases across all milestones; 1/6 in v1.3)

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |

## Performance Metrics

**Velocity:**
- Total plans completed: 31 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 3)
- v1.3 plans completed: 3

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

### Pending Todos

None.

### Blockers/Concerns

- ~~McpProxy re-initializes MCP session on every request (no session stickiness)~~ -- FIXED in 14-01
- postMessage wildcard origin ('*') in bridge code is a CVE-class vulnerability -- must fix in Phase 14
- WASM client uses hardcoded request IDs causing concurrent call corruption -- must fix in Phase 15
- Bridge contract divergence between preview mock and ChatGPT Skybridge -- must address across Phases 14-16

## Session Continuity

Last session: 2026-02-24
Stopped at: Completed 14-02-PLAN.md (preview UI: resource picker, auto-load, DevTools enhancements)
Resume: Phase 14 complete. Start Phase 15 (WASM Bridge) planning or execution.
