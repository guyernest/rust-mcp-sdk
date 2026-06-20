---
phase: 100-excel-workbook-built-in-servers-v2
plan: 03
subsystem: artifact-model-dag
tags: [multi-tool, artifact-model, dag-reachability, upstream-input-leaves, build-tools, transitional-shim, cargo-fuzz, proptest, golden-regen]

# Dependency graph
requires:
  - phase: 100-excel-workbook-built-in-servers-v2
    plan: 02
    provides: "TableRecord harvest + pure §3.3 projectors + the read_workbook_contained catch_unwind seam (relocated to the eager read)"
  - phase: 11
    provides: "the reader-free artifact_model.rs (CellEntry/CellMap) + dag.rs (Dag/toposort) on the umya-free runtime boundary"
provides:
  - "The shared multi-tool model: pub struct Tool {name, description, input_keys, outputs, oracle} + CellMap{inputs[], tools[]} in artifact_model.rs (the reader-free contract Plan 04 served fan-out consumes)"
  - "A transitional #[deprecated] CellMap::outputs() accessor flattening tools[].outputs (keeps the whole workspace compiling green at the end of wave 3; Plan 04 Task 1 deletes it)"
  - "pub fn Dag::upstream_input_leaves(dag, output_cell, input_cells) -> BTreeSet — the cycle-safe, total, subset-correct per-tool minimal input derivation (§4.2), property- AND fuzz-proven"
  - "pub fn build_tools(manifest, dag, output_tables) -> (Vec<Tool>, Vec<LintFinding>) + OutputTable membership — groups output cells by Table, derives minimal input_keys, emits the §4.2 feeds-no-tool lint"
  - "A regenerated tax-calc@1.1.0 golden bundle in the {inputs, tools[]} shape"
affects: [plan-04-served-fan-out, schema-per-tool, handler-per-tool, reconcile-per-tool, accessor-removal]

# Tech tracking
tech-stack:
  added:
    - "proptest 1 (dev-dep on pmcp-workbook-runtime) — the SC2 reachability property harness; existing workspace dep, dev-only, off the purity cone"
  patterns:
    - "Interface-first model lift: define the multi-tool Tool/CellMap + the pure DAG algorithm in the reader-free runtime FIRST, hand Plan 04 the types"
    - "Transitional #[deprecated] accessor as the sanctioned cross-wave compile bridge (NOT a merge of plans, NOT a SATD TODO): flat outputs() derived from tools[] keeps old call sites compiling, deleted atomically by the next plan"
    - "Caller-supplied membership (OutputTable) for grouping the manifest cannot express: a CellRole records no owning Table, so the offline caller passes the harvested Table membership explicitly"

key-files:
  created:
    - crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dag_upstream_leaves.rs
    - .planning/phases/100-excel-workbook-built-in-servers-v2/deferred-items.md
  modified:
    - crates/pmcp-workbook-runtime/src/artifact_model.rs
    - crates/pmcp-workbook-runtime/src/dag.rs
    - crates/pmcp-workbook-runtime/src/lib.rs
    - crates/pmcp-workbook-runtime/src/bundle_loader.rs
    - crates/pmcp-workbook-runtime/Cargo.toml
    - crates/pmcp-workbook-compiler/src/artifact/cell_map.rs
    - crates/pmcp-workbook-compiler/src/artifact/mod.rs
    - crates/pmcp-workbook-compiler/src/reemit_golden.rs
    - crates/pmcp-workbook-compiler/fuzz/Cargo.toml
    - crates/pmcp-server-toolkit/src/workbook/schema.rs
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - crates/pmcp-server-toolkit/src/workbook/mod.rs
    - crates/pmcp-server-toolkit/src/workbook/input.rs
    - crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
    - crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/cell_map.json
    - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock

key-decisions:
  - "build_tools takes an explicit output_tables: &[OutputTable] membership param (NOT the bare (manifest, dag) the plan named) — a CellRole records no owning Table, so the (manifest, dag) pair alone cannot group output cells by Table; the offline caller (which harvested the Table areas) supplies membership, the unit tests build it synthetically (Rule 3 blocking deviation)"
  - "build_cell_map KEPT as the transitional single-tool entry: it wraps all outputs in ONE Tool so the existing emit + served call sites keep working; build_tools is the new tested multi-tool primitive Plan 04 wires into the orchestrator"
  - "Regenerated the tax-calc@1.1.0 golden to {inputs, tools[]} via the env-gated regenerate_committed_golden — the served loader deserializes cell_map.json into the NEW CellMap, so the old outputs[]-shaped golden would fail to deserialize at runtime (not just compile); the evidence hash + BUNDLE.lock follow"
  - "feeds-no-tool lint is Severity::Warning (not Error): an orphan input is a workbook-authoring smell, not a conformance blocker (matches the §4.2 disposition and the D-05 only-Error-blocks rule)"

