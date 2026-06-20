---
phase: 100-excel-workbook-built-in-servers-v2
verified: 2026-06-20T12:00:00Z
status: gaps_found
score: 4/5 must-haves verified
overrides_applied: 0
gaps:
  - truth: "Each output table becomes a distinct, named, described MCP tool with a DAG-derived input schema and an emitted output schema (structuredContent)"
    status: failed
    reason: "The production compile pipeline (compile_workbook -> compile_workbook_inner -> gate::accept::promote -> emit_bundle) calls build_cell_map() at artifact/mod.rs:180, NOT build_tools(). build_cell_map() wraps all Role::Output cells in ONE transitional tool named manifest.workflow with input_keys: Vec::new(). build_tools / reconcile_tools / tool_name_collision_findings have ZERO non-test callers — confirmed by exhaustive grep. Separately, input_schema_for_tool() in schema.rs:336-348 projects only keys in tool.input_keys; for a build_cell_map-produced tool with empty input_keys, the served schema advertises an empty inputs.properties object while validate_input / seed_supplied_inputs accepts any known input key — schema is stricter than runtime, inverting the V5 invariant. WBV2-04 is the primary deliverable of this phase and it does not reach the production compile path."
    artifacts:
      - path: "crates/pmcp-workbook-compiler/src/artifact/mod.rs"
        issue: "Line 180: let cell_map = build_cell_map(&ratified).map_err(EmitError::CellMap)? — single-tool path always used; build_tools not called anywhere on the production path"
      - path: "crates/pmcp-workbook-compiler/src/artifact/cell_map.rs"
        issue: "build_tools (line 86), reconcile_tools (line 321), tool_name_collision_findings (line 347) are pub fns with zero non-test callers — only called from #[cfg(test)] mod tests{} blocks in cell_map.rs itself"
      - path: "crates/pmcp-server-toolkit/src/workbook/schema.rs"
        issue: "input_schema_for_tool (line 336): projects only tool.input_keys; for a build_cell_map-produced tool with empty input_keys, inserts NOTHING — served schema advertises no inputs while runtime accepts all"
      - path: "crates/pmcp-workbook-compiler/src/reemit_golden.rs"
        issue: "The committed golden (tax-calc@1.1.0) was regenerated out-of-band into the two-Table shape; the producer/consumer proof asserts only is_subset relations (lines 138-145), so a fresh compile yielding one tool with empty input_keys passes these checks while the golden has two tools with populated input_keys"
    missing:
      - "Wire build_tools() + tool_name_collision_findings() into emit_bundle() (artifact/mod.rs) replacing the build_cell_map() call; requires OutputTable membership from the harvested TableRecord data (ingest layer already harvests table names/areas in Plan 02)"
      - "Add collision-lint findings into the stage-1 Error gate in compile_workbook_inner"
      - "Wire reconcile_tools() into the production reconcile step in compile_workbook_inner (replacing comparison_from_outputs / reconcile::reconcile for the per-Table path)"
      - "Either fix input_schema_for_tool fallback for empty input_keys (as CR-02 suggests: treat empty as project-all) OR ensure build_tools always populates input_keys before served registration"
      - "Regenerate the committed golden via a real compile once the wiring lands, so the producer/consumer proof covers the actual production path"
deferred: []
human_verification: []
---

# Phase 100: Excel Workbook Built-in Servers V2 Verification Report

**Phase Goal:** Redesign the workbook→MCP tool surface around a table-based authoring contract so a BA authors named Excel Tables (columns name|value|description|tier) and the compiler derives a well-named/described/typed MCP tool surface.
**Verified:** 2026-06-20T12:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Preamble: Independent Verification of CR-01 and CR-02

The code review (100-REVIEW.md) raised two CRITICAL findings. The instructions require independent verification against the source. All verdicts below are based on reading the actual files — not SUMMARY.md claims and not the reviewer's prose.

### CR-01 Verification (multi-tool fan-out dead on the production path)

