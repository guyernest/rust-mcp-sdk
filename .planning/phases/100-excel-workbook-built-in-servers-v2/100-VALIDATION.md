---
phase: 100
slug: excel-workbook-built-in-servers-v2
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-20
---

# Phase 100 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Detailed dimension-by-dimension validation architecture lives in `100-RESEARCH.md`
> ("Validation Architecture" section). This file is the executable sampling contract;
> the planner/Nyquist auditor fills the Per-Task Verification Map from the PLAN.md tasks.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (unit + integration), `proptest` (property), `cargo fuzz` (fuzz), `cargo run --example` (example) |
| **Config file** | workspace `Cargo.toml` (no extra config) |
| **Quick run command** | `cargo test -p <workbook-crate> --lib` |
| **Full suite command** | `make quality-gate` (fmt --all + clippy pedantic/nursery + build + test + audit) + PMAT complexity gate + purity test |
| **Estimated runtime** | ~minutes (workspace build dominates) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <workbook-crate> --lib`
- **After every plan wave:** Run `make quality-gate` (+ PMAT + purity for any touched compiler code)
- **Before `/gsd:verify-work`:** Full suite must be green (Success Criterion 5: quality-gate + PMAT + purity all green)
- **Max feedback latency:** keep `--lib` quick run under the dev loop; full gate at wave boundaries

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| _filled by planner from PLAN.md tasks_ | | | | | | | | | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Provenance-valid `template.xlsx` reference fixture (replaces misleading hand-authored fixtures) — gates Success Criterion 4
- [ ] Table-harvest unit fixtures (input/output Excel Tables with type/unit/enum/tier witnesses)
- [ ] Fail-helpful lint negative fixtures (blank name, duplicate key, value-less row, no-caption output, unmappable tool name, input-feeds-no-tool)

*Existing `cargo test`/`proptest`/`cargo fuzz` infrastructure covers the rest.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cargo pmcp workbook explain <file>` preview reads as a coherent tool surface to a human | Success Criterion 3 | "reads well for LLM selection" is a human-judged property | Run on the shipped template; confirm tool names/descriptions/IO schemas render |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency acceptable for Rust build loop
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
