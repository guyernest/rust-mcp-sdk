---
phase: 100-workbook-accuracy-verification-surface
plan: 04
subsystem: workbook-runtime + workbook-toolkit
tags: [workbook, reconcile, verify_accuracy, reference-inputs, oracle, meta-tool, WBVER-03, reader-free, layering-fence]

# Dependency graph
requires:
  - phase: 100-workbook-accuracy-verification-surface
    plan: 01
    provides: tax-calc@1.1.0 fixture extended with text (bracket_label) + bool (is_taxable) formula outputs; RESERVED_TOOL_NAMES bumped to [&str;5] incl. verify_accuracy; H3 string-literal placeholder
  - phase: 100-workbook-accuracy-verification-surface
    plan: 03
    provides: prior wave (render_workbook inputs_only) — independent; this plan only depends on 01's fixture + reservation
provides:
  - "pub fn seed_reference_inputs(manifest) -> BTreeMap<String, CellValue> (runtime-native, reader-free): reads each Role::Input InputTier default as a CellValue; untiered Role::Input contributes no seed"
  - "pub fn reconcile_reference(cell_map, manifest, ir, dag, tol) -> Result<ReconcileReport, Box<LintFinding>>: seeds from seed_reference_inputs, re-runs the SHARED executor, projects per tool vs Tool.oracle within TOL"
  - "ReconcileReport { tolerance, all_within_tol, cells_checked, tools } + ToolReport { tool, all_within_tol, outputs } + OutputRow { key, cell: Option<String>, server_value, oracle_value, abs_delta, within_tol } (serde + schemars)"
  - "VerifyAccuracyHandler — the 6th served (5th meta) tool, honestly framed, optional 'tool' filter (D-03 unknown -> Err listing tools; filtered aggregates recomputed over the filtered set)"
  - "verify_accuracy_input_schema + verify_accuracy_output_schema in schema.rs"
  - "H3 binding test now binds VerifyAccuracyHandler::NAME (Plan-01 placeholder drift CLOSED — repo releasable again from the workbook-tool surface POV)"
