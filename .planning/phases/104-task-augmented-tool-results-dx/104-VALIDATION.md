---
phase: 104
slug: task-augmented-tool-results-dx
status: approved
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-04
updated: 2026-07-04
update_reason: cross-AI review replan (D-04a bypass hardening + review feedback folded into all 5 plans)
---

# Phase 104 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Updated 2026-07-04 to stay in sync with the cross-AI-review replan (D-04a
> hardening tests, from_metadata compose path, helper-level tripwire test,
> set_result_meta collision/repeated tests, s47 suppressed-BEFORE/store-minted-AFTER).

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
| 01-T1 TaskMetadata type (unit-documented) | 104-01 | 1 | TOUT-03 | T-104-01-03 | fallible deserialize, no panic on bad shape; ms/secs units documented | unit | `cargo test --features full --lib types::tasks` | ✅ (extends src/types/tasks.rs tests) | ⬜ pending |
| 01-T2 with_related_task/related_task | 104-01 | 1 | TOUT-03 | T-104-01-03 | minimal-shape tolerant, None on malformed _meta | unit | `cargo test --features full --test task_augmented_result related_task` | ❌ W0 (tests/task_augmented_result.rs) | ⬜ pending |
| 01-T3 Client::wait_for_task (wasm-safe, composes w/ TaskMetadata, clamped) | 104-01 | 1 | TOUT-03 | T-104-01-01, T-104-01-02 | bounded poll (web_time::Instant timeout), owner-scoped via tasks_get, from_metadata compose (no hand-copy), interval clamp (no hot spin) | integration + wasm-check | `cargo test --features full --test task_augmented_result wait_for_task && cargo test --features full --test task_augmented_result from_metadata && cargo check --target wasm32-unknown-unknown --lib` | ❌ W0 | ⬜ pending |
| 02-T1 ToolOutput + handle_output default (loud bypass rustdoc) | 104-02 | 1 | TOUT-01 | T-104-02-01, T-104-02-03 | additive, non_exhaustive; existing handlers untouched; Result variant rustdoc states bypass (D-04a #1) | unit | `cargo test --features full --lib server::` | ✅ (src/server/mod.rs tests) | ⬜ pending |
| 02-T2 Shared pass-through branch (native) | 104-02 | 1 | TOUT-01 | T-104-02-01, T-104-02-04, T-104-02-05 | create-path precedence; single shared decision; request middleware still runs; workflow path untouched | integration (passthrough + parity) | `cargo test --features full --test tool_output_passthrough && cargo test --features full --test tool_as_task_lifecycle` | ❌ W0 (tests/tool_output_passthrough.rs) | ⬜ pending |
| 02-T3 D-04a middleware/error battery | 104-02 | 1 | TOUT-01 | T-104-02-03, T-104-02-02 | Result bypasses RESPONSE middleware (locked D-04a); REQUEST middleware still fires before Result tool; handler Err still routes through handle_tool_error; both dispatchers | integration | `cargo test --features full --test tool_output_passthrough middleware && cargo test --features full --test tool_output_passthrough error_path` | ❌ W0 | ⬜ pending |
| 03-T1 looks_like_call_tool_result marker | 104-03 | 2 | TOUT-02 | T-104-03-02 | high-precision markers, no full-deserialize; O(n) short-circuit | unit + property | `cargo test --features full --test double_wrap_tripwire looks_like` | ❌ W0 (tests/double_wrap_tripwire.rs) | ⬜ pending |
| 03-T2 Tripwire decision helper + wiring + suppress (threaded into ServerCore) | 104-03 | 2 | TOUT-02 | T-104-03-01, T-104-03-03 | debug_assert compiled out in release; helper-level #[should_panic] test seam; per-tool opt-out survives builder→ServerCore (no drift); rare/reviewed rustdoc | unit (debug + release) + helper-panic | `cargo test --features full --test double_wrap_tripwire && cargo test --release --features full --test double_wrap_tripwire` | ❌ W0 | ⬜ pending |
| 04-T1 ServerBuilder::tool_with_result (schema-gated, bypass rustdoc) | 104-04 | 3 | TOUT-01 | T-104-02-01, T-104-02-03 | verbatim _meta wire result via handle_output override; #[cfg(feature="schema-generation")] + JsonSchema explicit; loud bypass rustdoc | integration | `cargo test --features full --test tool_with_result tool_with_result` | ❌ W0 (tests/tool_with_result.rs) | ⬜ pending |
| 04-T2 RequestHandlerExtra::set_result_meta (encapsulated std::sync::Mutex) | 104-04 | 3 | TOUT-01 | T-104-04-01, T-104-04-03, T-104-04-04 | Arc<std::sync::Mutex> slot round-trips across by-value move; lock never held across await; merge precedence (handler-set overwrites same key, unrelated preserved); collision + repeated tested; ignored on Result path | integration + wasm-check | `cargo test --features full --test tool_with_result set_result_meta && cargo check --target wasm32-unknown-unknown --lib` | ❌ W0 | ⬜ pending |
| 05-T1 s47 BEFORE/AFTER example (suppressed BEFORE, store-minted AFTER) | 104-05 | 4 | TOUT-04 | — | runnable, no external services; BEFORE suppress_double_wrap_check (no debug-assert abort); AFTER real store-minted task id | example | `cargo run --example s47_task_augmented_result --features full` | ❌ W0 (examples/s47_task_augmented_result.rs) | ⬜ pending |
| 05-T2 D-14 live-HTTP _meta gate | 104-05 | 4 | TOUT-04 | T-104-05-01, T-104-05-03 | _meta at top level over real transport; ephemeral port + abort shutdown | integration (HTTP loopback) | `cargo test --features full --test tool_output_result_http` | ❌ W0 (tests/tool_output_result_http.rs) | ⬜ pending |
| 05-T3 Migration guide (book + design + README, incl. D-04a bypass callout) | 104-05 | 4 | TOUT-04 | T-104-05-02, T-104-05-04 | reframes incident at API level; wire-compat proof; response-middleware bypass callout | docs/doctest | `test -f docs/design/sep-1686-task-augmented-results.md && make doc-check` | ❌ W0 (docs/design + pmcp-book chapter) | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Every task carries an `<automated>` command. No 3 consecutive tasks lack automated verification. `File Exists` marks whether the target test/artifact already exists (✅) or is created within the phase as a Wave 0 (❌ W0) dependency of its own plan/task.

---

## Wave 0 Requirements

- [x] RED-first test: full `CallToolResult` with `_meta` returned by a tool handler survives `Server` dispatch un-re-wrapped (tests/tool_output_passthrough.rs, Plan 02 Task 2 — currently fails at `src/server/mod.rs:1493`)
- [x] D-04a battery: response middleware fires for `ToolOutput::Payload` and is bypassed for `ToolOutput::Result`; REQUEST middleware still fires before a Result tool; handler `Err(_)` still routes through `handle_tool_error` (tests/tool_output_passthrough.rs, Plan 02 Task 3)
- [x] Tripwire test stubs: WARN fires when dispatch is about to text-wrap a Value that looks like a built `CallToolResult`; helper-level #[should_panic] seam for the debug_assert (tests/double_wrap_tripwire.rs, Plan 03)
- [x] set_result_meta collision/repeated-call tests + tool_with_result verbatim wire test (tests/tool_with_result.rs, Plan 04)
- [x] Existing infrastructure (cargo test / proptest / cargo-fuzz targets) covers framework needs — no install required (tracing capture, if used, via in-tree `tracing::subscriber::with_default`, no new dep)

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
- [x] Verification map re-synced with the cross-AI-review replan (D-04a + review feedback)

**Approval:** approved
