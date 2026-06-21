---
phase: 100-excel-workbook-built-in-servers-v2
plan: 08
verified: 2026-06-20T00:00:00Z
status: passed
score: 6/6 findings verified closed
scope: pmcp.run dev-team review gap-closure (H1, H2, H3, M4, M5, M6)
branch: fix/cargo-pmcp-deploy-stack-ts
commits_verified: [a908ea21, e0efb666, 4ac2213f]
test_results:
  pmcp-workbook-compiler: "382 passed, 2 ignored, 0 failed"
  pmcp-server-toolkit: "269 passed, 1 ignored, 0 failed (workbook + workbook-embedded features)"
  cargo-pmcp-workbook_explain: "5 passed, 0 failed"
  cargo-pmcp-projection-property: "1 passed, 0 failed"
purity_check: PASSED
low_findings_deferred: [L-i, L-ii, L-iii, L-iv]
---

# Phase 100 Plan 08: pmcp.run Gap-Closure Verification (H1–H3 + M4–M6)

**Scope:** Six verified pmcp.run dev-team findings from `100-REVIEW-pmcp-run.md`.
**Branch:** `fix/cargo-pmcp-deploy-stack-ts`
**Verified:** 2026-06-20
**Status:** PASSED — all six findings are genuinely closed in the live codebase.

This report is SEPARATE from `100-VERIFICATION.md` (which covers the original Phase 100 goal).
It does NOT overwrite that file.

---

## Verification Approach

Each finding was verified by reading the actual source files and running the actual test
suite. SUMMARY.md claims were treated as leads to follow up, not as evidence.

For each finding: (1) the prescribed fix was located in source, (2) the acceptance
criterion grep checks were run, (3) the relevant tests were executed and confirmed green,
(4) live code paths were traced to confirm both compile lanes are gated.

---

## H1 — explain is now the production projection (HIGH — VERIFIED)

**Finding:** `cargo-pmcp/src/commands/workbook/explain_surface.rs` re-derived the tool
surface from scratch (bespoke A1 walker: `reachable_addrs`, `extract_a1_refs`,
`is_input_table`, `harvest_input_pool`, `tool_for_table`), allowing the preview to lie
about the served surface.

**Prescribed fix:** Delete the bespoke walker. Make `explain` drive
`project_tool_surface_from_workbook` (a public, non-persisting production projection in
the compiler). Add a parity test proving explain == served surface over `template.xlsx`.

### Verification

**Bespoke walker deleted (0 matches):**

```
grep -nE "fn (reachable_addrs|extract_a1_refs|is_input_table|harvest_input_pool|tool_for_table)" \
  cargo-pmcp/src/commands/workbook/explain_surface.rs
# Result: 0 matches (OK — bespoke walker deleted)
```

**Production projection called (3 references in explain_surface.rs):**

`cargo-pmcp/src/commands/workbook/explain_surface.rs` line 80:
```rust
let projection = project_tool_surface_from_workbook(path)
    .with_context(|| format!("failed to project the served surface of {}", path.display()))?;
```

The function is imported at line 29 from `pmcp_workbook_compiler` and referenced 3 times
(import + call + doc comment). The explain path has NO re-derivation logic at all — it is
a thin render mapper over the production `Tool` list.

**Public compiler function exists:**

`crates/pmcp-workbook-compiler/src/lib.rs` line 536:
```rust
pub fn project_tool_surface_from_workbook(
    workbook_path: &Path,
) -> Result<ToolSurfaceProjection, CompileError> {
```

The function runs the SAME pre-emit pipeline as `compile_workbook_inner`: ingest →
stage1 → promote_named_outputs → name_named_inputs → promote_harvested_tables →
build_ir_and_dag → output_tables_from_harvest → build_tools, STOPPING before
ratify/reconcile/emit (a pure read-only projection; writes nothing).

**Parity test present and green:**

`crates/pmcp-workbook-compiler/src/template_compile_e2e.rs` line 174:
`fn explain_projection_matches_the_served_tool_surface()` — compiles `template.xlsx`
through the production path AND runs the preview projection on the SAME workbook,
asserting tool names + per-tool input/output keys are byte-identical to
`input_schema_for_tool`/`output_schema_for_tool` (stripped, no `in_`/`out_` prefix).

