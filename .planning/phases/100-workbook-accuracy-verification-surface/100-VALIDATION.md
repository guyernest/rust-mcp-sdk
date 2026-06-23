---
phase: 100
slug: workbook-accuracy-verification-surface
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-06-22
---

# Phase 100 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: 100-RESEARCH.md § Validation Architecture (HIGH confidence, grounded in source).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `proptest` (the "fuzz" surface is proptest-as-fuzz, per existing `prop_decode_total`) |
| **Config file** | none — cargo workspace |
| **Quick run command** | `cargo test -p pmcp-workbook-runtime render::` / `cargo test -p pmcp-server-toolkit workbook::` |
| **Full suite command** | `make quality-gate` (fmt/clippy/build/test/audit) + `make purity-check` + `make doc-check` |
| **Estimated runtime** | ~120 seconds (quality-gate); per-crate module test ~5–15s |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <touched crate> <module>::`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** `make quality-gate` + `make purity-check` + `make doc-check` all green; PMAT cog-25 (CI); doctests on new public runtime fns (`reconcile_reference`, `RenderMode`)
- **Max feedback latency:** ~120 seconds

---

## Per-Task Verification Map

| Req ID | Behavior | Test Type | Automated Command | File Exists |
|--------|----------|-----------|-------------------|-------------|
| WBVER-01 | text & bool formula cells → `<f>`+`<v>` in xlsx XML | unit | `cargo test -p pmcp-workbook-runtime render::` | ✅ extend `render/mod.rs` tests (text/bool VALUES already tested ~line 632; add formula+result `<f>`+`<v>` assertion on unzipped sheet XML) |
| WBVER-02 | InputsOnly → formula cells have `<f>` and NO cached `<v>` | unit | `cargo test -p pmcp-workbook-runtime render::` | ❌ W0 — new test in `render/mod.rs` |
| WBVER-02 | `workbook://` URI round-trips carrying `mode`; stays ≤ `MAX_ENCODED_URI_LEN` (64 KiB) | property | `cargo test -p pmcp-server-toolkit render_uri` | ✅ extend `render_uri.rs` proptests (~line 248–288) |
| WBVER-02 | unknown mode → `Err` (never panic) | unit | `cargo test -p pmcp-server-toolkit workbook::handler` | ❌ W0 — new `render_workbook` handler test |
| WBVER-02 | per-mode render byte-determinism | property | `cargo test -p pmcp-workbook-runtime render::` | ✅ extend `render_xlsx_is_deterministic_byte_identical` (~line 522) per mode |
| WBVER-03 | golden bundle → `all_within_tol`; perturbed oracle → flagged mismatch | unit | `cargo test -p pmcp-workbook-runtime reconcile` | ❌ W0 — new `reconcile.rs` tests |
| WBVER-03 | `all_within_tol ⇔ every output within TOL` | property | `cargo test -p pmcp-workbook-runtime reconcile` | ❌ W0 |
| WBVER-03 | unknown tool filter → `Err` listing tools (D-03); empty oracle → vacuous within tol (D-04); A1 `cell` from `seed_coord` (D-01/D-02) | unit | `cargo test -p pmcp-server-toolkit workbook` | ❌ W0 — new `VerifyAccuracyHandler` tests |
| WBVER-04 | reader-free; no wire regression | gate | `make purity-check` + existing `workbook_integration.rs` | ✅ |
| WBVER-04 (D-06/D-07) | one example demos `render_workbook(filled)` / `render_workbook(inputs_only)` / `verify_accuracy` over the tax bundle (with added text+bool outputs) | example | `cargo run --example <workbook example> --features workbook-embedded -p pmcp-server-toolkit` | ✅ extend / add example |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `render/mod.rs` tests — InputsOnly no-`<v>` assertion; text/bool formula+result `<f>`+`<v>` assertion. **Needs an xlsx-XML-extraction helper** (unzip the in-memory buffer; current tests only check ZIP magic).
- [ ] `reconcile.rs` — new reader-free module with `ReconcileReport` / `reconcile_reference` + unit + property + perturbed-oracle tests.
- [ ] `render_uri.rs` — extend proptests for `mode` round-trip + 64 KiB size bound.
- [ ] `workbook/handler.rs` (or new file) — `VerifyAccuracyHandler` tests (D-03 unknown filter → Err, D-04 empty oracle vacuous, D-01/D-02 `cell` from `seed_coord`).
- [ ] **Fixture:** add a text output cell and a boolean output cell to `tax-calc@1.1.0` (`Calculate_Tax`), re-folding the integrity-locked `BUNDLE.lock` (all 5 artifacts: manifest/cell_map/executable.ir/layout/lock). Confirm generator vs hand-fold path (`build_bundle_lock`/`fold_evidence_hash`).
- [ ] Update `RESERVED_TOOL_NAMES` (runtime) + the H3 binding test (`handler.rs:671`) + the "five tools" doc/count in `workbook/mod.rs` for the 6th meta-tool.
- [ ] Doctests on `reconcile_reference` and `RenderMode` (CLAUDE.md: doctests on new public runtime fns).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Excel actually recomputes text/bool formula outputs on open via `fullCalcOnLoad="1"` | WBVER-01 | Requires a real Excel/LibreOffice client to open the produced `.xlsx`; not automatable in CI | Open the `filled` and `inputs_only` downloads in Excel; confirm all output types recompute and match. Covered structurally in CI by asserting `<f>` present + (filled) `<v>` cached / (inputs_only) `<v>` absent. |

*Automated tests cover the structural XML; the live-Excel recompute is the only manual leg and is asserted structurally in CI.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (reconcile.rs, InputsOnly test, handler tests, fixture re-fold, RESERVED_TOOL_NAMES/H3)
- [ ] No watch-mode flags
- [ ] Feedback latency < 120s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
