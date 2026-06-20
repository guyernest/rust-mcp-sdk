---
phase: 100-excel-workbook-built-in-servers-v2
plan: 07
subsystem: workbook-compiler-multi-tool-production-wiring
tags: [gap-closure, multi-tool, build_tools, harvest-promotion, collision-gate, per-tool-reconcile, cr-01, cr-02, e2e-proof, reemit-tightening]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 03
    provides: "build_tools / OutputTable / Dag::upstream_input_leaves — the per-Table fan-out primitive"
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 04
    provides: "reconcile_tools / tool_name_collision_findings / sanitize_tool_name / the two-Table served golden — built but never wired into production"
provides:
  - "emit_bundle wires the build_tools multi-tool fan-out (WBV2-04) when a NON-EMPTY OutputTable set is supplied, with a single-tool build_cell_map fallback when empty (the named-range corpus)"
  - "promote_harvested_tables — the ADDITIVE harvest-driven role promotion that lets a Table-authored workbook (template.xlsx) flow through the production refuse/reconcile/emit gates alongside the surviving named-range promotions (Rule-4 deferral honored)"
  - "output_tables_from_harvest — per-Table OutputTable membership derived from harvested table_records + the role-promoted manifest"
  - "the stage-1 tool-name-collision gate (T-100-17) + per-tool reconcile (WBV2-05) on the production compile path (CompileError::Lint / CompileError::Reconcile)"
  - "input_schema_for_tool full-pool fallback for empty input_keys (CR-02 closed — the served schema is never stricter than validate_input)"
  - "the authoritative WBV2-04 real-compile E2E proof (template_compile_e2e) + a tightened reemit_golden that fails a single-tool/empty-keys regression"
affects: [served-workbook-tools, workbook-compile-cli, reemit-golden-proof]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ADDITIVE harvest promotion: promote_harvested_tables sits beside promote_named_outputs/name_named_inputs (NOT a replacement) so the Table-authored AND named-range paths coexist — Rule-4 deferral (no named-range removal) honored while WBV2-04 reaches production"
    - "Empty-set fallback as the corpus bridge: an empty OutputTable set routes emit_bundle to the transitional single-tool build_cell_map, keeping tax-calc/loan-calc/leap1900 green with zero re-authoring"
    - "Defense-in-depth schema fallback: input_schema_for_tool projects the full shared-input pool when input_keys is empty, so a hand-built/fallback bundle's served schema is never stricter than the runtime (CR-02)"

key-files:
  created:
    - crates/pmcp-workbook-compiler/src/template_compile_e2e.rs
  modified:
    - crates/pmcp-workbook-compiler/src/artifact/mod.rs
    - crates/pmcp-workbook-compiler/src/artifact/cell_map.rs
    - crates/pmcp-workbook-compiler/src/gate/accept.rs
    - crates/pmcp-workbook-compiler/src/lib.rs
    - crates/pmcp-workbook-compiler/src/prepare_candidate_tests.rs
    - crates/pmcp-workbook-compiler/src/reemit_golden.rs
    - crates/pmcp-workbook-compiler/src/fixture_author.rs
    - crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx
    - crates/pmcp-server-toolkit/src/workbook/schema.rs
    - cargo-pmcp/src/commands/workbook/compile.rs
    - cargo-pmcp/src/templates/workbook_bundle/template.xlsx
    - cargo-pmcp/src/templates/workbook_bundle/template.gen.json

