---
phase: 100-excel-workbook-built-in-servers-v2
plan: 08
subsystem: workbook-explain-projection-and-compile-gates
tags: [gap-closure, pmcp-run-review, H1, H2, H3, M4, M5, M6, explain-parity, reserved-name-gate, per-tool-oracle, freshness-preview]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 07
    provides: "emit_bundle→build_tools multi-tool fan-out, promote_harvested_tables, output_tables_from_harvest, the template_compile_e2e harness + the production pre-emit pipeline this plan projects through"
provides:
  - "project_tool_surface_from_workbook — the public NON-persisting served-surface projection the explain CLI drives (the SAME pre-emit pipeline build_tools/json_key_for_role); the bespoke explain walker is DELETED so the preview cannot lie about the served surface (H1)"
  - "FreshnessPolicy::Preview — a read-only freshness policy that demotes the oracle/* staleness refusal for a structural preview (production-safe: only the read-only projection constructs it)"
  - "RESERVED_TOOL_NAMES (runtime leaf) + refuse_reserved_output_table_names — a blocking dual-lane compile gate rejecting an output-Table name sanitizing into {explain,get_manifest,diff_version,render_workbook}, the set DERIVED from the handler NAME constants via a toolkit binding test (H3)"
  - "value-shaped served-key rejection (is_value_shaped_key) on inputs (H2) AND validate_output_keys/refuse_uncallable_outputs over output keys (dup/empty/value-shaped, M4) — wired into the stage-1 Error gate in BOTH compile lanes"
  - "get_manifest input/output projections advertising the STRIPPED json_key (== the served tool schema keys), raw prefixed name kept as governance_name (M5)"
  - "the per-tool reconcile oracle wired from the authored cached-<v> map (output_oracle_map → build_tools output_oracles) so a perturbed output blocks the emit — no longer vacuous (M6)"
affects: [workbook-explain-cli, served-get_manifest, workbook-compile-gates, per-tool-reconcile]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Explain drives the production projection (no re-derivation): explain_surface maps the build_tools Tool list 1:1 into the BA render DTOs, so the preview is the served surface BY CONSTRUCTION — the H1 divergence class is structurally impossible, not merely test-covered"
    - "Read-only Preview freshness policy: a structural preview grades no oracle value, so the oracle/* staleness refusal is demoted ONLY on the read-only projection; the compile/emit path always enforces it (the policy enum carries the production-safety invariant)"
    - "Reserved set in the runtime LEAF (RESERVED_TOOL_NAMES), bound to the toolkit handler NAME constants by a test — the compiler gate reads the one const WITHOUT a compiler→toolkit dep (purity-safe), and the binding test makes the set un-driftable"
    - "Additive oracle wiring: build_tools gains an output_oracles param preferred over the tier-default fallback (oracle_value), so the production cached-<v> path populates the per-tool oracle while every pre-M6 tier-based test stays green"

key-files:
  created: []
  modified:
    - crates/pmcp-workbook-compiler/src/lib.rs
    - crates/pmcp-workbook-compiler/src/stage1.rs
    - crates/pmcp-workbook-compiler/src/artifact/cell_map.rs
    - crates/pmcp-workbook-compiler/src/artifact/mod.rs
    - crates/pmcp-workbook-compiler/src/template_compile_e2e.rs
    - crates/pmcp-workbook-runtime/src/manifest_model.rs
    - crates/pmcp-workbook-runtime/src/lib.rs
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - cargo-pmcp/src/commands/workbook/explain_surface.rs
    - cargo-pmcp/tests/workbook_explain.rs

