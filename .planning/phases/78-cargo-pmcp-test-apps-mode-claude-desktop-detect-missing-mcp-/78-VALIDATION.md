---
phase: 78
slug: cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-02
---

# Phase 78 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (rust 1.x) + proptest 1.7 + cargo-fuzz (libfuzzer-sys 0.4) |
| **Config file** | Cargo.toml (workspace root + crates/mcp-tester/Cargo.toml) |
| **Quick run command** | `cargo test -p mcp-tester app_validator` |
| **Full suite command** | `make quality-gate` (matches CI: fmt + clippy pedantic+nursery + build + test + audit) |
| **Estimated runtime** | quick: ~10s; full: ~120s |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p mcp-tester app_validator` (or scoped equivalent for the file under change)
- **After every plan wave:** Run `cargo test -p mcp-tester && cargo test -p cargo-pmcp`
- **Before `/gsd-verify-work`:** Full `make quality-gate` must be green
- **Max feedback latency:** 30 seconds for unit tests; 120 seconds for full quality-gate

---

## Per-Task Verification Map

> The planner fills concrete task IDs after PLAN.md generation. The matrix below
> captures the mandatory verification dimensions derived from RESEARCH.md so
> Dimension 8 (Nyquist) coverage is enforced.
>
> Note (post-checker revision): test command names below match the actual
> tests created in each plan. See the linked plan for the exact `<behavior>`
> block defining each test.

| Dimension | Plan | Wave | Acceptance Criterion | Test Type | Automated Command | Status |
|-----------|------|------|----------------------|-----------|-------------------|--------|
| Strict mode WIRES UP behavior (no longer "same as Standard") | 01 | 1 | `AppValidationMode::ClaudeDesktop` produces ERROR-level issues for missing handlers/imports | unit | `cargo test -p mcp-tester app_validator::tests::claude_desktop_mode_emits_failed_for_missing_handlers` | ⬜ pending |
| Standard mode emits ONE summary WARN per widget (Q4 RESOLVED) | 01 | 1 | `AppValidationMode::Standard` emits exactly 1 Warning row per widget listing missing signals | unit | `cargo test -p mcp-tester app_validator::tests::standard_mode_emits_one_summary_warn_per_widget` | ⬜ pending |
| Handler signal scanner (4 handlers + ontoolresult + connect + new App) | 01 | 1 | Each protocol-handler property assignment is detected; ≥3 of 4 fallback path covered | unit | `cargo test -p mcp-tester app_validator::tests::scan_widget_detects_handlers_via_property_assignment` | ⬜ pending |
| Scanner robustness (no panics on arbitrary HTML/JS) | 03 | 3 | Property test never panics; idempotent on whitespace normalization | property | `cargo test -p mcp-tester --test property_tests` | ⬜ pending |
| Fuzz target for regex/AST scan path | 03 | 3 | `cargo +nightly fuzz run app_widget_scanner` runs ≥60s without crash; compile-only check on stable per Phase 77 Plan 09 convention | fuzz | `(cd fuzz && cargo build --bin app_widget_scanner)` (stable) / `cargo +nightly fuzz run app_widget_scanner -- -max_total_time=60` (nightly) | ⬜ pending |
| `resources/read` plumbing (Vec<(uri, html)>) | 02 | 2 | apps.rs forwards widget bodies to validator; missing/large/non-text bodies handled; integration boundary covered | unit + integration | `cargo test -p cargo-pmcp --bin cargo-pmcp commands::test::apps && cargo test -p cargo-pmcp --test apps_helpers test_apps_resources_read_e2e` | ⬜ pending |
| Cost Coach reproducer FAILS in claude-desktop mode | 03 | 3 | Broken widget exits non-zero with errors naming missing handler(s) | integration (acceptance) | `cargo test -p mcp-tester --test app_validator_widgets test_broken_widget_fails_claude_desktop` | ⬜ pending |
| `cargo pmcp test apps` (Standard) emits ONE summary WARN on broken widget | 03 | 3 | No regression on permissive default; emission shape matches Q4 RESOLVED | integration | `cargo test -p mcp-tester --test app_validator_widgets test_standard_mode_one_summary_warn_for_broken` | ⬜ pending |
| `--mode chatgpt` behavior unchanged | 03 | 3 | Existing chatgpt mode test outputs unchanged | regression | `cargo test -p mcp-tester --test app_validator_widgets test_chatgpt_mode_unchanged_for_widget` | ⬜ pending |
| README + `--help` document the mode | 04 | 2 | grep finds claude-desktop section in README; `--help` shows the new mode | docs check | `cargo run -p cargo-pmcp -- test apps --help \| grep claude-desktop && grep -q "claude-desktop" cargo-pmcp/README.md` | ⬜ pending |
| Error messages link to GUIDE anchor | 04 | 2 | grep finds `[guide:handlers-before-connect]` (or chosen anchor) in error output | integration | `cargo test -p mcp-tester --test error_messages_anchored error_messages_anchored` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

The planner MUST emit task IDs that map onto every row above; no row may be left
without a `task_id` reference once plans are generated.

### Wave map (post-checker revision)

| Plan | Wave | depends_on |
|------|------|------------|
| 01 (validator core) | 1 | [] |
| 02 (CLI plumbing) | 2 | [78-01] |
| 03 (ALWAYS reqs + fixtures + tests) | 3 | [78-01, 78-02] (Plan 03's narrative claims its integration tests verify the wired CLI; adding 78-02 to depends_on makes that genuinely true. Wave 3 = max(1, 2) + 1.) |
| 04 (docs + GUIDE anchors + expander) | 2 | [78-01] (Wave 2 = max(1) + 1; runs in parallel with Plan 02) |

---

## Wave 0 Requirements

- [ ] `crates/mcp-tester/Cargo.toml` — add `proptest = "1"` to `[dev-dependencies]`
- [ ] `crates/mcp-tester/fuzz/` — initialize cargo-fuzz workspace if absent (or attach to existing one)
- [ ] `examples/mcp-apps-claude-desktop-validator/` — fixture pair (broken_minimal.html + corrected_minimal.html) plus driver `main.rs`
- [ ] `crates/mcp-tester/tests/` — directory exists for integration tests against the fixture pair

*Existing infrastructure covers ServerTester::read_resource, regex 1.x, and the cargo test harness.*

---

## Manual-Only Verifications

| Behavior | Acceptance Ref | Why Manual | Test Instructions |
|----------|----------------|------------|-------------------|
| Vite singlefile minification fidelity | RESEARCH §A1 (Q1 RESOLVED — deferred to follow-up) | Confirms `\.\s*onteardown\s*=` patterns survive `vite build --mode production` | Per RESEARCH Q1 RESOLVED, the fixture pair is hand-authored to mirror Vite output. Empirical Vite verification is moved to a follow-up phase if scanner false-negatives are observed in the wild. |
| Cost Coach reproducer parity | Acceptance §1 (Q2 RESOLVED — synthetic fixture in-tree) | Phase 78 ships a synthetic minimal fixture; full Cost Coach bundle is not vendored | Per RESEARCH Q2 RESOLVED. If Cost Coach later ships a regression bundle, add as `cost_coach_repro.html` alongside without changing tests. |

---

## Validation Sign-Off

- [ ] Every plan task has `<automated>` verify or Wave 0 dependency
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (proptest dep, fuzz workspace, fixture dir)
- [ ] No watch-mode flags in any test command
- [ ] Feedback latency < 30s for unit; < 120s for full
- [ ] `nyquist_compliant: true` set in frontmatter once gate passes
- [ ] All ALWAYS requirements (unit + property + fuzz + example) mapped to plan tasks
- [ ] Every test command in the verification map maps to a real test name in the corresponding plan (Warning 5 fix)
- [ ] Wave numbering is consistent with depends_on (Wave 2 fix per Warning 3)

**Approval:** pending
