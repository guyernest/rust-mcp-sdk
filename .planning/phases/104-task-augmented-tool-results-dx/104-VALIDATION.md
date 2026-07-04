---
phase: 104
slug: task-augmented-tool-results-dx
status: draft
nyquist_compliant: false
wave_0_complete: false
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
| (filled by planner) | — | — | TOUT-01..04 | — | — | unit/property/fuzz | `cargo test` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] RED-first test: full `CallToolResult` with `_meta` returned by a tool handler survives `Server` dispatch un-re-wrapped (currently fails at `src/server/mod.rs:1493`)
- [ ] Tripwire test stubs: WARN fires when dispatch is about to text-wrap a Value that looks like a built `CallToolResult`
- [ ] Existing infrastructure (cargo test / proptest / cargo-fuzz targets) covers framework needs — no install required

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