patterns-established:
  - "Pattern: the per-tool input-derivation traversal is a single cycle-safe DFS (seen-guard + BTreeSet) modeled byte-for-byte on PATTERNS §3 / RESEARCH Pattern 2 — cog well under 25, proven total by a fuzz target feeding cyclic edge sets"
  - "Pattern: golden bundle regeneration via the existing #[ignore] regenerate_committed_golden test is the single source for refreshing cell_map.json + BUNDLE.lock together (the byte-stability guard then re-asserts member-for-member equality)"

requirements-completed: [WBV2-03]

# Metrics
duration: ~70min
completed: 2026-06-20
---

# Phase 100 Plan 03: Multi-tool Model Lift + DAG-Derived Per-Tool Inputs Summary

**The shared artifact model is lifted from single-tool `CellMap{inputs[], outputs[]}` to multi-tool `CellMap{inputs[], tools[]}` (each `Tool` owns its outputs + a minimal DAG-derived `input_keys`); the new cycle-safe `Dag::upstream_input_leaves` derives each tool's minimal input set (property- AND fuzz-proven `⊆ inputs` over hostile DAGs); `build_tools` groups output cells by Table and emits the §4.2 feeds-no-tool lint; and a transitional `#[deprecated]` `CellMap::outputs()` accessor keeps the whole workbook workspace compiling green at the end of wave 3 (Plan 04 deletes it).**

## Performance

- **Duration:** ~70 min
- **Tasks:** 5
- **Files:** 17 modified, 2 created

## Accomplishments

- **Task 1 — Tool type + CellMap lift + transitional accessor** (`51aa0f92`): Added `pub struct Tool {name, description, input_keys, outputs, oracle}` to `artifact_model.rs` (reader-free boundary; drops `Eq` — `oracle` is `f64`-bearing `CellValue`; full serde+schemars derive set). Lifted `CellMap`: the bare `outputs: Vec<CellEntry>` FIELD became `tools: Vec<Tool>` (N=1 single-Table case is just `tools.len()==1`, never special-cased). Added the `#[deprecated]` `CellMap::outputs()` accessor flattening `tools[].outputs` with a `// TRANSITIONAL (Plan 03→04)` non-SATD marker. Exported `Tool`; updated the in-crate `bundle_loader` test fixtures. 4 tests: Tool round-trip, CellMap{tools} round-trip, `outputs()` flatten + N=1 equality.
- **Task 2 — upstream_input_leaves + property test** (`a9e310b9`): Added `pub fn upstream_input_leaves(dag, output_cell, input_cells) -> BTreeSet<String>` beside `toposort` — a single cycle-safe DFS over `dependencies_of` (seen-guard; `BTreeSet` determinism) collecting `Role::Input` leaves; constant-only paths excluded by construction. 5 fixture tests (exact-reachable, constant-only-exclusion, input-is-leaf, shared-intermediate union-per-output, cycle-terminates) + a proptest property (256 cases): derived `⊆ inputs` ∧ every leaf reachable upstream. Added `proptest` dev-dep.
- **Task 3 — build_tools per output Table** (`72581bd5`): Added `build_tools(manifest, dag, output_tables) -> (Vec<Tool>, Vec<LintFinding>)` + the `OutputTable{name, description, output_cells}` membership type. Per Table: union `upstream_input_leaves` over its output cells → minimal `input_keys` (mapped to json_keys), per-tool `outputs` + `oracle`. Fail-loud on zero output Tables; `WARNING` feeds-no-tool lint per orphan input; constant-only paths silently excluded. Decomposed into `build_one_tool`/`feeds_no_tool_findings` (cog ≤25 each). The §4.2 motivating example proves `Calculate_Tax.input_keys == [filing, income]` and `Estimate_Refund.input_keys == [filing, income, withheld]`. Regenerated the `tax-calc@1.1.0` golden to the `{inputs, tools[]}` shape; mechanically switched every still-old `.outputs` FIELD read to the transitional `.outputs()` accessor.
- **Task 4 — fuzz target** (`36f3dc51`): `dag_upstream_leaves` decodes arbitrary bytes into a bounded-node `Dag` with arbitrary (incl. cyclic/self-loop) edges, a random output cell, and a random input subset, then calls `upstream_input_leaves`. Invariant: ALWAYS returns (never panics/hangs/overflows — the seen-guard) AND result `⊆ input_cells` (T-100-06: no non-input cell leaks as a derived tool input). 20 000 runs / 60 s: zero crashes.
- **Task 5 — cross-wave compile gate** (`610f1c3d`): Verified the whole workbook cone (runtime + compiler + server-toolkit`[workbook]` + workbook-server) BUILDS and TEST-COMPILES green at the end of wave 3 via the transitional accessor; `make purity-check` PASSED. Recorded the pre-existing out-of-scope reds.

