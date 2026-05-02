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

| Dimension | Plan | Wave | Acceptance Criterion | Test Type | Automated Command | Status |
|-----------|------|------|----------------------|-----------|-------------------|--------|
| Strict mode WIRES UP behavior (no longer "same as Standard") | 01 | 1 | `AppValidationMode::ClaudeDesktop` produces ERROR-level issues for missing handlers/imports | unit | `cargo test -p mcp-tester app_validator::claude_desktop_strict_mode` | ⬜ pending |
| Handler signal scanner (4 handlers + ontoolresult + connect + new App) | 01 | 1 | Each protocol-handler property assignment is detected; ≥3 of 4 fallback path covered | unit | `cargo test -p mcp-tester app_validator::widget_signals` | ⬜ pending |
| Scanner robustness (no panics on arbitrary HTML/JS) | 01 | 1 | Property test never panics; idempotent on whitespace normalization | property | `cargo test -p mcp-tester app_validator::proptest` | ⬜ pending |
| Fuzz target for regex/AST scan path | 03 | 3 | `cargo +nightly fuzz run widget_scanner` runs ≥60s without crash | fuzz | `cargo +nightly fuzz run widget_scanner -- -max_total_time=60` | ⬜ pending |
| `resources/read` plumbing (Vec<(uri, html)>) | 02 | 2 | apps.rs forwards widget bodies to validator; missing/large/non-text bodies handled | unit + integration | `cargo test -p cargo-pmcp test_apps_resources_read` | ⬜ pending |
| Cost Coach reproducer FAILS in claude-desktop mode | 03 | 3 | Broken widget exits non-zero with errors naming missing handler(s) | integration (acceptance) | `cargo run --example validate_widget_pair -- --mode claude-desktop` exits non-zero on broken/exits zero on corrected | ⬜ pending |
| `cargo pmcp test apps` (Standard) still passes broken widget | 03 | 3 | No regression on permissive default | integration | `cargo run -p cargo-pmcp -- test apps` (no flag) on the broken fixture exits zero with WARN | ⬜ pending |
| `--mode chatgpt` behavior unchanged | 03 | 3 | Existing chatgpt mode test outputs unchanged | regression | `cargo test -p mcp-tester app_validator::chatgpt_mode` | ⬜ pending |
| README + `--help` document the mode | 04 | 3 | grep finds claude-desktop section in README; `--help` shows the new mode | docs check | `cargo run -p cargo-pmcp -- test apps --help \| grep claude-desktop && grep -q "claude-desktop" cargo-pmcp/README.md` | ⬜ pending |
| Error messages link to GUIDE anchor | 04 | 3 | grep finds `[guide:handlers-before-connect]` (or chosen anchor) in error output | unit | `cargo test -p mcp-tester app_validator::error_messages_anchored` | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

The planner MUST emit task IDs that map onto every row above; no row may be left
without a `task_id` reference once plans are generated.

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
| Vite singlefile minification fidelity | RESEARCH §A1 | Confirms `\.\s*onteardown\s*=` patterns survive `vite build --mode production` | Run `npm run build` against `examples/mcp-apps-claude-desktop-validator/source/` (a real Vite project), copy the emitted single-file HTML into the fixture path, confirm scanner still detects ≥3 handlers + import string |
| Cost Coach reproducer parity | Acceptance §1 | Vendored fixture must match the actual broken bundle Cost Coach is shipping | Once Cost Coach ships the regression bundle, diff against in-repo `broken_minimal.html`; if signals diverge, file follow-up phase |

---

## Validation Sign-Off

- [ ] Every plan task has `<automated>` verify or Wave 0 dependency
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (proptest dep, fuzz workspace, fixture dir)
- [ ] No watch-mode flags in any test command
- [ ] Feedback latency < 30s for unit; < 120s for full
- [ ] `nyquist_compliant: true` set in frontmatter once gate passes
- [ ] All ALWAYS requirements (unit + property + fuzz + example) mapped to plan tasks

**Approval:** pending
