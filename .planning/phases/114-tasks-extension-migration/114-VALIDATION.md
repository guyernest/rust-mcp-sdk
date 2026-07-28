---
phase: 114
slug: tasks-extension-migration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-27
---

# Phase 114 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --features full <module>` (scoped to touched module) |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~60s scoped / ~10min full gate |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features full <touched module>`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 600 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| *(filled by planner)* | | | TASK-01..06 | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Verify `ClientRequest` semver additivity (RESEARCH Q5) before any wire-type task
- [ ] Vendored ext-tasks schema pinned at commit (PROVENANCE discipline) so tests are offline-deterministic

*Existing cargo test infrastructure covers all phase requirements; no new framework install needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| *(none expected — planner to confirm)* | | | |

*All phase behaviors are expected to have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 600s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
