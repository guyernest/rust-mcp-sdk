---
phase: 102-http-task-dispatch
plan: 01
subsystem: server
tags: [tasks, refactor, dispatch, capability, wasm-gated]
requires:
  - "Phase 101 ServerCore tasks/* lifecycle (tests/tool_as_task_lifecycle.rs)"
provides:
  - "src/server/task_dispatch.rs shared task-lifecycle unit (apply_tasks_capability_rule free fn + TaskDispatch borrow-struct + single-source success_response/error_response)"
  - "Self-enforcing maybe_build_task_created create-path gate (explicit task_requested) proven by in-module gate_tests"
  - "ServerCore + ServerCoreBuilder delegating to the shared unit (zero behavior change)"
affects:
  - "src/server/core.rs"
  - "src/server/builder.rs"
  - "src/server/mod.rs"
tech-stack:
  added: []
  patterns:
    - "Free-fn capability rule over explicit params (two builders hold tool_infos at different lifecycle points)"
    - "Borrow-struct (TaskDispatch<'a>) over &task_store/&task_router for shared dispatch"
    - "Single-source JSON-RPC envelope builders with thin delegating wrappers"
    - "Self-enforcing gate (raw facts in, full gate internal) proven by in-module truth-table"
key-files:
  created:
    - "src/server/task_dispatch.rs"
  modified:
    - "src/server/core.rs"
    - "src/server/builder.rs"
    - "src/server/mod.rs"
decisions:
  - "Lift to a free-standing task_dispatch unit (research Option A) — retires the two-dispatcher drift rather than coupling Server to ServerCore (Option B) or duplicating (Option C)"
  - "maybe_build_task_created takes explicit task_requested: bool and enforces the COMPLETE gate internally (no caller precondition); proven HERE in Plan 01 by gate_tests"
  - "build_task_created_response re-extracts task_id + terminal result from the value (single source); ToolCallOutcome::TaskCreated slimmed to { task_value }"
  - "pub(crate) items in a pub(crate) mod + scoped #![allow(clippy::redundant_pub_crate)] — reconciles the crate-level unreachable_pub warn vs the nursery redundant_pub_crate lint for an internal module"
metrics:
  duration: "~50m"
  completed: "2026-06-22"
  tasks: 3
  files_created: 1
  files_modified: 3
---

# Phase 102 Plan 01: Shared task_dispatch Unit Summary

Extracted Phase 101's `ServerCore`-only task machinery into ONE shared
`src/server/task_dispatch.rs` unit (research Option A) and refactored `ServerCore`
+ `ServerCoreBuilder` to delegate to it with ZERO behavior change — the foundational
"share" half of HTTP task dispatch, proven against the already-green Phase 101 suite.

## What was built

- **`src/server/task_dispatch.rs`** (new, entirely `#[cfg(not(target_arch = "wasm32"))]`):
  - `apply_tasks_capability_rule(&mut ServerCapabilities, &HashMap<String, ToolInfo>, has_backend)`
    — the endpoint-backed `tasks`-capability rule as a free fn over explicit params
    (the two builders hold `tool_infos` at different lifecycle points), error string
    preserved byte-identical.
  - `default_tasks_capability()` — the single FROZEN `ServerTasksCapability` shape.
  - `TaskDispatch<'a>` borrow-struct over `&Option<Arc<dyn TaskStore>>` +
    `&Option<Arc<dyn TaskRouter>>` exposing `resolve_owner`, `extract_terminal_result`,
    `build_task_created_response`, `maybe_build_task_created`, `handle_tasks_result`,
    and `route_tasks_endpoint` (split into per-endpoint `route_tasks_get/list/cancel`).
  - `success_response` / `error_response` — the SINGLE-SOURCE JSON-RPC envelope
    builders.
  - `#[cfg(test)] mod gate_tests` — 7-row truth-table for `maybe_build_task_created`
    (task_requested=false, no-backend, Forbidden, None, Optional+shaped, Required+shaped,
    Required+missing-fields), with the store-minted three-way-id invariant asserted on
    every Some-case.
- **`src/server/mod.rs`** — registered `pub(crate) mod task_dispatch;` (wasm-gated)
  next to `task_store`.
- **`src/server/builder.rs`** — `ServerCoreBuilder::apply_tasks_capability_rule` and
  `default_tasks_capability` now delegate to the shared unit (the dead inherent
  `default_tasks_capability` fn was removed once it had no caller).
