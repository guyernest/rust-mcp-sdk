---
phase: 106
slug: 106-client-host-surface
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-17
---

# Phase 106 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest 1.x |
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
| (populated by planner) | | | HOST-01..06 | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements — `tests/common/duplex.rs` harness, `tests/handler_peer_integration.rs` round-trip template, proptest already a dev-dependency. No new framework installs.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Book/rustdoc disambiguation reads correctly | HOST-06 | Prose quality judgment | Read the LLM-server pattern section in rustdoc + book chapter; confirm the two directions are unambiguous |

All other phase behaviors have automated verification (duplex round-trips per handler type, capability-derivation unit tests, approval-hook tests, property tests over preference/params passthrough).

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 600s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
