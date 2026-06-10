---
phase: 92
slug: bundlesource-served-tool-toolkit-module
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-10
---

# Phase 92 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Populated from 92-RESEARCH.md "## Validation Architecture" — the planner
> fills the Per-Task Verification Map when PLAN.md tasks are finalized.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest (property) + cargo-fuzz |
| **Config file** | root `Cargo.toml` workspace; crate-level `Cargo.toml` per member |
| **Quick run command** | `cargo test -p pmcp-workbook-runtime -p pmcp-server-toolkit --features workbook` |
| **Full suite command** | `make quality-gate` (fmt + clippy pedantic/nursery + build + test + audit) |
| **Estimated runtime** | quick ~60s; full ~10min |

---

## Sampling Rate

- **After every task commit:** Run the quick run command
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green; `make purity-check` (incl. new toolkit `workbook` combo) must pass
- **Max feedback latency:** ~600 seconds (full gate)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| (planner fills per PLAN.md tasks) | | | WBSV-01..09 | | | | | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Synthetic tax-calc fixture generator (test-support) + committed golden bundle under fixtures dir — every WBSV test depends on it
- [ ] Byte-stability check: regenerating the golden is byte-identical (CI-checkable command)
- [ ] Tamper helpers (copy-to-tempdir + corrupt) for WBSV-08/WBSV-06 negative paths
- [ ] `workbook` feature wiring in pmcp-server-toolkit so `cargo test --features workbook` compiles

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `include_dir 0.7.4` dependency vetting | D-06 | New external dep ([ASSUMED] in research) | checkpoint:human-verify before install — confirm crate name/version/maintenance on crates.io |
| Streamable-HTTP example end-to-end | D-12 | Real client against running server | `cargo run --example ...` then mcp-tester against the HTTP endpoint; confirm all five tools respond |
