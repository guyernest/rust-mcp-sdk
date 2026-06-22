# Phase 102: Lift the task lifecycle onto the high-level `Server` / HTTP path - Context

**Gathered:** 2026-06-21
**Status:** Ready for planning
**Source:** PRD Express Path (.planning/phases/102-http-task-dispatch/102-PRD.md)

<domain>
## Phase Boundary

Make the SDK's `tasks/*` lifecycle available over the **high-level `Server` and
`StreamableHttpServer`**, so an HTTP-hosted server (e.g. pmcp.run's Lambdas) can serve
task-based tools with **NO `ServerCore::handle_request` shim**.

Phase 101 put the entire task lifecycle (`task_store` plumbing, endpoint-backed
capability rule, task-augmented create-path, `tasks/get | result | list | cancel`
dispatch) on `ServerCore` / `ServerCoreBuilder` ONLY. The SDK has two parallel
request dispatchers:
- `ServerCore::handle_request` (`src/server/core.rs`, `impl ProtocolHandler` ~`:884`) — HAS task dispatch.
- `Server::handle_request` (`src/server/mod.rs`) — the HTTP-facing dispatcher used by
  `StreamableHttpServer` (`Arc<Mutex<Server>>` → `server.handle_request`,
  `src/server/streamable_http_server.rs:1172`/`:1391`). It HARD-REJECTS `tasks/*` at
  `src/server/mod.rs:1166-1169`.

The deliverable is ONE shared task-lifecycle implementation called by BOTH dispatchers
— retiring the two-dispatcher drift, not adding a third copy.

**End state for consumers:** `Server::builder().tool(t).task_store(store).build()` →
serve over `StreamableHttpServer` → tasks just work.
</domain>

<decisions>
## Implementation Decisions

### Sharing strategy (recommended, confirm against code first)
- **(A, recommended)** Extract the task create-path + `tasks/*` endpoint dispatch into a
  free-standing `server::task_dispatch` unit parameterized by `(task_store, task_router,
  tool_infos, resolve_owner, …)`, called by both `handle_request` bodies. Cleanest;
  directly retires the drift.
- **(B)** Have `Server` own/delegate task methods to an internal `ServerCore`. Lower code
  movement but couples the two types and may double-handle `tools/call`.
- **(C)** Duplicate into `Server` — **REJECTED** (reintroduces the exact drift this phase exists to retire).
- The hard part is the create-path: entangled with `tools/call` result handling and the
  `tool_infos`/`task_support` lookup. The planner MUST map the shared seam against
  `core.rs` (the `TaskCreated` branch + tasks routing) and `mod.rs` (the `Server`
  `tools/call` + dispatch) BEFORE committing to A vs B.

### HTASK-01 — high-level builder gains the task backend + capability rule
- High-level `ServerBuilder`/`Server` accepts a `TaskStore` and honors `with_task_support` tools.
- SAME endpoint-backed capability behavior Phase 101 added to `ServerCoreBuilder`:
  registering a store auto-advertises the top-level `tasks` capability; a
  `TaskSupport::Required` tool with no backing store/router makes `build()` return `Err`;
  an explicitly-configured capability set is never clobbered.
- The capability rule is the SAME implementation Phase 101 wrote (`apply_tasks_capability_rule`
  in `src/server/builder.rs`) — SHARED, not re-derived.

### HTASK-02 — `Server::handle_request` serves the full lifecycle, sharing logic with `ServerCore`
- Replace the `tasks/*` hard-reject at `mod.rs:1166-1169` with the real lifecycle:
  - task-augmented `tools/call` create-path (store-minted id written to wire `task.taskId`
    + `_meta` related-task, terminal-result persistence)
  - `tasks/get | result | list | cancel` (store-first typed serialization, `TaskRouter`
    fall-through, `-32002` specified pending error, owner-scoping via `resolve_task_owner`).
- The task-lifecycle logic MUST be SHARED with `ServerCore` (factored into one reusable
  unit both dispatchers call), NOT a divergent second copy.

### HTASK-03 — end-to-end over `StreamableHttpServer`, proven by a real HTTP round-trip
- Acceptance = a **live HTTP loopback round-trip**: real `StreamableHttpServer` +
  `StreamableHttpTransport` on `127.0.0.1:0` (the `tests/workflow_prompt_e2e_test.rs:54-97`
  harness), driving `initialize → call(task) → tasks/get (poll) → tasks/result`.
- Assert the same invariants as Phase 101: non-empty typed `CallToolResult.content`;
  `CreateTaskResult.task.taskId == tasks/get id == _meta.relatedTask.taskId`; `initialize`
  advertises `tasks`; pending `tasks/result` returns the specified `-32002` error.
- This **replaces** the in-process duplex harness Phase 101 was forced to use.

### HTASK-04 — no regressions, additive API, consumer shim removed
- No regression to the `ServerCore` task path (all Phase 101 tests green) or existing
  `Server`/HTTP behavior.
- No breaking change to public wire shapes (`Task`/`GetTaskResult`/`CallToolResult`/
  `CreateTaskResult`/`ServerCapabilities`) or to the `Server`/`ServerBuilder` public API
  (additive only).
- `#[cfg(not(target_arch="wasm32"))]` boundary preserved.
- ALWAYS coverage (unit/property/integration/example/doctest) + a worked HTTP example
  (`s46_http_tool_as_task`). `make quality-gate` AND `make doc-check` green. New minor
  `pmcp` version publishable.

### Claude's Discretion
- Exact module name/location of the shared task-dispatch unit (within `src/server/`).
- Internal signature/parameterization of the shared seam, provided both dispatchers call it.
- Test file organization for the new HTTP round-trip (mirror `tool_as_task_lifecycle.rs`).
- Example file structure for `s46_http_tool_as_task` (follow numbered-example convention).
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Task lifecycle logic to SHARE (the core of this phase)
- `src/server/core.rs` — `ServerCore` task create-path (`ToolCallOutcome::TaskCreated { task_id, task_value, result }`) + the `tasks/*` routing block (store-first, `TaskRouter` fall-through, `-32002` pending error); `impl ProtocolHandler for ServerCore` ~`:884`.
- `src/server/mod.rs` — `Server` `:315`, `ServerBuilder` `:1878`, the `tasks/*` hard-reject `:1166-1169` (to replace), `tool_infos` cache `:3568`.
- `src/server/builder.rs` — `task_store(Arc<dyn TaskStore>)`, `with_task_store` (legacy `TaskRouter`), `apply_tasks_capability_rule()` (the capability rule to reuse).
- `src/server/task_store.rs` — `TaskStore` trait (`create`/`get`/`update_status`/`list`/`cancel` + `set_result`/`get_result`/`supports_results`), `InMemoryTaskStore`. `resolve_task_owner(auth_context)` owner-scoping consumed by `tasks/*` routing.

### HTTP transport / serving path
- `src/server/streamable_http_server.rs` — `Arc<Mutex<Server>>`, `server.handle_request` `:1172`/`:1391`.

### Wire-shape contract (FROZEN — diff must be none)
- `src/types/tasks.rs` — `Task` / `GetTaskResult` / `CreateTaskResult`.

### Test harnesses
- `tests/workflow_prompt_e2e_test.rs:54-97` — HTTP loopback harness (`StreamableHttpServer` + `StreamableHttpTransport` on `127.0.0.1:0`).
- `tests/tool_as_task_lifecycle.rs` — Phase 101 in-process acceptance test to mirror over HTTP.

### Phase 101 prior art
- `.planning/phases/101-*/` — Phase 101 plan/summary (the lifted/shared machinery's origin).
</canonical_refs>

<specifics>
## Specific Ideas

- Worked HTTP example named `s46_http_tool_as_task` demonstrating a pmcp.run-shaped server
  serving `tasks/*` through the high-level `Server` + `StreamableHttpServer` with NO
  `ServerCore::handle_request` shim.
- The HTTP round-trip test mirrors Phase 101's `tool_as_task_lifecycle.rs` invariants but
  over the real HTTP transport via the `workflow_prompt_e2e_test` harness.
- Net downstream effect: the pmcp.run migration note's "drive `ServerCore` directly" shim
  section is removed.
</specifics>

<deferred>
## Deferred Ideas (out of scope — LOCKED scope fences)

- Do NOT change the `tasks/*` wire contract — Phase 101 froze it; this phase only changes
  WHICH dispatcher serves it.
- Do NOT duplicate the task logic into a divergent third copy (a planner proposing
  copy-paste fails the goal).
- Do NOT break the existing `ServerCore` path or the `TaskRouter` fallback.
- Durable/persistent `TaskStore` backends (DynamoDB etc.) — consumer's `TaskStore` impl, not the SDK's.
- Task streaming over SSE/WebSocket.
- The legacy `pmcp-tasks` crate.
- Any change to the high-level vs low-level builder split beyond adding the task backend.
- Keep WASM out (task path is non-wasm, as in Phase 101).
</deferred>

---

*Phase: 102-http-task-dispatch*
*Context gathered: 2026-06-21 via PRD Express Path*
