---
phase: 100-excel-workbook-built-in-servers-v2
plan: 01
subsystem: testing
tags: [rust_xlsxwriter, umya-spreadsheet, excel-tables, provenance, workbook-compiler, fixture-author]

# Dependency graph
requires:
  - phase: 96-excel-workbook-generalization
    provides: "fixture_author.rs reusable #[cfg(test)] rust_xlsxwriter author (genuine Excel identity, cached-<v> oracle, env-gated regenerate_fixtures)"
  - phase: 93-workbook-compiler
    provides: "provenance::gate::classify RAW classifier + quarantined raw_parts reader (umya-isolated compiler)"
provides:
  - "A shipped provenance-valid template.xlsx (Inputs Excel Table + 2 named output Tables + tier/enum dropdowns + currency/percent unit witnesses), the anchor fixture for Plans 02-05"
  - "A Table-emitting author surface (TableSpec/DataValidationSpec/DvKind/NumberFmt) extending fixture_author.rs"
  - "Public provenance::classify_xlsx_bytes (override-free RAW provenance classification)"
  - "template_provenance.rs CI test: RAW ExcelTrusted + no override sidecar + byte-identical copies + named tables/dropdowns"
affects: [harvest, multi-tool, dag-derived-inputs, workbook-explain, plan-02, plan-03, plan-04, plan-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Excel Tables (ListObjects) as the §3 declaration primitive authored via rust_xlsxwriter add_table/set_columns + add_data_validation"
    - "Deterministic .xlsx authoring via a pinned DocProperties creation datetime (core.xml-only; provenance identity untouched)"
    - "One canonical template + byte-for-byte copy with a CI byte-equality drift guard"
    - "Override-free RAW provenance assertion proves the AUTHORING path (not an override sidecar)"

key-files:
  created:
    - cargo-pmcp/src/templates/workbook_bundle/template.xlsx
    - cargo-pmcp/src/templates/workbook_bundle/template.gen.json
    - crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx
    - crates/pmcp-workbook-compiler/tests/template_provenance.rs
  modified:
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
    - crates/pmcp-workbook-compiler/src/provenance/mod.rs

key-decisions:
  - "Committed a GENERATED template (Pitfall 4 option b) — kept fixture_author #![cfg(test)], never pulled rust_xlsxwriter into a non-test build"
  - "NO provenance-override sidecar — the template is genuinely rust_xlsxwriter-authored and classifies RAW ExcelTrusted (review finding #5)"
  - "Added a public provenance::classify_xlsx_bytes so the integration test can assert override-free RAW classification (pub(crate) classify is unreachable from tests/)"
  - "Pinned a fixed DocProperties creation datetime to make regeneration byte-deterministic (Rule 1 bug fix for the must-have determinism property)"
  - "Added AuthoredCell::NumberFmt + TableSpec.body_rows as additive surface (zero churn to the existing leap/loan/quirk fixtures)"

patterns-established:
  - "Pattern 1: Table author surface — TableSpec{name,sheet,top_left,columns,caption,rows,body_rows,data_validations}; one helper fn per concern (write_table/write_caption/write_table_body/build_table/write_data_validations) to hold cognitive complexity <=25"
  - "Pattern 2: env-gated regenerate_template arm authors the canonical CLI copy, copies it byte-for-byte to tests/fixtures, writes a .gen.json sidecar, writes NO override sidecar"

requirements-completed: [WBV2-01]

# Metrics
duration: 45min
completed: 2026-06-20
---

# Phase 100 Plan 01: Table-Based Template Authoring Summary

**Shipped a provenance-valid, byte-deterministic `template.xlsx` (Inputs Excel Table with tier + sample enum dropdowns and currency/percent unit witnesses, plus Calculate_Tax/Estimate_Refund named output Tables with captions) authored entirely by rust_xlsxwriter, proven RAW ExcelTrusted with no override sidecar and byte-identical across both committed locations.**

## Performance

- **Duration:** ~45 min
- **Started:** 2026-06-20
- **Completed:** 2026-06-20
- **Tasks:** 2
- **Files modified:** 2 modified, 4 created

## Accomplishments
- Extended `fixture_author.rs` with a Table-emitting author surface (`TableSpec`, `DataValidationSpec`, `DvKind`, `NumberFmt`, `WorkbookSpec.tables`) that overlays `rust_xlsxwriter` Excel Tables with per-column headers, a caption-above-header, and `list` data-validation dropdowns — all test-covered by three umya round-trip self-tests, staying `#![cfg(test)]`.
- Authored and committed the §7 tax-suite `template.xlsx` in both consumer locations (CLI templates dir + compiler test fixtures), byte-identical, regeneratable via the env-gated `regenerate_template` arm with a `template.gen.json` reproducibility sidecar and NO provenance-override sidecar.
- Added a public `provenance::classify_xlsx_bytes` (the override-free RAW classification the production gate performs internally) and `tests/template_provenance.rs` asserting RAW `classify() == ExcelTrusted`, no override sidecar, byte-identical copies, and the named tables + list dropdowns present.

## Task Commits

Each task was committed atomically (TDD for Task 1: RED tests + GREEN impl committed together as a coherent module change):

1. **Task 1: Extend fixture_author with a Table-emitting surface** - `5d5e5d53` (feat)
2. **Task 2: Author + commit the shipped template.xlsx; assert RAW ExcelTrusted + byte-equal copies** - `43d85b98` (feat)

_Note: Task 1 followed the TDD flow (failing umya round-trip tests written first → confirmed RED compile failure → implemented the Table surface → GREEN), committed as one feat commit since the tests and types live in the same `#![cfg(test)]` module._

## Files Created/Modified
- `crates/pmcp-workbook-compiler/src/fixture_author.rs` - Added `TableSpec`/`DataValidationSpec`/`DvKind`/`AuthoredCell::NumberFmt`/`WorkbookSpec.tables`/`TableSpec.body_rows`, the `write_table`+helpers author path, a fixed-datetime `DocProperties` for deterministic output, `template_spec()`, the env-gated `regenerate_template` arm, and three Table round-trip self-tests.
- `crates/pmcp-workbook-compiler/src/provenance/mod.rs` - Added public `classify_xlsx_bytes(bytes) -> Result<ProvenanceClass, ProvenanceError>` (override-free RAW classification).
- `cargo-pmcp/src/templates/workbook_bundle/template.xlsx` - The shipped BA starting point + training artifact + honest reference fixture (canonical copy).
- `cargo-pmcp/src/templates/workbook_bundle/template.gen.json` - Regeneration metadata sidecar (generator fn, input cells B4/B6, formula oracles 18241/0.182/-3241).
- `crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx` - Byte-identical compiler test-fixtures copy.
- `crates/pmcp-workbook-compiler/tests/template_provenance.rs` - The RAW ExcelTrusted + no-override + byte-equality + tables/dropdowns CI test (5 tests).

## Decisions Made
- **Generated, not promoted (Pitfall 4 option b):** kept `fixture_author` `#![cfg(test)]` and committed a generated template + `.gen.json` sidecar rather than promoting `rust_xlsxwriter` into a non-test build (avoids the linker-pull-into-production landmine; resolves Assumption A2).
- **No override sidecar (review finding #5):** the template classifies RAW `ExcelTrusted` by its genuine `rust_xlsxwriter` `calcPr`/`app.xml` identity; an override would be unnecessary and would stop the test from proving the authoring path.
- **Canonical + copy (review finding #8):** the CLI-templates copy is canonical; the generator copies its exact bytes to the fixtures dir, with a CI byte-equality test guarding drift.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added a public `classify_xlsx_bytes` to reach RAW classification from the integration test**
- **Found during:** Task 2 (authoring `tests/template_provenance.rs`)
- **Issue:** The plan's acceptance criteria require the test to assert RAW `classify() == ExcelTrusted`, but `provenance::gate::classify` and `raw_parts::read_app_props`/`read_calc_pr` are all `pub(crate)` and the `#[cfg(test)]` `classify_authored` helper lives in `fixture_author.rs` — none reachable from a `tests/` integration crate. The public `gate()` requires a full `WorkbookMap` + `Manifest` and returns findings, not the class.
- **Fix:** Added `pub fn provenance::classify_xlsx_bytes(bytes) -> Result<ProvenanceClass, ProvenanceError>` composing the same raw reader + `classify` the production gate uses internally — an honest, override-free RAW classification entry point (also useful production API: a caller can check a `.xlsx`'s provenance without a full compile).
- **Files modified:** crates/pmcp-workbook-compiler/src/provenance/mod.rs
- **Verification:** `cargo test -p pmcp-workbook-compiler --test template_provenance` (5 tests green); zero clippy warnings.
- **Committed in:** `43d85b98` (Task 2 commit)

**2. [Rule 1 - Bug] Pinned a fixed DocProperties creation datetime for byte-deterministic regeneration**
- **Found during:** Task 2 (verifying the must-have "regeneratable deterministically")
- **Issue:** `rust_xlsxwriter`'s default `DocProperties` stamps `ExcelDateTime::utc_now()` into `docProps/core.xml`, so two regeneration runs produced different bytes — violating the reproducible-fixture determinism property (confirmed by an `unzip` diff isolating `core.xml`).
- **Fix:** `author_xlsx` now sets `DocProperties::new().set_creation_datetime(2026-01-01)` before save. This touches `core.xml` only; the provenance gate reads `app.xml` `<Application>`/`<AppVersion>` + `calcPr`, so the `ExcelTrusted` identity is untouched (re-verified). Two consecutive regenerations now produce identical SHA-256.
- **Files modified:** crates/pmcp-workbook-compiler/src/fixture_author.rs
- **Verification:** Regenerated twice → identical hashes; `template_provenance.rs` RAW ExcelTrusted still green; existing committed leap/loan/quirk fixtures NOT regenerated (Phase 96-05 "no edits to existing fixtures" held).
- **Committed in:** `43d85b98` (Task 2 commit)

**3. [Rule 2 - Missing Critical] Added `AuthoredCell::NumberFmt` for §3.3 number-format unit witnesses**
- **Found during:** Task 2 (authoring the Inputs table)
- **Issue:** The §7 template + must-have truth require `value` cells carrying their unit via number format (currency → USD, percent → rate) so the Plan-02 harvest can read units; the existing `AuthoredCell::Number` carries no format.
- **Fix:** Added an additive `AuthoredCell::NumberFmt { addr, value, paint, num_format }` variant (a distinct variant, not a field on `Number`, to keep the existing leap/loan/quirk fixtures byte-stable with zero churn). Also taught `write_gen_metadata` to record `NumberFmt` input cells.
- **Files modified:** crates/pmcp-workbook-compiler/src/fixture_author.rs
- **Verification:** `template.gen.json` records input cells B4/B6; income/withheld carry `$#,##0`, rate/ref carry `0.0%`; full crate test suite green.
- **Committed in:** `43d85b98` (Task 2 commit)

**4. [Rule 3 - Blocking] Added `TableSpec.body_rows` for the table-area span when the body lives in `WorkbookSpec::cells`**
- **Found during:** Task 2 (template tables overlay body cells authored in `cells`, not inline `rows`)
- **Issue:** `write_table` computed the ListObject last-row from `rows.len()`, but the template authors body cells via `WorkbookSpec::cells` (to co-locate the `0_meta` + reference regions), leaving `rows` empty → a 1-row (header-only) table area.
- **Fix:** Added a `body_rows: u32` field; `write_table` spans `header_row ..= header_row + max(body_rows, rows.len())`.
- **Files modified:** crates/pmcp-workbook-compiler/src/fixture_author.rs
- **Verification:** umya `tables()` reports the Inputs/Calculate_Tax/Estimate_Refund tables; round-trip + provenance tests green.
- **Committed in:** `43d85b98` (Task 2 commit)

---

**Total deviations:** 4 auto-fixed (2 blocking, 1 bug, 1 missing-critical)
**Impact on plan:** All four were necessary to satisfy the plan's own acceptance criteria (override-free RAW classification reachable from the test, deterministic regen, unit witnesses, correct table area). No scope creep — all confined to the two task files; the existing legacy fixtures were left untouched per the Phase 96-05 decision.

## Issues Encountered
- The integration-test visibility boundary (`pub(crate)` provenance internals unreachable from `tests/`) and the `utc_now()` non-determinism were both surfaced and resolved during Task 2 (see Deviations 1 & 2). The misordered `top_left` tuple in the first self-test draft (`(5,0)` vs the `(col,row)` field order) was caught by the RED caption test and fixed before GREEN.

## Threat Model Outcome
- **T-100-01 (Spoofing — provenance identity):** mitigated. `template_provenance.rs` asserts RAW `classify() == ExcelTrusted` with NO override sidecar; a umya-authored regression OR an override-masked classification fails the test.
- **T-100-02 (Tampering — fixture drift):** mitigated. The env-gated `regenerate_template` + committed `.gen.json` make the template reproducible (now byte-deterministic); a byte-equality test between the canonical CLI copy and the test-fixtures copy fails CI on drift.
- **T-100-SC (package installs):** accept — no package installs (rust_xlsxwriter 0.95 + umya 3.0 are existing workspace deps).

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- The anchor `template.xlsx` exists, is provenance-valid, and carries the §3 declaration Tables — Plan 02 (harvest) can now target a real `Inputs` Table + named output Tables via umya `get_tables()`/`data_validations()`.
- The legacy `tax-calc`/`leap1900` fixtures are deliberately left in place (removed in Plan 04 once the harvest+emit path no longer needs the legacy oracle).
- The `template.xlsx` does not yet compile end-to-end (no `in_*`/`out_*` defined names — by design; output keys come from table `name` columns starting in Plan 02). This is the intended state: this plan ships the artifact + the honest reference fixture, not a compile path.

## Self-Check: PASSED
- Created files verified present: template.xlsx (both locations), template.gen.json, template_provenance.rs, fixture_author.rs, provenance/mod.rs.
- Commits verified in git log: `5d5e5d53` (Task 1), `43d85b98` (Task 2).
- No `template.provenance-override.json` sidecar (confirmed absent).
- `cargo test -p pmcp-workbook-compiler`: 329 passed, 2 ignored; `cargo clippy -p pmcp-workbook-compiler --all-targets`: 0 warnings; rustfmt clean.

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
