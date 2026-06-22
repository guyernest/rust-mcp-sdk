---
phase: 102
slug: http-task-dispatch
status: draft
nyquist_compliant: false
wave_0_complete: false
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
| HTASK-01 | store-backed `Server` advertises `tasks`; `Required`-no-backend → `build()` Err; explicit caps preserved | unit | `cargo test --features full server_builder_tasks_capability` | ❌ W0 | ⬜ pending |
| HTASK-02 | `Server` serves create-path + `tasks/get\|result\|list\|cancel` via the shared unit | integration | `cargo test --features full tasks_dispatch_shared` | ❌ W0 | ⬜ pending |
| HTASK-02 | shared unit produces identical output for `ServerCore` (no regression) | integration | `cargo test --features full tool_as_task_lifecycle` | ✅ `tests/tool_as_task_lifecycle.rs` | ⬜ pending |
| HTASK-03 | live HTTP round-trip: 3-way id match, non-empty content, advertised `tasks`, `-32002` pending | integration (HTTP loopback) | `cargo test --features full tool_as_task_lifecycle_http` | ❌ W0 | ⬜ pending |
| HTASK-04 | plain `tools/call` over HTTP unchanged (no task-envelope leakage) | integration | `cargo test --features full server_call_tool_non_task` | ❌ W0 | ⬜ pending |
| HTASK-04 | worked example compiles/runs | example | `cargo run --example s46_http_tool_as_task --features full` | ❌ W0 | ⬜ pending |
| HTASK-04 | property: any non-task tool `Value` never becomes a `CreateTaskResult` | property | `cargo test --features full proptest_task_branch_gate` | ❌ W0 | ⬜ pending |
| HTASK-04 | doctest on new `task_store()`/`with_task_store()` on `ServerBuilder` | doctest | `make doc-check` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/tool_as_task_lifecycle_http.rs` — HTASK-03 live HTTP round-trip (mirror `tool_as_task_lifecycle.rs` + the `workflow_prompt_e2e_test.rs:54-97` loopback harness)
- [ ] Unit tests for the shared capability rule against `ServerBuilder` (HTASK-01) — mirror the existing `ServerCoreBuilder` capability tests
- [ ] `examples/s46_http_tool_as_task.rs` + `Cargo.toml` `[[example]]` block (mirror `Cargo.toml:541-544`)
- [ ] Property test that the create-path gate (`req.task` + `taskId`+`status` + `TaskSupport`) never mis-fires
- [ ] No framework install needed (Rust built-in test harness + existing `proptest`)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| pmcp.run-shaped HTTP server serves tasks with NO `ServerCore::handle_request` shim | HTASK-04 / SC-6 | "shim absent" is a structural claim; the worked example demonstrates it but reviewer confirms no `ServerCore::handle_request` call exists | Read `examples/s46_http_tool_as_task.rs`: assert it uses only `Server::builder()...task_store()` + `StreamableHttpServer`, no `ServerCore::handle_request` |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
