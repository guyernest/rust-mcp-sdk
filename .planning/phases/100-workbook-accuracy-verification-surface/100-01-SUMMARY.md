---
phase: 100-workbook-accuracy-verification-surface
plan: 01
subsystem: testing
tags: [workbook, fixtures, bundle-lock, integrity, rust_xlsxwriter, zip, reserved-tool-names]

# Dependency graph
requires:
  - phase: 96-shape-b-scaffold-dialect-version-declaration-generalization-validation
    provides: tax-calc@1.1.0 golden bundle, fixture_author/reemit_golden compiler regeneration path, render_xlsx
provides:
  - tax-calc@1.1.0 fixture extended with one text + one bool formula output cell (synthetic, re-folded BUNDLE.lock)
  - RESERVED_TOOL_NAMES bumped to [&str; 5] including verify_accuracy (runtime-leaf const, non-releasable groundwork)
  - five→six served-tool doc/count corrected in workbook/mod.rs
  - test-only sheet-XML extraction + cell-scoped slice helpers in render/mod.rs (cell-addressed <f>/<v> assertions)
affects: [100-02, 100-03, 100-04, 100-05]

# Tech tracking
tech-stack:
  added: [zip (dev-dependency, test-only, purity-safe) in pmcp-workbook-runtime]
  patterns:
    - "Synthetic-fixture provenance: workbook_hash denotes fixture identity; re-folded BUNDLE.lock attests artifact-set integrity, NOT source-.xlsx derivation"
    - "Cell-scoped XML assertion helper (cell_xml by A1 address) instead of brittle whole-sheet <f>/<v> counts"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0.PROVENANCE.md
  modified:
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/{manifest,executable.ir,cell_map,layout}.json
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock
    - crates/pmcp-workbook-runtime/src/manifest_model.rs
    - crates/pmcp-workbook-runtime/src/render/mod.rs
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - crates/pmcp-server-toolkit/src/workbook/mod.rs

key-decisions:
  - "Hand-folded BUNDLE.lock via the runtime build_bundle_lock/fold_evidence_hash helpers (driven, not hex-edited) rather than the compiler regeneration path — the reemit override is #[cfg(test)]-only in a different crate and regenerating would perturb the committed golden beyond the two added cells"
  - "Text output bracket_label = IF(taxable_income>=40000,\"bracket_2\",\"bracket_1\") ⇒ oracle \"bracket_2\"; bool output is_taxable = taxable_income>0 ⇒ oracle true (executor confirmed to return CellValue::Text/Bool, not Error)"
  - "H3 binding test uses a string-literal \"verify_accuracy\" placeholder (with a forward-reference // Why: note); Plan 04 swaps it for VerifyAccuracyHandler::NAME once the handler lands"
  - "zip added as a dev-dependency (test surface only) so make purity-check stays green"

patterns-established:
  - "Synthetic test-fixture provenance documented explicitly at the fixture (PROVENANCE.md) so the lock's meaning is unambiguous"
  - "Per-address cell XML slicing for formula-with-cached-result assertions, consumed by Plans 02/03"

requirements-completed: []  # Groundwork only — touches WBVER-01..04 prerequisites but completes none end-to-end (see objective). Plans 02-05 complete the requirements.

# Metrics
duration: ~33min (first→last task commit)
completed: 2026-06-23
---

# Phase 100 Plan 01: Wave-0 Prerequisites Summary

**Landed the cross-cutting groundwork for the three accuracy-verification items: a text+bool-bearing synthetic tax-calc@1.1.0 fixture with a re-folded integrity lock, the runtime-leaf verify_accuracy reservation, and a cell-scoped sheet-XML test helper.**

> ⚠ NON-RELEASABLE INTERMEDIATE STATE: the docs/constants reference `verify_accuracy` before its handler exists (handler lands in Plan 04). Do NOT ship the repo between this plan and Plan 04 completion. The H3 binding test binds a string literal, not `::NAME`, until Plan 04 Task 2 closes the drift.

## Performance

- **Duration:** ~33 min (first commit 17:36 → last commit 18:09)
- **Completed:** 2026-06-23
- **Tasks:** 3/3 committed
- **Files modified:** 17 (across 3 task commits)

## Accomplishments
- Extended the `tax-calc@1.1.0` synthetic fixture with one **text** formula output (`bracket_label`) and one **bool** formula output (`is_taxable`) on `Calculate_Tax`, edited all four data artifacts hash-consistently, and re-folded `BUNDLE.lock` via the runtime hashing helpers — the bundle boots with no integrity panic and the provstamp combined hash matches the lock.
- Documented synthetic-fixture provenance in a new `tax-calc@1.1.0.PROVENANCE.md`: `workbook_hash` denotes fixture provenance; the re-folded lock attests artifact-set integrity, not source-.xlsx derivation.
- Bumped `RESERVED_TOOL_NAMES` to `[&str; 5]` including `verify_accuracy`, updated the H3 binding test (documented placeholder), and corrected the five→six served-tool doc/count in `workbook/mod.rs` — runtime-leaf const only, no handler yet.
- Added a `#[cfg(test)]` `extract_sheet_xml` helper plus a cell-scoped `cell_xml` slice helper to `render/mod.rs`, self-verified to detect `<f>`/`<v>` on a specific known numeric formula cell by A1 address (ready for Plans 02/03). `zip` added test-only; purity gate unaffected.

## Task Commits

1. **Task 1: Extend tax fixture with text + bool formula outputs, re-fold BUNDLE.lock** — `968c5c95` (test)
2. **Task 2: Reserve verify_accuracy in RESERVED_TOOL_NAMES + H3 placeholder + five→six doc fix** — `9bd29318` (feat)
3. **Task 3: Cell-scoped sheet-XML extraction test helper** — `5f8ee0b5` (test)

## Verification

- `cargo test -p pmcp-server-toolkit --features workbook-embedded golden` → green (provstamp combined hash == BUNDLE.lock; fixture boots)
- `cargo test ... reserved_tool_names_match_the_registered_meta_tool_names` → ok (H3 binding matches the 5-element const)
- `cargo test -p pmcp-workbook-runtime render::` → 16 passed (incl. `extract_sheet_xml_locates_formula_and_value_on_a_specific_cell` and the text/bool render test)
- `make purity-check` → PASSED (reader-free; `zip` test-only)

## Notes / Recovery
- The executor agent completed all 3 task commits but the session hit an API socket error before writing this SUMMARY. The orchestrator closed the plan out manually: confirmed the 3 commits map 1:1 to the 3 tasks and re-ran all four plan verifications green before authoring this file.

## Next
Plan 100-02 (WBVER-01): make text & bool formula output cells render as formula-with-cached-result, using the new fixture cells and the cell-scoped XML helper to assert `<f>`+`<v>` on those output cells.
