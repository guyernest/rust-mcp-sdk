---
phase: 109
slug: team-reference-servers
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-18
---

# Phase 109 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest for property tests |
| **Config file** | `crates/pmcp-team-servers/Cargo.toml` (Wave 0 creates crate) |
| **Quick run command** | `cargo test -p pmcp-team-servers --all-features` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~60 seconds (quick) / ~10 minutes (full gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-team-servers --all-features`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by planner) | | | TEAM-01..06 | | | | | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-team-servers/` — new workspace crate with per-server feature flags
- [ ] Conformance-fixture test harness wired to Phase 107 (PKG-03) contract fixtures
- [ ] Spike: task-augmented `pmcp::Client` call for the team-mcp member hop (TEAM-05 `_meta[related_task]`)
- [ ] Spike: `pmat comply check` invocation against existing `contracts/binding.yaml` (D-18)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| "Small team, one process" local run | TEAM-01 | Interactive dev-binary smoke run | Launch each of the four dev binaries; verify tools/list responds |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