**Evidence — artifact/mod.rs line 180:**
```rust
let cell_map = build_cell_map(&ratified).map_err(EmitError::CellMap)?;
```
This is the ONLY cell_map construction call in `emit_bundle`. `build_cell_map` (cell_map.rs:460) is documented inline as "the TRANSITIONAL single-tool path" and wraps every `Role::Output` cell in ONE tool named `manifest.workflow` with `input_keys: Vec::new()` (cell_map.rs:485-491).

**Evidence — grep for non-test callers of build_tools / reconcile_tools / tool_name_collision_findings:**
```
grep -rn "build_tools|reconcile_tools|tool_name_collision_findings" \
  crates/pmcp-workbook-compiler/src/ --include="*.rs"
```
Results: Every match is either the `pub fn` definition, a `pub use` re-export, or code inside `#[cfg(test)] mod tests`. There is NO non-test call site. `lib.rs` imports only `build_cell_map` from the artifact surface (lib.rs:199). `gate/accept.rs` calls `emit_bundle` only — not `build_tools` directly.

**Verdict: CR-01 CONFIRMED.** A real `cargo pmcp workbook compile` on any workbook always produces a single workflow-named tool with `input_keys: []` — the per-Table fan-out never executes on the production path.

### CR-02 Verification (served schema empty for production-compiled bundles)

**Evidence — schema.rs:336-348:**
```rust
pub fn input_schema_for_tool(manifest: &Manifest, cell_map: &CellMap, tool: &Tool) -> Value {
    let mut input_props = Map::new();
    for entry in &cell_map.inputs {
        if tool.input_keys.iter().any(|k| k == &entry.json_key) {
            input_props.insert(...);
        }
    }
    assemble_input_schema(manifest, input_props)
}
```
For a `build_cell_map`-produced tool where `tool.input_keys = Vec::new()`, the condition `tool.input_keys.iter().any(...)` is always false. `input_props` remains empty. The served tool advertises `"inputs": {"additionalProperties": false, "properties": {}}` — an empty strict object, signaling to callers that no inputs are accepted.

**Evidence — input.rs:142-165 (seed_supplied_inputs):**
Validates against the FULL `cell_map.inputs` pool (iterates `cell_map.inputs`, looks up `entry.json_key`). A client that trusts the (empty) advertised schema sends no inputs; the runtime would accept them anyway. The V5 invariant ("a client trusting the advertised schema must never be able to send a key the runtime then rejects") is NOT violated in the direction of a client-trust panic — but the inverse IS violated: the schema is STRICTER than the runtime, hiding every real input from discovery.

**Verdict: CR-02 CONFIRMED.** The schema/runtime parity invariant is inverted for any production-compiled bundle.

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | A BA authors inputs/outputs as named Excel Tables; compiler synthesizes tool surface by iterating rows (type/unit/enum/tier harvested) | VERIFIED | Plan 01 ships fixture_author.rs with TableSpec + add_table surface; Plan 02 ships ingest/cell_map.rs with TableRecord harvest (get_name/get_columns + catch_unwind seam); synth.rs projects per-row type/unit/enum/tier. Harvested table names and column data exist in the ingest layer. |
| 2 | Each output table becomes a distinct, named, described MCP tool with a DAG-derived input schema and an emitted output schema (structuredContent) | FAILED (BLOCKER) | build_tools, reconcile_tools, tool_name_collision_findings have zero non-test callers. emit_bundle calls build_cell_map (single-tool transitional path) at artifact/mod.rs:180. A production compile always yields one workflow-named tool with input_keys:[] and empty per-tool input schema. The served registration loop (mod.rs:252) does iterate bundle.cell_map.tools, but the bundle's cell_map is produced by the single-tool path, so it always has exactly 1 tool with empty DAG-derived keys. |
| 3 | A structurally-broken workbook fails compile with a fail-helpful, cell-precise message; cargo pmcp workbook explain previews the emitted tool surface before deploy | VERIFIED | Plan 05 ships cargo-pmcp/src/commands/workbook/explain.rs with ingest→synth→render pipeline. Broken workbook errors propagate as CompileError variants with cell-precise LintFinding locations. |
| 4 | A shipped provenance-valid template .xlsx doubles as starting point, training artifact, and honest reference fixture | VERIFIED | cargo-pmcp/src/templates/workbook_bundle/template.xlsx + crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx are shipped; template_provenance.rs asserts RAW ExcelTrusted classification + byte-equality. No provenance-override sidecar present. |
| 5 | Per-cell in_*/out_* model retired; F2 retained; make quality-gate + PMAT + purity all green | PARTIALLY VERIFIED (WARNING) | CalculateHandler GONE (verified by 100-06-SUMMARY.md retired-symbol sweep). Plan-03 CellMap::outputs() shim GONE. F2 override advertising retained in schema.rs:387. However: promote_named_outputs / name_named_inputs / strip_governance_prefix SURVIVE in lib.rs production code (lines 327, 331, 609, 643) — documented as a Rule-4 architectural deferral in deferred-items.md and 100-06-SUMMARY.md. The quality-gate binding verdict (make lint + PMAT) is GREEN per 100-06-SUMMARY; make quality-gate workspace-wide is blocked by a pre-existing pmcp-toolkit-mysql sqlx E0277 (out of scope, pre-existing). Purity check GREEN (umya/calamine absent from served trees). The PMAT gate is GREEN for src/-prefixed paths. |