```
cargo test -p pmcp-workbook-compiler explain_projection_matches
# Result: 1 passed
```

**WR-01 (range + cross-sheet) proven by synthetic test:**

`crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` line 875:
`fn build_tools_surfaces_range_and_cross_sheet_inputs()` — confirms the DAG-based
`upstream_input_leaves` surfaces range-member and cross-sheet inputs that the old
bespoke `extract_a1_refs` walker silently dropped.

```
cargo test -p pmcp-workbook-compiler build_tools_surfaces_range_and_cross_sheet
# Result: 1 passed
```

**Projection-equivalence property arm present and green:**

`cargo-pmcp/src/commands/workbook/explain_surface.rs` line 413:
`fn projection_preserves_build_tools_input_keys()` (proptest) — for arbitrary
(manifest, Tool) pairs, the projected per-tool input-key SET equals the production
`Tool.input_keys` set. Cannot diverge by construction.

```
cargo test -p cargo-pmcp --lib projection_preserves_build_tools_input_keys
# Result: 1 passed
```

**workbook_explain integration test (snapshot updated to TRUE served surface):**

`cargo-pmcp/tests/workbook_explain.rs` — 5 tests cover the CLI render path against the
committed snapshot. The snapshot now matches the served surface (`calculate_tax: [income]`
only; `filing` absent because the DAG does not reach it; `income` has no `[USD]` unit
because the colour-synth role carries none). The old snapshot encoded the walker's lie.

```
cargo test -p cargo-pmcp --test workbook_explain
# Result: 5 passed
```

**Status: VERIFIED**

---

## H2 — value-shaped input name rejected (HIGH — VERIFIED)

**Finding:** `validate_input_keys` did not reject a numeric/value-shaped served key.
A BA typing `60000` in the `name` column produced a served key `"60000"`.

**Prescribed fix:** Reject value-shaped served keys (numeric / pure-number-like) with
a cell-precise `Severity::Error` in `validate_input_keys`, wired through the existing
`refuse_uncallable_inputs` gate in BOTH compile lanes.

### Verification

**`is_value_shaped_key` function exists:**

`crates/pmcp-workbook-compiler/src/lib.rs` line 1174:
```rust
fn is_value_shaped_key(key: &str) -> bool {
```

Called at line 1156 within `validate_input_keys` (and mirrored for outputs at line 1311
within `validate_output_keys`).

**Cell-located error (`value_shaped_input_key_finding`) confirmed:**

`crates/pmcp-workbook-compiler/src/lib.rs` line 1182:
`fn value_shaped_input_key_finding(cell: &str, key: &str) -> LintFinding` — mirrors
`unnamed_input_finding`/`empty_input_key_finding`, carries `Severity::Error` and the
offending cell coordinate.

**Unit tests green:**

`crates/pmcp-workbook-compiler/src/lib.rs` lines 1834, 1849, 1859:
- `value_shaped_input_name_fails_with_cell_located_error`: input name `"60000"` → fails
  with `CompileError::Lint` naming `input-value-shaped-key` and cell `1_Inputs!B2`
- `decimal_and_signed_value_shaped_names_fail`: `"1.5"`, `"-3"`, `"+7"`, `"0.0"` all fail
- `valid_identifier_name_compiles`: `"income"`, `"q1_2024"`, `"tax_owed"`, `"x1"` all pass

```
cargo test -p pmcp-workbook-compiler value_shaped
# Result: 4 passed
```

**Wired in BOTH compile lanes:**

- Seed lane (`compile_workbook_inner`): line 377: `refuse_uncallable_inputs(&manifest)?;`
- Update lane (`prepare_candidate_inner`): line 1583: `refuse_uncallable_inputs(&manifest)?;`

`refuse_uncallable_inputs` calls `validate_input_keys` which calls `is_value_shaped_key`.

**Status: VERIFIED**

---

## H3 — reserved-tool-name gate in BOTH lanes (HIGH — VERIFIED)

**Finding:** An output Table sanitizing to a reserved meta-tool name (`explain`,
`get_manifest`, `diff_version`, `render_workbook`) was silently overwritten without a
compile error.

**Prescribed fix:** `refuse_reserved_output_table_names` blocking gate derived from
`RESERVED_TOOL_NAMES` (a const in the runtime leaf bound to the handler NAME constants
by a toolkit test), wired at stage-1 in BOTH compile lanes.