affects: [100-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Runtime-native reference seeding: seed_reference_inputs reads InputTier defaults as CellValue (no serde_json round-trip, no toolkit dep) — a layering-fence-respecting mirror of the toolkit-private seed_tier_defaults at the manifest-tier level"
    - "Total comparison kernel: compare_output(server, oracle) is total over Number/Text/Bool/Empty/Error + missing + type-mismatch; deterministic discrete abs_delta (0.0 equal / 1.0 not), numeric |Δ| with finite-guard, fail-closed default — never NaN/unspecified, never panics"
    - "Filtered-rollup recompute (scope_report): a tool-name filter recomputes top-level cells_checked + all_within_tol from the RETAINED tool only, so a partial filter never leaves stale full-bundle aggregates (T-100-08)"

key-files:
  created:
    - crates/pmcp-workbook-runtime/src/reconcile.rs
  modified:
    - crates/pmcp-workbook-runtime/src/lib.rs
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - crates/pmcp-server-toolkit/src/workbook/schema.rs
    - crates/pmcp-server-toolkit/src/workbook/mod.rs

key-decisions:
  - "Seeded the CellEnv via CellEnv::seed_cell(key, &CellValue) (runtime-native) rather than the toolkit's env.with_value(json) loop — both sides are runtime-native CellValue so no serde_json round-trip is needed (the grounded HIGH-#1 fix: reconcile.rs lives in the runtime and reads tier defaults directly, NOT the toolkit's seed_tier_defaults)"
  - "compare_output uses a match GUARD (Number/Number if both finite) so a non-finite numeric pairing falls through to the fail-closed `_ => (1.0,false)` arm — collapses the inner if (clippy collapsible_if) AND keeps the finiteness fail-closed without a separate branch"
  - "D-02 cell=None rows: any oracle key with NO matching outputs entry is STILL graded (server_value=None -> fail-closed within_tol=false) with cell=None, so a malformed bundle surfaces visibly rather than silently dropping the key"
  - "scope_report recomputes aggregates from the filtered tool list (option (b) in the plan) rather than threading the filter into reconcile_reference — keeps the pure runtime fn filter-agnostic and the toolkit owns the (attacker-controlled) filter semantics"
  - "verify_accuracy honest framing (CONTEXT Claude's Discretion): attests the reference point only; explicitly points BAs to render_workbook filled/inputs_only for arbitrary inputs where Excel is the oracle"

patterns-established:
  - "A reader-free pure-diff service module (reconcile.rs) composing only executor + model + serde/schemars — the layering fence is grep-provable (no pmcp_server_toolkit / umya / quick-xml / calamine import)"
  - "Meta-tool with an optional attacker-controlled filter: unknown -> Err listing valid options (invalid_enum), filtered aggregates recomputed (never stale)"

requirements-completed: [WBVER-03]

# Metrics
duration: ~8min
completed: 2026-06-24
---

# Phase 100 Plan 04: verify_accuracy Reference Reconciliation (WBVER-03) Summary

**Added `verify_accuracy` — the 6th served (5th meta) tool — which re-runs the SHARED executor at the workbook's runtime-native reference inputs (`seed_reference_inputs` reads each `Role::Input` `InputTier` default as a `CellValue`, NO toolkit dep) and returns a per-output `ReconcileReport` diffing each computed value against its authored `Tool.oracle` within `TOL` (0.01) — making the compile-time penny-reconcile runtime-inspectable, stateless, reader-free, panic-free, and honestly framed. The Plan-01 H3 placeholder is closed (string literal swapped for `VerifyAccuracyHandler::NAME`).**

## Performance

- **Duration:** ~8 min (Task 1 commit 08:14 -> Task 2 commit 08:22)
- **Completed:** 2026-06-24
- **Tasks:** 2/2 committed (both TDD-shaped — tests + impl + doctests per task)
- **Files:** 1 created, 4 modified

## Accomplishments

- **Task 1 (`618aab76`):** New reader-free `crates/pmcp-workbook-runtime/src/reconcile.rs`.
  - `seed_reference_inputs(manifest) -> BTreeMap<String, CellValue>`: the grounded HIGH-#1 fix — iterates `manifest.cells`, keeps `Role::Input`, reads each `InputTier::{Variable,BoundedVariable}` `default` as a runtime-native `CellValue` (an untiered `Role::Input` contributes no seed). A runtime-native mirror of the TOOLKIT-private `seed_tier_defaults` at the manifest-tier level — NO toolkit dependency, NO serde_json round-trip.
  - `reconcile_reference(cell_map, manifest, ir, dag, tol)`: seeds the `CellEnv` via `seed_cell` (runtime-native), re-runs the SHARED `run_executor` (no second evaluator), projects per tool. Returns `ReconcileReport`/`ToolReport`/`OutputRow` (serde + schemars; `Eq` dropped — `abs_delta` is `f64`). `OutputRow.cell` is the D-01 `Option<String>` (`Some(seed_coord)` normally; `None` only for a D-02 oracle-without-output-entry).
  - `compare_output`: total over every `CellValue` pairing + missing + type-mismatch; numeric `|Δ|` with a finiteness guard, deterministic `0.0`/`1.0` discrete delta (Text/Bool), fail-closed `(1.0, false)` default — never `NaN`, never panics.
  - 12 unit tests (seed reads/skips, golden within-tol, perturbed-oracle, Text/Bool determinism, type-mismatch/missing fail-closed, D-04 vacuous empty oracle, D-02 cell=None) + a property test (`all_within_tol` == conjunction of tool/row flags) + 2 doctests (`seed_reference_inputs`, `reconcile_reference`). lib.rs re-exports the fns + report types. Reader-free (purity-check green), cog-25 clean.

- **Task 2 (`1e677c77`):** `VerifyAccuracyHandler` (the 6th served / 5th meta tool).
  - `compute`: parses the optional `tool` filter (`parse_tool_filter` — non-string -> Err, panic-free), D-03 `ensure_known_tool` (unknown -> `invalid_enum` Err carrying the available tool names in `allowed`), calls `pmcp_workbook_runtime::reconcile_reference(...)`, then `scope_report` retains the filtered tool AND recomputes top-level `cells_checked`/`all_within_tol` over the FILTERED set (MEDIUM #4 / T-100-08 — no stale full-bundle rollup), serialized + provenance-stamped.
  - Honest framing in the description (attests the reference point; points to `render_workbook` filled/inputs_only for arbitrary inputs).
  - `schema.rs`: `verify_accuracy_input_schema` (optional `tool` filter, advertise == accept) + `verify_accuracy_output_schema` (ReconcileReport rollups + per-tool `outputs[]` rows incl. the D-01 `cell`).
  - `mod.rs`: registered via `.tool_arc(VerifyAccuracyHandler::NAME, ...)` before `.resources_arc`.
  - **H3 placeholder closed:** the `reserved_tool_names_match_the_registered_meta_tool_names` test's `registered` array now references `VerifyAccuracyHandler::NAME` (not the Plan-01 `"verify_accuracy"` string literal) — the deliberate non-releasable drift Plan 01 left is closed.
  - 6 handler tests: golden no-filter green (incl. text+bool outputs + D-01 A1 addresses, `cells_checked == 7`), filtered-rollup aggregate recompute (`Estimate_Refund` -> `cells_checked == 1`, not 7), D-03 unknown filter -> Err listing tools, non-string filter -> Err, async isError envelope (T-92-10), schema-advertise.

## Task Commits

1. **Task 1 (TDD): reader-free reconcile_reference + seed_reference_inputs** — `618aab76` (feat)
2. **Task 2 (TDD): VerifyAccuracyHandler (6th meta tool) + register + H3 ::NAME swap** — `1e677c77` (feat)

## Verification

- `cargo test -p pmcp-workbook-runtime reconcile` -> **12 passed**; `--doc reconcile` -> **2 passed**.
- `cargo test -p pmcp-server-toolkit --features workbook-embedded verify_accuracy` -> **6 passed**.
- `cargo test -p pmcp-server-toolkit --features workbook-embedded workbook` -> **81 passed** (was 75; +6) — H3 binding green, existing registration + tools/list integration tests unchanged.
- `make purity-check` -> **PASSED** (reader-free; reconcile.rs has NO `pmcp_server_toolkit`/umya/quick-xml/calamine import — the layering fence held).
- `pmat analyze complexity --max-cognitive 25` -> **no violations** on reconcile.rs / handler.rs / schema.rs.
- `cargo clippy -p pmcp-workbook-runtime` and `-p pmcp-server-toolkit --features workbook-embedded --all-targets` -> **no new warnings** from this plan's code (the two pre-existing warnings — `render_resource.rs` unused `RenderMode` import from Plan 03, `code_mode.rs` unused `CodeExecutor` — are out of scope per the executor SCOPE BOUNDARY).
- Both commits passed the pre-commit `make quality-gate` hook (no `--no-verify`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - clippy collapsible_if] `compare_output` numeric branch collapsed into a match guard**
- **Found during:** Task 1 (clippy on the runtime crate flagged `reconcile.rs:181` `this if can be collapsed into the outer match`).
- **Issue:** The numeric arm wrapped an inner `if s.is_finite() && o.is_finite()` — a clippy `collapsible_if` lint (the CLAUDE.md zero-warnings gate).
- **Fix:** Moved the finiteness check into a match GUARD (`(Number(s), Number(o)) if s.is_finite() && o.is_finite() => ...`); a non-finite numeric pairing now falls through to the existing fail-closed `_ => (1.0, false)` arm — same behavior, one fewer branch, lint clean.
- **Files modified:** crates/pmcp-workbook-runtime/src/reconcile.rs.
- **Commit:** `618aab76`.

## Threat Surface

Threat-register dispositions held:
- **T-100-08** (typo'd tool filter masquerading as "all good" / stale aggregates): D-03 `ensure_known_tool` returns `invalid_enum` listing the available tools for any unknown filter (proven by `verify_accuracy_unknown_filter_errors_listing_tools`); a known filter's aggregates are recomputed from the retained tool by `scope_report` (proven by `verify_accuracy_filter_scopes_and_recomputes_aggregates`: `Estimate_Refund` -> `cells_checked == 1`, never the full 7).
- **T-100-09** (panic/unwrap in new reconcile/handler code): `deny(panic/unwrap/expect)` on both value paths; `seed_reference_inputs` + `compare_output` are total; `parse_tool_filter` non-string -> Err; `reconcile_reference` returns `Result` on executor failure; D-03 -> Err, D-04 -> vacuous — all panic-free.
- **T-100-10** (output-forging via the reconcile path — accept): `verify_accuracy` is read-only, seeds ONLY the fixed runtime-native reference defaults (no caller-supplied seeds), cannot mutate bundle state; honest framing prevents over-trust.
- **T-100-SC** (installs): no new packages — internal pure-diff over already-vetted serde/schemars + the existing executor.

No new security-relevant surface beyond the plan's threat model.

## Deferred Issues

- **Pre-existing `render_resource.rs:43` unused `RenderMode` import (out of scope):** introduced by Plan 03; not in this plan's diff, not auto-fixed per the executor SCOPE BOUNDARY. (Also noted in 100-03-SUMMARY's deferred list region; the toolkit crate is not clippy-gated by CI `make lint`, which lints only root `pmcp --features full`, so it does not block the gate.)
- **Pre-existing `code_mode.rs:557` unused `CodeExecutor` import (out of scope):** carried from before Phase 100 (already documented in 100-03-SUMMARY).

## Known Stubs

None. `verify_accuracy` is fully wired end-to-end: it calls the real executor over the embedded bundle and grades real authored oracles — verified green against the Plan-01 fixture (incl. the synthetic text + bool outputs).

## Notes

- **NON-RELEASABLE INTERMEDIATE STATE is now CLEARED** (from the workbook-tool surface POV): Plans 01-03 flagged that docs/constants referenced `verify_accuracy` before its handler existed and the H3 test bound a string literal. This plan landed `VerifyAccuracyHandler` and swapped the H3 binding to `VerifyAccuracyHandler::NAME`, so `RESERVED_TOOL_NAMES` once again derives exactly from the registered handler `NAME` constants. Plan 05 (the WBVER-04 ALWAYS-bar example/demo) remains to complete the phase.

## Next

Plan 100-05 (WBVER-04): the ALWAYS-bar example/demo extending the tax bundle to exercise `render_workbook(filled)`, `render_workbook(inputs_only)`, and `verify_accuracy` end-to-end in one cohesive narrative.

## Self-Check: PASSED

- SUMMARY.md present at the expected path.
- Both task commits (`618aab76`, `1e677c77`) present in git history.
- `crates/pmcp-workbook-runtime/src/reconcile.rs` exists; `grep -c seed_reference_inputs` >= 2 (def + uses).
- reconcile.rs has NO `pmcp_server_toolkit` / umya / quick-xml / calamine import (purity-check green).
- `VerifyAccuracyHandler` present in handler.rs; registered in mod.rs; `verify_accuracy_output_schema` present in schema.rs.
- H3 binding test references `VerifyAccuracyHandler::NAME` and is green (81-passed workbook suite).
