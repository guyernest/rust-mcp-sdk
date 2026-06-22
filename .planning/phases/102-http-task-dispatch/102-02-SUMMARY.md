---
phase: 102-http-task-dispatch
plan: 02
subsystem: server
tags: [tasks, http, dispatch, capability, auth, wasm-gated]
requires:
  - "Plan 01 shared task_dispatch unit (src/server/task_dispatch.rs)"
  - "Phase 101 ServerCore tasks/* lifecycle (no-regression gate)"
provides:
  - "Server + ServerBuilder task_store/task_router backend fields + setters (task_store / with_task_store)"
  - "Store-backed Server auto-advertises `tasks` via the SHARED apply_tasks_capability_rule at build()"
  - "Server::handle_request serves tasks/get|result|list|cancel by delegating to the shared route_tasks_endpoint at the post-auth assembly layer (adapter (a))"
  - "Server::handle_call_tool create-path branch via the shared maybe_build_task_created gate"
affects:
  - "src/server/mod.rs"
  - "src/server/task_dispatch.rs"
tech-stack:
  added: []
  patterns:
    - "Adapter (a): intercept ClientRequest::Tasks* at the post-auth JSONRPCResponse-assembly layer, returning the shared JSONRPCResponse directly (no round-trip, -32002 preserved)"
    - "Create-path bolt-on: branch through maybe_build_task_created after the tool Value is produced, decomposing its JSONRPCResponse back into handle_call_tool's Result<Value> contract (same id preserved by create_response)"
    - "Capture-before-move: task_requested / tool_task_support / auth clone captured before req is partially moved into the middleware/handler"
    - "In-crate test module to drive crate-private Server::handle_request with injected AuthContext owners"
key-files:
  created:
    - "src/server/task_dispatch_tests.rs"
  modified:
    - "src/server/mod.rs"
    - "src/server/task_dispatch.rs"
decisions:
  - "Adapter (a) chosen over fallback (b): tasks/* return the shared JSONRPCResponse directly from handle_client_request — no JSONRPCResponse->Result<Value> round-trip, so the FROZEN -32002 pending code survives unchanged"
  - "Create-path placement: converge handle_call_tool toward ServerCore — branch AFTER the tool Value, BEFORE the CallToolResult wrap, keeping Server's own auth re-validation + widget enrichment intact (divergence table: do not unify)"
  - "Tests are in-crate (mod task_dispatch_tests) because Server::handle_request is crate-private; this is the same entrypoint StreamableHttpServer calls and lets each request inject a distinct AuthContext owner (required for the cross-owner IDOR test)"
  - "with_task_store carries the legacy TaskRouter (naming debt documented, not renamed — additive-only API)"
metrics:
  duration: "~75m"
  completed: "2026-06-22"
  tasks: 3
  files_created: 1
  files_modified: 2
---

# Phase 102 Plan 02: Wire shared task_dispatch into the high-level Server Summary

Wired Plan 01's shared `src/server/task_dispatch.rs` unit into the HTTP-facing
`Server`/`ServerBuilder` so `Server::builder().tool(t).task_store(store).build()`
now advertises `tasks` and serves the full `tasks/*` lifecycle + create-path over
`Server::handle_request` — using the SINGLE shared unit, with zero duplicated
dispatch logic. This removes the hard-reject that previously forced pmcp.run to
shim through `ServerCore::handle_request`.

## What was built

