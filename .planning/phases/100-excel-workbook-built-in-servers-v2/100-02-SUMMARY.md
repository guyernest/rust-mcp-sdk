---
phase: 100-excel-workbook-built-in-servers-v2
plan: 02
subsystem: compiler-ingest
tags: [umya-spreadsheet, excel-tables, harvest, catch_unwind, panic-containment, proptest, cargo-fuzz, workbook-compiler]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 01
    provides: "provenance-valid template.xlsx (Inputs/Calculate_Tax/Estimate_Refund Excel Tables + tier/enum dropdowns + currency/percent unit witnesses) committed in tests/fixtures + the Table author surface"
  - phase: 93-workbook-compiler
    provides: "umya-isolated ingest::ingest + owned cell_map (SheetRecord/CellRecord/DataValidationRecord) + synth.rs apply_dv_fork/freeze_or_reason DV→enum machinery"
provides:
  - "TableRecord{name,area,columns} harvested onto SheetRecord — the §4 tool-name + §3.2 schema raw material (Plan 03/04 consume it)"
  - "A panic-containment seam (read_workbook_contained) mapping umya's malformed-table-XML panic to a clean IngestError::MalformedTable (T-100-03 DoS)"
  - "Pure §3.3 per-row projectors (number_format_to_unit/harvest_dtype/harvest_tier) + harvest_input_row/harvest_output_row/harvest_allowed_values"
  - "A fuzz target (workbook_table_ingest) + a property test + a real-template e2e — the full ALWAYS set for WBV2-02"
affects: [manifest-model-lift, multi-tool, dag-derived-inputs, plan-03, plan-04]

# Tech tracking
tech-stack:
  added:
    - "proptest 1.7 (dev-dep) — the WBV2-02 PROPERTY harness"
  patterns:
    - "catch_unwind containment of an EAGER umya read: umya parses xl/tables/tableN.xml during reader::xlsx::read and .unwrap()s on malformed XML, so the read (not the accessor) is the panic origin — wrap reader::xlsx::read, suppress the panic hook for the contained span, map a caught panic to a typed error"
    - "Pure, CLOSED-codomain projectors (number_format_to_unit → {USD,rate,date,None}; harvest_tier → {strict,variable}) so the projection is property-provable total + stable"
    - "ENUM harvest reuses the EXISTING freeze_or_reason DV machinery (no copy) via a pub harvest_allowed_values entry"

key-files:
  created:
    - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/workbook_table_ingest.rs
    - crates/pmcp-workbook-compiler/fuzz/corpus/workbook_table_ingest/malformed-table-xlsx
    - crates/pmcp-workbook-compiler/tests/harvest_roundtrip_prop.rs
    - crates/pmcp-workbook-compiler/tests/template_harvest_e2e.rs
  modified:
    - crates/pmcp-workbook-compiler/src/ingest/cell_map.rs
    - crates/pmcp-workbook-compiler/src/ingest/mod.rs
    - crates/pmcp-workbook-compiler/src/manifest/synth.rs
    - crates/pmcp-workbook-compiler/fuzz/Cargo.toml
    - crates/pmcp-workbook-compiler/Cargo.toml

key-decisions:
  - "The catch_unwind seam wraps the EAGER reader::xlsx::read, NOT just the accessor span — umya parses table XML during read and panics there (Rule 1 bug: the plan's premise that the panic originates in table_records was wrong for umya 3.0.0)"
  - "IngestError::MalformedTable is the new typed error (lib.rs already maps IngestError → CompileError::Ingest); kept the worksheet-accessor catch_unwind (extract_tables_contained) as belt-and-suspenders"
  - "tables: Vec<RangeRef> KEPT additive alongside the new table_records: Vec<TableRecord> (no breaking change to area-only consumers)"
  - "Strict tier → Role::Constant + tier None (the is_strict_constant shape); variable → Role::Input + InputTier::Variable{default} — maps the {variable,strict} dropdown onto the existing runtime tier model rather than a new enum field"
  - "Used the non-deprecated umya accessors t.name()/t.columns()/c.name() (get_name/get_columns are #[deprecated] and would trip the deny gate)"

