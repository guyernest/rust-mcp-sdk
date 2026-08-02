---
phase: 116
slug: auth-hardening-seps
status: approved
nyquist_compliant: true
wave_0_complete: false
created: 2026-08-02
---

# Phase 116 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test / cargo nextest (Rust) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --features full,oauth --lib` |
| **Full suite command** | `make quality-gate` PLUS `cargo test --features full,oauth` (oauth is NOT in the `full` feature — the gate alone proves nothing for this phase) |
| **Estimated runtime** | ~120 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --features full,oauth --lib`
- **After every plan wave:** Run `make quality-gate` + `cargo test --features full,oauth`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 180 seconds

---

## Per-Task Verification Map

One row per plan × test binary (task-level `<automated>` blocks in each PLAN.md carry the exact
commands; all nextest filters use `binary(<name>)` form per the Phase 114 selector lesson).

| Plan | Wave | Requirement | Test Binaries | Automated Command Form | Status |
|------|------|-------------|---------------|------------------------|--------|
| 116-01 | 1 | AUTH-01/02/03 | `oauth_dcr_integration`, `v2_bounded_reads_tripwire` | `cargo nextest run --features full,oauth -E 'binary(...)'` | ⬜ pending |
| 116-02 | 2 | AUTH-01 | `oauth_iss_validation`, `pmcp` (lib/doctests) | same | ⬜ pending |
| 116-03 | 2 | AUTH-02 | `pmcp` (lib/doctests) | same | ⬜ pending |
| 116-04 | 3 | AUTH-02/03 | `oauth_application_type`, `oauth_discovery_urls` | same | ⬜ pending |
| 116-05 | 3 | AUTH-03 | `oauth_credential_store` (+ wasm32 CI fence) | same | ⬜ pending |
| 116-06 | 4 | AUTH-01/03 | `oauth_discovery_validation`, `oauth_dcr_integration`, `pmcp` | same | ⬜ pending |
| 116-07 | 5 | AUTH-03 | `oauth_provider_discovery` | same | ⬜ pending |
| 116-08 | 4 | AUTH-01/03 | fuzz target `oauth_authorization_response` + `cargo run --example` (ALWAYS reqs; no nextest binary) | `cargo fuzz run ... -max_total_time=180` | ⬜ pending |
| 116-09 | 5 | AUTH-01 | `oauth_iss_integration`, `oauth_state_csrf`, `oauth_dcr_integration` | same | ⬜ pending |
| 116-10 | 6 | AUTH-02/03 | `oauth_dcr_integration`, `oauth_iss_integration` | same | ⬜ pending |
| 116-11 | 7 | AUTH-03 | `oauth_store_wiring`, `oauth_dcr_integration`, `oauth_iss_integration` | same | ⬜ pending |
| 116-12 | 8 | AUTH-03 | `oauth_refresh`, `oauth_state_csrf`, `oauth_store_wiring`, `oauth_dcr_integration`, `oauth_iss_integration` | same | ⬜ pending |
| 116-13 | 9 | AUTH-03 | `cargo_pmcp` (cargo-pmcp workspace tests) | same | ⬜ pending |
| 116-14 | 9 | AUTH-03 | `v2_bounded_reads_tripwire` | same | ⬜ pending |
| 116-15 | 10 | AUTH-01/02/03 | all 12 binaries (closing full-sweep) | same | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

None as a separate wave — TDD-shaped tasks in the plans land test + implementation together, so no
standalone stub-creation wave is required. Existing infrastructure covers all phase requirements,
with one standing check:

- [ ] Confirm `cargo test --features full,oauth` compiles and selects the oauth integration tests
  (measured pre-planning: `binary(oauth_dcr_integration)` selects 0 tests under `--features full`,
  5 under `--features full,oauth`) — re-verified in 116-01 baselines

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Live Entra ID discovery-URL probe behavior | AUTH-03 (D-13/A3) | Requires network access to login.microsoftonline.com | Probe tenant discovery URL candidates in documented order; appended form must be attempted first |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (checker-verified: every task has a real `<automated>` command with non-zero-count assertions)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (no separate Wave 0 needed — tests land with implementation)
- [x] No watch-mode flags
- [x] Feedback latency < 180s (one flagged exception: 116-08 fuzz campaign runs AT the 180s ceiling — inherent to the CLAUDE.md ALWAYS-fuzz requirement, accepted by plan-checker as awareness-only)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-08-02 (plan-checker pass: 0 blockers; map populated from committed plans `6b57ca10`)
