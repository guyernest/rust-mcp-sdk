---
phase: 103
slug: web-channel-wasm-client-reference-oauth-browser-pkce-mcp-tas
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-30
---

# Phase 103 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `cargo test` + `proptest` (property) + `cargo fuzz`/`libfuzzer` (fuzz) |
| **Config file** | `Cargo.toml` (workspace); wasm via `make wasm-build` (separate target — `make quality-gate` does NOT build wasm, see RESEARCH SC-4 gap) |
| **Quick run command** | `cargo test -p pmcp --lib <module>` (per-task, native target) |
| **Full suite command** | `make quality-gate && make wasm-build` (native gate + wasm build — BOTH required for SC-4) |
| **Estimated runtime** | ~60–180 seconds (native gate); wasm build adds ~30–60s |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp --lib <touched module>` (PKCE helper / transport)
- **After every plan wave:** Run `make quality-gate` (and `make wasm-build` once example/transport land)
- **Before `/gsd:verify-work`:** `make quality-gate` green AND `make wasm-build` green (SC-4)
- **Max feedback latency:** 180 seconds

---

## Per-Task Verification Map

> Filled by the planner per task; the rows below are the validation skeleton derived from the
> RESEARCH "## Validation Architecture" section and the 4 ROADMAP success criteria. The pure
> PKCE crypto helper is the strongest Nyquist sampling target (deterministic invariants).

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 103-PKCE-* | pkce-helper | — | WEBCH-01/02 | — | code_verifier is 43–128 chars, unreserved charset; no predictable values | unit | `cargo test -p pmcp --lib pkce` | ❌ W0 | ⬜ pending |
| 103-PKCE-* | pkce-helper | — | WEBCH-01/02 | — | S256 challenge = base64url(SHA256(verifier)), deterministic | property | `cargo test -p pmcp --lib pkce_prop` | ❌ W0 | ⬜ pending |
| 103-PKCE-* | pkce-helper | — | WEBCH-01/02 | — | base64url no-pad roundtrip; never panics on arbitrary input | fuzz | `cargo fuzz run pkce_verifier` (or proptest fallback) | ❌ W0 | ⬜ pending |
| 103-TRANS-* | wasm-http-transport | — | WEBCH-04 | — | send()→receive() correlation returns the POST response (one-slot buffer) | unit | `cargo test -p pmcp --lib wasm_http` (wasm32 where applicable) | ❌ W0 | ⬜ pending |
| 103-TASK-* | demo-server | — | WEBCH-05/06 | T-103-auth | bearer required; tasks/* wire shapes mirror `tests/tool_as_task_lifecycle_http.rs` | integration | `cargo test --test tool_as_task_lifecycle_http` (parity) | ✅ | ⬜ pending |
| 103-E2E | example | — | WEBCH-07/08 | — | browser example builds + runs against live server | manual | see Manual-Only below | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*Requirement IDs (WEBCH-01..09) and exact task IDs are assigned by the planner; this table is the validation skeleton.*

---

## Wave 0 Requirements

- [ ] PKCE helper test module (unit + property; fuzz target or proptest fallback) — stubs for WEBCH-01/02
- [ ] `WasmHttpTransport` correlation test (one-slot pending-response buffer) — stub for WEBCH-04
- [ ] Spike: OAuth2 IdP route-merge with MCP router on one origin + CORS (RESEARCH Open Question 1)
- [ ] Spike: confirm `AuthContext.subject` from bundled IdP bearer matches `store.list(owner)` (Open Question 2)
- [ ] Confirm `getrandom` wasm_js backend cfg requirement via `make wasm-build` (Open Question 4)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Full browser PKCE redirect round-trip (window.location → ?code=&state= → token exchange → bearer in Fetch) | WEBCH-07 (SC-1/3) | Requires a real browser + redirect navigation; not headless-CI automatable this phase | Run `examples/web-channel-client/build.sh`, serve `index.html`, click Login, complete bundled IdP consent, confirm token stored + authenticated tools/list |
| Tasks lifecycle visible in UI (Working→Completed poll loop + Cancel button) | WEBCH-08/09 (SC-2) | Visual/interaction; the explicit poll loop is the teaching artifact | In the running demo, invoke the long task, observe ~500ms polls flipping status line, click Cancel mid-run and confirm tasks/cancel |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