**Score: 4/5 — Truth #2 FAILED (BLOCKER)**

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `cargo-pmcp/src/templates/workbook_bundle/template.xlsx` | Shipped BA starting point | VERIFIED | File exists, ExcelTrusted by RAW classification |
| `crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx` | Byte-identical copy of canonical | VERIFIED | Byte-equality enforced by template_provenance.rs |
| `cargo-pmcp/src/templates/workbook_bundle/template.gen.json` | Regeneration sidecar | VERIFIED | File exists per 100-01-SUMMARY |
| `crates/pmcp-workbook-compiler/src/fixture_author.rs` | Table-emitting author surface | VERIFIED | Contains add_table, TableSpec, add_data_validation |
| `crates/pmcp-workbook-compiler/tests/template_provenance.rs` | ExcelTrusted + byte-equal assertion | VERIFIED | Test asserts RAW classify() == ExcelTrusted |
| `crates/pmcp-workbook-compiler/src/ingest/cell_map.rs` | TableRecord{name,area,columns} | VERIFIED | struct TableRecord exists per Plan 02 |
| `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` | build_tools, reconcile_tools, tool_name_collision_findings | STUB | Functions EXIST and are substantive, but have zero non-test callers — ORPHANED from the production path |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` | WorkbookToolHandler per output Table | VERIFIED | WorkbookToolHandler exists, serves per-tool compute/schema; BUT relies on the golden's pre-populated tool data, not a production compile |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` | N-handler registration loop over bundle.tools | VERIFIED | Loop exists at line 252; wiring is correct on the served side |
| `cargo-pmcp/src/commands/workbook/explain.rs` | workbook explain subcommand | VERIFIED | fn execute present |
| `crates/pmcp-workbook-compiler/src/reemit_golden.rs` | Producer/consumer proof | ORPHANED | Tests pass only because the golden was regenerated out-of-band; fresh compile yields single-tool bundle that is a subset of the two-tool golden (subset checks pass vacuously) |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `emit_bundle` (artifact/mod.rs:180) | `build_tools` | per-Table multi-tool emit | NOT_WIRED | emit_bundle calls build_cell_map, not build_tools; the connection does not exist in production code |
| `compile_workbook_inner` | `tool_name_collision_findings` | pre-emit collision lint | NOT_WIRED | No call to tool_name_collision_findings anywhere outside test code |
| `compile_workbook_inner` | `reconcile_tools` | per-tool reconcile | NOT_WIRED | lib.rs calls comparison_from_outputs / reconcile::reconcile (old single-tool path); reconcile_tools has no production caller |
| `input_schema_for_tool` (schema.rs:336) | `tool.input_keys` | per-tool schema projection | PARTIAL — HOLLOW | Function exists and is called from handler metadata(); for a build_cell_map-produced tool the projection produces an empty schema because input_keys is always Vec::new() |
| `mod.rs:252` (registration loop) | `WorkbookToolHandler::new(bundle, tool)` | bundle.cell_map.tools iteration | WIRED (for served side) | Correct on the served side; the gap is that the bundle's cell_map.tools has only one tool with empty input_keys from a production compile |
| `ingest/cell_map.rs::TableRecord` | `artifact/cell_map.rs::OutputTable` | table membership for build_tools | NOT_WIRED | build_tools requires OutputTable membership from the harvest layer; this connection is never established in the production compile pipeline |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|---------------------|--------|
| `handler.rs::WorkbookToolHandler::metadata()` | `tool.input_keys` | `bundle.cell_map.tools[n].input_keys` | NO — always [] from production compile | HOLLOW: the per-tool schema is structurally wired but data is always empty in a production-compiled bundle |
| `handler.rs::WorkbookToolHandler::compute()` | `tool.outputs` | `bundle.cell_map.tools[n].outputs` | YES — but only ONE tool exists in production | PARTIAL: outputs present for the single transitional tool; multi-tool fan-out never occurs |
| `mod.rs registration loop` | `bundle.cell_map.tools` | `emit_bundle` → `build_cell_map` | SINGLE TOOL — always one element | STATIC: always produces exactly one tool named manifest.workflow regardless of number of output Tables |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| WBV2-01 | 100-01 | Provenance-valid template.xlsx as starting point/fixture | SATISFIED | template.xlsx shipped, ExcelTrusted by RAW classification, byte-equal copies enforced by test |
| WBV2-02 | 100-02 | Ingest harvests Table name/columns/rows for per-row type/unit/enum/tier | SATISFIED | TableRecord in ingest/cell_map.rs; harvest_roundtrip_prop.rs and template_harvest_e2e.rs exist |
| WBV2-03 | 100-03 | Shared artifact model carries Tool type + tools[] + DAG upstream_input_leaves | SATISFIED | Tool type in artifact_model.rs; upstream_input_leaves in dag.rs; build_tools in cell_map.rs (though not wired to production emit) |
| WBV2-04 | 100-04 | Each output Table = distinct named MCP tool with DAG-derived inputSchema + outputSchema | BLOCKED | build_tools has zero non-test callers; emit_bundle uses single-tool build_cell_map; CR-01 confirmed |
| WBV2-05 | 100-04 | Per-tool reconcile + fail-helpful row lints + collision lint | BLOCKED | reconcile_tools and tool_name_collision_findings exist but are never called from the production compile pipeline; the old single-tool reconcile::reconcile is still the active path |
| WBV2-06 | 100-05 | cargo pmcp workbook explain previews tool surface before deploy | SATISFIED | explain.rs exists with execute fn; wired via WorkbookCommand::Explain dispatch |
| WBV2-07 | 100-05 | BA-facing docs (pmcp-book + pmcp-course chapters) | SATISFIED | pmcp-book/src/workbook-table-authoring.md and pmcp-course/src/workbook-table-authoring.md created per 100-05-SUMMARY |
| WBV2-08 | 100-06 | make quality-gate + PMAT + purity all green | PARTIALLY SATISFIED | make lint GREEN; PMAT src/ GREEN; purity GREEN; make quality-gate workspace blocked by pre-existing pmcp-toolkit-mysql E0277 (documented out of scope); promoted_named_outputs/name_named_inputs survive as Rule-4 deferral |