key-decisions:
  - "Wired build_tools via a cell_map_for_emit branch inside emit_bundle (non-empty OutputTable set → fan-out, empty → build_cell_map fallback); PromoteInputs gained output_tables + dag borrowed fields, Candidate gained an owned output_tables Vec, so both the seed lane (compile_workbook_inner) AND the gated-update CLI lane fan out identically"
  - "ADDED promote_harvested_tables (harvest-driven role promotion) — Rule 2/Rule 3 deviation, NOT the Rule-4 named-range retirement: template.xlsx could not compile at all before (its inputs/outputs are Table rows with no in_*/out_* named ranges, so refuse_uncallable_inputs blocked and no cell was ever Role::Output). This is the missing piece that makes WBV2-04 reach production; named-range promotions survive untouched"
  - "Rule 1 fix: template.xlsx was self-INCONSISTENT — tax_owed formula ROUND(B4*G3-1759,0) recomputed to 20241 while the cached <v> oracle is 18241, so the production reconcile correctly refused it. Corrected the constant to -3759 (100000*0.22-3759 = 18241) so recomputation == the authored oracle; regenerated both template.xlsx copies + gen.json + the cargo-pmcp embedded mirror via the env-gated regenerate_template"
  - "Placed the E2E proof as an in-src #[cfg(test)] module template_compile_e2e (NOT the frontmatter's external tests/template_compile_e2e.rs) — the same CR-01 reachability reason that places reemit_golden in src/: compile_workbook_with_fixture_override is #[cfg(test)]-only and invisible to an external integration crate"

patterns-established:
  - "Table area → value-column cells: the value column is one right of the name column (next_col over the area.start), body rows run header+1..=area.end.row — the membership grouping output_tables_from_harvest + promote_harvested_tables share"

requirements-completed: [WBV2-04]

# Metrics
duration: ~95min
completed: 2026-06-20
---

# Phase 100 Plan 07: Wire the WBV2-04 Multi-Tool Fan-out into Production Summary

**The Phase-100 BLOCKER gap is closed: a REAL compile of the Table-authored `template.xlsx` now emits exactly TWO MCP tools — `calculate_tax` (`input_keys: [income]`) and `estimate_refund` (`input_keys: [income, withheld]`) — each with a DAG-derived, populated, disjoint `input_keys` and a non-empty served input schema. `emit_bundle` reaches `build_tools` on a non-test call site (CR-01 closed); `tool_name_collision_findings` + `reconcile_tools` are folded into the production stage-1/reconcile gates; `input_schema_for_tool` advertises the full pool when `input_keys` is empty (CR-02 closed); and `reemit_golden` now asserts exact golden tool count + populated keys plus a positive fresh-template multi-tool compile, so a single-tool / empty-input_keys regression can never pass the proof again. The named-range corpus (tax-calc/loan-calc/leap1900) stays green via the empty-set single-tool fallback; `make purity-check` is green; zero clippy warnings; zero PMAT cog-25 violations.**

## Performance
- **Duration:** ~95 min
- **Tasks:** 3 (committed as 2 atomic commits — Tasks 1-2 are coupled through the shared PromoteInputs/Candidate types)
- **Files:** 1 created, 12 modified

## Accomplishments

- **Tasks 1-2 (`2c7b1f95`)** — `emit_bundle` gained `output_tables: &[OutputTable]` + `dag: &Dag` params and a `cell_map_for_emit` branch: non-empty → `build_tools` fan-out (WBV2-04), empty → `build_cell_map` fallback. `PromoteInputs` threads both fields; `cell_map.rs` exposes `shared_inputs`/`entry` as `pub(crate)` (one source for the shared-input pool). `compile_workbook_inner` + `prepare_candidate_inner` now: (a) ADDITIVELY promote a Table-authored workbook's harvested Tables (`promote_harvested_tables`) so its input rows get callable names and its output formulas become `Role::Output`; (b) derive `output_tables_from_harvest` from `table_records` + the role-promoted manifest; (c) fold collision Errors into the stage-1 gate (`refuse_colliding_output_tables` → `CompileError::Lint`); (d) reconcile each derived tool against its own oracle (`reconcile_output_tables` → `CompileError::Reconcile`). The gated-update CLI lane carries the membership via a new `Candidate.output_tables` field. New unit tests: `emit_with_output_tables_fans_out`, `output_tables_from_harvest_groups_output_cells`, `colliding_output_tables_block_compile_path`, `distinct_output_tables_do_not_block`.
- **Task 3 (`c44f9669`)** — (a) `input_schema_for_tool` projects the full shared-input pool when `tool.input_keys.is_empty()` (CR-02 defense-in-depth) + toolkit tests `empty_input_keys_projects_full_pool` / `populated_input_keys_projects_only_reached`. (b) The authoritative WBV2-04 proof: in-`src` `#[cfg(test)] mod template_compile_e2e` — a real override-compile of `template.xlsx` asserts 2 tools, the sanitized names `{calculate_tax, estimate_refund}`, populated input_keys disjoint on `withheld`, and a non-empty served schema per tool. (c) `reemit_golden` tightened with `golden_carries_two_tools_with_populated_input_keys` + `fresh_template_compile_yields_multi_tool_with_populated_keys` — the positive multi-tool assertions the old subset-only checks lacked.