## Task Commits

1. **Task 1: Tool type + CellMap lift + transitional outputs() accessor** — `51aa0f92` (feat)
2. **Task 2: Dag::upstream_input_leaves + property test** — `a9e310b9` (feat)
3. **Task 3: build_tools per output Table with DAG-derived input_keys** — `72581bd5` (feat)
4. **Task 4: dag_upstream_leaves fuzz target** — `36f3dc51` (test)
5. **Task 5: cross-wave compile gate + out-of-scope ledger** — `610f1c3d` (chore)

## Decisions Made

- **`build_tools` takes explicit `output_tables` membership** (Rule 3 deviation, see below): the bare `(manifest, dag)` signature the plan named cannot group output cells by Table — a `CellRole` records no owning Table. The offline caller (which harvested the Table areas in Plan 02) passes membership; the unit tests build it synthetically.
- **`build_cell_map` kept as the transitional single-tool entry**: it wraps all outputs in ONE `Tool` so the existing emit + served call sites keep working; `build_tools` is the new multi-tool primitive Plan 04 wires into the orchestrator.
- **Regenerated the golden to `{inputs, tools[]}`**: the served loader deserializes `cell_map.json` into the NEW `CellMap`, so the old `outputs[]` golden would fail to *deserialize* (a runtime break, not just a compile one). The evidence hash + BUNDLE.lock follow via the env-gated regenerate path.
- **feeds-no-tool lint is `Warning`** (D-05: only `Error` blocks): an orphan input is an authoring smell, not a conformance blocker.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `build_tools` signature carries an explicit `output_tables: &[OutputTable]` membership param (not the bare `(manifest, dag)`)**
- **Found during:** Task 3
- **Issue:** The plan's `<action>` named `build_tools(manifest, dag)` and said grouping is "by their owning output-Table name (from the harvested TableRecord membership)". But a `CellRole` records NO owning-Table field, and `Dag` carries only cell-key edges — so `(manifest, dag)` alone cannot group output cells by Table. Deriving membership inside `build_tools` from raw RangeRef A1-area containment would add parsing complexity (cog) and re-derive what the harvest already knows.
- **Fix:** Introduced `pub struct OutputTable {name, description, output_cells: Vec<String>}` and the signature `build_tools(manifest, dag, output_tables) -> (Vec<Tool>, Vec<LintFinding>)`. The offline caller (Plan 04 orchestrator) supplies the harvested Table membership; the unit tests build it synthetically. This keeps `build_tools` a thin pure function (cog ≤25) and matches the plan's intent ("from the harvested TableRecord membership") — the membership is just passed, not re-derived.
- **Files modified:** crates/pmcp-workbook-compiler/src/artifact/cell_map.rs, crates/pmcp-workbook-compiler/src/artifact/mod.rs
- **Verification:** `cargo test -p pmcp-workbook-compiler --lib build_tools` (4 tests green, incl. the §4.2 two-Tool example).
- **Committed in:** `72581bd5` (Task 3)

