---
phase: 106
slug: 106-client-host-surface
status: ready
nyquist_compliant: true
wave_0_complete: false
created: 2026-07-17
---

# Phase 106 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest 1.x + cargo-fuzz |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --features full client_host` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | quick ~60s · full ~10min |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features full client_host` (plus the specific new test files)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 600 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 106-01-T1 | 106-01 | 1 | HOST-01, HOST-02, HOST-03 | T-106-01 | Object-safe host handler traits + registry; no tokio-only types (wasm-clean) | unit | `cargo test --lib client::host` | ❌ Wave 0 | ⬜ pending |
| 106-01-T2 | 106-01 | 1 | HOST-01, HOST-02, HOST-03, HOST-06 | T-106-01, T-106-02 | Dispatch replaces the :2234 error; unhandled method -> -32601 (connection stays alive); parse-alias routed | unit | `cargo test --lib client` | ❌ Wave 0 | ⬜ pending |
| 106-01-T3 | 106-01 | 1 | HOST-01, HOST-02, HOST-03 | T-106-01 | Duplex round-trips answer sampling/elicitation/roots; passthrough proptest over tool params | integration + property + example | `cargo test --test client_host_roundtrip` · `cargo run --example s49_sampling_host` | ❌ Wave 0 | ⬜ pending |
| 106-02-T1 | 106-02 | 2 | HOST-04 | T-106-05, T-106-08 | Approval consulted before returning completion; Deny -> -32603 Response (no teardown); default allow | unit | `cargo test --lib client` | ❌ Wave 0 | ⬜ pending |
| 106-02-T2 | 106-02 | 2 | HOST-05 | T-106-06 | Registry authoritative for sampling/elicitation/roots; caller-set + no-handler => absent (no capability lie) | unit | `cargo test --lib client` | ❌ Wave 0 | ⬜ pending |
| 106-02-T3 | 106-02 | 2 | HOST-04, HOST-05 | T-106-07 | Adversarial inbound params never panic/hang deserialization | fuzz | `cd fuzz && cargo build --bin client_host_dispatch` | ❌ Wave 0 | ⬜ pending |
| 106-03-T1 | 106-03 | 2 | HOST-06 | T-106-09 | Book disambiguates the two sampling directions (LLM-server vs spec host) | doc assertion | `test -f pmcp-book/src/ch17-04-sampling-hosting.md && grep -q 'LLM-server pattern' pmcp-book/src/ch17-04-sampling-hosting.md` | ❌ Wave 0 | ⬜ pending |
| 106-03-T2 | 106-03 | 2 | HOST-06 | T-106-09 | New page reachable from the book TOC; book builds | doc assertion | `grep -q 'ch17-04-sampling-hosting.md' pmcp-book/src/SUMMARY.md` | ❌ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements — `tests/common/duplex.rs` harness, `tests/handler_peer_integration.rs` round-trip template, proptest already a dev-dependency, `fuzz_peer_handle.rs` fuzz template. No new framework installs. The new test/example/fuzz files listed in the map are created by their owning tasks (they do not yet exist — "File Exists" = ❌ until executed).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Book/rustdoc disambiguation reads correctly | HOST-06 | Prose quality judgment | Read the LLM-server pattern section in rustdoc + book chapter; confirm the two directions are unambiguous |

All other phase behaviors have automated verification (duplex round-trips per handler type, capability-derivation/override unit tests, approval-hook tests, property tests over params passthrough, fuzz over inbound params).

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 600s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-07-17
