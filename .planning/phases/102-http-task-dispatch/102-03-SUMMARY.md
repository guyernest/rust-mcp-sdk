---
phase: 102-http-task-dispatch
plan: 03
subsystem: server
tags: [tasks, http, live-round-trip, example, acceptance-gate, wasm-gated]
requires:
  - "Plan 02 high-level Server + ServerBuilder task_store/handle_request tasks/* dispatch"
  - "Plan 01 shared src/server/task_dispatch.rs unit"
  - "Phase 101 in-process tool_as_task_lifecycle test (no-regression gate)"
provides:
  - "tests/tool_as_task_lifecycle_http.rs — LIVE HTTP loopback round-trip (HTASK-03): ephemeral-port readback + JoinHandle::abort() shutdown + HTTP-level cross-owner isolation"
  - "examples/s46_http_tool_as_task.rs — worked pmcp.run-shaped HTTP task server (Server::builder only, NO ServerCore shim)"
  - "Cargo.toml [[example]] s46_http_tool_as_task registration"
  - "102-VALIDATION.md sign-off (nyquist_compliant: true)"
affects:
  - "tests/tool_as_task_lifecycle_http.rs"
  - "examples/s46_http_tool_as_task.rs"
  - "Cargo.toml"
tech-stack:
  added: []
  patterns:
    - "Deterministic HTTP readiness via StreamableHttpServer::start() returning (local_addr, JoinHandle) — listener is bound BEFORE start() returns, so the read-back addr is already accepting (no fixed sleep)"
    - "Ephemeral-port readback: bind 127.0.0.1:0, use the OS-assigned addr from start() (no hardcoded port)"
    - "Server-task shutdown guard: run the round-trip in a factored-out fn, then ALWAYS abort() the JoinHandle and assert the join resolves Cancelled (surface any early non-cancel error)"
    - "Cross-owner injection over HTTP without an auth provider: distinct owners via the x-pmcp-user-id proxy header (Server derives owner from AuthContext.subject)"
key-files:
  created:
    - "tests/tool_as_task_lifecycle_http.rs"
    - "examples/s46_http_tool_as_task.rs"
  modified:
    - "Cargo.toml"
    - ".planning/phases/102-http-task-dispatch/102-VALIDATION.md"
decisions:
  - "Used StreamableHttpServer::start()'s existing (SocketAddr, JoinHandle) return for BOTH ephemeral-port readback AND deterministic readiness — start() binds the TcpListener before returning, so no readiness poll/sleep is needed and no hardcoded port is introduced"
  - "HTTP-level cross-owner isolation ADDED (not deferred): the no-auth-provider path extracts an AuthContext from x-pmcp-user-id proxy headers, so two clients with distinct header values map to distinct task owners over the real HTTP boundary — proving IDOR protection at the live boundary, complementing Plan 02's Server-layer tasks_cross_owner_isolation"
  - "New NEW test file (tests/tool_as_task_lifecycle_http.rs) keeping the Phase 101 in-process tests/tool_as_task_lifecycle.rs untouched (research Open Question 3 / HTASK-04 no-regression)"
  - "ServerBuilder::task_store doctest already landed in Plan 02 with Server::builder() (no_run, compile-checked) — confirmed present and exercised by make doc-check; no change needed in Plan 03"
metrics:
  duration: "~40m"
  completed: "2026-06-22"
  tasks: 3
  files_created: 2
  files_modified: 2
---

# Phase 102 Plan 03: Live HTTP Tool-as-Task Round-Trip + Worked Example Summary

Proved the phase end-to-end with the acceptance gate: a REAL HTTP loopback
round-trip (no in-process duplex shim) that drives `initialize → call(task) →
tasks/get → tasks/result` through a high-level, store-backed `Server` over
`StreamableHttpServer` + `StreamableHttpTransport`, plus the worked
`s46_http_tool_as_task` example and the HTASK-04 doctest/coverage. This replaces
the carried-from-Phase-101 in-process shim with a live round-trip and demonstrates
that pmcp.run can drop its `ServerCore::handle_request` shim.

