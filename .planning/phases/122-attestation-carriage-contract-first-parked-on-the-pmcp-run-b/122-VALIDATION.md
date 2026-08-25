---
phase: 122
slug: attestation-carriage-contract-first-parked-on-the-pmcp-run-b
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-25
---

# Phase 122 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + apollo-compiler contract tests + cargo-deny bans |
| **Config file** | Makefile (quality-gate chain), crate-local deny.toml (Wave 0 installs for pmcp-package) |
| **Quick run command** | `cargo test --manifest-path crates/pmcp-package/Cargo.toml` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~60 seconds (quick) / several minutes (full) |

---

## Sampling Rate

- **After every task commit:** Run the touched crate's tests (`cargo test --manifest-path crates/pmcp-package/Cargo.toml` or `cargo test -p cargo-pmcp`)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (filled by planner) | | | PKGX-01 | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Gate-reach guard: the new contract test binary must actually RUN in `make quality-gate` — RESEARCH.md measured that `cargo-pmcp/tests/*` integration binaries are in NO current gate (`--lib` scoping); mirror `test-openapi-server`'s `REQUIRED_TEST_BINARIES` guard
- [ ] `crates/pmcp-package/deny.toml` — crate-local allowlist config for the no-crypto boundary (D-12/D-13), wired as a sibling purity list in the Makefile

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live issuance/verification leg | PKGX-01 | Parked — pmcp.run backend does not exist yet | `#[ignore]`d env-gated test (double-gate pattern); run only when backend ships |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
