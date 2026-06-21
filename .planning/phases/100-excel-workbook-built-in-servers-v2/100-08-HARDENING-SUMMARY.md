---
phase: 100-excel-workbook-built-in-servers-v2
plan: 08-HARDENING
subsystem: workbook-explain-parity-range-cross-sheet
tags: [gap-closure, pmcp-run-review, H1, WR-01, explain-parity, multi-sheet-fixture, test-hardening]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 08
    provides: "project_tool_surface_from_workbook (explain drives the production projection) + FreshnessPolicy::Preview + the build_tools-DIRECT synthetic WR-01 test + the template.xlsx parity test"
provides:
  - "author_multi_sheet_xlsx + MultiSheetSpec/SheetSpec — a #[cfg(test)] MULTI-SHEET fixture author (the additive sibling of author_xlsx) honouring per-table sheet so a fixture can carry a cross-sheet reference; byte-deterministic + ExcelTrusted, shared identity helpers with the single-sheet author"
  - "range_cross_sheet_spec — a REAL multi-sheet Table-authored fixture whose output reaches q1/q2 ONLY via SUM(B2:B3) and adjustment ONLY via cross-sheet Aux!B2 (cached <v>=350 reconciles); committed as range-cross-sheet.xlsx"
  - "explain_projection_matches_served_surface_over_range_and_cross_sheet — the load-bearing parity test: compiles the fixture through the production pipeline and asserts explain per-tool input_keys == served input_schema_for_tool keys, including BOTH the range and cross-sheet inputs (H1(d) met by construction)"
  - "explain_surfaces_range_and_cross_sheet_inputs — the cargo-pmcp CLI-render half: explain_workbook over the committed fixture surfaces both inputs"
affects: [workbook-explain-cli, workbook-explain-parity, fixture-author]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Multi-sheet authoring as an ADDITIVE sibling (author_multi_sheet_xlsx/MultiSheetSpec), NOT a field added to WorkbookSpec — every existing single-sheet fixture is byte-unchanged (zero regen churn), and the two authors share the deterministic-identity + per-sheet-write helpers"
    - "A REAL compiled fixture, not a synthetic build_tools call, proves the explain↔served parity over ranges/cross-sheet: the assertion runs THROUGH project_tool_surface_from_workbook (the explain projection) and compares to input_schema_for_tool (the served surface), so it fails if explain ever diverges on those formula shapes"

key-files:
  created:
    - crates/pmcp-workbook-compiler/tests/fixtures/range-cross-sheet.xlsx
    - crates/pmcp-workbook-compiler/tests/fixtures/range-cross-sheet.gen.json
    - crates/pmcp-workbook-compiler/tests/fixtures/range-cross-sheet.provenance-override.json
  modified:
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
    - crates/pmcp-workbook-compiler/src/template_compile_e2e.rs
    - cargo-pmcp/tests/workbook_explain.rs

key-decisions:
  - "The load-bearing explain↔served parity assertion lives in the compiler's #[cfg(test)] template_compile_e2e (NOT cargo-pmcp/tests/workbook_explain.rs as the objective named): only there are BOTH the trusted-fixture override compile (compile_workbook_with_fixture_override, #[cfg(test)]-only) AND the served input_schema_for_tool (pmcp-server-toolkit, which cargo-pmcp does not depend on) reachable — the SAME CR-01 reachability rule that placed the original H1 parity test there (100-08 deviation #4). cargo-pmcp's test crate gets the complementary explain-CLI assertion (both inputs surface), which is all explain_workbook can reach."
  - "Multi-sheet authoring is an ADDITIVE author_multi_sheet_xlsx + MultiSheetSpec, not a new field on WorkbookSpec: WorkbookSpec is built with ~9 explicit struct literals (no ..Default), so adding a field would churn every existing spec and risk a byte-regen of the committed corpus. The sibling author shares new_deterministic_workbook + write_sheet_contents, so both carry the identical deterministic ExcelTrusted identity. TableSpec.sheet is now LOAD-BEARING (consulted by the multi-sheet author's debug_assert_eq!), retiring its #[allow(dead_code)]."
  - "BOTH halves landed (the objective's preferred path), not the single-sheet minimum: a SUM(range)-reached input AND a cross-sheet-reached input through a real compile."

patterns-established:
  - "When a 'preview vs served' parity must cover a formula shape (range/cross-sheet) the shipped fixture doesn't have, author a REAL fixture in that shape and run BOTH the explain projection and the served schema over it — proving parity by construction rather than relying on a synthetic build_tools call + a projection-equivalence proptest."

requirements-completed: []

# Metrics
duration: ~55min
completed: 2026-06-20
---

