---
phase: 52
slug: reduce-transitive-dependencies
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-18
---

# Phase 52 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo check --workspace` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo check --workspace`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 52-01-01 | 01 | 1 | DEP-REDUCE | build | `cargo check --workspace` | ✅ | ⬜ pending |
| 52-01-02 | 01 | 1 | DEP-REDUCE | build | `cargo check --workspace --no-default-features` | ✅ | ⬜ pending |
| 52-02-01 | 02 | 2 | DEP-REDUCE | build+test | `make quality-gate` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dep count reduction | DEP-REDUCE | Requires `cargo tree` inspection | Run `cargo tree --depth 1 \| wc -l` before/after |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