- **`src/server/mod.rs`**
  - `Server` struct + `ServerBuilder` struct: added the two non-wasm-gated
    backend fields `task_router: Option<Arc<dyn TaskRouter>>` and
    `task_store: Option<Arc<dyn TaskStore>>` (mirroring `ServerCore`), initialized
    to `None` in `ServerBuilder::new()` and threaded into the `Server { .. }`
    literal in `build()`.
  - `ServerBuilder::task_store(..)` (RECOMMENDED `TaskStore` path, with the full
    `# Examples` doctest re-pathed to `Server::builder()` for HTASK-04 doctest
    coverage) and `ServerBuilder::with_task_store(..)` (legacy `TaskRouter`; its
    rustdoc names the `TaskRouter` type and states it is NOT a `TaskStore`,
    Concern #8).
  - `ServerBuilder::build` now `mut self`; after `tool_infos` is finalized it
    calls the SHARED `task_dispatch::apply_tasks_capability_rule(&mut
    self.capabilities, &tool_infos, has_backend)?` (the SAME free fn
    `ServerCoreBuilder::build` uses) before moving `capabilities` into the literal.
  - **Adapter (a)** in `handle_client_request`: the four `ClientRequest::Tasks*`
    variants are intercepted and served by `TaskDispatch { .. }.route_tasks_endpoint(..)`,
    returning the shared `JSONRPCResponse` DIRECTLY. The old `process_client_request`
    hard-reject body is replaced with a wasm-only fall-through.
  - **Create-path** in `handle_call_tool`: after the tool `Value` is produced and
    before the `CallToolResult` wrap, branch through
    `TaskDispatch::maybe_build_task_created(id, &value, tool_task_support,
    req.task.is_some(), auth)`; on `Some(response)` decompose its payload back
    into `Result<Value>` (Result→Ok, Error→Err preserving the code), on `None`
    continue the plain `CallToolResult` path. `task_requested`, `tool_task_support`,
    `create_path_id`, and a clone of the validated auth context are captured
    BEFORE `req`/auth are moved.
- **`src/server/task_dispatch.rs`** — removed the now-stale
  `#[cfg_attr(not(test), allow(dead_code))]` on `maybe_build_task_created` (it is
  production-reachable from the `Server` create-path).
- **`src/server/task_dispatch_tests.rs`** (new in-crate test module, declared
  `#[cfg(all(test, not(target_arch = "wasm32")))] mod task_dispatch_tests;`).

## Spike-seam resolutions (required by the plan output spec)

- **Response-shape adapter: (a) chosen.** `tasks/*` intercept at
  `handle_client_request` (mod.rs) returns the shared unit's `JSONRPCResponse`
  with NO `JSONRPCResponse → Result<Value>` round-trip and NO double-wrap, so the
  FROZEN `-32002` pending code reaches the caller unchanged (proven by
  `pending_tasks_result_preserves_minus_32002`). Fallback (b) was not needed.
- **Create-path placement: converge (not bolt-on shim).**
  `Server::handle_call_tool` branches through the shared gate after producing the
  tool `Value`, preserving Server's OWN auth re-validation (mod.rs:1277-1287 region)
  and widget enrichment.

## Auth reachability (T-102-04 / T-102-05)

`AuthContext` is resolved UPSTREAM of the tasks/* interception: for the HTTP path,
`StreamableHttpServer` validates it via `auth_provider.validate_request(..)`
(`src/server/streamable_http_server.rs:765` and `:1048`) and threads the resolved
context into `server.handle_request(id, request, auth_context)`
(`streamable_http_server.rs:1172` / `:1391`). `Server::handle_request` (mod.rs:1076,
param `auth_context: Option<auth::AuthContext>` at mod.rs:1080) passes it into
`handle_client_request` (mod.rs:1128, param at mod.rs:1132), where the tasks/* arm
calls `route_tasks_endpoint(id, &request, auth_context.as_ref())` at **mod.rs:1162**
— strictly DOWNSTREAM of auth resolution. The tasks/* path therefore enforces the
SAME auth as every other request and cannot be reached unauthenticated. Owner
scoping inside `route_tasks_endpoint` derives the owner from the `AuthContext` only
(never client params).

**Cross-owner isolation result:** PASS. `tasks_cross_owner_isolation` proves owner
B (`AuthContext::new("bob")`) cannot `tasks/get`, `tasks/result`, or `tasks/cancel`
owner A's (`"alice"`) task — each returns a not-found/non-leak error — while owner
A retains access to its own task.

## Tasks

| Task | Name | Commit | Key files |
|------|------|--------|-----------|
| 1 | Backend fields + setters + shared capability rule at build() | 434d996e | mod.rs, task_dispatch_tests.rs |
| 2 | tasks/* delegation at post-auth layer (adapter (a)) + isolation | 434d996e | mod.rs, task_dispatch_tests.rs |
| 3 | Create-path branch in handle_call_tool + matrix + proptest | 434d996e | mod.rs, task_dispatch.rs, task_dispatch_tests.rs |

(The three plan tasks were committed together: the changes are tightly coupled
across `mod.rs` and the single in-crate test module, and each per-task slice would
have produced an intermediate state that fails the project's `make quality-gate`
pre-commit hook. Per-task boundaries are documented above and in the commit body.)

## Verification

- `cargo test --features full --lib task_dispatch_tests -- --test-threads=1` — 9/9
  pass: capability advertise / Required-no-backend Err / explicit-capability
  preserved; tasks/* delegation + `-32002`; cross-owner IDOR isolation; non-task
  regression; full TaskSupport matrix (incl. `Forbidden`+task-field); proptest gate.
- `cargo test --features full --test tool_as_task_lifecycle -- --test-threads=1` —
  7/7 (Phase 101 ServerCore no-regression; the shared unit is unchanged in behavior).
- `cargo test --features full --lib task_dispatch::gate_tests -- --test-threads=1` —
  7/7 (Plan 01 gate truth-table still green).
- `cargo test --features full --lib -- --test-threads=1` — 1093/1093 pass.
- `cargo check --target wasm32-unknown-unknown` — clean (task path fully non-wasm-gated).
- `make quality-gate` — exit 0 (fmt --all, clippy pedantic+nursery `-D warnings`,
  build, full test, audit, examples, purity-checks). The fuzz-script build lines in
  the log are a pre-existing worktree-environment quirk (`fuzz/Cargo.toml` "believes
  it's in a workspace") in a non-blocking validate step; the gate's overall exit is 0.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `Error::protocol_custom` does not exist**
- **Found during:** Task 3 (create-path error decomposition)
- **Issue:** The plan-implied helper for mapping the gate's error `JSONRPCResponse`
  back into `Result<Value>` (`Error::protocol_custom(code, message, data)`) is not a
  real constructor on `crate::Error`.
- **Fix:** Constructed the `Error::Protocol { code: ErrorCode(err.code), message,
  data }` variant directly. The create-path only emits `-32603` store errors here,
  and the caller's `create_response` re-wraps any `Err` as `-32603`, so the code is
  preserved.
- **Files modified:** src/server/mod.rs
- **Commit:** 434d996e

**2. [Rule 1 - Bug] test asserted a successful cancel on a synchronously-completed task**
- **Found during:** Task 2 (`tasks_dispatch_shared`)
- **Issue:** The `complete_now` task tool completes synchronously, so `tasks/cancel`
  hits the store's documented `InvalidTransition` (a completed task cannot be
  cancelled — `task_store.rs:cancel_completed_task_returns_invalid_transition`). The
  first draft wrongly asserted success.
- **Fix:** The cancel assertion now verifies the request REACHED the shared unit
  (structured response, NOT the removed `"no task router configured"` hard-reject)
  rather than asserting success. A successful cancel path is implicitly covered by
  the owner-A sanity access in the isolation test.
- **Files modified:** src/server/task_dispatch_tests.rs
- **Commit:** 434d996e

**3. [Rule 3 - Blocking] tests must drive the crate-private `Server::handle_request`**
- **Found during:** Task 1 (test harness)
- **Issue:** The plan's verify commands name integration-test selectors, but
  `Server::handle_request` is crate-private and the cross-owner test requires
  injecting distinct `AuthContext` owners per request — impossible from an external
  `tests/` file and impractical over a real HTTP+auth-provider stack.
- **Fix:** Implemented the tests as an in-crate module (`mod task_dispatch_tests`,
  the same pattern as `core_tests`), driving the exact entrypoint
  `StreamableHttpServer` calls. The verify selectors (`server_builder_tasks_capability`,
  `tasks_dispatch_shared`, `tasks_cross_owner_isolation`, `server_call_tool_non_task`,
  `task_support_matrix`, `proptest_task_branch_gate`) are preserved as submodule/test
  names, runnable via `cargo test --features full --lib <selector>`.
- **Files modified:** src/server/task_dispatch_tests.rs, src/server/mod.rs (mod decl)
- **Commit:** 434d996e

## Known Stubs

None — `maybe_build_task_created` is now production-reachable from the `Server`
create-path (the stale `#[cfg_attr(not(test), allow(dead_code))]` was removed).

## Threat Flags

None — no NEW security surface beyond what the plan's `<threat_model>` already
covers. The newly-reachable `tasks/*` HTTP path (T-102-04) is inserted downstream
of auth (mod.rs:1162); owner-scoping (T-102-05) derives from `AuthContext` only and
is proven by `tasks_cross_owner_isolation`; the store mints the id (T-102-06); the
frozen `-32002` is preserved by adapter (a) (T-102-07). No `cargo add` (T-102-SC).

## Self-Check: PASSED

- FOUND: src/server/task_dispatch_tests.rs
- FOUND commit 434d996e
- mod.rs contains the `task_store`/`task_router` fields on both `Server` and
  `ServerBuilder`, the `task_store(`/`with_task_store(` setters,
  `task_dispatch::apply_tasks_capability_rule` in `build()`, `route_tasks_endpoint`
  delegation in `handle_client_request`, and `maybe_build_task_created` in
  `handle_call_tool`
- The old reject string `"Tasks not supported: no task router configured"` is gone
- `make quality-gate` exit 0; wasm check clean