# Phase 100 Plan 08-HARDENING: explain↔served parity over a real SUM(range)+cross-sheet workbook Summary

**The one non-blocking gap the pmcp.run dev team flagged on the H1 fix is closed: the explain↔served PARITY is now proven over a REAL `SUM(range)` + cross-sheet workbook compiled through the production pipeline, so H1(d) is met BY CONSTRUCTION rather than only by the projection-equivalence proptest. The `#[cfg(test)]` fixture author gained a MULTI-SHEET sibling (`author_multi_sheet_xlsx` + `MultiSheetSpec`/`SheetSpec`) that honours each table's `sheet` (retiring its `#[allow(dead_code)]`) while sharing the byte-deterministic `ExcelTrusted` identity helpers with the single-sheet `author_xlsx` — so every existing single-sheet fixture is byte-unchanged. A new `range_cross_sheet_spec` authors a genuine two-sheet, Table-authored workbook whose `Total_Sales` output reaches `q1`/`q2` ONLY via `SUM(B2:B3)` and `adjustment` ONLY via the cross-sheet `Aux!B2` ref (cached `<v>`=350 reconciles); it is committed as `range-cross-sheet.xlsx` with its gen + override sidecars. The load-bearing `explain_projection_matches_served_surface_over_range_and_cross_sheet` (compiler `template_compile_e2e`) compiles that fixture through the production pipeline and asserts the explain projection's per-tool `input_keys` EQUAL the served `input_schema_for_tool` keys, INCLUDING both the range-reached and cross-sheet-reached inputs — it fails if explain ever diverges from the served surface on ranges/cross-sheet. The complementary `explain_surfaces_range_and_cross_sheet_inputs` (cargo-pmcp) drives the `explain_workbook` CLI over the committed fixture and asserts both inputs surface. The synthetic `build_tools`-direct test (`cell_map.rs:876`) is kept as complementary. `make purity-check` GREEN (no `rust_xlsxwriter` in any prod build), `cargo fmt` clean, zero new clippy warnings on touched crates, zero PMAT cog-25 violations on touched `src/`.**

## Performance
- **Duration:** ~55 min
- **Tasks:** 2 (2 atomic commits)
- **Files:** 3 modified, 3 created (the committed fixture + 2 sidecars)

## Accomplishments

- **Task 1 (`69189424`) — multi-sheet author + fixture + load-bearing compiler parity test.**
  - Extended `fixture_author.rs` with `author_multi_sheet_xlsx` + `MultiSheetSpec`/`SheetSpec`, refactoring `author_xlsx` to share `new_deterministic_workbook` (the pinned-datetime identity) and `write_sheet_contents` (the per-sheet cell+table writer). `TableSpec.sheet` is now consulted (a `debug_assert_eq!` in `write_sheet_contents`) — its `#[allow(dead_code)]` is retired.
  - Authored `range_cross_sheet_spec`: sheet `Data` carries an `Inputs` Table (`q1`=100, `q2`=200, blue-font inputs) + an output Table `Total_Sales` whose `total` = `ROUND(SUM(B2:B3)+Aux!B2,0)`; sheet `Aux` carries an `Adjustments` Table (`adjustment`=50). Cached `<v>` = `ROUND(SUM(100,200)+50,0)` = 350 → the production reconcile grades the recomputation against it and the fixture compiles green via the trusted-fixture override.
  - Added `explain_projection_matches_served_surface_over_range_and_cross_sheet` (compiler `template_compile_e2e`): one output Table → one `total_sales` tool; the served `input_schema_for_tool` keys are exactly `{adjustment, q1, q2}`; the offline `project_tool_surface_from_workbook` projection's per-tool `input_keys` EQUAL those served keys (the load-bearing parity); output parity (`total`) holds.
  - Added 3 multi-sheet author self-tests (both sheets+their tables round-trip via umya, byte-determinism, direct `ExcelTrusted` classification) + a `write_multi_sheet_gen_metadata` sidecar writer. Committed `range-cross-sheet.xlsx` + `.gen.json` + `.provenance-override.json` via the env-gated `regenerate_fixtures` generator.
- **Task 2 (`953a63e7`) — cargo-pmcp explain CLI render half.** Added `explain_surfaces_range_and_cross_sheet_inputs` to `cargo-pmcp/tests/workbook_explain.rs`: drives the `explain_workbook` CLI entrypoint over the committed `range-cross-sheet.xlsx` (read-only `Preview` policy — no override marker needed) and asserts the `total_sales` tool surfaces BOTH the `SUM(range)` members (`q1`, `q2`) AND the cross-sheet input (`adjustment`), plus the single `total` output.