### Verification

**`RESERVED_TOOL_NAMES` in the runtime leaf:**

`crates/pmcp-workbook-runtime/src/manifest_model.rs` line 212:
```rust
pub const RESERVED_TOOL_NAMES: [&str; 4] =
    ["explain", "get_manifest", "diff_version", "render_workbook"];
```

Not hardcoded anywhere else. The compiler imports it via:
`crates/pmcp-workbook-compiler/src/lib.rs` line 192:
```rust
pub use pmcp_workbook_runtime::RESERVED_TOOL_NAMES;
```

No compiler→toolkit dependency introduced (purity boundary intact).

**Toolkit binding test:**

`crates/pmcp-server-toolkit/src/workbook/handler.rs` line 672:
`fn reserved_tool_names_match_the_registered_meta_tool_names()` — asserts
`pmcp_workbook_runtime::RESERVED_TOOL_NAMES == [ExplainHandler::NAME, GetManifestHandler::NAME,
DiffVersionHandler::NAME, RenderWorkbookHandler::NAME]`. If a handler NAME constant changes,
this test fails. Green:

```
cargo test -p pmcp-server-toolkit --features "workbook workbook-embedded" reserved_tool_names_match
# Result: 1 passed
```

**Gate count (defn + 2 call sites = at least 3):**

```
grep -c refuse_reserved_output_table_names crates/pmcp-workbook-compiler/src/lib.rs
# Result: 6 (defn + 2 call sites + tests)
```

**Wired in BOTH compile lanes:**

- Seed lane (`compile_workbook_inner`): line 428
- Update lane (`prepare_candidate_inner`): line 1623

**Tests confirm reserved-name → `CompileError::Lint` with cell location:**

Lines 2157-2224 in lib.rs: `output_table_sanitizing_to_reserved_name_fails` (covers
`Explain`, `explain `, ` explain`, `EXPLAIN` → all fail with cell `Data!B10` located);
`reserved_name_gate_covers_all_four_meta_tools`; `non_reserved_output_table_name_passes_reserved_gate`;
`reserved_set_is_derived_from_the_shared_const`.

```
cargo test -p pmcp-workbook-compiler reserved
# Result: 5 passed
```

**Status: VERIFIED**

---

## M4 — validate_output_keys in BOTH lanes (MEDIUM — VERIFIED)

**Finding:** No `validate_output_keys` existed. Duplicate/empty/value-shaped output keys
were unguarded.

**Prescribed fix:** `validate_output_keys` + `refuse_uncallable_outputs` mirroring
`validate_input_keys` / `refuse_uncallable_inputs`, wired in BOTH compile lanes.

### Verification

**`validate_output_keys` exists:**

`crates/pmcp-workbook-compiler/src/lib.rs` line 1301:
```rust
fn validate_output_keys(manifest: &Manifest, report: &mut LintReport) {
```

Mirrors `validate_input_keys` over `Role::Output` cells. Checks: duplicate served keys,
empty/whitespace served key, value-shaped served key — each a cell-precise `Severity::Error`.

**`refuse_uncallable_outputs` exists and gate count confirmed:**

```
grep -c refuse_uncallable_outputs crates/pmcp-workbook-compiler/src/lib.rs
# Result: 7 (defn + 2 call sites + 4 test references)
```

Call sites: line 381 (seed lane), line 1586 (update lane).

**Tests green:**

- `duplicate_output_served_keys_fail` (line 1873): two outputs stripping to `tax` fail
  with `output-key-collision` naming both cell coords
- `value_shaped_output_key_fails` (line 1892): output name `"18241"` fails with
  `output-value-shaped-key` naming the cell

```
cargo test -p pmcp-workbook-compiler value_shaped
# Result: 4 passed (covers both input and output value-shaped tests)
```

**Status: VERIFIED**

---

## M5 — get_manifest advertises the stripped served key (MEDIUM — VERIFIED)

**Finding:** `curated_manifest` / `input_projection` emitted `role.name` (the raw
`in_income`/`out_tax_owed` prefixed name), not the stripped served key.

**Prescribed fix:** Emit `json_key_for_role(role)` as the advertised `name` in both
`input_projection` and the inline output `json!` block. Keep raw name as `governance_name`.