- **`src/server/core.rs`** — `resolve_task_owner`, `build_task_created_response`, the
  tasks/* dispatch arms (collapsed into one `route_tasks_endpoint` delegation), and
  `success_response`/`error_response` (delegating wrappers, wasm branch keeps an inline
  body) all delegate to `TaskDispatch`. `ToolCallOutcome::TaskCreated` slimmed to
  `{ task_value }`; the now-dead `extract_terminal_result` + `handle_tasks_result`
  wrapper were removed.

## Tasks

| Task | Name | Commit | Key files |
|------|------|--------|-----------|
| 1 | Shared free capability rule + module registration | 2abd03cf | task_dispatch.rs, mod.rs, builder.rs |
| 2 | Lift task lifecycle into TaskDispatch + delegate ServerCore | 4a5baa26 | task_dispatch.rs, core.rs |
| 3 | PMAT cog-25 + clippy + wasm-boundary + quality gate | 51b799f5 | task_dispatch.rs, core.rs |

## Verification

- `cargo test --features full --test tool_as_task_lifecycle` — 7/7 pass (Phase 101 no-regression: store-minted id, -32002 pending, three-way id, TaskRouter fallback).
- `cargo test --features full --lib task_dispatch::gate_tests` — 7/7 pass (create-path gate truth table incl. Forbidden/None/missing-fields → None, no error leak).
- `pmat analyze complexity --max-cognitive 25 --top-files 0` — zero `task_dispatch` violations (project-wide src/ is clean; the only two cog>25 hits are pre-existing test files in `mcp-tester` / `pmcp-server-toolkit`, out of scope).
- `cargo check --target wasm32-unknown-unknown` — clean; the wasm build sees no `TaskStore`/`TaskRouter` symbols (module fully gated).
- `git diff src/types/tasks.rs` — empty (wire types frozen).
- `make quality-gate` — green (fmt --all, clippy pedantic+nursery, build, test, audit, examples, purity-checks).

### Single-source / precedence invariants held

- Envelope builders single-source: `grep -c "JSONRPCResponse::Result" src/server/core.rs` did not increase; ServerCore's `success_response`/`error_response` delegate to `task_dispatch`.
- `-32002`/`-32601` precedence: core.rs `32002` count went 4 → 3 (the `handle_tasks_result` body moved to task_dispatch — decreased, not duplicated divergently); task_dispatch owns the precedence.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] clippy `redundant_pub_crate` vs `unreachable_pub` conflict in a `pub(crate) mod`**
- **Found during:** Task 3 (`make quality-gate`)
- **Issue:** `pub(crate)` items inside a `pub(crate)` module trip clippy's nursery `redundant_pub_crate`; switching them to `pub` trips the crate-level `unreachable_pub` warn (denied by `-D warnings`) because the module is internal and not re-exported in `lib.rs`.
- **Fix:** Kept the semantically-correct `pub(crate)` items and added a scoped, `// Why:`-annotated `#![allow(clippy::redundant_pub_crate)]` at the module top (idiomatic resolution; mirrors the codebase intent of an internal `pub(crate)` module). No `cognitive_complexity` allow was used anywhere.
- **Files modified:** src/server/task_dispatch.rs
- **Commit:** 51b799f5

**2. [Rule 3 - Blocking] `doc_lazy_continuation` on the owner-priority `>` chain + test-module doc/wrap lints**
- **Found during:** Task 3
- **Issue:** A `resolve_owner` doc line used `subject > client ID > session ID` which clippy parsed as a stray markdown blockquote; the `gate_tests` `///` summaries name args by literal spelling (`doc_markdown`) and `store_backend()` always returns `Some` (`unnecessary_wraps`).
- **Fix:** Rephrased the priority chain prose; added a module-scoped `#[allow(clippy::doc_markdown, clippy::unnecessary_wraps)]` on `gate_tests` (test-ergonomic noise).
- **Files modified:** src/server/task_dispatch.rs
- **Commit:** 51b799f5

**3. [Rule 3 - Blocking] cfg-gated unused imports in core.rs after delegation**
- **Found during:** Task 2/3
- **Issue:** Once `success_response`/`error_response` delegate and `build_task_created_response`/`handle_tasks_result` bodies moved out, `ResponsePayload`/`JSONRPCError`/`TaskStatus`/`RELATED_TASK_META_KEY` became unused in non-wasm/non-test builds.
- **Fix:** cfg-gated `ResponsePayload` to `any(wasm32, test)`, `JSONRPCError` to `wasm32`-only, removed the now-unused `TaskStatus`/`RELATED_TASK_META_KEY` import.
- **Files modified:** src/server/core.rs
- **Commit:** 4a5baa26 / 51b799f5

## Known Stubs

None — `maybe_build_task_created` is intentionally not yet wired into a production dispatcher in Plan 01 (carries a `// Why:`-annotated `#[cfg_attr(not(test), allow(dead_code))]`); it is proven HERE by `gate_tests` and is wired into both `ServerCore` and `Server` create-paths in Plan 02 (by design, per the plan objective).

## Threat Flags

None — no new network endpoints, auth paths, file access, or schema surface introduced. The lift preserves the existing owner-scoping (`resolve_owner` derives owner from auth/router, never client params — T-102-01), store-mints-id invariant (T-102-02), and the frozen `-32002` pending shape (T-102-03); the self-enforcing gate forecloses the `Forbidden`/`None` info-leak (T-102-11), proven by `gate_tests`.

## Self-Check: PASSED

- FOUND: src/server/task_dispatch.rs
- FOUND commit 2abd03cf, 4a5baa26, 51b799f5
- task_dispatch.rs contains `pub(crate) fn apply_tasks_capability_rule`, `struct TaskDispatch`, `fn resolve_owner`, `fn build_task_created_response`, `fn handle_tasks_result`, `fn extract_terminal_result`, `fn maybe_build_task_created` (with explicit `task_requested: bool`), `fn route_tasks_endpoint`
- mod.rs contains `pub(crate) mod task_dispatch;` gated `#[cfg(not(target_arch = "wasm32"))]`
- builder.rs `apply_tasks_capability_rule` calls `crate::server::task_dispatch::apply_tasks_capability_rule`
- core.rs references `task_dispatch::` / `TaskDispatch`
