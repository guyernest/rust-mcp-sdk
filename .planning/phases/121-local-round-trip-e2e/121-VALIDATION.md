---
phase: 121
slug: local-round-trip-e2e
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-08-23
---

# Phase 121 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `121-RESEARCH.md` § Validation Architecture. Task IDs are filled in by the planner;
> rows below are keyed by test-function name until then.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` / `#[tokio::test]` (libtest); `tokio` 1 with `macros`, `rt-multi-thread`, `time` |
| **Config file** | none — cargo-native; targets declared implicitly by `crates/pmcp-openapi-server/tests/*.rs` |
| **Quick run command** | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` |
| **Full suite command** | `cargo test -p pmcp-openapi-server -- --test-threads=1` |
| **Estimated runtime** | ~2-5 seconds (measured baseline: `parity_replay` = 1.11s, `3 passed; 1 ignored`, exit 0, run 2026-08-23) |

**Gate command (does not exist yet — Wave 0 blocker):** `make test-openapi-server`, chained into
`make test-all`. Until that exists, `make quality-gate` is green on a phase whose entire deliverable
it never executes (RESEARCH CF-2).

> ⚠ **Selector discipline (repo-specific; bit 7× in Phase 114).** Under `cargo nextest`,
> `-E 'test(/foo/)'` silently selects **zero** tests and exits 0. Every `<verify>` block in this
> phase must use plain `cargo test --test <name>`, which fails loudly on an unknown target.
> No `nextest -E 'test(...)'` selector may appear in any plan.

---

## Sampling Rate

- **After every task commit:** `cargo test -p pmcp-openapi-server --test <target> -- --test-threads=1`
- **After every plan wave:** `cargo test -p pmcp-openapi-server -- --test-threads=1` **plus**
  `make pmcp-package-gate` (the `pmcp-package` half *is* genuinely gated — RESEARCH CF-1 — so a
  slot-API regression surfaces there)
- **Before `/gsd-verify-work`:** `make quality-gate` green — **meaningful only once
  `test-openapi-server` is chained into `test-all`**
- **Max feedback latency:** ~5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | 0 | PKG-04 (CF-2 gate) | — | N/A | build | `make test-openapi-server` | ❌ W0 | ⬜ pending |
| TBD | TBD | 0 | PKG-04 (D-01) | — | N/A | build | `cargo test -p pmcp-openapi-server --no-run` | ❌ W0 | ⬜ pending |
| TBD | TBD | 0 | PKG-04 (D-03) | — | Dev-dep pin is caret `"0.2"` in `[dev-dependencies]` | unit | `cargo test -p pmcp-openapi-server --test pmcp_package_pin` | ❌ W0 | ⬜ pending |
| TBD | TBD | 0 | PKG-04 (D-02 lift) | — | N/A | integration | `cargo test -p pmcp-openapi-server --test parity_replay -- --test-threads=1` → must remain `3 passed; 1 ignored` | ✅ green | ⬜ pending |
| TBD | TBD | 1 | PKG-04 / SC1 | — | Two OCI layouts + temp dirs asserted distinct; differing endpoint/credential/auth; fully offline, two `wiremock` instances | integration | `cargo test -p pmcp-openapi-server --test roundtrip_e2e -- --test-threads=1` | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | PKG-04 / SC2a | — | `required_slots` set-equals the hardcoded 3-slot literal | integration | same target, `roundtrip_required_slots_match_expected_literal` | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | PKG-04 / SC2b | — | `detect_deviation` reports B's endpoint drift; returns `None` for the credential | integration | same target, `roundtrip_endpoint_drift_is_reported` | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | PKG-04 / SC3a | — | B's served `(name, inputSchema)` set equals A's; both non-empty and containing the 4 known names | integration | same target, `roundtrip_tool_surface_parity` | ❌ W0 | ⬜ pending |
| TBD | TBD | 1 | PKG-04 / SC3b | — | `london-tube-scenarios.yaml` replays green in B, per-step gated, `steps_total > 0` | integration | same target, `roundtrip_scenarios_replay_green_in_env_b` | ❌ W0 | ⬜ pending |
| TBD | TBD | 2 | PKG-04 / SC4-red | — | Degraded B (tool removed) → comparison returns `Err` naming that tool | integration | same target, `degraded_env_b_missing_tool_is_reported` | ❌ W0 | ⬜ pending |
| TBD | TBD | 2 | PKG-04 / SC4-red | — | Degraded B (named slot unfilled) → assembly/comparison fails naming that slot | integration | same target, `degraded_env_b_unfilled_slot_is_reported` | ❌ W0 | ⬜ pending |
| TBD | TBD | 2 | PKG-04 / SC4-green | — | No assertion on manifest field names / layer ordering / digest values, with a nonzero-lines-scanned floor | integration | same target, `roundtrip_e2e_asserts_nothing_about_manifest_shape` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-openapi-server/Cargo.toml` — add `pmcp-package = { version = "0.2", path = "../pmcp-package" }` and `toml = "0.8"` to `[dev-dependencies]`; prove resolution with `cargo test -p pmcp-openapi-server --no-run`
- [ ] `crates/pmcp-openapi-server/tests/common/mod.rs` — the D-02 helper lift, with `#![allow(dead_code)]`, per-binary-mutex reasoning, and `mount_london_tube` parameterized by `app_key` (RESEARCH CF-6)
- [ ] `Makefile` — `test-openapi-server` target chained into `test-all` (RESEARCH CF-2) **with a nonzero-test-count guard**
- [ ] `crates/pmcp-openapi-server/tests/pmcp_package_pin.rs` — D-03 tripwire, reading the `[dev-dependencies]` table (not `[dependencies]`)
- [ ] `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` — the E2E, the D-08 negatives, and the D-09 structural guard
- [ ] `.planning/ROADMAP.md` (2 sites) + `.planning/REQUIREMENTS.md:26` — finish D-05's correction (RESEARCH CF-5 / OQ-4)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| — | — | — | — |

*All phase behaviors have automated verification.* This is a test-only phase; its deliverable **is**
the automated verification. The one judgement call that resists automation — whether the D-09
structural guard's deny-list is the *right* deny-list — is mitigated by a nonzero-lines-scanned
floor rather than by manual review.

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] No `nextest -E 'test(...)'` selectors in any `<verify>` block (repo-specific false-green)
- [ ] Every new test asserts a **nonzero** count of the thing it measures (repo has shipped
      exit-0-measuring-nothing shapes three times: CR-01, the RTK-truncated gate run, and
      `list_tools()`'s `unwrap_or_default()` — RESEARCH CF-3)
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