key-decisions:
  - "H1: explain now CALLS project_tool_surface_from_workbook (the production projection) and maps its Tool list — the bespoke reachable_addrs/extract_a1_refs/is_input_table/harvest_input_pool/tool_for_table walker is DELETED. The preview cannot diverge from the served surface because it IS the served surface."
  - "Added FreshnessPolicy::Preview (NOT a reuse of the #[cfg(test)] TrustedFixture): the trusted-fixture gate is test-only and collapses to Enforce in a non-test build, so the cargo-pmcp integration test (a non-test consumer of the lib) could not preview template.xlsx's fullCalcOnLoad=1 fixture. Preview runs the production gate for provenance but demotes its oracle/* refusal — production-safe because only the read-only projection constructs it."
  - "H1 parity test placed in pmcp-workbook-compiler's template_compile_e2e (#[cfg(test)] in-src), the SAME CR-01 reachability rule Plan 07 documented — it must reach compile_workbook_with_fixture_override + the served input_schema_for_tool/output_schema_for_tool. It compiles template.xlsx (served bundle) AND runs the preview projection on the SAME workbook, asserting names + per-tool input/output keys are byte-identical (stripped)."
  - "Rule 1 (review-correctness): the committed explain snapshot encoded the OLD walker's LIE (it invented `filing` on every tool + a `[USD]` unit on income). The DAG never reaches filing's value cell (tax_owed=ROUND(B4*G3-3759) references income/G3, not B5) and colour-synth carries no unit, so the SERVED binary advertises NEITHER. Updated the snapshot + assertions to the TRUE served surface and documented why — the H1 fix's whole point is that the preview now matches what is served. (No fixture regeneration was needed; WR-01 range/cross-sheet coverage is a synthetic build_tools test instead.)"
  - "WR-01 closed via a synthetic build_tools range+cross-sheet test (upstream_input_leaves surfaces a range-member input AND a cross-sheet input — exactly the inputs extract_a1_refs dropped) + the projection-equivalence property arm, rather than regenerating template.xlsx (lower-risk; the parity test already proves preview==served over the real fixture)."
  - "H3 reserved set lives in pmcp-workbook-runtime (the shared leaf) as RESERVED_TOOL_NAMES, bound to ExplainHandler::NAME/GetManifestHandler::NAME/DiffVersionHandler::NAME/RenderWorkbookHandler::NAME by a toolkit binding test. The compiler reads the const directly — no compiler→toolkit dep (purity-safe)."
  - "M6 wired (preferred path, NOT the removal fallback): output_oracle_map(map, manifest) builds the cached-<v> map; reconcile_output_tables threads it into build_tools in BOTH lanes. oracle_value is kept as a tier-default fallback so the synthetic tier-based tests stay green; harvested outputs (no tier) now get their oracle solely from the cached map."

patterns-established:
  - "A production projection (compiler) + a thin render mapper (CLI) is the anti-divergence pattern for any 'preview vs served' surface: the preview must CALL the production primitive, never re-derive."

requirements-completed: [WBV2-04, WBV2-05, WBV2-06]

# Metrics
duration: ~135min
completed: 2026-06-20
---

# Phase 100 Plan 08: Close the pmcp.run dev-team review gaps (H1-H3 + M4-M6) Summary

**All six verified pmcp.run review findings are closed and the publish blockers are cleared: `cargo pmcp workbook explain` now DRIVES the production projection (`project_tool_surface_from_workbook` → `build_tools`/`json_key_for_role`) with the bespoke A1 walker DELETED, so the preview is the served surface by construction — proven by the load-bearing `explain_projection_matches_the_served_tool_surface` parity test over `template.xlsx` (tool names + per-tool input/output keys byte-identical to `input_schema_for_tool`/`output_schema_for_tool`, stripped), which ALSO closes WR-01 (a synthetic range+cross-sheet `build_tools` test surfaces the inputs the old walker dropped). A value-shaped input name (`60000`) now fails compile with a cell-located Error (H2); an output Table sanitizing to a reserved meta-tool name fails compile, the reserved set DERIVED from the handler NAME constants via a toolkit binding test (H3); colliding/empty/value-shaped output keys fail compile (M4), all in BOTH compile lanes. `get_manifest` advertises the stripped served key == the served tool schema keys (M5); and the per-tool reconcile oracle is wired from the authored cached-`<v>` map so a perturbed output blocks the emit (M6). `make lint` GREEN, `make purity-check` GREEN, zero clippy warnings on touched crates, zero PMAT cog-25 violations over touched src; the named-range corpus + `template.xlsx` still compile; the Rule-4 named-range path is intact; the 4 LOW findings remain deferred.**

## Performance
- **Duration:** ~135 min
- **Tasks:** 3 (3 atomic commits)
- **Files:** 10 modified, 0 created

## Accomplishments

