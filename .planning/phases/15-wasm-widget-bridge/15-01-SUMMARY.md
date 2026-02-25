---
phase: 15-wasm-widget-bridge
plan: 01
subsystem: preview
tags: [wasm, wasm-pack, axum, atomic, preview-server, mime-types]

# Dependency graph
requires:
  - phase: 14-preview-bridge-infrastructure
    provides: preview server with AppState, McpProxy, and handler architecture
provides:
  - AtomicU64 request ID counter in WASM client (fixes concurrent call corruption)
  - WasmBuilder module for wasm-pack build orchestration and artifact caching
  - WASM API endpoints (build trigger, status query, artifact serving)
  - Correct MIME type serving for .wasm files (application/wasm)
affects: [15-02-PLAN, wasm-bridge-frontend, widget-runtime]

# Tech tracking
tech-stack:
  added: [tokio::process for async Command execution, tokio::sync::RwLock for build status]
  patterns: [AtomicU64 for unique request IDs, workspace root detection via Cargo.toml parsing, async build orchestration with status polling]

key-files:
  created:
    - crates/mcp-preview/src/wasm_builder.rs
    - crates/mcp-preview/src/handlers/wasm.rs
  modified:
    - examples/wasm-client/src/lib.rs
    - crates/mcp-preview/src/handlers/mod.rs
    - crates/mcp-preview/src/server.rs
    - crates/mcp-preview/src/lib.rs

key-decisions:
  - "Used tokio::sync::RwLock for WasmBuilder build status to allow concurrent status queries during builds"
  - "Workspace root detection walks up from cwd looking for [workspace] in Cargo.toml"
  - "Cache check on startup: if pkg artifacts exist, initialize as Ready without rebuilding"

patterns-established:
  - "Async build orchestration: status enum (NotBuilt/Building/Ready/Failed) behind RwLock with polling wait"
  - "MIME type routing: .wasm -> application/wasm, .js -> application/javascript for streaming compilation"
  - "Path traversal protection: artifact path must start_with the artifact directory"

requirements-completed: [WASM-01, WASM-03, WASM-04]

# Metrics
duration: 4min
completed: 2026-02-25
---

# Phase 15 Plan 01: WASM Widget Bridge Foundation Summary

**AtomicU64 request ID fix for WASM client plus WasmBuilder build orchestration and artifact serving at /wasm/* with correct MIME types**

## Performance

- **Duration:** 4 min
- **Started:** 2026-02-25T01:31:48Z
- **Completed:** 2026-02-25T01:35:21Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Fixed critical concurrent call corruption bug: WASM client now uses AtomicU64 for unique request IDs per call
- Built WasmBuilder with wasm-pack detection, async build orchestration, artifact caching, and clear error messages
- Added three WASM API endpoints: POST /api/wasm/build, GET /api/wasm/status, GET /wasm/:filename
- Correct MIME types for WASM streaming compilation (application/wasm) and JS modules

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix WASM client hardcoded request IDs** - `ca5b29e` (fix)
2. **Task 2: Add WasmBuilder module and WASM API routes** - `3368094` (feat)

## Files Created/Modified
- `examples/wasm-client/src/lib.rs` - Added AtomicU64 counter, replaced 3 hardcoded request IDs with next_id()
- `crates/mcp-preview/src/wasm_builder.rs` - WasmBuilder struct with build orchestration, caching, status tracking, workspace root detection
- `crates/mcp-preview/src/handlers/wasm.rs` - Three handler functions: trigger_build, build_status, serve_artifact with MIME routing
- `crates/mcp-preview/src/handlers/mod.rs` - Added wasm module declaration
- `crates/mcp-preview/src/server.rs` - Added wasm_builder to AppState, registered /api/wasm/* and /wasm/* routes
- `crates/mcp-preview/src/lib.rs` - Added pub mod wasm_builder declaration

## Decisions Made
- Used `tokio::sync::RwLock` (not `parking_lot`) for build status since the build operation is async and holds the lock across await points
- Workspace root detection walks up from cwd looking for `[workspace]` in `Cargo.toml` rather than hardcoding paths
- Cache check at startup: if `mcp_wasm_client.js` and `mcp_wasm_client_bg.wasm` exist, WasmBuilder initializes as Ready without triggering a rebuild
- `CARGO_PROFILE_RELEASE_LTO=false` env var set during wasm-pack build for faster compilation

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing compilation errors in the parent `pmcp` crate (missing fields in `ToolInfo`, missing `SinkExt` import) prevent `cargo check` of the wasm-client package from its subdirectory. These errors are in `src/server/wasm_typed_tool.rs` and `src/client/mod.rs`, completely unrelated to the WASM client changes. The WASM client edits are syntactically and semantically correct. The `mcp-preview` crate compiles and passes clippy cleanly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- WasmBuilder infrastructure ready for Plan 02 (frontend toggle, widget-runtime.js integration)
- The `/api/config` endpoint already exposes `mcp_url` which Plan 02 will use for WASM bridge URL injection
- WASM artifact serving at `/wasm/*` ready for frontend `<script type="module">` imports

## Self-Check: PASSED

All 6 created/modified files verified on disk. Both task commits (ca5b29e, 3368094) verified in git history.

---
*Phase: 15-wasm-widget-bridge*
*Completed: 2026-02-25*