## What was built

- **`tests/tool_as_task_lifecycle_http.rs`** (new, `#![cfg(all(feature =
  "streamable-http", not(target_arch = "wasm32")))]`):
  - `live_http_round_trip_typed_lifecycle_id_consistency_and_capability` — over
    REAL HTTP, asserts the auto-advertised `tasks` capability, the store-minted
    wire id (`!= "tool-fabricated"`), a pollable `tasks/get` whose id equals the
    client id, a non-empty `tasks/result`, and the frozen `-32002` pending error.
  - `live_http_cross_owner_isolation` — owner A (`x-pmcp-user-id: alice`) creates a
    task; owner B (`x-pmcp-user-id: bob`) cannot `tasks/get` / `tasks/result` /
    `tasks/cancel` it, while owner A retains access (IDOR over the live boundary).
  - Server built ONLY via `Server::builder().tool(..).task_store(InMemoryTaskStore)`
    — grep-clean of `ServerCore`. Ephemeral port via `127.0.0.1:0` + the read-back
    addr from `StreamableHttpServer::start()`; spawned server `JoinHandle` is
    `abort()`ed (and the join asserted `Cancelled`) after the client completes.
- **`examples/s46_http_tool_as_task.rs`** (new): a pmcp.run-shaped HTTP task server
  built only via `Server::builder()...task_store()`, served over
  `StreamableHttpServer`, driving the 4-step lifecycle from a live
  `StreamableHttpTransport` client with HARD assertions (returns `Err` on failure).
  Same ephemeral-port readback + `abort()` shutdown; doc comment emphasizes the
  NO-`ServerCore::handle_request`-shim claim.
- **`Cargo.toml`**: `[[example]] name = "s46_http_tool_as_task"` registered
  immediately after the `s45` block (`required-features = ["full"]`).
- **`102-VALIDATION.md`**: all Per-Task Verification Map rows marked green,
  `nyquist_compliant: true`, sign-off approved.

## Ephemeral-port + shutdown mechanism (Codex HIGH concerns)

`StreamableHttpServer::start()` already binds the `TcpListener` and returns
`(local_addr, JoinHandle)` BEFORE spawning the serve loop
(`src/server/streamable_http_server.rs:370-387`). Both the test and the example:
- bind `127.0.0.1:0` and use the OS-assigned `local_addr` read back from `start()`
  — **no hardcoded port** (no `18765`);
- treat the returned (already-bound, already-listening) address as the **readiness
  signal** — **no fixed `sleep`**;
- keep the returned `JoinHandle` and `abort()` it after the round-trip — the test
  additionally awaits the handle and asserts it resolves `Cancelled` (any other
  outcome panics, surfacing an early server error) so the **process cannot hang**.

## Cross-owner isolation over HTTP: ADDED (not deferred)

Auth context IS injectable through `StreamableHttpTransport` in this harness: when
no auth provider is configured, `extract_and_validate_auth` falls through to
`extract_auth_from_proxy_headers`, which builds an `AuthContext` from `x-pmcp-*`
headers (`src/server/streamable_http_server.rs:776-888`). The `Server` derives the
task owner from `AuthContext.subject` (`task_dispatch.rs::resolve_owner`, with
`client_id == None`). So two clients with distinct `x-pmcp-user-id` headers map to
distinct owners over the real boundary — the HTTP-level IDOR test was added rather
than deferred, complementing Plan 02's Server-layer `tasks_cross_owner_isolation`.

## pmcp.run shim removal: CONFIRMED

The example builds the server ONLY via `Server::builder()` (no `ServerCore` symbol)
and serves `tasks/*` over `StreamableHttpServer`, and the live test exercises the
same path end-to-end. pmcp.run can therefore drop its `ServerCore::handle_request`
shim and serve tasks through the high-level `Server`/HTTP path.

## Tasks

