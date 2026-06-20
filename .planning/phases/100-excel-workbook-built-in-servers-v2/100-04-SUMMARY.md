---
phase: 100-excel-workbook-built-in-servers-v2
plan: 04
subsystem: served-multi-tool-fanout
tags: [multi-tool, per-tool-schema, sanitize-tool-name, collision-lint, per-tool-reconcile, shim-removal, golden-regen, embedded-mirror, proptest]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 03
    provides: "Tool type + CellMap{inputs, tools[]} + Dag::upstream_input_leaves + build_tools + the transitional .outputs() accessor"
provides:
  - "WorkbookToolHandler — ONE served MCP tool per output Table (per-tool DAG-derived inputSchema + non-empty outputSchema), replacing the generic CalculateHandler"
  - "sanitize_tool_name in pmcp-workbook-runtime (single shared source; toolkit registration + compiler collision lint both call it) with locked five-rule semantics"
  - "N-handler registration loop in with_workbook_bundle (one tool_arc per bundle.cell_map.tools, fail-closed on an unmappable name) + the four meta tools unchanged"
  - "comparison_from_outputs_for_tool + ToolReconcileReport{any_mismatch, render} + reconcile_tools (per-tool oracle partition; any mismatch => non-zero gate)"
  - "tool_name_collision_findings (T-100-17 post-sanitize collision lint, cell-precise) + a tax-calc@1.1.0 golden regenerated into the two-Table (calculate_tax + estimate_refund) shape"
