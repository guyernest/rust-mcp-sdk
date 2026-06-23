---
phase: 100-workbook-accuracy-verification-surface
plan: 02
subsystem: workbook-runtime
tags: [workbook, render, rust_xlsxwriter, formula-cached-result, WBVER-01, fullCalcOnLoad]

# Dependency graph
requires:
  - phase: 100-workbook-accuracy-verification-surface
    plan: 01
    provides: tax-calc@1.1.0 text(bracket_label)+bool(is_taxable) formula outputs; cell-scoped extract_sheet_xml + cell_xml sheet-XML test helpers
provides:
  - write_formula_or_value helper (flat 4-arm (formula, fmt) dispatcher + typed per-value-type literal-writer closure) unifying the Number/Text/Bool formula-or-literal render path
  - text & bool formula output cells now emit <f>+<v> (formula-with-cached-result), so Excel fullCalcOnLoad can independently recompute ALL output types, not just numeric
  - write_number_literal (non-formula numeric literal path, extracted from the former write_number_cell)
affects: [100-03, 100-04, 100-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Formula-or-literal render: ONE shared 4-arm (cell.formula, fmt) helper takes a TYPED FnOnce literal-writer per value type; value-type knowledge stays in the caller's closure so the helper never `match`es CellValue (keeps it flat, under cog-25)"

key-files:
  created: []
  modified:
    - crates/pmcp-workbook-runtime/src/render/mod.rs

key-decisions:
  - "write_formula_or_value takes a typed FnOnce(&mut Worksheet, Option<&Format>) literal-writer (the non-formula arm) instead of branching on CellValue internally — the LOW review item's prescribed shape; the formula arm runs Formula::new(normalize_formula_for_writer(f)).set_result(cached_result) for both fmt variants"
  - "Number arm's non-finite guard (n.is_finite) is preserved BEFORE the formula write; the former write_number_cell is replaced by write_number_literal (the non-formula numeric closure target) since the formula path now lives in the shared helper"
  - "Bool cached result is the Excel boolean literal TRUE/FALSE (set_result), while the NON-formula bool literal stays byte-identical b.to_string() (\"true\"/\"false\") to avoid regressing the existing non-formula bool test"

requirements-completed: [WBVER-01]

# Metrics
duration: ~12min
completed: 2026-06-23
---

# Phase 100 Plan 02: Text/Bool Formula Outputs Render as Formula-With-Cached-Result (WBVER-01) Summary

**Generalized the proven numeric `Formula::set_result` branch into a shared `write_formula_or_value` helper (a flat 4-arm `(formula, fmt)` dispatcher fed a typed per-value-type literal-writer closure) and routed the Text/Bool arms of `write_computed_value` through it — so text and boolean formula output cells now emit `<f>`+`<v>` exactly like numeric ones, closing the verification blind spot where Excel's `fullCalcOnLoad` could not recompute non-numeric outputs.**

## Performance

- **Duration:** ~12 min (RED commit → SUMMARY)
- **Completed:** 2026-06-23
- **Tasks:** 1/1 (TDD: RED + GREEN; no REFACTOR needed)
- **Files modified:** 1

## Accomplishments

- **RED:** Added `render_xlsx_text_and_bool_formula_cells_carry_f_and_v_per_cell` — renders a text formula output (`B6` = `IF(...,"bracket_2","bracket_1")`, oracle `"bracket_2"`) and a bool formula output (`B7` = `taxable_income>0`, oracle `true`), then asserts BOTH `<f>` AND `<v>` appear WITHIN each cell's own `<c>` slice (located by A1 via the Plan-01 `extract_sheet_xml`+`cell_xml` helpers, not a whole-sheet count). Also added a no-regression test that non-formula text/bool cells stay plain literals (no `<f>`). Confirmed failing: the Text arm rendered `<c r="B6" t="s"><v>0</v></c>` — a shared-string literal with no formula.
- **GREEN:** Factored `write_formula_or_value<W: FnOnce>` — a flat 4-arm `(formula, fmt)` match. The two `Some(f)` arms write `Formula::new(normalize_formula_for_writer(f)).set_result(cached_result)` (with/without format); the `(None, _)` arm invokes the caller-supplied typed literal-writer closure. No in-helper `match CellValue` (LOW review item), so it stays a flat dispatcher under cog-25.
- Re-expressed all three formula-bearing value types through the helper:
  - **Number** → cached result `format_number(n)`; literal closure `write_number_literal`. The `n.is_finite()` guard (T-12-05) is preserved BEFORE the formula write.
  - **Text** → cached result `s` verbatim; literal closure `write_string_cell`.
  - **Bool** → cached result `"TRUE"`/`"FALSE"`; literal closure writes the byte-identical `b.to_string()` (`"true"`/`"false"`) plain literal.
- `write_computed_value` stays a thin dispatcher (each arm computes its cached-result string + literal closure and delegates); the `_ => cell.value` fallback and non-finite guard are intact. `deny(panic/unwrap/expect)` upheld — writer errors propagate via `.map_err(writer_err)?`.

## Task Commits

1. **Task 1 (RED): failing per-cell `<f>`+`<v>` assertions for text/bool formula outputs** — `8c9b5a53` (test)
2. **Task 1 (GREEN): route text/bool formula outputs through write_formula_or_value** — `88ed730d` (feat)

## Verification

- `cargo test -p pmcp-workbook-runtime render::` → **18 passed** (was 16; +2 new tests). Includes the new per-cell text/bool `<f>`+`<v>` assertion, the non-formula no-regression assertion, and the pre-existing `render_xlsx_writes_text_and_bool_and_falls_back_on_error_value` (632) + `render_xlsx_writes_formula_with_finite_cached_result` (577).
- `cargo test -p pmcp-workbook-runtime` (full crate) → **178 passed** (2 suites) — no regressions.
- `grep -c 'write_formula_or_value'` → 5 (definition + 3 calls + 1 doc reference) ≥ 2.
- `pmat analyze complexity --max-cognitive 25` → **no violations** on `render/mod.rs` (neither `write_computed_value` nor `write_formula_or_value`).
- `make purity-check` → **PASSED** (writer-only change; rust_xlsxwriter present, readers absent, cargo-deny bans clean).
- Pre-commit `make quality-gate` hook ran on the GREEN commit (no `--no-verify`) → passed.

## Deviations from Plan

None — plan executed as written (TDD RED→GREEN; REFACTOR not needed, the helper was clean on first write).

## Deferred Issues

- **Pre-existing clippy lint (out of scope):** `crates/pmcp-workbook-runtime/src/render/mod.rs:~944` has `clippy::unnecessary_map_or` (`.map_or(false, |rest| ...)` → `.is_some_and(...)`). This line is NOT in the 100-02 diff (pre-dates this plan). `pmcp-workbook-runtime` is not covered by `make lint` (which lints only root `pmcp --features full`), so it does not block the CI clippy gate. Logged to `deferred-items.md`. Per the executor SCOPE BOUNDARY rule, only issues directly caused by this task's changes are auto-fixed.

## Known Stubs

None.

## Notes

- Threat register dispositions held: T-100-04 (DoS / non-finite) mitigation preserved — the `n.is_finite()` guard runs BEFORE the formula write and surfaces malformed values as `Err`, never a panic. T-100-03 (Tampering) accepted — the cached text/bool result is the executor's own computed value, the value path is identical to the proven numeric branch, no new trust boundary. No new packages (T-100-SC n/a).
- ⚠ NON-RELEASABLE INTERMEDIATE STATE persists from Plan 01: docs/constants still reference `verify_accuracy` before its handler exists (handler lands in Plan 04). Do NOT ship the repo between Plan 01 and Plan 04 completion.

## Next

Plan 100-03 (WBVER-02): `inputs_only` render mode — the next leg of the accuracy-verification surface, reusing the same cell-scoped XML assertion approach.