| Task | Name | Commit | Key files |
|------|------|--------|-----------|
| 1 | Live HTTP loopback round-trip (HTASK-03) + HTTP-level cross-owner isolation | 270fafe8 | tests/tool_as_task_lifecycle_http.rs |
| 2 | Worked example s46_http_tool_as_task (HTASK-04) + Cargo.toml registration | c950083b | examples/s46_http_tool_as_task.rs, Cargo.toml |
| 3 | Doctest confirm + frozen-surface verify + phase gate + VALIDATION sign-off | bb6f0326 | 102-VALIDATION.md |

## Verification

- `cargo test --features full --test tool_as_task_lifecycle_http -- --test-threads=1`
  — 2/2 pass (live round-trip + HTTP cross-owner isolation), process does not hang.
- `cargo test --features full --test tool_as_task_lifecycle -- --test-threads=1` —
  7/7 pass (Phase 101 in-process test untouched and green).
- `cargo test --features full --lib task_dispatch -- --test-threads=1` — 16/16 pass
  (Plan 01 gate truth-table + Plan 02 capability/dispatch/matrix/proptest/
  `tasks_cross_owner_isolation`).
- `cargo run --example s46_http_tool_as_task --features full` — exit 0: advertised
  `tasks`, store-minted id, terminal poll, non-empty `tasks/result`, clean shutdown.
- `cargo test --features full --doc "server::ServerBuilder::task_store"` — 1/1
  (the `Server::builder()` doctest compiles; `no_run`, exercised by `make doc-check`).
- `make doc-check` — exit 0 (zero rustdoc warnings).
- `make quality-gate` — exit 0 (fmt --all, clippy pedantic+nursery `-D warnings`,
  build, full test, audit, ALL examples incl. `s46`, purity-checks).
- PMAT `analyze complexity --max-cognitive 25`: ZERO cognitive violations in the
  touched `src/server/{task_dispatch,mod,core,builder}` files (the only two repo-wide
  cog>25 hits are pre-existing test files in `mcp-tester` / `pmcp-server-toolkit`,
  out of scope and unchanged).
- `git diff dd18e7e4 -- src/types/tasks.rs` — empty (wire shapes frozen).
- Public API: Plan 03 is additive-only (two new test/example files + a Cargo
  `[[example]]` block); `src/` is unchanged from Plan 02, so no public item was
  removed or renamed; the wasm boundary is intact (the test/example are
  `streamable-http` / non-wasm gated).

## Deviations from Plan

None — plan executed as written. Two planned conditionals resolved in the
permissive direction:
- **HTTP-level cross-owner isolation** (Task 3, Concern #7) was ADDED rather than
  deferred, because auth context proved injectable via the `x-pmcp-user-id` proxy
  header (documented above).
- **ServerBuilder::task_store doctest** (Task 3, HTASK-04) was already present from
  Plan 02 with `Server::builder()` and is exercised by `make doc-check`; confirmed,
  no edit needed.

## Known Stubs

None — both new files are fully wired live round-trips with hard assertions; the
example is a runnable end-to-end demonstration.

## Threat Flags

None — no new security surface beyond the plan's `<threat_model>`. The live HTTP
round-trip is the security assertion itself: the three-way store-minted id holds
over HTTP framing (T-102-08), the frozen `-32002` reaches the client unchanged
(T-102-09), the bind is loopback-only `127.0.0.1:0` with read-back and an aborted
listener (T-102-10), and HTTP-level owner-scoped IDOR is proven (T-102-12). No
`cargo add` / package installs (T-102-SC).

## Self-Check: PASSED

- FOUND: tests/tool_as_task_lifecycle_http.rs
- FOUND: examples/s46_http_tool_as_task.rs
- FOUND: Cargo.toml `[[example]] s46_http_tool_as_task`
- FOUND commit 270fafe8 (test), c950083b (example), bb6f0326 (validation sign-off)
- `make quality-gate` exit 0; `make doc-check` exit 0; PMAT cog-25 clean on
  touched `src/server/*`; `git diff src/types/tasks.rs` empty