patterns-established:
  - "Pattern: a quiet-hook catch_unwind seam (take_hook → set no-op hook → catch_unwind → restore hook) that contains a third-party .unwrap() panic to a typed error without leaking an internal backtrace"
  - "Pattern: force-add a NAMED seed corpus entry (corpus/ is gitignored) so the corrupted-tableN.xml path is reproducibly reached on the first fuzz run"

requirements-completed: [WBV2-02]

# Metrics
duration: ~55min
completed: 2026-06-20
---

# Phase 100 Plan 02: Harvest Excel Tables (type/unit/enum/tier) Summary

**Ingest now HARVESTS each Excel Table's name + column headers into owned `TableRecord`s and projects each row's type/unit/enum/tier per §3.3 — with umya's malformed-table-XML `.unwrap()` panic contained at a `catch_unwind` seam around the eager `reader::xlsx::read`, proven by a fuzz target, a property test (total/stable/closed), and a real-`template.xlsx` end-to-end integration test.**

## Performance

- **Duration:** ~55 min
- **Tasks:** 5
- **Files:** 5 modified, 4 created

## Accomplishments

- **Task 1 — Table harvest + panic seam:** Added owned `TableRecord{name,area,columns}` (reader-free serde set) + a sibling `table_records: Vec<TableRecord>` on `SheetRecord`; `table_records()` harvests `t.name()`/`t.columns()`/`t.area()` (non-deprecated umya 3.0.0 accessors). Added `IngestError::MalformedTable` and the `read_workbook_contained` `catch_unwind` seam wrapping the eager `reader::xlsx::read` (umya parses table XML there and `.unwrap()`s on malformed parts), mapping a caught panic to a clean `IngestError` (→ `CompileError::Ingest`). Threaded the new field through 12 SheetRecord test-helper literals.
- **Task 2 — Per-row §3.3 projection:** Added pure projectors `number_format_to_unit` (CLOSED codomain {USD,rate,date,None}; percent-before-currency precedence), `harvest_dtype`, `harvest_tier` (TOTAL+CLOSED {strict,variable}), plus `harvest_input_row`/`harvest_output_row` building a `CellRole` (strict→`Role::Constant`+tier None; variable→`Role::Input`+`InputTier::Variable{default}`; outputs never tiered). Enum reuses the existing `freeze_or_reason` machinery. Additive — coexists with the named-range synth path during the Plan 02→04 transition.
- **Task 3 — Fuzz target:** `workbook_table_ingest` feeds arbitrary bytes into `ingest::ingest` and asserts it always returns a `Result`, never panics — the proof of the catch_unwind seam. Force-added a named malformed-table xlsx seed (real template with corrupted `xl/tables/*.xml`). 20 000 runs in ~8 s, zero crashes; corpus replay over all 586 seeds (incl the 7679-byte malformed-table xlsx) clean.
- **Task 4 — Property test:** `harvest_roundtrip_prop` proves totality (defined dtype + tier, no panic), stability (harvest-twice equality), unit closure, and tier closure over arbitrary well-formed rows (≥256 cases each, 4 properties).
- **Task 5 — Real-template e2e:** `template_harvest_e2e` ingests the committed `template.xlsx` via the real `ingest::ingest` (no hand-built records) and asserts income=Number/USD, filing enum=[single,married], withheld=Number/USD, rate=rate-unit + strict→Constant, each input description, the Calculate_Tax/Estimate_Refund name + caption→tool-description linkage, and per-output dtype + cached-`<v>` oracle (18241/0.182/-3241). Added `pub harvest_allowed_values` reusing `freeze_or_reason`.

## Task Commits

1. **Task 1: TableRecord harvest + catch_unwind seam** — `71d9ec1e` (feat)
2. **Task 2: per-row type/unit/enum/tier projection** — `a59588a2` (feat)
3. **Task 3: malformed-table fuzz target + seed** — `b9ec4f4e` (test)
4. **Task 4: harvest totality/stability/closure property test** — `c8ecb45e` (test)
5. **Task 5: real-template e2e + caption→tool linkage** — `3a0b3d53` (test)

## Files Created/Modified

