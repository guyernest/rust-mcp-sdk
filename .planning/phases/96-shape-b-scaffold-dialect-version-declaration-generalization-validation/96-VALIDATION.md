---
phase: 96
slug: shape-b-scaffold-dialect-version-declaration-generalization-validation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-14
---

# Phase 96 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust) + `cargo nextest` if available |
| **Config file** | none — workspace `Cargo.toml` test harness |
| **Quick run command** | `cargo test -p pmcp-workbook-dialect -p cargo-pmcp` |
| **Full suite command** | `make quality-gate` (fmt + clippy + build + test + audit) + `make purity-check` |
| **Estimated runtime** | ~120–300 seconds |

---

## Sampling Rate

- **After every task commit:** Run the relevant crate's `cargo test -p <crate>`
- **After every plan wave:** Run `cargo test --workspace` for touched crates
- **Before `/gsd:verify-work`:** `make quality-gate` must be green AND `make purity-check` must pass
- **Max feedback latency:** ~300 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | WBCL-05 / WBDL-02 / WBEX-01 / WBEX-02 | — | N/A (local CLI / compiler) | unit/integration | `cargo test ...` | ❌ W0 | ⬜ pending |

*Filled by planner; Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `.xlsx` fixture authoring recipe (rust_xlsxwriter with Excel identity + `compile_workbook_with_fixture_override` / `FreshnessPolicy::TrustedFixture`) proven reusable for the loan workbook (WBEX-01) and the quirk corpus (WBEX-02)
- [ ] Doc↔const binding test harness extended to cover the dialect-version surface (WBDL-02)

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cargo pmcp new --kind workbook-server` produces a crate that `cargo run`s and serves | WBCL-05 | end-to-end scaffold smoke is best confirmed by a one-time manual run | scaffold into a tmp dir, `cargo run`, hit `tools/list` |

*Automated coverage preferred; planner should add an integration test for the scaffold round-trip where feasible.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
