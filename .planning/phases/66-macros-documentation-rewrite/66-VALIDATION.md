---
phase: 66
slug: macros-documentation-rewrite
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-11
---

# Phase 66 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test + cargo doc + make quality-gate |
| **Config file** | `Cargo.toml` (workspace), `pmcp-macros/Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp-macros --doc` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~15s quick, ~5min full |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-macros --doc` (validates README doctests compile)
- **After every plan wave:** Run `cargo build -p pmcp-macros && cargo test -p pmcp-macros`
- **Before `/gsd-verify-work`:** `make quality-gate` must be green (matches CI exactly)
- **Max feedback latency:** ~15 seconds for README doctest iteration

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | MACR-01/02/03 | — | N/A — docs phase | TBD | TBD | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

*Note: Per-task rows are filled by the planner agent during plan creation (step 8). This document is pre-created with infrastructure constants; the planner populates task-level rows.*

---

## Wave 0 Requirements

- [ ] Proof-of-concept: verify `#![doc = include_str!("../README.md")]` + same-crate proc-macro doctest imports compile (~2 min task, de-risks assumptions A2 and A7 from research)
- [ ] Existing infrastructure (`pmcp-macros/Cargo.toml:27` already has `pmcp` dev-dependency) is sufficient; no framework install required

*The Wave 0 POC is recommended by research section "Assumptions" — validate before committing to full README rewrite.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `docs.rs/pmcp-macros` renders rewritten README as crate-level docs | MACR-02 | Requires publishing to docs.rs (post-release) | After v0.5.0 publish, visit `https://docs.rs/pmcp-macros/0.5.0/` and confirm README content is visible on the landing page |
| docs.rs auto-generated feature badges appear correctly | N/A (phase 67 territory) | Requires published crate | Post-release: check feature flag rendering on docs.rs |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers the `include_str!` + doctest POC
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter (planner to flip after populating task rows)

**Approval:** pending