- `src/ingest/cell_map.rs` — `TableRecord` struct + `table_records` field on `SheetRecord` (+ owned-only serde round-trip test).
- `src/ingest/mod.rs` — `IngestError::MalformedTable`, `read_workbook_contained` (the load-bearing catch_unwind seam over the eager read), `table_records()` (non-deprecated accessors), `extract_tables_contained` (belt-and-suspenders accessor seam); harvest/zero-table/corrupted-table unit tests.
- `src/manifest/synth.rs` — `number_format_to_unit`/`harvest_dtype`/`harvest_tier`/`HarvestedTier`/`HarvestRow`/`harvest_input_row`/`harvest_output_row`/`row_default`/`harvest_allowed_values` + 6 harvest unit tests.
- `fuzz/fuzz_targets/workbook_table_ingest.rs` + `fuzz/Cargo.toml` + `fuzz/corpus/workbook_table_ingest/malformed-table-xlsx` — the fuzz target, its `[[bin]]`, and the named malformed-table seed.
- `tests/harvest_roundtrip_prop.rs` — the proptest property harness.
- `tests/template_harvest_e2e.rs` — the real-template integration test.
- `Cargo.toml` — proptest 1.7 dev-dep.

## Decisions Made

- **The panic originates in the EAGER read, not the accessor** (Rule 1 bug, see Deviations): the plan assumed umya panics inside `table_records` (the accessor span); a RED unit test proved umya `.unwrap()`s during `reader::xlsx::read` itself (it parses `xl/tables/tableN.xml` eagerly). The load-bearing `catch_unwind` therefore wraps the read. Both seams remain (read + accessor) so the contract holds regardless of where a future umya version panics.
- **`tables` kept additive** alongside `table_records` — no breaking change to area-only consumers.
- **Tier maps onto the existing runtime model** — strict→untiered `Role::Constant` (the `is_strict_constant` shape), variable→`Role::Input`+`InputTier::Variable{default}` — rather than introducing a new tier enum.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] The catch_unwind seam must wrap the eager `reader::xlsx::read`, not `table_records`**
- **Found during:** Task 1 (the corrupted-tableN.xml unit test)
- **Issue:** The plan's `<action>` (and PATTERNS §4) specified wrapping the umya table-extraction span inside `table_records`/`extract_tables_contained`. But the RED test panicked at `umya-spreadsheet-3.0.0/src/reader/xlsx/table.rs:176` DURING `reader::xlsx::read(path)` — umya parses every `xl/tables/tableN.xml` eagerly at read time and `.unwrap()`s there, BEFORE any worksheet accessor runs. A `catch_unwind` only around the accessor span never executes, so the panic would still abort.
- **Fix:** Added `read_workbook_contained` wrapping `reader::xlsx::read` in `catch_unwind` (with a temporary no-op panic hook to suppress the internal backtrace), mapping a caught panic to `IngestError::MalformedTable`. Kept `extract_tables_contained` as a belt-and-suspenders accessor-span seam so the contract holds if a future umya version defers table parsing.
- **Files modified:** crates/pmcp-workbook-compiler/src/ingest/mod.rs
- **Verification:** `corrupted_table_xml_returns_clean_ingest_error_not_a_panic` (unit) + the `workbook_table_ingest` fuzz target (20k runs + 586-seed corpus replay) both green — no panic crosses the boundary.
- **Committed in:** `71d9ec1e` (Task 1)

**2. [Rule 3 - Blocking] Used non-deprecated umya accessors (`name()`/`columns()`/`name()`) instead of `get_name()`/`get_columns()`**
- **Found during:** Task 1 (first build)
- **Issue:** The plan's `<action>` named `t.get_name()`/`t.get_columns()`/`c.get_name()`, but umya 3.0.0 marks those `#[deprecated]` ("Use name()/columns()"). The crate's zero-warning clippy/build gate (and `#[warn(deprecated)]`) would fail the pre-commit hook.
- **Fix:** Switched to the non-deprecated `t.name()`/`t.columns()`/`c.name()` (identical semantics).
- **Files modified:** crates/pmcp-workbook-compiler/src/ingest/mod.rs
- **Verification:** `cargo clippy -p pmcp-workbook-compiler --all-targets` zero warnings.
- **Committed in:** `71d9ec1e` (Task 1)

