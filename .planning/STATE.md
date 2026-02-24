# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-02-24)

**Core value:** Developers can author, preview, test, and publish MCP Apps with rich UI widgets entirely from the Rust toolchain.
**Current focus:** Milestone v1.3 -- MCP Apps Developer Experience (Phase 14 in progress)

## Current Position

Milestone: v1.3 MCP Apps Developer Experience
Phase: 14 of 19 (Preview Bridge Infrastructure)
Plan: 1 of 2 complete
Status: Executing
Last activity: 2026-02-24 -- Completed 14-01 (preview bridge backend: session persistence, resource proxy, API routes)

Progress: [=========================..........] 72% (13/19 phases across all milestones; 0/6 in v1.3)

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |

## Performance Metrics

**Velocity:**
- Total plans completed: 29 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 1)
- v1.3 plans completed: 1

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

Recent decisions affecting current work:
- Bridge-first approach: Phase 14 (preview bridge) is the load-bearing dependency -- nothing downstream works until widgets render with live tool calls
- Extract shared library after proving: Build two independent bridge implementations (proxy + WASM) before extracting widget-runtime.js to ensure the abstraction is correct
- Used RwLock<Option<SessionInfo>> instead of OnceCell for resettable session support (reconnect button requires reset capability)
- UI resource filtering done in handler layer, not proxy layer (proxy returns all resources; handler filters for HTML MIME types)

### Pending Todos

None.

### Blockers/Concerns

- ~~McpProxy re-initializes MCP session on every request (no session stickiness)~~ -- FIXED in 14-01
- postMessage wildcard origin ('*') in bridge code is a CVE-class vulnerability -- must fix in Phase 14
- WASM client uses hardcoded request IDs causing concurrent call corruption -- must fix in Phase 15
- Bridge contract divergence between preview mock and ChatGPT Skybridge -- must address across Phases 14-16

## Session Continuity

Last session: 2026-02-24
Stopped at: Completed 14-01-PLAN.md (preview bridge backend)
Resume: Execute 14-02-PLAN.md (frontend: resource picker, auto-load, DevTools enhancements)