**2. [Rule 3 - Blocking] Regenerated the `tax-calc@1.1.0` golden bundle + switched ALL still-old `.outputs` FIELD reads to `.outputs()` to keep the workspace deserializing AND compiling**
- **Found during:** Task 3 (the `reemit_golden`/`fixture_byte_stability` tests + the server-toolkit build)
- **Issue:** The model lift changed `cell_map.json`'s shape from `outputs[]` to `tools[]`. The served loader deserializes `cell_map.json` into the NEW `CellMap`, so the committed golden (and the cargo-pmcp embedded copy) with `outputs[]` would fail to *deserialize* at load — a runtime break, not only the compile break the plan's transitional accessor addresses. In addition, the still-old `CellMap.outputs` FIELD reads in `schema.rs`/`handler.rs`/`mod.rs`/`input.rs` + several test fixtures broke the build.
- **Fix:** (a) Regenerated the committed `tax-calc@1.1.0` golden to `{inputs, tools[]}` (single transitional tool) via the env-gated `regenerate_committed_golden` test; the byte-stability guard re-asserts member equality. (b) Updated `fixture_gen.rs` + the `reemit_golden` seed-coord extractor to read `tools[].outputs`. (c) Mechanically switched every still-old `.outputs` FIELD read to the transitional `.outputs()` accessor (each with a `#[allow(deprecated)]` + a `// TRANSITIONAL (Plan 03→04)` marker), restoring compilation — Plan 04 reshapes these per-tool and deletes the accessor. This work was originally scoped to Task 5 but had to land in Task 3 so the compiler/server-toolkit tests could build.
- **Files modified:** the server-toolkit workbook files + fixture_gen.rs + fixture_byte_stability.rs + the regenerated golden cell_map.json/BUNDLE.lock + reemit_golden.rs
- **Verification:** `cargo test -p pmcp-workbook-compiler` (525 passed) + `cargo test -p pmcp-server-toolkit --features workbook` (252 passed) + the golden byte-stability test green.
- **Committed in:** `72581bd5` (Task 3) and verified in `610f1c3d` (Task 5)

---

**Total deviations:** 2 auto-fixed (both Rule 3 blocking). Both are mechanical/structural consequences of the model lift, confined to the plan's own task files plus the golden the lift necessarily reshapes. No behavioral scope creep — `build_cell_map` stays single-tool-transitional; Plan 04 owns the per-Table fan-out.

## Cross-Wave Compile Gate (Task 5)

- `cargo build` + `cargo test --no-run` for the workbook cone (runtime + compiler + server-toolkit`[workbook]` + workbook-server) BOTH exit 0 — wave 4 starts from green.
- Every still-old `CellMap` output read uses the transitional `.outputs()` accessor — grep shows NO remaining bare `.outputs` FIELD access on a `CellMap` anywhere in `crates/`/`cargo-pmcp/`.

### Call sites switched to `.outputs()` (for Plan 04 to reshape per-tool + then delete the accessor)

| File | Site | Kind |
|------|------|------|
| `crates/pmcp-server-toolkit/src/workbook/schema.rs:78` | `output_schema_for_manifest` iterates outputs | production |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs:80` | `project_outputs` iterates outputs | production |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs:235` | zero-output operator warning | production |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` (tests) | uncomputed-output + all-present projection tests | test |
| `crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs:79` | four-named-outputs assertion | test |
| `crates/pmcp-workbook-compiler/src/artifact/mod.rs` (test) | emitted-bundle output count | test |
| `crates/pmcp-workbook-compiler/src/reemit_golden.rs` | golden output count + seed-coord equality (now reads `tools[].outputs`) | test |

The accessor lives at `crates/pmcp-workbook-runtime/src/artifact_model.rs` (`impl CellMap { fn outputs() }`). Plan 04 Task 1 removes it; the grep gate `fn outputs` no longer existing on `CellMap` is its removal criterion.

## Threat Model Outcome

- **T-100-06 (Tampering — a computed/constant cell wrongly derived as a tool input):** mitigated. `upstream_input_leaves` only collects cells in `input_cells` (`Role::Input`); constants/formulas are excluded by construction. The proptest property asserts derived `⊆ inputs`; the `dag_upstream_leaves` fuzz target asserts `⊆ inputs` over 20 000 hostile (incl. cyclic) DAGs.
- **T-100-07 (Elevation — Tool type pulling a umya/reader dep into the served tree):** mitigated. `Tool` lives in reader-free `artifact_model.rs` with the serde-only derive set; `make purity-check` PASSED (no umya/calamine/quick-xml/swc/pmcp-code-mode on any workbook cone; proptest is dev-only).
- **T-100-16 (Tampering — transitional `outputs()` shim surviving past Plan 04 as dead SATD):** in force. The accessor is `#[deprecated]`-annotated with a Plan 04 removal note + a non-SATD `// TRANSITIONAL` marker; Plan 04 Task 1 has an explicit removal criterion (grep `fn outputs` absent on `CellMap`).
- **T-100-SC (package installs):** accept. `proptest 1` is an existing workspace dev-dep (the Plan 02 compiler harness already pins it); no new external package install.