affects: [served-workbook-tools, cargo-pmcp-workbook-scaffold-mirror]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Single shared sanitizer in the reader-free runtime: pmcp-workbook-runtime::sanitize_tool_name is the ONE definition the served registration AND the offline compiler collision lint call, so 'what we register' and 'what we collision-check' cannot drift"
    - "Per-tool projection: input_schema_for_tool/output_schema_for_tool iterate ONE Tool's DAG-derived input_keys / its own outputs (not a workbook-wide union); the manifest-level builders remain for the workbook-wide meta/generalization consumers"
    - "Subset producer/consumer proof: when the served golden becomes a superset of the legacy named-range source, the reemit proof asserts the compile output is a SUBSET of the golden rather than 1:1"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/workbook_multi_tool.rs
    - crates/pmcp-server-toolkit/tests/workbook_tool_name_prop.rs
    - crates/pmcp-server-toolkit/examples/workbook_table_authoring.rs
  modified:
    - crates/pmcp-server-toolkit/src/workbook/schema.rs
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - crates/pmcp-server-toolkit/src/workbook/mod.rs
    - crates/pmcp-server-toolkit/src/workbook/error.rs
    - crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
    - crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs
    - crates/pmcp-server-toolkit/tests/workbook_integration.rs
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/* (regenerated golden)
    - crates/pmcp-server-toolkit/Cargo.toml
    - crates/pmcp-workbook-runtime/src/manifest_model.rs
    - crates/pmcp-workbook-runtime/src/lib.rs
    - crates/pmcp-workbook-runtime/src/artifact_model.rs
    - crates/pmcp-workbook-runtime/src/bundle_loader.rs
    - crates/pmcp-workbook-compiler/src/artifact/cell_map.rs
    - crates/pmcp-workbook-compiler/src/artifact/mod.rs
    - crates/pmcp-workbook-compiler/src/lib.rs
    - crates/pmcp-workbook-compiler/src/reemit_golden.rs
    - crates/pmcp-workbook-compiler/src/reemit_loan.rs
    - cargo-pmcp/src/templates/workbook_bundle/* (embedded mirror refresh)

key-decisions:
  - "sanitize_tool_name lifted into pmcp-workbook-runtime (single shared source) rather than duplicated in the toolkit + compiler — the toolkit wraps it into WorkbookToolError::unmappable_tool_name; the compiler calls it directly"
  - "Rule 4 deferral: fully retiring promote_named_outputs/name_named_inputs/strip_governance_prefix from the PRODUCTION compile orchestrator + re-sourcing outputs from harvested Tables is an architectural pipeline rewrite the existing named-range fixture corpus (tax-calc/loan-calc/leap1900) depends on — landed + tested the per-Table reconcile/collision/build_tools primitives instead, ready to wire when the harvest-driven compile path replaces the named-range path"
  - "Regenerated the served golden into a real two-Table bundle (added a withheld input + a 4_Refund output) so calculate_tax and estimate_refund have GENUINELY DISJOINT DAG-derived input sets — the multi-tool proof a reader can see in the printed schemas"
  - "Stale fixtures KEPT (grep-verified live consumers): tax-calc.xlsx/loan-calc.xlsx/leap1900-probe.xlsx still drive the named-range compile path + the Phase-96 quirk corpus, so none were retired"

patterns-established:
  - "Pattern: locked sanitize semantics (lowercase / illegal-run-to-single-_ / trim-edges / truncate-64 / reject-empty) as a pure reader-free fn, property-proven over arbitrary unicode"
  - "Pattern: ToolReconcileReport renders FAILING tools first so an operator sees the blocking mismatch at the top"

requirements-completed: [WBV2-04, WBV2-05]

# Metrics
duration: ~150min
completed: 2026-06-20
---

# Phase 100 Plan 04: Multi-tool Served Fan-out + Per-tool Reconcile + Legacy Retirement Summary

**The table-based contract is now observable end to end: `tools/list` returns ONE named MCP tool per output Table (`calculate_tax` + `estimate_refund`), each with a DAG-derived `inputSchema` (disjoint on `withheld`) and a non-empty `outputSchema`; tool names are sanitized to `^[a-zA-Z0-9_-]{1,64}$` via a single shared runtime sanitizer (fail-closed on empty/all-illegal); a per-tool `ToolReconcileReport` grades each tool against its own oracle (any mismatch → non-zero); a cell-precise `tool-name-collision` lint catches two Tables collapsing to one name; and the Plan 03 transitional `outputs()` shim + the generic `CalculateHandler` are deleted (zero dead compat code).**

## Performance

- **Duration:** ~150 min
- **Tasks:** 4 (executed as Checkpoints A, B+C, then Tasks 3 & 4)
- **Files:** 3 created, ~19 modified (+ the regenerated golden + embedded mirror)

## Accomplishments

- **Task 1 (Checkpoint A) — per-tool schema + N handlers + sanitize** (`a0f10a73`): Added `input_schema_for_tool`/`output_schema_for_tool` (per-tool projection over a Tool's DAG-derived `input_keys` / its own outputs; shared `input_prop_for_entry`/`output_props_for_entries` helpers; F2 `variable_tier_keys` block retained verbatim). Added `WorkbookToolHandler` (one per Tool, `project_tool_outputs` projects ONLY its outputs) and `sanitize_tool_name` (locked five-rule semantics) + `unmappable_tool_name` error. Retired `CalculateHandler`. `with_workbook_bundle` now loops `for tool in &bundle.cell_map.tools` registering one `tool_arc` per Table (fail-closed on an unmappable name), then the four meta tools unchanged. Six sanitize unit tests.
- **Task 2 (Checkpoints B+C) — reconcile report + collision lint + reshape + shim removal** (`cd9154f8`): Lifted `sanitize_tool_name` into `pmcp-workbook-runtime` (single shared source). Added `comparison_from_outputs_for_tool` + `Comparison{is_match}` + `ToolReconcileReport{any_mismatch, render}` + `reconcile_tools` (per-tool oracle partition; failing tools render first). Added `tool_name_collision_findings` (T-100-17, names all offenders + cell locations) + a `tool-name-unmappable` lint. Reshaped the three F1 input-key findings' repair text to table-ROW oriented (dropped the `in_*` named-range guidance; rule codes unchanged). DELETED the Plan 03 `CellMap::outputs()` shim — every consumer (schema/bundle_loader/compiler reemit+artifact tests/byte-stability) now iterates `tools[].outputs` per-tool. Seven new compiler unit tests (reconcile + collision).
- **Task 3 — golden regen + multi-tool test + example + mirror refresh** (`676bea94`, `d22ebd8d`): Regenerated the `tax-calc@1.1.0` golden into the WBV2-04 two-Table shape — `Calculate_Tax` (4 tax outputs) + `Estimate_Refund` (`refund = withheld - tax_owed`), adding a `withheld` input + a `4_Refund` output sheet so the two tools' DAG-derived input sets are DISJOINT on `withheld`. Refreshed the cargo-pmcp embedded mirror (bundle + `tax-calc.xlsx`) byte-for-byte (the deferred-items mirror-drift resolution — `embedded_bundle_matches_committed_golden` + `embedded_xlsx_matches_committed_source` now PASS). Added `workbook_multi_tool.rs` (tools/list returns exactly the two tools with disjoint DAG-derived input keys, non-empty output schemas, strict envelope) and `examples/workbook_table_authoring.rs` (the ALWAYS `cargo run --example` — prints the per-Table tool surface). Updated the reemit_golden proof to compare the named-range compile output as a SUBSET of the now-superset golden.
- **Task 4 — property test** (`e00fe215`): `workbook_tool_name_prop.rs` (512 cases each): Property 1 (T-100-10 charset — arbitrary input → Err OR an Ok name matching `^[a-zA-Z0-9_-]{1,64}$`); Property 2 (T-100-17 collision — distinct raw names with equal Ok sanitization always group as a collision); Property 3 (T-100-11 strict envelope — arbitrary Tool → `additionalProperties == false`). Explicit seeds for empty/all-whitespace/all-punctuation/oversized + the `Calculate Tax`/`calculate_tax`/`calculate-tax` triple.

## Task Commits

1. **Task 1 (Checkpoint A): per-tool schema + N handlers + registration loop + sanitize** — `a0f10a73` (feat)
2. **Task 2 (Checkpoints B+C): per-tool reconcile + collision lint + reshape F1 + remove shim** — `cd9154f8` (feat)
3. **Task 3: golden regen + multi-tool test + example + mirror refresh** — `676bea94` (feat)
4. **Task 3 ripple fix: reemit_golden subset proof** — `d22ebd8d` (fix)
5. **Task 4: property test** — `e00fe215` (test)

## Decisions Made

- **Shared sanitizer in the runtime** (not duplicated): `pmcp-workbook-runtime::sanitize_tool_name` is the ONE source; the toolkit wraps it into a `WorkbookToolError`, the compiler calls it directly. Registration and collision-lint cannot drift on the locked semantics.
- **Two-Table golden with a real `withheld`/`refund` split**: makes the DAG-derived per-tool input disjointness GENUINE (not a synthetic relabel), so the integration test + example prove the contract a reader can see.
- **Stale fixtures kept** (grep-verified): the named-range compile path + Phase-96 quirk corpus still consume `tax-calc.xlsx`/`loan-calc.xlsx`/`leap1900-probe.xlsx`, so none were retired.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `sanitize_tool_name` lifted into `pmcp-workbook-runtime` (the plan placed it only in the served handler)**
- **Found during:** Task 2 (the compiler's collision lint needs the SAME sanitization, but the toolkit is not a compiler dependency).
- **Issue:** The plan's `<action>` defined `sanitize_tool_name` in `handler.rs` only. The Checkpoint-B collision lint (in the compiler) must sanitize identically, and the compiler cannot depend on the toolkit. Two copies would let the registration charset and the collision charset drift.
- **Fix:** Added `pub fn sanitize_tool_name(raw) -> Result<String, String>` to the reader-free `pmcp-workbook-runtime::manifest_model` (the SINGLE shared definition both crates already depend on); the toolkit's `sanitize_tool_name` wraps it into `WorkbookToolError::unmappable_tool_name`; the compiler calls it directly.
- **Files modified:** `crates/pmcp-workbook-runtime/src/manifest_model.rs`, `.../lib.rs`, `crates/pmcp-server-toolkit/src/workbook/handler.rs`, `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs`.
- **Verification:** the shared sanitizer is exercised by both the toolkit property test and the compiler collision unit tests.
- **Committed in:** `a0f10a73` (Task 1) + `cd9154f8` (Task 2).

**2. [Rule 3 - Blocking] Regenerated the served golden ripple touched `get_manifest`/byte-stability/integration assertions + the reemit_golden subset proof**
- **Found during:** Task 3 (the two-Table golden changed the input/output counts and made the named-range source a SUBSET of the served golden).
- **Issue:** The served golden moved from 3 inputs / 4 outputs / 1 tool to 4 inputs / 5 outputs / 2 tools. The `get_manifest` test (3/4), the byte-stability test (4 outputs), and the `workbook_integration` "five tools" test asserted the old shape; the `reemit_golden` producer/consumer proof asserted the named-range `tax-calc.xlsx` compile output == the served golden 1:1, which no longer holds (the source has no `withheld`/`refund`).
- **Fix:** Updated `get_manifest` (4/5), byte-stability (5 outputs / 2 tools / 4 inputs), and `workbook_integration` (the two per-Table tools + four meta tools) to the multi-tool surface; reshaped the three `reemit_golden` structural checks to assert the compile output is a SUBSET of the golden (every emitted IR cell / seed_coord / output role-dtype-name matches; the golden may carry extra refund-tool cells).
- **Files modified:** `handler.rs` (test), `fixture_byte_stability.rs`, `workbook_integration.rs`, `reemit_golden.rs`.
- **Verification:** all four suites green.
- **Committed in:** `676bea94` (Task 3) + `d22ebd8d` (ripple fix).

**3. [Rule 3 - Blocking] The example lives in `crates/pmcp-server-toolkit/examples/` (the plan listed repo-root `examples/`)**
- **Found during:** Task 3 (the root `examples/` belongs to the `pmcp` crate, which does NOT depend on `pmcp-server-toolkit`, so the workbook types are unreachable there).
- **Fix:** Placed `workbook_table_authoring.rs` in the toolkit examples dir (alongside `workbook_server_http.rs`) with `required-features = ["workbook-embedded"]`; runs via `cargo run --example workbook_table_authoring --features workbook-embedded -p pmcp-server-toolkit`.
- **Committed in:** `676bea94`.

### Architectural Deferral (Rule 4)

**Fully retiring the named-range model from the PRODUCTION compile orchestrator is deferred.** The plan's Checkpoint C assumed "the table-harvest-driven manifest from Plan 02/03" already drives the orchestrator; in fact the production `compile_workbook`/`prepare_candidate` pipeline still uses `promote_named_outputs`/`name_named_inputs`/`strip_governance_prefix`/`build_cell_map` over the `out_*`/`in_*` named ranges, and the existing fixture corpus (`tax-calc.xlsx`/`loan-calc.xlsx`/`leap1900-probe.xlsx`) + the Phase-96 generalization/quirk proofs (`reemit_golden`/`reemit_loan`/`quirks_reconcile`) depend on it. Re-sourcing outputs from harvested Tables + re-authoring every fixture as Excel Tables is a multi-plan pipeline rewrite that would break the corpus and cannot keep the tree green within this plan. **What WAS landed + tested** (the observable WBV2-04/05 value): the served multi-tool fan-out, the per-tool reconcile primitives (`comparison_from_outputs_for_tool`/`ToolReconcileReport`/`reconcile_tools`), the post-sanitize collision lint, the F1 row-lint repair-text reshape, and the Plan 03 shim removal — all ready to wire when the harvest-driven compile path replaces the named-range path. Consequently `promote_named_outputs`/`name_named_inputs`/`strip_governance_prefix` are NOT removed (criteria C1/C2/C3 partially deferred); `CalculateHandler` (C-Task-1) IS removed, and the `.outputs()` shim (C5/T-100-16) IS removed.

---

**Total deviations:** 3 auto-fixed (all Rule 3 blocking) + 1 documented Rule-4 architectural deferral.

## Threat Model Outcome

- **T-100-10 (unmappable/empty tool name):** mitigated. `sanitize_tool_name` enforces the charset + rejects empty/all-illegal; registration fails closed on a reject; Task 4 Property 1 proves the charset invariant over arbitrary input strings.
- **T-100-11 (strict-envelope relaxation):** mitigated. Every per-tool `input_schema_for_tool` keeps `additionalProperties:false`; Task 4 Property 3 proves it over arbitrary Tools.
- **T-100-16 (Plan 03 shim surviving as dead compat code):** mitigated. The `#[deprecated] CellMap::outputs()` accessor is DELETED; `grep -rn 'fn outputs(&self)' artifact_model.rs` and `grep -rn '.outputs()' crates/` (non-comment) both return nothing.
- **T-100-17 (two Tables collapsing to one MCP name):** mitigated. `tool_name_collision_findings` emits a cell-precise `tool-name-collision` error naming all offenders before registration; Task 4 Property 2 proves equal-sanitization distinct raw names are always flagged.
- **T-100-08/09 (strict-constant / computed-cell as a caller input):** unchanged — the `is_strict_constant`/`is_computed` reject gates + the DAG-derived per-tool input projection (Plan 03) are retained; per-tool schemas advertise only `Role::Input` leaves.

## Known Stubs

- **Per-tool reconcile + collision lint are tested PRIMITIVES, not yet wired into the production orchestrator.** `reconcile_tools`/`ToolReconcileReport`/`tool_name_collision_findings` are landed + unit-tested but the production `compile_workbook` still grades via the single-`Comparison` `comparison_from_outputs` over the named-range manifest. This is the Rule-4 deferral above: they wire in when the harvest-driven compile path replaces the named-range path. The served fan-out (the observable WBV2-04 deliverable) IS live.

## Threat Flags

None — no new network endpoint, auth path, file-access pattern, or trust-boundary schema beyond the planned multi-tool fan-out (already in the `<threat_model>`).

## Deferred Items (resolved this plan)

- **cargo-pmcp embedded-mirror drift** (deferred-items.md): RESOLVED. The embedded `tax-calc@1.1.0` bundle + `tax-calc.xlsx` were refreshed byte-for-byte to match the regenerated golden; the two embedded-mirror tests now PASS (cargo-pmcp 439 passed vs prior 437). The only remaining cargo-pmcp failure is the pre-existing, unrelated `test_support_cache::proptests::normalize_round_trip_idempotent` (out of scope, not a regression).
- **Legacy `tax-calc`/`leap1900` fixtures** (Plan 01 "for removal in Plan 04"): KEPT — grep confirmed live consumers (the named-range compile path + the Phase-96 quirk corpus). Removal is bound to the Rule-4 orchestrator retirement.

## Self-Check: PASSED

- Created files verified present: `crates/pmcp-server-toolkit/tests/workbook_multi_tool.rs`, `.../tests/workbook_tool_name_prop.rs`, `.../examples/workbook_table_authoring.rs`.
- Commits verified in git log: `a0f10a73`, `cd9154f8`, `676bea94`, `d22ebd8d`, `e00fe215`.
- `cargo test -p pmcp-workbook-runtime -p pmcp-workbook-compiler`: exit 0.
- `cargo test -p pmcp-server-toolkit --features workbook,workbook-embedded`: exit 0.
- `cargo run --example workbook_table_authoring --features workbook-embedded -p pmcp-server-toolkit`: exit 0 (prints calculate_tax + estimate_refund with disjoint DAG-derived inputs).
- `cargo test -p cargo-pmcp --lib templates_workbook_server`: exit 0 (mirror refreshed).
- `make purity-check`: PASSED.
- `CellMap::outputs()` shim, `CalculateHandler`, all `.outputs()` callers: GONE.

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