## Task Commits
1. **multi-sheet author + range_cross_sheet_spec + compiler parity test + committed fixture** — `69189424` (test)
2. **cargo-pmcp explain CLI surfaces range+cross-sheet inputs** — `953a63e7` (test)

## Decisions Made
- **The load-bearing parity assertion lives in the compiler's `template_compile_e2e`, not `cargo-pmcp/tests/workbook_explain.rs`** — only there are the `#[cfg(test)]`-only override compile AND the served `input_schema_for_tool` (pmcp-server-toolkit, not a cargo-pmcp dependency) BOTH reachable (the CR-01 reachability rule, the SAME placement 100-08 chose for the original H1 parity test). cargo-pmcp gets the complementary CLI assertion, all `explain_workbook` can reach.
- **Multi-sheet authoring is an additive sibling, not a `WorkbookSpec` field** — avoids churning ~9 explicit struct literals and a byte-regen of the committed corpus; the sibling shares the deterministic-identity helpers so it is as trusted/reproducible as the single-sheet author.
- **Both halves landed** (range + cross-sheet through a real compile), the objective's preferred outcome — the single-sheet fallback was not needed.

## Deviations from Plan

### Documented placement deviation

**1. The load-bearing parity test lives in `pmcp-workbook-compiler`'s `template_compile_e2e`, not `cargo-pmcp/tests/workbook_explain.rs`**
- **Reason:** The objective named `cargo-pmcp/tests/workbook_explain.rs` as the parity-test home, but a TRUE explain↔served parity assertion needs BOTH the production compile (the `#[cfg(test)]`-only `compile_workbook_with_fixture_override`) AND the served `input_schema_for_tool` (in `pmcp-server-toolkit`, which `cargo-pmcp` does not depend on). Neither is reachable from cargo-pmcp's external test crate. The compiler's `#[cfg(test)]` `template_compile_e2e` is where both ARE reachable — and where the original H1 parity test was placed (100-08 deviation #4, the same CR-01 reachability rule). The cargo-pmcp `workbook_explain` test gets the complementary `explain_workbook`-CLI assertion (both inputs surface), which is the strongest assertion that crate can express.
- **Files:** `crates/pmcp-workbook-compiler/src/template_compile_e2e.rs` (load-bearing), `cargo-pmcp/tests/workbook_explain.rs` (CLI half).
- **Committed in:** `69189424` / `953a63e7`.

### Honored constraints
- Production projection logic is UNTOUCHED (the projection was correct; this is test + fixture hardening only).
- `make purity-check` GREEN — `rust_xlsxwriter` stays confined to the `#![cfg(test)]` author; no reader/writer leaked into any prod build.
- The synthetic `build_tools_surfaces_range_and_cross_sheet_inputs` (`cell_map.rs:876`) is KEPT — complementary (a direct `build_tools` unit), not replaced.
- Pre-existing committed fixtures (`leap1900-probe`, `loan-calc`, the quirk corpus) were NOT changed — the env-gated regen also re-emitted them with non-identical bytes (an out-of-scope rust_xlsxwriter/toolchain drift), so they were reverted; only the NEW `range-cross-sheet.*` files are committed.
- Pre-existing unrelated working-tree changes (`pmcp-course/src/theme/*`, the `.agents/.codex/.serena/.pmat` noise) were left unstaged.

## Known Stubs
None — both tests are LIVE: the compiler parity test runs a full production compile + the served schema, and the CLI test drives the real `explain_workbook` entrypoint over the committed fixture.

## Threat Flags
None — no new network endpoint, auth path, or trust-boundary schema; this is test + test-fixture hardening over the existing (correct) projection.

## Self-Check: PASSED
- Created files verified present: `range-cross-sheet.xlsx` (+ `.gen.json`, `.provenance-override.json`) committed in `69189424` (git blob confirmed).
- Commits verified in git log: `69189424`, `953a63e7`.
- `cargo test -p pmcp-workbook-compiler`: 375 lib + 4 + 2 + 5 passed, 2 ignored, 0 failed (incl. the new parity test + 3 multi-sheet author self-tests).
- `cargo test -p cargo-pmcp --test workbook_explain`: 6 passed (incl. `explain_surfaces_range_and_cross_sheet_inputs`).
- `cargo fmt -p pmcp-workbook-compiler -p cargo-pmcp -- --check`: clean.
- `cargo clippy -p pmcp-workbook-compiler --all-targets` / `-p cargo-pmcp --tests`: zero warnings on touched files (deprecated `get_cell`→`cell`, doc-list-indentation fixed).
- `make purity-check`: PASSED (reader-free served cone; `rust_xlsxwriter` confined to the test author).
- PMAT complexity (cog ≤25) over touched `src/`: 0 violations.

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