### Verification

**`json_key_for_role` count in handler.rs:**

```
grep -c json_key_for_role crates/pmcp-server-toolkit/src/workbook/handler.rs
# Result: 5
```

Call sites: line 398 (input projection `name`), line 421 (output projection `name`).

**`input_projection` source (line 388–405):**

```rust
fn input_projection(role: &pmcp_workbook_runtime::CellRole) -> Value {
    use pmcp_workbook_runtime::{json_key_for_role, InputTier};
    ...
    json!({
        "name": json_key_for_role(role),
        "governance_name": role.name,
        ...
    })
}
```

Raw prefixed name kept under `governance_name` only.

**`curated_manifest` output projection (line 420–425):**

```rust
Role::Output => outputs.push(json!({
    "name": json_key_for_role(role),
    "governance_name": role.name,
    ...
})),
```

**Toolkit M5 test green:**

`handler.rs` line 983: `fn get_manifest_advertises_the_stripped_served_keys()` — asserts:
1. No advertised input/output name carries `in_`/`out_` prefix.
2. `get_manifest` input names == workbook-wide `input_schema_for_manifest` keys.
3. `get_manifest` output names == workbook-wide `output_schema_for_manifest` keys.
4. Every per-tool served key is discoverable in `get_manifest`.

```
cargo test -p pmcp-server-toolkit --features "workbook workbook-embedded" get_manifest_advertises_the_stripped
# Result: 1 passed
```

**Status: VERIFIED**

---

## M6 — per-tool oracle wired from cached cell values (MEDIUM — VERIFIED)

**Finding:** `oracle_value` returned `None` for outputs (outputs are never tiered) →
every tool's oracle was empty → `reconcile_tools` never blocked on the production table
path (the per-tool safety net was a vacuous dead net).

**Prescribed fix:** Populate each tool's oracle from the authored cached `<v>` values
(the same map `comparison_from_outputs` builds). Pass `output_oracles: &BTreeMap<String,
CellValue>` into `build_tools`/`build_one_tool`. Wire `output_oracle_map` + `reconcile_output_tables`
in BOTH compile lanes. A perturbed cached output now blocks the per-tool reconcile.

### Verification

**`build_tools` signature updated:**

`crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` line 89:
```rust
pub fn build_tools(
    manifest: &Manifest,
    dag: &Dag,
    output_tables: &[OutputTable],
    output_oracles: &BTreeMap<String, CellValue>,
) -> Result<(Vec<Tool>, Vec<LintFinding>), String> {
```

**`build_one_tool` oracle fill (line 181–187):**

```rust
if let Some(value) = output_oracles
    .get(cell_key)
    .cloned()
    .or_else(|| oracle_value(role))
{
    oracle.insert(json_key_for_role(role), value);
}
```

Prefers the cached map; falls back to tier-based `oracle_value` for synthetic tests.
`oracle_value`'s stale "supplied by Plan 04" comment corrected.

**`output_oracle_map` function exists:**

`crates/pmcp-workbook-compiler/src/lib.rs` line 774:
`fn output_oracle_map(map: &WorkbookMap, manifest: &Manifest) -> BTreeMap<String, CellValue>`
— builds the cached-`<v>` value-by-cell-key map from the workbook's cached output cells
(the same source `comparison_from_outputs` uses).

**`reconcile_output_tables` wired in BOTH lanes:**

- Seed lane: lines 434–435
- Update lane: lines 1625–1626

Both call: `let output_oracles = output_oracle_map(&map, &manifest);` then
`reconcile_output_tables(&output_tables, &dag, &manifest, &run, &output_oracles)?;`

**M6 perturb test present and green:**

`crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` line 843:
`fn perturbed_cached_output_blocks_per_tool_reconcile()`:
- Builds a tool's oracle from cached value `18241.0`.
- Sets computed value to `99999.0` (perturbed) → `reconcile_tools` → `any_mismatch() == true`.
- Sets computed value to `18241.0` (matching) → `any_mismatch() == false`.

```
cargo test -p pmcp-workbook-compiler perturbed_cached_output
# Result: 1 passed
```

**Status: VERIFIED**

---

## Regression Checks

### purity-check

