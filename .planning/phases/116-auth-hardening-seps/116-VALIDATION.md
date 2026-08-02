---
phase: 116
slug: auth-hardening-seps
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-02
---

# Phase 116 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test / cargo nextest (Rust) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --features full,oauth --lib` |
| **Full suite command** | `make quality-gate` PLUS `cargo test --features full,oauth` (oauth is NOT in the `full` feature — the gate alone proves nothing for this phase) |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features full,oauth --lib`
- **After every plan wave:** Run `make quality-gate` + `cargo test --features full,oauth`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 180 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by planner) | — | — | AUTH-01 / AUTH-02 / AUTH-03 | — | — | unit | `cargo test --features full,oauth` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Verify `cargo test --features full,oauth` compiles and selects the oauth integration tests (measured: `binary(oauth_dcr_integration)` selects 0 tests under `--features full`, 5 under `--features full,oauth`)
- [ ] Test stubs for AUTH-01 (RFC 9207 `iss` validation), AUTH-02 (`application_type` in DCR), AUTH-03 (issuer-keyed credential storage + clarifications)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live Entra ID discovery-URL probe behavior | AUTH-03 (D-13) | Requires network access to login.microsoftonline.com | Probe tenant discovery URL candidates in documented order; appended form must be attempted first |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 180s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