## Known Stubs

- `build_cell_map` emits ONE transitional `Tool` wrapping all outputs (single-tool projection). This is intentional + documented (the `// TRANSITIONAL (Plan 03→04)` marker + the deprecated accessor): Plan 04 wires the per-Table `build_tools` fan-out (with the harvested tool name/description + DAG-derived input_keys) into the orchestrator and retires both the single-tool `build_cell_map` shape and the `.outputs()` accessor. This plan ships the model + the pure primitive (interface-first), not the served fan-out (Pitfall 3 — that lands atomically in Plan 04).
- The transitional golden carries one `calculate` tool with empty `input_keys` (the single-tool shape has no per-tool input derivation); Plan 04's regeneration populates per-Table `input_keys`.

## Threat Flags

None — no new network endpoint, auth path, file-access pattern, or trust-boundary schema introduced beyond the planned `Tool`/`CellMap` model lift (already in the `<threat_model>`).

## Deferred Items (out-of-scope, pre-existing — see deferred-items.md)

- `pmcp-toolkit-mysql` build error (sqlx `SqlSafeStr` API bump) — VERIFIED present at HEAD via `git stash`; unrelated SQL-connector crate. The full `cargo build --workspace` cannot be globally green until this is fixed independently; Plan 03 verifies the workbook cone green instead.
- cargo-pmcp 3 pre-existing test failures (embedded-mirror drift: the embedded `tax-calc.xlsx` differs at byte 19 + the embedded BUNDLE.lock evidence hash already diverged; + an unrelated auth-cache proptest). VERIFIED identical 437-passed/3-failed at HEAD AND with Plan 03 — ZERO new failures. NOTE for Plan 04: when it regenerates the golden into the multi-tool shape, also refresh the cargo-pmcp embedded mirror so these go green.
- One pre-existing clippy `map_or` test-only warning + one `code_mode.rs` unused-import warning — not introduced by Plan 03.

## User Setup Required

None.

## Next Phase Readiness

- The `Tool`/`CellMap{inputs, tools[]}` model + `upstream_input_leaves` + `build_tools`/`OutputTable` are the exact contract Plan 04's served fan-out consumes (schema.rs/handler.rs/mod.rs per-tool + the reconcile re-key). Interface-first: Plan 04 gets the types, not a scavenger hunt.
- Plan 04 Task 1's first acts: wire `build_tools` into the orchestrator (replacing the transitional single-tool `build_cell_map`), regenerate the golden into the per-Table multi-tool shape (refreshing the cargo-pmcp embedded mirror too), reshape the 7 listed `.outputs()` call sites to per-tool iteration, and DELETE the `CellMap::outputs()` accessor (grep `fn outputs` absent on `CellMap` is the removal gate).

## Self-Check: PASSED

- Created files verified present: `crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dag_upstream_leaves.rs`, `.planning/phases/100-excel-workbook-built-in-servers-v2/deferred-items.md`.
- Commits verified in git log: `51aa0f92`, `a9e310b9`, `72581bd5`, `36f3dc51`, `610f1c3d`.
- `cargo test -p pmcp-workbook-runtime -p pmcp-workbook-compiler`: 525 passed, 2 ignored, 0 failed.
- `cargo test -p pmcp-server-toolkit --features workbook`: 252 passed, 0 failed.
- `cargo test -p pmcp-workbook-server`: 24 passed.
- `cargo +nightly fuzz run dag_upstream_leaves -- -runs=20000 -max_total_time=60`: 20 000 runs, zero crashes.
- `cargo build` + `cargo test --no-run` (workbook cone): both exit 0.
- `make purity-check`: PASSED.
- `cargo fmt --all -- --check`: clean. Clippy on changed files: clean (only pre-existing out-of-scope warnings remain).

---
*Phase: 100-excel-workbook-built-in-servers-v2*
*Completed: 2026-06-20*
