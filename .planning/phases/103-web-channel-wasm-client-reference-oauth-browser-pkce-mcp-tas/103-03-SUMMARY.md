---
phase: 103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
plan: 03
subsystem: testing
tags: [oauth2, axum, route-merge, wasm, getrandom, pkce, tasks, streamable-http]

requires:
  - phase: 102-http-task-dispatch
    provides: HTTP tasks/* dispatch via StreamableHttpServer + with_task_support, store-backed owner resolution
provides:
  - "Proven PUBLIC route-merge seam (pmcp::axum::router) for serving OAuth IdP + MCP on one origin"
  - "Durable committed test (tests/web_channel_oauth_route_merge_spike.rs) asserting the merge"
  - "Owner-resolution decision: AuthContext.subject = IdP user_id; store.list(subject) finds the task"
  - "Example package-split layout (wasm client + native server) validated dependency-clean"
  - "getrandom wasm cfg verdict: cfg-needed = NO (feature wasm_js suffices for getrandom 0.4.2)"
affects: [103-04 demo server, 103-05 browser wasm client]

tech-stack:
  added: []
  patterns:
    - "Single-origin OAuth+MCP composition via pmcp::axum::router(server).merge(oauth_routes)"
    - "Durable in-tree spike test (ephemeral port, bind-before-serve, abort-on-done) instead of throwaway probe binary"

key-files:
  created:
    - .planning/phases/103-web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas/103-SPIKE.md
    - tests/web_channel_oauth_route_merge_spike.rs
  modified: []

key-decisions:
  - "Route-merge seam is PUBLIC: pmcp::axum::router()/router_with_config() returns a layered axum::Router; .merge() OAuth routes — NO SDK change needed (Open Question 1)"
  - "build_mcp_router (pub(crate)) is NOT the seam; plan 04 must use the public pmcp::axum::router_with_config"
  - "Task owner = AuthContext.subject = the IdP user_id chosen at /oauth2/authorize; store.list(subject) finds the minted task (Open Question 2)"
  - "Bearer validator: plan 04 adds a thin AuthProvider adapter mapping InMemoryOAuthProvider.validate_token's TokenInfo.user_id -> AuthContext.subject (no public concrete bearer AuthProvider exists)"
  - "Example package split: examples/web-channel-client/{client (wasm cdylib), server (native)} — two manifests keep wasm build native-dep-free (HIGH-2)"
  - "getrandom 0.4.2 needs only feature=wasm_js, NOT the --cfg getrandom_backend rustflag; no .cargo/config.toml created (Open Question 4)"

patterns-established:
  - "Single-origin IdP+MCP merge: pmcp::axum::router(server).merge(oauth_routes) on one axum::serve listener"
  - "Spike de-risking leaves a DURABLE committed test, not prose, so the result cannot silently regress"

requirements-completed: [WEBCH-04, WEBCH-05, WEBCH-07]

duration: 18min
completed: 2026-06-30
---

# Phase 103 Plan 03: Wave-0 De-Risking Spike Summary

**Proved the OAuth IdP routes merge with the MCP router on ONE origin via the PUBLIC `pmcp::axum::router()` seam (no SDK change), backed by a durable passing test — and settled owner resolution, the example package split, and the getrandom wasm cfg as implementable facts for plans 04/05.**

## Performance

- **Duration:** ~18 min
- **Started:** 2026-06-30
- **Completed:** 2026-06-30
- **Tasks:** 2 completed
- **Files modified:** 2 created

## Accomplishments

- **Open Question 1 (HIGH-3) resolved against the REAL public API:** the route-merge seam is `pmcp::axum::router(server) -> axum::Router` (re-exported `src/lib.rs:56`, defined `src/server/axum_router.rs:72,91`). It returns a fully-layered router that composes with OAuth routes via `axum::Router::merge` — single origin, no CORS, NO SDK-internal change required. `build_mcp_router` (`pub(crate)`) was correctly identified as NOT the seam to use.
- **Durable artifact (HIGH-3):** `tests/web_channel_oauth_route_merge_spike.rs` stands up the merged MCP + `/oauth2/*` router on an ephemeral-port listener and asserts both `GET /oauth2/authorize`, `POST /oauth2/token`, AND MCP `POST /` (initialize) respond. The test **PASSES** under `cargo test --features full`.
- **Open Question 2 resolved:** with a `TaskStore`, `resolve_owner` returns `AuthContext.subject` verbatim (`task_dispatch.rs:182-186`); the IdP mints `TokenInfo.user_id` (`oauth2.rs:522,624`) which maps to `subject`, so `store.list(subject)` finds the create-path-minted task. Bearer validator decision recorded (AuthProvider adapter; no public concrete bearer AuthProvider exists today).
- **HIGH-2 resolved:** the two-manifest example split (`client/` wasm cdylib + `server/` native) keeps the wasm build free of `hyper`/`tokio`/`axum`.
- **Open Question 4 resolved empirically:** the wasm build (`cargo build --target wasm32-unknown-unknown --no-default-features --features wasm`) exits 0 with a forced clean getrandom recompile — getrandom **0.4.2** selects the wasm backend from the `wasm_js` Cargo feature, so the `--cfg getrandom_backend="wasm_js"` rustflag is NOT needed. No `.cargo/config.toml` created.

## Task Commits

1. **Task 1 + Task 2: route-merge proof + getrandom verdict** - `1cbac862` (test)
   - Both findings live in `103-SPIKE.md` (committed together); Task 2 produced no separate file (cfg-needed = NO → no config file to create).

**Plan metadata:** (this SUMMARY + STATE/ROADMAP) committed separately.

## Files Created/Modified

- `.planning/phases/103-.../103-SPIKE.md` - Decisions for plans 04/05: named public route-merge API, owner resolution, bearer validator, package split, getrandom cfg verdict.
- `tests/web_channel_oauth_route_merge_spike.rs` - Durable committed proof of the single-origin OAuth+MCP route merge (PASSES under `--features full`).

## Decisions Made

- Route-merge: `pmcp::axum::router(server).merge(oauth_routes)` on one `axum::serve` listener (PUBLIC, no SDK change).
- Owner: `AuthContext.subject` = the IdP `user_id` from `/oauth2/authorize`; updater lists `store.list(subject)`.
- Bearer: plan 04 adds an `AuthProvider` adapter over `InMemoryOAuthProvider::validate_token` (maps `TokenInfo.user_id` → `subject`).
- Package split: `examples/web-channel-client/{client (wasm), server (native)}`.
- getrandom: cfg-needed = NO; feature `wasm_js` suffices for getrandom 0.4.2.

## Deviations from Plan

None — plan executed exactly as written. The escalation branch of Task 1 (SDK change required) did not trigger because the public seam was found. Task 2's optional `server/.cargo/config.toml` was correctly NOT created (cfg-needed = NO).

## Verification

- `103-SPIKE.md` present with 4 "Open Question" answers, named public route-merge API, owner resolution, bearer validator, package split, getrandom verdict.
- `tests/web_channel_oauth_route_merge_spike.rs` present (16 `oauth2` references) and PASSES.
- `cargo build --target wasm32-unknown-unknown --no-default-features --features wasm` exits 0.
- `cargo clippy --features full --test web_channel_oauth_route_merge_spike -- -D warnings`: no issues.
- No throwaway probe binary under `examples/web-channel-client/` (directory not created).

## Known Stubs

None. The spike produced a real passing test and source-cited decisions, not stubs.

## Self-Check: PASSED

- FOUND: 103-SPIKE.md
- FOUND: 103-03-SUMMARY.md
- FOUND: tests/web_channel_oauth_route_merge_spike.rs
- FOUND commit: 1cbac862
