---
phase: 107
slug: contracts-package-format
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-17
---

# Phase 107 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | crates/pmcp-package/Cargo.toml (after adoption) |
| **Quick run command** | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~60 seconds (crate-local); quality-gate several minutes |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --manifest-path crates/pmcp-package/Cargo.toml`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by planner) | | | PKG-01..03 | — | | unit / golden-fixture | | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-package/tests/digest_stability.rs` — extend golden fixtures to agent + team package kinds
- [ ] Conformance fixtures for team-server contracts (`contracts/` YAML)
- [ ] No framework install needed — cargo test is built-in

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| docs.rs rendering of published crate | PKG-01 | docs.rs builds post-publish | `cargo doc --no-deps` locally as proxy; verify docs.rs after publish |
| crates.io publish availability | PKG-02 | requires actual publish | `cargo publish --dry-run` as proxy; verify `pmcp-package = "0.1"` resolves after release |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
