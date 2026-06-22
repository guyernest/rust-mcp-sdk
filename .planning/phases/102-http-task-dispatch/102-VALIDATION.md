---
phase: 102
slug: http-task-dispatch
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-06-21
---

# Phase 102 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> This is protocol-shape work — the acceptance gate is a **live HTTP round-trip** (HTASK-03),
> the "resolved only via a live round-trip" rule carried from Phase 101.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[tokio::test]` (integration tests in `tests/`) + `proptest` + `make` targets |
| **Config file** | none — `cargo test` + `Makefile` (`test-unit`/`test-integration`/`test-examples`/`test-property`/`test-fuzz`) |
| **Quick run command** | `cargo test --features full tool_as_task_lifecycle` |
| **Full suite command** | `make quality-gate` (fmt + clippy pedantic/nursery + build + test + audit) AND `make doc-check` |
| **Estimated runtime** | ~90–180 seconds (full `make quality-gate`); ~5s for the quick task arm |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features full tool_as_task_lifecycle` (Phase 101 no-regression guard) + the new test arm under that task.
- **After every plan wave:** Run `cargo test --features full` (full integration set incl. the HTTP round-trip).
- **Before `/gsd:verify-work`:** `make quality-gate` AND `make doc-check` green; PMAT complexity gate (CI) green (≤ cog 25).
- **Max feedback latency:** ~180 seconds (full suite).

---

## Per-Task Verification Map

> Task IDs are assigned by the planner; rows below are requirement-anchored and map onto tasks at plan time.

| Req | Behavior | Test Type | Automated Command | File Exists | Status |
|-----|----------|-----------|-------------------|-------------|--------|
| HTASK-01 | store-backed `Server` advertises `tasks`; `Required`-no-backend → `build()` Err; explicit caps preserved | unit | `cargo test --features full server_builder_tasks_capability` | ✅ `src/server/task_dispatch_tests.rs` | ✅ green |
| HTASK-02 | `Server` serves create-path + `tasks/get\|result\|list\|cancel` via the shared unit | integration | `cargo test --features full tasks_dispatch_shared` | ✅ `src/server/task_dispatch_tests.rs` | ✅ green |
| HTASK-02 | shared unit produces identical output for `ServerCore` (no regression) | integration | `cargo test --features full tool_as_task_lifecycle` | ✅ `tests/tool_as_task_lifecycle.rs` | ✅ green |
| HTASK-03 | live HTTP round-trip: 3-way id match, non-empty content, advertised `tasks`, `-32002` pending | integration (HTTP loopback) | `cargo test --features full tool_as_task_lifecycle_http` | ✅ `tests/tool_as_task_lifecycle_http.rs` | ✅ green |
| HTASK-04 | plain `tools/call` over HTTP unchanged (no task-envelope leakage) | integration | `cargo test --features full server_call_tool_non_task` | ✅ `src/server/task_dispatch_tests.rs` | ✅ green |
| HTASK-04 | worked example compiles/runs | example | `cargo run --example s46_http_tool_as_task --features full` | ✅ `examples/s46_http_tool_as_task.rs` | ✅ green |
| HTASK-04 | property: any non-task tool `Value` never becomes a `CreateTaskResult` | property | `cargo test --features full proptest_task_branch_gate` | ✅ `src/server/task_dispatch_tests.rs` | ✅ green |
| HTASK-04 | doctest on new `task_store()`/`with_task_store()` on `ServerBuilder` | doctest | `make doc-check` | ✅ `src/server/mod.rs` | ✅ green |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [x] `tests/tool_as_task_lifecycle_http.rs` — HTASK-03 live HTTP round-trip (`StreamableHttpServer::start()` ephemeral-port readback + `JoinHandle::abort()` shutdown; also HTTP-level cross-owner isolation via the `x-pmcp-user-id` proxy header)
- [x] Unit tests for the shared capability rule against `ServerBuilder` (HTASK-01) — `src/server/task_dispatch_tests.rs::server_builder_tasks_capability` (Plan 02)
- [x] `examples/s46_http_tool_as_task.rs` + `Cargo.toml` `[[example]]` block (registered after the `s45` block)
- [x] Property test that the create-path gate (`req.task` + `taskId`+`status` + `TaskSupport`) never mis-fires — `proptest_task_branch_gate` (Plan 02)
- [x] No framework install needed (Rust built-in test harness + existing `proptest`)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| pmcp.run-shaped HTTP server serves tasks with NO `ServerCore::handle_request` shim | HTASK-04 / SC-6 | "shim absent" is a structural claim; the worked example demonstrates it but reviewer confirms no `ServerCore::handle_request` call exists | Read `examples/s46_http_tool_as_task.rs`: assert it uses only `Server::builder()...task_store()` + `StreamableHttpServer`, no `ServerCore::handle_request` |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 180s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved — `make quality-gate` + `make doc-check` green; HTTP round-trip + HTTP-level cross-owner isolation + worked example all green; PMAT cog-25 clean on touched `src/server/*`; `git diff src/types/tasks.rs` empty (wire frozen); public API additive-only; wasm boundary intact.
