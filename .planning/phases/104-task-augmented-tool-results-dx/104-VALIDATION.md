---
phase: 104
slug: task-augmented-tool-results-dx
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-04
---

# Phase 104 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest + cargo-fuzz |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --lib server:: tasks::` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~quick: 60s / full: 600s |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib server:: tasks::`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-T1 TaskMetadata type | 104-01 | 1 | TOUT-03 | T-104-01-03 | fallible deserialize, no panic on bad shape | unit | `cargo test --features full --lib types::tasks` | ✅ (extends src/types/tasks.rs tests) | ⬜ pending |
| 01-T2 with_related_task/related_task | 104-01 | 1 | TOUT-03 | T-104-01-03 | minimal-shape tolerant, None on malformed _meta | unit | `cargo test --features full --test task_augmented_result related_task` | ❌ W0 (tests/task_augmented_result.rs) | ⬜ pending |
| 01-T3 Client::wait_for_task (wasm-safe) | 104-01 | 1 | TOUT-03 | T-104-01-01, T-104-01-02 | bounded poll (timeout), owner-scoped via tasks_get | integration + wasm-check | `cargo test --features full --test task_augmented_result wait_for_task && cargo check --target wasm32-unknown-unknown --lib` | ❌ W0 | ⬜ pending |
| 02-T1 ToolOutput + handle_output default | 104-02 | 1 | TOUT-01 | T-104-02-01 | additive, non_exhaustive; existing handlers untouched | unit | `cargo test --features full --lib server::` | ✅ (src/server/mod.rs tests) | ⬜ pending |
| 02-T2 Shared pass-through branch (native) | 104-02 | 1 | TOUT-01 | T-104-02-01, T-104-02-04, T-104-02-05 | create-path precedence; single shared decision; workflow path untouched | integration (passthrough + parity) | `cargo test --features full --test tool_output_passthrough && cargo test --features full --test tool_as_task_lifecycle` | ❌ W0 (tests/tool_output_passthrough.rs) | ⬜ pending |
| 02-T3 Response-middleware regression | 104-02 | 1 | TOUT-01 | T-104-02-03 | Result bypasses response middleware (locked D-04); Payload retains it | integration | `cargo test --features full --test tool_output_passthrough response_middleware` | ❌ W0 | ⬜ pending |
| 03-T1 looks_like_call_tool_result marker | 104-03 | 2 | TOUT-02 | T-104-03-02 | high-precision markers, no full-deserialize; O(n) short-circuit | unit + property | `cargo test --features full --test double_wrap_tripwire looks_like` | ❌ W0 (tests/double_wrap_tripwire.rs) | ⬜ pending |
| 03-T2 Tripwire wiring + suppress opt-out | 104-03 | 2 | TOUT-02 | T-104-03-01, T-104-03-03 | debug_assert compiled out in release; per-tool opt-out; no dispatcher drift | unit (debug + release) | `cargo test --features full --test double_wrap_tripwire && cargo test --release --features full --test double_wrap_tripwire` | ❌ W0 | ⬜ pending |
| 04-T1 ServerBuilder::tool_with_result | 104-04 | 3 | TOUT-01 | T-104-02-01 | verbatim _meta wire result via handle_output override | integration | `cargo test --features full --test tool_with_result tool_with_result` | ❌ W0 (tests/tool_with_result.rs) | ⬜ pending |
| 04-T2 RequestHandlerExtra::set_result_meta | 104-04 | 3 | TOUT-01 | T-104-04-01, T-104-04-03 | Arc slot round-trips across by-value move; no lock across await | integration + wasm-check | `cargo test --features full --test tool_with_result set_result_meta && cargo check --target wasm32-unknown-unknown --lib` | ❌ W0 | ⬜ pending |
| 05-T1 s47 BEFORE/AFTER example | 104-05 | 4 | TOUT-04 | — | runnable, no external services | example | `cargo run --example s47_task_augmented_result --features full` | ❌ W0 (examples/s47_task_augmented_result.rs) | ⬜ pending |
| 05-T2 D-14 live-HTTP _meta gate | 104-05 | 4 | TOUT-04 | T-104-05-01, T-104-05-03 | _meta at top level over real transport; ephemeral port + abort shutdown | integration (HTTP loopback) | `cargo test --features full --test tool_output_result_http` | ❌ W0 (tests/tool_output_result_http.rs) | ⬜ pending |
| 05-T3 Migration guide (book + design + README) | 104-05 | 4 | TOUT-04 | T-104-05-02 | reframes incident at API level; wire-compat proof | docs/doctest | `test -f docs/design/sep-1686-task-augmented-results.md && make doc-check` | ❌ W0 (docs/design + pmcp-book chapter) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Every task carries an `<automated>` command. No 3 consecutive tasks lack automated verification. `File Exists` marks whether the target test/artifact already exists (✅) or is created within the phase as a Wave 0 (❌ W0) dependency of its own plan/task.

---

## Wave 0 Requirements

- [x] RED-first test: full `CallToolResult` with `_meta` returned by a tool handler survives `Server` dispatch un-re-wrapped (tests/tool_output_passthrough.rs, Plan 02 Task 2 — currently fails at `src/server/mod.rs:1493`)
- [x] Response-middleware behavior test: a response middleware fires for `ToolOutput::Payload` and is bypassed for `ToolOutput::Result` (tests/tool_output_passthrough.rs, Plan 02 Task 3)
- [x] Tripwire test stubs: WARN fires when dispatch is about to text-wrap a Value that looks like a built `CallToolResult` (tests/double_wrap_tripwire.rs, Plan 03)
- [x] Existing infrastructure (cargo test / proptest / cargo-fuzz targets) covers framework needs — no install required

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 120s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved
