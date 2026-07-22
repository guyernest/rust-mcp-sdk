---
phase: 112
slug: version-plumbing-spine
status: draft
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-22
---

# Phase 112 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]`; property tests via `proptest`/`quickcheck` (CLAUDE.md ALWAYS-requirements) |
| **Config file** | none — `cargo test`; CI runs `--test-threads=1` |
| **Quick run command** | `cargo test --lib <touched module>` (e.g. `cargo test --lib protocol::version`) |
| **Full suite command** | `make quality-gate` (fmt --all + clippy pedantic/nursery + build + test + audit) |
| **Additive gate** | `cargo semver-checks check-release` (must classify MINOR) |
| **Estimated runtime** | ~112 seconds |

---

## Sampling Rate

- **After every task commit:** Run the plan's quick `cargo test --lib <module>` command
- **After every plan wave:** Run `make quality-gate` + `cargo semver-checks check-release`
- **Before `/gsd:verify-work`:** Full suite green + semver-checks MINOR + all v1 fixtures green (dual-version regression)
- **Max feedback latency:** 112 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 112-01-01 | 01 | 1 | VERS-02 | T-112-01 / T-112-SC | LATEST stays pinned; no silent v2 upgrade | unit | `cargo test --lib protocol::version` | ❌ W0 (extend) | ⬜ pending |
| 112-01-02 | 01 | 1 | VERS-01, VERS-09 | — | ProtocolContext/TraceContext additive types; `from_meta` parses untrusted `_meta` (proptest) | unit + property | `cargo test --lib protocol::context` | ❌ W0 | ⬜ pending |
| 112-02-01 | 02 | 2 | VERS-01, VERS-03 | T-112-02 / T-112-08 | client_info() documented untrusted; additive-only | unit | `cargo test --lib cancellation::` | ❌ W0 | ⬜ pending |
| 112-02-02 | 02 | 2 | VERS-09 | — | trace_context over existing request_meta | unit | `cargo test --lib cancellation::` | ❌ W0 | ⬜ pending |
| 112-03-01 | 03 | 2 | VERS-06 | T-112-06 / T-112-06d | frozen -32002 verbatim + capability -32002 preserved by name; v2 values TODO; **error::ErrorCode's 11 consts delegate to error_codes:: (dominant 210-site surface sourced from the table); per-name consistency test** | unit | `cargo test --lib protocol::error_codes` | ❌ W0 (frozen test ✅) | ⬜ pending |
| 112-03-02 | 03 | 2 | VERS-04 | T-112-08 | ServerDiscover variant; delegation + variant semver minor | unit + tooling | `cargo test --lib protocol:: && cargo semver-checks check-release` | ❌ W0 | ⬜ pending |
| 112-04-01 | 04 | 3 | VERS-02, VERS-08 | T-112-01 | default v1-only; extensions populatable | unit | `cargo test --lib server::builder` | ❌ W0 | ⬜ pending |
| 112-04-02 | 04 | 3 | VERS-01, VERS-03 | T-112-05 / T-112-03b | per-request signal authoritative; both sites+wasm | unit + integration | `cargo test --lib protocol_context` | ❌ W0 | ⬜ pending |
| 112-05-01 | 05 | 4 | VERS-04, VERS-08 | T-112-10 / T-112-04b | read-only projection; v1→-32601 | integration | `cargo test --lib server_discover` | ❌ W0 | ⬜ pending |
| 112-05-02 | 05 | 4 | VERS-03, VERS-07 | T-112-07 | resultType v2-only; v1 byte-identical | unit + snapshot | `cargo test --lib result_type_envelope` | ❌ W0 | ⬜ pending |
| 112-06-01 | 06 | 4 | VERS-05 | — | header-name constants | unit | `cargo build --lib` | ❌ W0 | ⬜ pending |
| 112-06-02 | 06 | 4 | VERS-05, VERS-06 | T-112-03 / T-112-04 / T-112-04c | v2-signal reconciliation (header vs _meta, fail closed) + strict reject + body cross-check; **new header-violation errors use error_codes::**; untrusted gate proptest | integration (HTTP) + property | `cargo test --test '*' v2_required_headers && cargo test --lib v2_header_gate_proptest` | ❌ W0 (HTTP target) | ⬜ pending |
| 112-07-01 | 07 | 5 | VERS-06 | T-112-06b / T-112-06c | dispatch literals → error_codes:: (core/mod/task_dispatch); frozen -32002/-32601 byte-identical | unit + regression | `cargo test --lib server::core && cargo test --lib server::task_dispatch && cargo test --lib pending_tasks_result_preserves_minus_32002` | ❌ W5 (frozen test ✅) | ⬜ pending |
| 112-07-02 | 07 | 5 | VERS-06 | T-112-06c | jsonrpc.rs production error construction → error_codes:: | unit | `cargo test --lib jsonrpc` | ❌ W5 | ⬜ pending |
| 112-08-01 | 08 | 5 | VERS-06 | T-112-06e / T-112-06f | streamable-HTTP transport's 25 production literals → error_codes:: (parse/invalid-request/internal/method-not-found/auth-required); wire byte-identical; #[cfg(test)] oracle preserved | unit + regression | `cargo test --lib streamable_http && cargo test --test '*' v2_required_headers` | ❌ W5 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Install `cargo-semver-checks` + `cargo-public-api` (Plan 01 Task 1) — the additive-guarantee gate
- [ ] Extend `version.rs` tests: `protocol_era` unit tests; keep `latest_version_is_2025_11_25` green — VERS-02
- [ ] `protocol::context` unit tests (ProtocolContext constructor + TraceContext::from_meta) — VERS-01/09
- [ ] `cancellation::` parity + accessor + trace_context tests (native + wasm) — VERS-01/03/09
- [ ] `protocol::error_codes` table compile/consistency test: standard consts == ProtocolErrorCode enum, both -32002 names asserted, AND per-name error::ErrorCode::FOO.as_i32() == error_codes::FOO (delegation guard for the 210-site surface); DO NOT edit `pending_tasks_result_preserves_minus_32002` — VERS-06
- [ ] Plan 03: `error::ErrorCode`'s 11 consts delegate to `error_codes::` (Self(error_codes::NAME)); `cargo build --lib` proves all 210 downstream `ErrorCode::` sites still compile; public surface unchanged (semver minor) — VERS-06
- [ ] `protocol_context` cross-dispatch-site + wasm parity test — VERS-01
- [ ] `server_discover` projection + v1 `-32601` era-gate test — VERS-04
- [ ] `result_type_envelope` v2-only injection + v1 byte-identity test — VERS-07
- [ ] `v2_required_headers` tests routed through the HTTP `ConformanceTarget` (NOT in-memory transport, Pitfall 11) — VERS-05
- [ ] `v2_header_gate_proptest`: proptest over arbitrary (header-version, _meta-version, Mcp-Method, Mcp-Name, body-method) tuples; reconciliation fail-closed invariants; never panics — VERS-05
- [ ] `from_meta` proptest over arbitrary `_meta` JSON (no panic; absent traceparent⇒None; present⇒Some exact) — VERS-09
- [ ] Plan 06: new header-violation JSON-RPC errors use `error_codes::` constants (no new bare literal) — VERS-06
- [ ] Plan 07 (wave 5) call-site migration: `error_codes::` adopted at all emitting sites (core/mod/task_dispatch/jsonrpc); frozen `pending_tasks_result_preserves_minus_32002` untouched + green — VERS-06
- [ ] Plan 08 (wave 5) streamable-HTTP migration: all 25 production transport literals → `error_codes::`; #[cfg(test)] oracle literals preserved; transport + Plan 06 header tests green — VERS-06

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 112s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-22 (revised iteration 2: VERS-06 centralization extended to the dominant error::ErrorCode surface via delegation + streamable-HTTP Plan 08)
</content>
</invoke>