## Task Commits
1. **Tasks 1-2: wire build_tools multi-tool fan-out into the production compile path** — `2c7b1f95` (feat)
2. **Task 3: CR-02 schema fallback + real-compile E2E proof + tighten reemit** — `c44f9669` (test)

## Decisions Made
- **Empty-set fallback** is the corpus bridge: the named-range workbooks harvest zero Tables → empty OutputTable set → single-tool `build_cell_map`. No re-authoring of the corpus.
- **`promote_harvested_tables` is additive** (beside the named-range promotions), so the Rule-4 deferral (no named-range removal) is honored while WBV2-04 reaches production. `promote_named_outputs`/`name_named_inputs` are untouched.
- **The collision gate uses the SAME `stage1::render_aggregate`** the `refuse_uncallable_inputs` F1 gate uses (one aggregate render for both stage-1 refusals).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing critical functionality] Added `promote_harvested_tables` (harvest-driven manifest role promotion)**
- **Found during:** Tasks 1-2. The plan's Task 2 assumed `template.xlsx` already carried `Role::Output` cells ("KEEP only those present in `manifest.cells` with `role == Role::Output`"). A probe showed it does NOT: `template.xlsx` is authored as Excel Tables with NO `in_*`/`out_*` named ranges, so synthesis classifies its inputs as unnamed `Role::Input` (blocked by `refuse_uncallable_inputs`) and its outputs as `Role::Formula` (never `Role::Output`). The workbook could not compile through the production path at all — so `output_tables_from_harvest` would always return empty and the E2E could never see 2 tools.
- **Fix:** Added `promote_harvested_tables` (+ `promote_one_harvested_table`/`cell_value_text`/`split_a1_col_row`/`next_col`/`index_to_col`) — the ADDITIVE Table analogue of `promote_named_outputs`/`name_named_inputs`: names input rows from the Table's `name` column and re-roles output-Table `value` formula cells to `Role::Output`. A named-range workbook harvests zero Tables → no-op. Wired into BOTH compile lanes before `refuse_uncallable_inputs`.
- **Files modified:** `crates/pmcp-workbook-compiler/src/lib.rs`
- **Verification:** the `template_compile_e2e` module + the named-range corpus (reemit_golden/reemit_loan/quirks) all green.
- **Committed in:** `2c7b1f95`

**2. [Rule 1 - Bug] Fixed `template.xlsx`'s self-inconsistent `tax_owed` oracle**
- **Found during:** Tasks 1-2. After harvest promotion, the production reconcile (step 7) refused `template.xlsx` with 3 named-output mismatches: the `tax_owed` formula `ROUND(B4*G3-1759,0)` recomputes to `20241` (100000·0.22−1759) while the authored cached `<v>` oracle is `18241` (the value the harvest E2E asserts). The fixture was self-inconsistent; the reconcile correctly refused it.
- **Fix:** Corrected the formula constant `-1759 → -3759` in `fixture_author.rs::template_spec` (100000·0.22−3759 = 18241 = the cached oracle; the downstream `effective_rate`/`refund` oracles then reconcile too). Regenerated `template.xlsx` (both the cargo-pmcp canonical copy and the compiler fixtures copy, byte-identical) + the `template.gen.json` sidecar via the env-gated `regenerate_template`. The embedded cargo-pmcp mirror is the same file, refreshed in lock-step.
- **Files modified:** `crates/pmcp-workbook-compiler/src/fixture_author.rs`, `crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx`, `cargo-pmcp/src/templates/workbook_bundle/template.xlsx`, `cargo-pmcp/src/templates/workbook_bundle/template.gen.json`
- **Verification:** `template_provenance.rs` byte-equality + RAW ExcelTrusted tests green; the E2E compiles cleanly.
- **Committed in:** `2c7b1f95`

