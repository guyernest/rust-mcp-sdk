---
phase: 96
slug: shape-b-scaffold-dialect-version-declaration-generalization-validation
status: draft
nyquist_compliant: true
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
| T1 | 96-01 | 1 | WBDL-02 | T-96-03 | version consts bound to spec by drift guard | unit | `cargo test -p pmcp-workbook-dialect` | ❌ W0 | ⬜ pending |
| T2 | 96-01 | 1 | WBDL-02 | T-96-01 | fail-closed typed CompileError on incompatible/malformed version | unit + property | `cargo test -p pmcp-workbook-compiler dialect_version` | ❌ W0 | ⬜ pending |
| T3 | 96-01 | 1 | WBDL-02 | T-96-01 | version-string parse never panics (fuzz); absent→baseline preserved | unit + fuzz | `cargo test -p pmcp-workbook-compiler` | ❌ W0 | ⬜ pending |
| T1 | 96-02 | 1 | WBCL-05 | T-96-05 | scaffold Cargo.toml default-features=false (purity-safe) | unit | `cargo build -p cargo-pmcp` | ❌ W0 | ⬜ pending |
| T2 | 96-02 | 1 | WBCL-05 | T-96-04 / T-96-06 | crate-name path-traversal guard; bundle-bytes drift lock | unit | `cargo test -p cargo-pmcp workbook_server` | ❌ W0 | ⬜ pending |
| T3 | 96-02 | 1 | WBCL-05 | — | scaffold output round-trip demonstrated (ALWAYS example) | example | `cargo run -p cargo-pmcp --example workbook_server_scaffold` | ❌ W0 | ⬜ pending |
| T1 | 96-03 | 2 | WBEX-01 / WBEX-02 | T-96-07 / T-96-08 | genuine Excel identity via trusted-fixture override (`#[cfg(test)]`-only) | integration | `cargo test -p pmcp-workbook-compiler fixture_author` | ❌ W0 | ⬜ pending |
| T2 | 96-03 | 2 | WBEX-02 | T-96-09 | 1900-leap disposition recorded; no DATE functions added | integration + doc | `test -f crates/pmcp-workbook-compiler/SPIKE-1900-leap.md && grep -q "Disposition" crates/pmcp-workbook-compiler/SPIKE-1900-leap.md && cargo test -p pmcp-workbook-compiler` | ❌ W0 | ⬜ pending |
| T1 | 96-04 | 3 | WBEX-01 | T-96-12 | synthetic loan fixture (no customer/TowelRads material) | integration | `test -f crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.xlsx && test -f crates/pmcp-workbook-compiler/tests/fixtures/loan-calc.provenance-override.json && cargo test -p pmcp-workbook-compiler fixture_author` | ❌ W0 | ⬜ pending |
| T2 | 96-04 | 3 | WBEX-01 | T-96-10 / T-96-11 | loan serves OWN manifest via generic driver; production-refusal counter-test | integration | `cargo test -p pmcp-workbook-compiler reemit_loan` | ❌ W0 | ⬜ pending |
| T1 | 96-05 | 4 | WBEX-02 | T-96-15 | per-quirk scalar_eval assertions against excel_round (no naive round) | unit | `cargo test -p pmcp-workbook-runtime scalar_eval` | ❌ W0 | ⬜ pending |
| T2 | 96-05 | 4 | WBEX-02 | T-96-13 / T-96-14 | penny reconcile via within_tol (±0.01); never exact-float `==` on money | integration | `cargo test -p pmcp-workbook-compiler quirks` | ❌ W0 | ⬜ pending |

*Filled by planner; Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `.xlsx` fixture authoring recipe (rust_xlsxwriter with Excel identity + `compile_workbook_with_fixture_override` / `FreshnessPolicy::TrustedFixture`) proven reusable for the loan workbook (WBEX-01) and the quirk corpus (WBEX-02) — established by 96-03 Task 1 (`fixture_author.rs`)
- [ ] Doc↔const binding test harness extended to cover the dialect-version surface (WBDL-02) — established by 96-01 Task 1 (`SUPPORTED_DIALECT_VERSION` / `BASELINE_DIALECT_VERSION` consts + spec-doc drift guard)
- [ ] `dialect_version.rs` reader + semver-compat decision created (WBDL-02) — 96-01 Task 2
- [ ] Compiler fuzz harness for the version-string parse created (WBDL-02 ALWAYS fuzz) — 96-01 Task 3 (`fuzz/fuzz_targets/dialect_version_parse.rs`)
- [ ] `workbook_server.rs` scaffold template + drift-lock & bundle-bytes golden tests (WBCL-05) — 96-02 Tasks 1–2
- [ ] 1900-leap-year disposition decided so the quirk corpus has a path (WBEX-02) — 96-03 Task 2 (`SPIKE-1900-leap.md`)
- [ ] `reemit_loan.rs` in-crate compile-and-serve proof (WBEX-01) — 96-04 Task 2
- [ ] `quirks_reconcile.rs` mini-reconcile harness + per-quirk scalar_eval tests (WBEX-02) — 96-05 Tasks 1–2

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cargo pmcp new --kind workbook-server` produces a crate that `cargo run`s and serves | WBCL-05 | end-to-end scaffold smoke is best confirmed by a one-time manual run | scaffold into a tmp dir, `cargo run`, hit `tools/list` |

*Automated coverage preferred; the scaffold round-trip is additionally covered by the `workbook_server_scaffold` example (96-02 Task 3) + the drift-lock/bundle-bytes/file-presence golden tests (96-02 Task 2), so this manual run is a confirmatory smoke only.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 300s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