**Orphaned requirements (in REQUIREMENTS.md but not claimed by any plan):** None — WBV2-01 through WBV2-08 are fully claimed.

**Note:** WBV2-04 maps directly to ROADMAP Success Criterion #2 ("Each output table becomes a distinct, named, described MCP tool..."). This criterion is the headline deliverable and it is not met in the production path.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/pmcp-workbook-compiler/src/artifact/mod.rs` | 180 | build_cell_map called instead of build_tools (documented as "TRANSITIONAL single-tool path") | BLOCKER | The multi-tool deliverable never executes |
| `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` | 449-493 | build_cell_map doc comment says "TRANSITIONAL single-tool path (Plan 03→04)" but Plan 04 never wired it out | BLOCKER | Doc comment implies a wire-up that never happened; misleads future readers |
| `crates/pmcp-workbook-compiler/src/reemit_golden.rs` | 82-89,107-111,138-145 | Comments admit golden was regenerated out-of-band; proof uses subset assertions | WARNING | The golden proves nothing about a fresh production compile; CR-01 gap is invisible to the test suite |
| `crates/pmcp-workbook-compiler/src/artifact/cell_map.rs` | 416-433 | oracle_value() doc comment is self-contradictory (IN-03 from REVIEW); Plan 04 wiring for cached `<v>` never landed | INFO | Output oracle is always None in production; tests synthesize oracles via tier hack |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` | 251-263 | No duplicate-registration guard for colliding sanitized tool names (WR-02) | WARNING | Since tool_name_collision_findings never runs at compile time, a hand-crafted or tampered bundle with colliding names boots silently with last-writer-wins behavior |

**Debt marker gate:** No TBD/FIXME/XXX markers found in files touched by this phase that lack a formal issue reference.

---

### Behavioral Spot-Checks

Step 7b: SKIPPED — the core gap (CR-01) is fully verifiable by static code tracing. Running `compile_workbook` would require a real .xlsx with Excel identity; the production path is deterministically traced to `build_cell_map` by reading `lib.rs` and `artifact/mod.rs`. Behavioral execution would confirm but not add to the static evidence.

---

### Probe Execution

Step 7c: No probe scripts declared in PLAN frontmatter for this phase. No `scripts/*/tests/probe-*.sh` files relevant to this phase. SKIPPED.

---

### Human Verification Required

None — the failure is fully observable by static code analysis. The CR-01 gap is unambiguous: `emit_bundle` calls `build_cell_map` and `build_tools` has no non-test caller. No human testing can override this structural gap.

---

## Gaps Summary

**One BLOCKER gap blocks phase goal achievement.**

The root cause is a single missing wire in the production compile pipeline: `emit_bundle()` in `crates/pmcp-workbook-compiler/src/artifact/mod.rs` at line 180 calls `build_cell_map()` (the "TRANSITIONAL single-tool path") instead of the per-Table `build_tools()` function that implements WBV2-04. This means:

1. **WBV2-04 is not met:** Every production compile produces one workflow-named tool with empty `input_keys`. The per-Table multi-tool fan-out — the headline deliverable of this phase — never executes.

2. **WBV2-05 is not met:** `reconcile_tools` and `tool_name_collision_findings` are never called from the production compile pipeline. The collision lint safety property does not hold at compile time.

3. **CR-02 follows as a consequence:** Because `input_keys` is always empty in a production-compiled bundle, `input_schema_for_tool` produces an empty `inputs.properties` for every tool, while `validate_input` / `seed_supplied_inputs` accepts the full input pool — the schema/runtime parity invariant is inverted.

4. **The test suite is blind to this gap:** The producer/consumer proof (`reemit_golden.rs`) uses the committed golden that was regenerated out-of-band into the two-Table shape. All proof assertions use `is_subset` checks — a fresh compile yielding a single-tool bundle is a valid subset of the two-tool golden. Handler tests use `golden_bundle()` which loads the hand-regenerated golden, not a freshly compiled one.

**The served-side wiring (mod.rs registration loop, WorkbookToolHandler, schema.rs, handler.rs) is CORRECT and complete.** The gap is exclusively in the compiler's emit path. The fix is targeted: replace `build_cell_map(&ratified)` in `emit_bundle` with the `build_tools` + `tool_name_collision_findings` pipeline, supplying `OutputTable` membership derived from the harvested `TableRecord`s (which the ingest layer already provides per Plan 02).

---

_Verified: 2026-06-20T12:00:00Z_
_Verifier: Claude (gsd-verifier)_
