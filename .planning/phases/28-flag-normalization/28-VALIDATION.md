---
phase: 28
slug: flag-normalization
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-12
---

# Phase 28 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in) + proptest |
| **Config file** | cargo-pmcp/Cargo.toml [dev-dependencies] |
| **Quick run command** | `cargo test -p cargo-pmcp` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p cargo-pmcp`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 28-01-01 | 01 | 1 | FLAG-01..07 | unit | `cargo test -p cargo-pmcp flag_parsing` | ❌ W0 | ⬜ pending |
| 28-01-02 | 01 | 1 | FLAG-07 | grep | `grep -r '#\[clap(' cargo-pmcp/src/` | ✅ | ⬜ pending |
| 28-02-01 | 02 | 2 | FLAG-01 | unit | `cargo test -p cargo-pmcp flag_parsing` | ❌ W0 | ⬜ pending |
| 28-02-02 | 02 | 2 | FLAG-02 | unit | `cargo test -p cargo-pmcp flag_parsing` | ❌ W0 | ⬜ pending |
| 28-02-03 | 02 | 2 | FLAG-03 | unit | `cargo test -p cargo-pmcp flag_parsing` | ❌ W0 | ⬜ pending |
| 28-02-04 | 02 | 2 | FLAG-04 | unit | `cargo test -p cargo-pmcp flag_parsing` | ❌ W0 | ⬜ pending |
| 28-02-05 | 02 | 2 | FLAG-05 | unit | `cargo test -p cargo-pmcp flag_parsing` | ❌ W0 | ⬜ pending |
| 28-02-06 | 02 | 2 | FLAG-06 | unit | `cargo test -p cargo-pmcp flag_parsing` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `cargo-pmcp/src/commands/flags.rs` — shared flag structs (FormatValue, OutputFlags, FormatFlags)
- [ ] Unit tests for clap parsing: verify positional URL, --yes, -o, --format via `try_parse_from`
- [ ] Remove `#[allow(dead_code)]` from `GlobalFlags.verbose`

*Existing infrastructure covers clippy and build checks via `make quality-gate`.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CLI help text shows positional URL | FLAG-01 | Visual inspection of --help output | Run `cargo pmcp test check --help`, verify URL is positional not --url |
| No #[clap()] in codebase | FLAG-07 | Static grep verification | `grep -r '#\[clap(' cargo-pmcp/src/` should return empty |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