**3. [Rule 2 - Missing Critical] Added `pub harvest_allowed_values` so the e2e can read the enum domain off the harvested map**
- **Found during:** Task 5 (authoring the e2e)
- **Issue:** The plan's enum harvest reuses `apply_dv_fork`/`freeze_or_reason`, but both are private `fn` unreachable from a `tests/` integration crate; `apply_dv_fork` also mutates a `CellRole` rather than returning the enum. The e2e needs to assert the frozen `[single,married]` domain from the real file.
- **Fix:** Added `pub fn harvest_allowed_values(sheet, value_addr, dtype, wb) -> Option<Vec<String>>` composing the EXACT same `addr_in_range` + `freeze_or_reason` the DV fork uses (a thin, test-visible entry — also a useful production API for reading an enum domain without a full CellRole).
- **Files modified:** crates/pmcp-workbook-compiler/src/manifest/synth.rs
- **Verification:** `template_harvest_e2e` asserts filing enum=[single,married] from the real template; full crate suite green.
- **Committed in:** `3a0b3d53` (Task 5)

---

**Total deviations:** 3 auto-fixed (1 bug, 1 blocking, 1 missing-critical). All confined to the plan's own task files; no scope creep. The Rule-1 fix is material — it relocates the load-bearing containment seam to where umya actually panics, which the fuzz target now proves.

## Threat Model Outcome

- **T-100-03 (DoS — malformed table XML umya panic):** mitigated. `read_workbook_contained` `catch_unwind` maps the eager-read panic to `IngestError::MalformedTable` (→ `CompileError::Ingest`); the `workbook_table_ingest` fuzz target proves clean-error-not-panic over 20k runs + a corrupted-table seed; a unit test feeds a corrupted `tableN.xml`.
- **T-100-04 (DoS — zip-bomb/oversized):** untouched and still in force (`MAX_CELL_COUNT` guard + quarantined provenance reader).
- **T-100-05 (boundary breach — umya leak):** mitigated. `TableRecord` holds only owned `String`/`RangeRef`; `make purity-check` PASSED (no umya in the runtime/served cones).
- **T-100-SC (package installs):** proptest 1.7 is an existing workspace dev-dep (not a new external package); umya 3.0.0 is an existing compiler-only dep. No package legitimacy concern.

## Known Stubs

None — the harvest projectors are wired to real data and proven against the real `template.xlsx`. The table-row harvest projection coexists additively with the named-range synth path by design (Plan 04 retires `promote_named_outputs`/`name_named_inputs`); this is documented, not a stub.

## User Setup Required

None.

## Next Phase Readiness

- The harvested `TableRecord` name/columns + the per-row §3.3 projection are the raw material Plan 03 (manifest model → multi-tool) and Plan 04 (multi-tool emission + DAG-derived inputs) consume.
- The pure projectors (`number_format_to_unit`/`harvest_dtype`/`harvest_tier`/`harvest_allowed_values`) and the `HarvestRow`/`harvest_input_row`/`harvest_output_row` entries are `pub` and ready for the lib.rs orchestration re-wire (PATTERNS §7).
- The caption→tool-description linkage is established (the cell directly above each output Table) and proven from the real file — Plan 04 can lift it directly.

## Self-Check: PASSED

- Created files verified present: workbook_table_ingest.rs, malformed-table-xlsx (seed), harvest_roundtrip_prop.rs, template_harvest_e2e.rs.
- Commits verified in git log: `71d9ec1e`, `a59588a2`, `b9ec4f4e`, `c8ecb45e`, `3a0b3d53`.
- `cargo test -p pmcp-workbook-compiler`: 334 lib + 4 property + 2 e2e + 5 provenance, 0 failed.
- `cargo clippy -p pmcp-workbook-compiler --all-targets`: 0 warnings; rustfmt clean.
- `cargo +nightly fuzz run workbook_table_ingest -- -runs=20000 -max_total_time=60`: no crash; corpus replay clean.
- `make purity-check`: PASSED (no umya leak from TableRecord).

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