```
make purity-check
# Result: PASSED
# - Layer 1: pmcp-workbook-runtime / pmcp-workbook-dialect reader-free
# - Phase 92: pmcp-server-toolkit workbook+workbook-embedded reader-free
# - Phase 93: pmcp-workbook-compiler reader-present (umya confined)
# - Phase 95: pmcp-workbook-server reader-free
# - Layer 2: cargo-deny bans clean
```

No new compiler→toolkit dependency was introduced. `RESERVED_TOOL_NAMES` lives in the
runtime leaf (the existing shared boundary), so the compiler reads it without breaching
the purity gate.

### Named-range corpus still compiles

The Rule-4 named-range path is intact: `promote_named_outputs`/`name_named_inputs`
are untouched (L-ii deferred). The named-range corpus (tax-calc/loan-calc/leap1900)
routes the single-tool fallback and stays green across all 382 compiler tests.

### Full test suites green

| Suite | Result |
|-------|--------|
| `cargo test -p pmcp-workbook-compiler` | 382 passed, 2 ignored, 0 failed |
| `cargo test -p pmcp-server-toolkit --features "workbook workbook-embedded"` | 269 passed, 1 ignored, 0 failed |
| `cargo test -p cargo-pmcp --test workbook_explain` | 5 passed, 0 failed |
| `cargo test -p cargo-pmcp --lib projection_preserves_build_tools_input_keys` | 1 passed |
| `cargo test -p pmcp-workbook-runtime` | (transitively green via toolkit suite) |

---

## LOW Findings — Correctly Deferred

The 4 LOW findings from `100-REVIEW-pmcp-run.md` were explicitly deferred to a tracked
follow-up and are UNTOUCHED in this plan.

| Finding | Status | Evidence |
|---------|--------|---------|
| L-i: overrides keys raw vs stripped in same schema | DEFERRED | `schema.rs:401` unchanged |
| L-ii: legacy `in_*`/`out_*` compile path alongside table path | DEFERRED | `promote_named_outputs`/`name_named_inputs` present (19 references in lib.rs) |
| L-iii: stale rustdoc "five tools / calculate" | DEFERRED | `mod.rs:152` still says "FIVE served tools" |
| L-iv: `tax-calc@1.1.0` byte-stability golden hand-built | DEFERRED | unchanged |

None of these were modified, consistent with the Rule-4 deferral contract.

---

## Pre-existing Out-of-Scope Reds (Not Failures)

The following pre-existing issues were noted but are out of scope for this plan and were
not introduced by it:

- `pmcp-toolkit-mysql`: `sqlx E0277` (pre-existing build failure)
- `code_mode.rs:557`: unused-import warning (pre-existing; not in touched crates)

---

## Findings Summary

| Finding | Severity | Status | Key Evidence |
|---------|----------|--------|-------------|
| H1 — explain divergent re-implementation | HIGH | VERIFIED | bespoke walker deleted; `project_tool_surface_from_workbook` called; parity test green |
| H2 — value-shaped input name ships | HIGH | VERIFIED | `is_value_shaped_key` + `value_shaped_input_key_finding` in `validate_input_keys` wired in both lanes; `60000` → `CompileError::Lint` |
| H3 — reserved-tool-name collision silently drops tool | HIGH | VERIFIED | `RESERVED_TOOL_NAMES` in runtime leaf; `refuse_reserved_output_table_names` in both lanes; binding test green |
| M4 — output-key collisions unguarded | MEDIUM | VERIFIED | `validate_output_keys`/`refuse_uncallable_outputs` in both lanes; dup/value-shaped output key → `CompileError::Lint` |
| M5 — get_manifest reports raw prefixed name | MEDIUM | VERIFIED | `json_key_for_role(role)` as `name`; `governance_name` for raw; toolkit test green |
| M6 — per-tool reconcile vacuous | MEDIUM | VERIFIED | `output_oracle_map` + `output_oracles` param in `build_tools`; wired in both lanes; perturb test blocks emit |

**Verdict: ALL SIX FINDINGS ARE GENUINELY CLOSED. The publish blockers are cleared.**

---

_Verified: 2026-06-20_
_Verifier: Claude (gsd-verifier) — goal-backward, adversarial stance, source-read + test-run_
_Branch: fix/cargo-pmcp-deploy-stack-ts_
