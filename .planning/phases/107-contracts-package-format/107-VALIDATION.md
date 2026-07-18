---
phase: 107
slug: contracts-package-format
status: planned
nyquist_compliant: true
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
| 01-T1 | 107-01 | 1 | PKG-01 | T-107-01 | crate builds standalone, unedited | build/unit | `cd crates/pmcp-package && cargo test` | ✅ (ported tests) | ⬜ pending |
| 01-T2 | 107-01 | 1 | PKG-01 | T-107-01/03 | publish metadata + license resolve; no internal refs in docs | publish dry-run | `cd crates/pmcp-package && cargo publish --dry-run --allow-dirty` | ✅ (cargo built-in) | ⬜ pending |
| 01-T3 | 107-01 | 1 | PKG-01 | T-107-01 | no internal ticket IDs leak to public rustdoc | doc-lint | `cd crates/pmcp-package && grep -rn "Phase 1\|Wave 0\|D-10\|I-2\|T-168\|Phase 169\|I-4\|guyernest/pmcp-run" src --include='*.rs' \| wc -l` == 0 + `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --no-deps` | ❌ W0 (doc-lint gate) | ⬜ pending |
| 02-T1 | 107-02 | 2 | PKG-02 | T-107-04 | all 4 package kinds digest-stable | golden-fixture | `cd crates/pmcp-package && cargo test --test digest_stability` | ❌ W0 (add agent_/team_ fixtures) | ⬜ pending |
| 02-T2 | 107-02 | 2 | PKG-02 | T-107-05 | release publishes via manifest-path (excluded crate) | integration | `cargo metadata --format-version 1 --no-deps` + `grep -q "manifest-path crates/pmcp-package/Cargo.toml" .github/workflows/release.yml` | ❌ W0 (wire release.yml) | ⬜ pending |
| 03-T1 | 107-03 | 1 | PKG-03 | T-107-06/07 | contract captures 4 surfaces + correct dispatch invariants | yaml-parse | `python3 -c "import yaml; yaml.safe_load(open('contracts/team-servers-v1.yaml'))"` + key/tool-name asserts | ❌ W0 (author contract) | ⬜ pending |
| 03-T2 | 107-03 | 1 | PKG-03 | T-107-06 | every fixture tool present in contract | conformance | `cargo test --test team_contracts_conformance` | ❌ W0 (author fixtures + test) | ⬜ pending |

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