- **Task 1 (`a908ea21`) — H1 + parity + WR-01.** Added `project_tool_surface_from_workbook` (public, non-persisting) to the compiler: runs the SAME pre-emit pipeline `compile_workbook_inner` uses (ingest→stage1 synth→promote_named_outputs→name_named_inputs→promote_harvested_tables→build_ir_and_dag→output_tables_from_harvest→build_tools) and STOPS before ratify/reconcile/emit. Added `FreshnessPolicy::Preview` (read-only; demotes the `oracle/*` staleness refusal). Rewrote `explain_surface.rs` to DRIVE that projection and map the production `Tool` list into the BA render DTOs; DELETED the bespoke walker (`reachable_addrs`/`extract_a1_refs`/`is_input_table`/`harvest_input_pool`/`tool_for_table` + their A1 helpers). Added the `explain_projection_matches_the_served_tool_surface` parity test (compiler `template_compile_e2e`), the synthetic `build_tools_surfaces_range_and_cross_sheet_inputs` WR-01 test, and the `projection_preserves_build_tools_input_keys` property arm. Updated the explain snapshot to the TRUE served surface (the old one encoded the walker's invented `filing`/`[USD]`).
- **Task 2 (`e0efb666`) — H2 + H3 + M4.** H2: `is_value_shaped_key` rejects a numeric served input key (`60000`/`1.5`/`-3`) in `validate_input_keys`. H3: `RESERVED_TOOL_NAMES` added to the runtime leaf (bound to the four handler `NAME` constants by `reserved_tool_names_match_the_registered_meta_tool_names`); `refuse_reserved_output_table_names` blocks an output Table sanitizing into the reserved set, wired at stage-1 in BOTH lanes. M4: `validate_output_keys`/`refuse_uncallable_outputs` (dup/empty/value-shaped over output keys) wired next to `refuse_uncallable_inputs` in BOTH lanes. Unit + the numeric-key reject property arm over arbitrary finite f64 on both lanes.
- **Task 3 (`4ac2213f`) — M5 + M6.** M5: `curated_manifest`/`input_projection` + the inline output `json!` now emit `json_key_for_role(role)` as `name` (stripped), keeping the raw name as `governance_name`; toolkit test asserts get_manifest names == the workbook-wide served input/output keys and every per-tool served key is discoverable. M6: `build_tools`/`build_one_tool` gained an `output_oracles` param (cached value by cell key) preferred over the `oracle_value` tier fallback; `output_oracle_map(map, manifest)` builds it; `reconcile_output_tables` threads it in BOTH lanes; `emit_bundle` + the preview pass an empty map. Tests: a non-tiered output's oracle is empty without the map but populated with it; a perturbed cached output blocks the per-tool reconcile; the unperturbed value reconciles. `oracle_value`'s stale "supplied by Plan 04" comment corrected.

## Task Commits
1. **H1 + parity test + WR-01 + projection-equivalence property** — `a908ea21` (feat)
2. **H2 value-shaped name + H3 reserved name + M4 output-key gates (both lanes)** — `e0efb666` (feat)
3. **M5 get_manifest stripped key + M6 per-tool oracle wiring** — `4ac2213f` (feat)

## Decisions Made
- **Explain drives the production projection** — the single anti-divergence decision: the preview cannot lie because it IS the served surface (`build_tools`/`json_key_for_role`), not a re-derivation.
- **`FreshnessPolicy::Preview`** is a NEW production-safe policy, not a reuse of the test-only `TrustedFixture` (which collapses to `Enforce` in a non-test build and would block the preview of the `fullCalcOnLoad=1` fixture from a non-test consumer).
- **The reserved set is the runtime leaf const**, bound to the handler NAMEs by a toolkit test — purity-safe (no compiler→toolkit dep) and un-driftable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Review correctness] Updated the explain snapshot to the TRUE served surface**
- **Found during:** Task 1. The committed `workbook_explain.rs` snapshot (and the `per_tool_inputs` assertion) expected `filing` on every tool and a `[USD]` unit on `income` — both produced by the OLD divergent walker. The production projection (the served surface) advertises NEITHER: `tax_owed = ROUND(B4*G3-3759)` references `income`(B4)/G3 but NOT `filing`(B5), so the DAG never reaches `filing` (it is a feeds-no-tool input, not a served param), and the colour-synth role for `income` carries no unit (the unit lived only on the old walker's table-harvest projector).
- **Fix:** Updated the snapshot + assertions to the correct served surface (`calculate_tax: [income]`, `estimate_refund: [income, withheld]`, `income: number` no unit) and documented in the test file why (the H1 fix's purpose is that the preview matches what is served). This is the exact H1 divergence the plan eliminates — the old snapshot was the lie.
- **Files modified:** `cargo-pmcp/tests/workbook_explain.rs`
- **Committed in:** `a908ea21`

### Documented design deviations

**2. `FreshnessPolicy::Preview` added (NOT in the plan's interface list)**
- **Reason:** The plan's projection must work from a NON-test consumer (the cargo-pmcp integration test compiles the cargo-pmcp lib, not the compiler's `#[cfg(test)]` tree), where `TrustedFixture` collapses to `Enforce` and refuses the `fullCalcOnLoad=1` template fixture. A read-only structural preview grades no oracle value, so `Preview` demotes the `oracle/*` staleness refusal while remaining production-safe (only the read-only projection constructs it; compile/emit always enforce). This is the minimal correct seam — the alternative (forking a second freshness-free pipeline) was rejected as it would re-introduce the divergence H1 forbids.
- **Committed in:** `a908ea21`

**3. WR-01 proven via a synthetic `build_tools` test + property arm, not a regenerated `template.xlsx`**
- **Reason:** The plan sanctioned extending the fixture to add a range/cross-sheet output formula, but that requires a byte-identical 2-copy + gen.json + cargo-pmcp-mirror regeneration with a re-consistent cached-`<v>` reconcile (the Plan 07 landmine). The parity test already proves preview==served over the REAL fixture; WR-01 (that `upstream_input_leaves`/`build_tools` surface a range-member AND a cross-sheet input the old `extract_a1_refs` dropped) is proven directly with a synthetic DAG test + the projection-equivalence property arm — lower-risk and equally binding.
- **Committed in:** `a908ea21`

**4. The H1 parity test lives in `pmcp-workbook-compiler`'s in-`src` `template_compile_e2e`, not a cargo-pmcp integration test**
- **Reason:** The plan permitted either placement. The compiler `#[cfg(test)]` module is where `compile_workbook_with_fixture_override` (the override compile of the committed template) AND the served `input_schema_for_tool`/`output_schema_for_tool` are BOTH reachable (the CR-01 reachability rule Plan 07 documented). The cargo-pmcp `workbook_explain.rs` integration test still covers the CLI render path against the updated snapshot.
- **Committed in:** `a908ea21`

### Honored constraints
- The Rule-4 deferral is intact: `promote_named_outputs`/`name_named_inputs` are untouched; the named-range corpus (tax-calc/loan-calc/leap1900) stays green via the single-tool fallback.
- The 4 LOW findings (L-i…L-iv) are untouched (deferred).
- The pre-existing out-of-scope reds are unchanged (pmcp-toolkit-mysql sqlx E0277 still red; the `code_mode.rs:557` unused-import warning still present; neither introduced nor fixed here).

## Threat Model Outcome
- **T-100-08-H1 (explain vs served divergence):** mitigated. Explain drives `project_tool_surface_from_workbook`; the bespoke walker is deleted; `explain_projection_matches_the_served_tool_surface` is the binding parity proof.
- **T-100-08-H2 (value-shaped input name):** mitigated. `is_value_shaped_key` rejects a numeric served input key with a cell-precise Error in both lanes.
- **T-100-08-H3 (reserved meta-tool collision):** mitigated. `refuse_reserved_output_table_names` blocks against `RESERVED_TOOL_NAMES` (derived from the handler NAME constants) in both lanes.
- **T-100-08-M4 (output-key collision):** mitigated. `validate_output_keys`/`refuse_uncallable_outputs` (dup/empty/value-shaped) in both lanes.
- **T-100-08-M5 (get_manifest raw key):** mitigated. get_manifest advertises the stripped `json_key` == the served schema keys.
- **T-100-08-M6 (vacuous per-tool reconcile):** mitigated. The per-tool oracle is wired from the cached-`<v>` map; a perturbed output blocks the per-tool reconcile.
- **T-100-08-SC:** accept (unchanged) — no new package installs.

## Known Stubs
None — every gate is LIVE on the production compile path and explain drives the production projection. The `oracle_value` tier-default fallback remains for synthetic tier-bearing test roles, but harvested production outputs are graded solely by the wired cached-`<v>` map.

## Threat Flags
None — no new network endpoint, auth path, or trust-boundary schema beyond the planned gates/projection (already in the `<threat_model>`).

## Self-Check: PASSED
- Modified files verified present (all 10 in `key-files.modified`).
- Commits verified in git log: `a908ea21`, `e0efb666`, `4ac2213f`.
- `cargo test -p pmcp-workbook-compiler`: 382 passed, 2 ignored, 0 failed.
- `cargo test -p pmcp-server-toolkit --features "workbook workbook-embedded"`: 269 passed, 1 ignored, 0 failed.
- `cargo test -p cargo-pmcp --test workbook_explain`: 5 passed; `cargo test -p cargo-pmcp --lib projection_preserves_build_tools_input_keys`: 1 passed.
- `cargo test -p pmcp-workbook-runtime`: 175 passed.
- Acceptance greps: bespoke walker == 0; `project_tool_surface_from_workbook` == 3; `refuse_uncallable_outputs` == 7 (defn + 2 call sites + tests); `refuse_reserved_output_table_names` == 6 (defn + 2 call sites + tests); `json_key_for_role` in handler == 5.
- `cargo fmt --all -- --check`: clean. `make lint` (root pmcp clippy gate): "✓ No lint issues". Touched-crate clippy: 0 warnings (only the pre-existing out-of-scope `code_mode.rs:557` filtered).
- `make purity-check`: PASSED.
- PMAT complexity (cog ≤25) over touched `src/` files: 0 violations.

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