### Documented placement deviation

**3. E2E proof lives in-`src` as `mod template_compile_e2e`, NOT the frontmatter's external `tests/template_compile_e2e.rs`**
- **Reason:** `compile_workbook_with_fixture_override` is `#[cfg(test)]`-only and INVISIBLE to an external `tests/` integration crate (the SAME CR-01 reachability rule that places `reemit_golden` in `src/`). The plan's `<output>` explicitly anticipated and sanctioned this placement.
- **Committed in:** `c44f9669`

## Threat Model Outcome
- **T-100-17 (two Tables → one MCP name):** mitigated. `refuse_colliding_output_tables` folds `tool_name_collision_findings` Errors into the stage-1 gate as a cell-precise `CompileError::Lint` BEFORE any bundle write. Unit test `colliding_output_tables_block_compile_path`.
- **T-100-04-CR02 (empty served schema vs runtime acceptance):** mitigated. `input_schema_for_tool` full-pool fallback for empty `input_keys`; the production multi-tool path always populates `input_keys`, so the served schema is never stricter than `validate_input`. Tests `empty_input_keys_projects_full_pool` + the E2E `template_compile_served_schema_is_non_empty_per_tool`.
- **T-100-18 (reemit proof passing a single-tool regression via is_subset):** mitigated. `golden_carries_two_tools_with_populated_input_keys` + `fresh_template_compile_yields_multi_tool_with_populated_keys` are positive multi-tool assertions; a single-tool / empty-keys compile fails them.
- **T-100-19 / T-100-SC:** accept (unchanged) — area expansion is over already-ingested owned cells (bounded); no new package installs.

## Known Stubs
None — the multi-tool fan-out is LIVE on the production path. `reconcile_tools` per-tool oracles ride the manifest's tier default (the `oracle_value` path), which for harvested outputs is currently `None` (IN-03, pre-existing) so the per-tool reconcile passes trivially on `template.xlsx`; the shared `comparison_from_outputs` reconcile (step 7) is the binding oracle gate and it grades the recomputation against the cached `<v>`. Populating per-tool oracles from the cached `<v>` is the IN-03 follow-up (out of this plan's scope).

## Threat Flags
None — no new network endpoint, auth path, or trust-boundary schema beyond the planned multi-tool fan-out (already in the `<threat_model>`).

## Self-Check: PASSED
- Created file verified present: `crates/pmcp-workbook-compiler/src/template_compile_e2e.rs`.
- Commits verified in git log: `2c7b1f95`, `c44f9669`.
- `cargo test -p pmcp-workbook-compiler`: 366 passed, 2 ignored, 0 failed.
- `cargo test -p pmcp-server-toolkit --features "workbook workbook-embedded"`: 267 passed, 1 ignored, 0 failed.
- `cargo fmt --all -- --check`: clean. `cargo clippy -p pmcp-workbook-compiler --all-targets`: 0 warnings. `cargo clippy -p pmcp-server-toolkit ... --features "workbook workbook-embedded"`: only the pre-existing out-of-scope `code_mode.rs:557` unused-import warning (not introduced here).
- `make purity-check`: PASSED.
- PMAT complexity (cog ≤25) over touched `src/` files: 0 violations.

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
