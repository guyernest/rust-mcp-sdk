---
phase: 100-workbook-accuracy-verification-surface
verified: 2026-06-24T00:00:00Z
status: passed
score: 4/4 must-haves verified
overrides_applied: 0
reverified: 2026-06-24T01:00:00Z
resolution: "The single gap (WBVER-04 integration-test snapshot + REQUIREMENTS tracking) was closed inline by the orchestrator in commit after initial verification. verify_accuracy added to WORKBOOK_TOOLS ([&str; 7]); the with_workbook_bundle/embedded boot integration tests now PASS asserting it (proving the 6th meta tool registers through the builder path, not just the handler unit test); stale fn names + docstrings refreshed; WBVER-04 marked [x]/Complete in REQUIREMENTS.md. Re-ran: cargo test -p pmcp-server-toolkit --features workbook-embedded,http --test workbook_integration → 3 passed. The verifier itself noted these were closeable in a single follow-up commit without re-planning."
gaps_resolved:
  - truth: "No regression / ALWAYS coverage / gates green (WBVER-04)"
    was: partial
    now: verified
    fix_commit: "test(100-05): assert verify_accuracy registered through builder boot path (WBVER-04 gap)"
advisory:
  - id: WR-01
    severity: warning
    source: 100-REVIEW.md
    description: "compare_output (reconcile.rs line 184) hardcodes the const TOL instead of the threaded tol parameter. The tol parameter of reconcile_reference is stamped into ReconcileReport.tolerance but never used for grading. Currently masked: the only production caller (handler.rs line 721) passes reconcile::TOL (identical value = 0.01), so reported tolerance == grading tolerance and behavior is correct today. A future caller passing a custom tol would produce a silently-wrong attestation in a trust feature. Not a current behavioral defect; recommended for follow-up."
    action_required: false
---

# Phase 100: Workbook Accuracy-Verification Surface Verification Report

**Phase Goal:** Give business analysts a way to TRUST that the generated MCP server reproduces their Excel workbook, and probe it with their own inputs, by extending the existing render_workbook / workbook:// download path with three additive capabilities: (1) text/boolean formula outputs render as formula-with-cached-result so Excel recomputes ALL output types on open; (2) a render_workbook inputs_only mode that fills inputs only and leaves every formula for Excel to compute from scratch; and (3) a verify_accuracy meta-tool that re-runs the engine at the workbook's reference inputs and reports, per output, whether it matches Excel's authored value within TOL. Reader-free, stateless/Lambda-safe, purity-gate preserved.

**Verified:** 2026-06-24T00:00:00Z (initial) · re-verified 2026-06-24 after gap closure
**Status:** passed (4/4 — single gap closed inline; see Resolution below)
**Re-verification:** Yes — the one gap found initially was fixed and the integration test now passes

---

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Text & boolean formula output cells render as formula-with-cached-result (`Formula::set_result`), so Excel's `fullCalcOnLoad="1"` recomputes all output types on open (WBVER-01) | VERIFIED | `write_formula_or_value` helper defined at render/mod.rs line 511, called for Text arm (line 452) and Bool arm (line 468); fixture tax-calc@1.1.0 layout.json lines 143/151 have non-null formula for bracket_label/is_taxable; cell-scoped unit tests assert `<f>` and `<v>` on those cells by A1 address |
| 2 | `render_workbook` accepts `mode: filled \| inputs_only` (default `filled`, additive); `inputs_only` emits bare formulas with NO cached results, deterministically; unknown mode → `Err` (never panic); the encoded `workbook://` URI stays ≤ 64 KiB (WBVER-02) | VERIFIED | `enum RenderMode { Filled, InputsOnly }` at render/mod.rs line 233; threaded through render_resource.rs line 101/108; render_uri.rs adds `mode` field with `#[serde(default)]` for back-compat; `parse_render_mode` in handler.rs strips mode before validate_input; unknown mode → Err (handler.rs line 640); prop_encode_decode_identity asserts length < MAX_ENCODED_URI_LEN; unit test `render_workbook_unknown_mode_is_iserror_not_panic` (line 1406); malformed-string decode returns Err (render_uri.rs tests) |
| 3 | A `verify_accuracy` meta-tool re-runs the executor at the workbook's reference inputs and returns a per-output reconciliation report vs `Tool.oracle` within TOL — stateless, reader-free (WBVER-03) | VERIFIED | reconcile.rs defines `seed_reference_inputs` (line 149), `reconcile_reference` (line 308), `ReconcileReport`/`ToolReport`/`OutputRow`; no toolkit/reader imports (use statements confirm only crate-internal + serde/schemars); VerifyAccuracyHandler registered in mod.rs line 294-296; H3 binding test uses `VerifyAccuracyHandler::NAME` (not string literal) line 883; golden test at handler.rs line 1483 asserts all_within_tol=true + cells_checked=7 including bracket_label and is_taxable; D-03 unknown filter → Err (line 1557); D-04 vacuous empty oracle (line 1591) |
| 4 | No regression to existing `render_workbook` / `workbook://` behavior or wire shapes; ALWAYS coverage (fuzz/property/unit/example); `make quality-gate` + `make purity-check` + `make doc-check` green; PMAT cog-25 (WBVER-04) | PARTIAL | Gates confirmed green by orchestrator + SUMMARY. Example `workbook_table_authoring.rs` demonstrates all three capabilities end-to-end. HOWEVER: integration test `workbook_integration.rs` does not include `verify_accuracy` in `WORKBOOK_TOOLS` (line 29) and its registration as the 6th tool is unasserted at the integration level; stale "five tools" / "four meta tools" docstrings; REQUIREMENTS.md WBVER-04 checkbox and traceability status not updated |

**Score:** 3/4 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-workbook-runtime/src/render/mod.rs` | `write_formula_or_value` helper covering Number/Text/Bool formula-or-literal paths; `enum RenderMode` with `Filled`/`InputsOnly` | VERIFIED | `write_formula_or_value` defined at line 511 with 3 call sites; `enum RenderMode` at line 233; re-exported from lib.rs line 110 |
| `crates/pmcp-workbook-runtime/src/reconcile.rs` | `ReconcileReport` + `ToolReport` + `OutputRow` + `reconcile_reference` + `seed_reference_inputs`; reader-free | VERIFIED | All types defined; `seed_reference_inputs` count ≥ 2 (12 occurrences); no toolkit/reader imports confirmed; `cell: Option<String>` present on OutputRow; min_lines far exceeded (~380 lines) |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` | `VerifyAccuracyHandler` (6th meta tool) | VERIFIED | 11 occurrences; struct at line 689; impl at line 694; `ToolHandler` impl at line 811; `const NAME: &str = "verify_accuracy"` |
| `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` | `mode` field; `#[serde(default)]` makes ABSENT mode → Filled; present malformed → Err | VERIFIED | `mode: RenderMode` on all three structs; `#[serde(default)]` on RenderPayload line 107; malformed decode returns Err (no `#[serde(other)]` / catch-all) |
| `crates/pmcp-server-toolkit/src/workbook/schema.rs` | `mode` property on render input schema only; `verify_accuracy_input_schema` + `verify_accuracy_output_schema` | VERIFIED | `mode` enum on render schema (line 526); not on calculate/explain schemas; `verify_accuracy_input_schema` (line 301); `verify_accuracy_output_schema` (line 322) |
| `crates/pmcp-server-toolkit/examples/workbook_table_authoring.rs` | Demonstrates `render_workbook(filled)` + `render_workbook(inputs_only)` + `verify_accuracy` | VERIFIED | `inputs_only` and `verify_accuracy` each appear ≥1 time; example shows all 3 WBVER capabilities including bracket_label (Text) and is_taxable (Bool) reconciled within tol |
| `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock` | Re-folded integrity lock consistent with edited 4 artifacts | VERIFIED | BUNDLE.lock exists with combined hash; fixture loads without panic (per orchestrator confirmation + golden test passing) |
| `crates/pmcp-workbook-runtime/src/manifest_model.rs` | `RESERVED_TOOL_NAMES` is `[&str; 5]` including `"verify_accuracy"` | VERIFIED | Line 219: `pub const RESERVED_TOOL_NAMES: [&str; 5]`; line 224: `"verify_accuracy"` is the 5th element |
| `crates/pmcp-server-toolkit/tests/workbook_integration.rs` | Updated to assert `verify_accuracy` is registered as the 6th tool (additive) | FAILED | `WORKBOOK_TOOLS` constant (line 29) has 6 entries but omits `verify_accuracy`; function name and module docstring still say "five tools"/"four meta tools" |
| `.planning/REQUIREMENTS.md` | WBVER-04 marked complete | FAILED | Line 77: `[ ]` checkbox unchecked; line 202: status = "Pending" |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `write_computed_value` Text/Bool arms | `write_formula_or_value` | `cell.formula.is_some()` branch | WIRED | render/mod.rs lines 452, 468 — Text and Bool arms call `write_formula_or_value` passing formula, cached result, typed literal-writer |
| `write_formula_or_value` | `Formula::new(..).set_result(..)` | `RenderMode::Filled` arm | WIRED | render/mod.rs line 549: `RenderMode::Filled => formula.set_result(cached_result)` |
| `RenderWorkbookHandler::compute` | `parse_render_mode` → `render_uri::encode(dto, stamp, mode)` | strip mode before `validate_input` | WIRED | handler.rs lines 608-610 |
| `render_resource::regenerate` | `render_xlsx(layout, run, decoded.mode)` | decoded.mode threaded | WIRED | render_resource.rs lines 101/108: `let mode = decoded.mode; ... render_xlsx(..., mode)` |
| `reconcile_reference` | `seed_reference_inputs(manifest)` → `CellEnv` | iterate manifest.cells, filter Role::Input, read InputTier defaults | WIRED | reconcile.rs lines 315-319 |
| `reconcile_reference` | `run_executor(ir, dag, &env)` + `run.computed.get(&entry.seed_coord)` | reconcile_tool projects per output | WIRED | reconcile.rs line 321 (run), line 221 (get computed) |
| `VerifyAccuracyHandler::compute` | `pmcp_workbook_runtime::reconcile_reference` | 6th meta-tool handler | WIRED | handler.rs line 716 |
| `workbook/mod.rs` registration | `VerifyAccuracyHandler::NAME` via `.tool_arc(...)` | mod.rs registration block | WIRED | mod.rs lines 294-297 |
| integration test | `verify_accuracy` registration asserted | `server.get_tool(name).is_some()` | NOT WIRED | `verify_accuracy` absent from `WORKBOOK_TOOLS` in workbook_integration.rs |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `render/mod.rs write_formula_or_value` | `formula: &Option<String>`, `cached_result: String` | cell.formula from LayoutDescriptor; computed CellValue from RunResult | Yes — formula from bundle layout.json; cached from executor run | FLOWING |
| `reconcile.rs reconcile_reference` | `seed_reference_inputs(manifest)` | InputTier defaults from Manifest.cells | Yes — reads live manifest; inserts CellValue defaults | FLOWING |
| `handler.rs VerifyAccuracyHandler::compute` | `report: ReconcileReport` | `reconcile_reference` over live bundle | Yes — runs executor, projects Tool.oracle; cells_checked=7 for tax fixture confirmed by unit test | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Fixture has text+bool formula outputs | `grep "bracket_label\|is_taxable" cell_map.json` | Oracle entries found with `{"Text":"bracket_2"}` and `{"Bool":true}` | PASS |
| `write_formula_or_value` covers Text+Bool | `grep -c "write_formula_or_value" render/mod.rs` | 6 occurrences (1 def + 3 call sites + 2 doc refs) | PASS |
| `RESERVED_TOOL_NAMES` is length 5 with verify_accuracy | `grep "RESERVED_TOOL_NAMES: \[&str; 5\]" manifest_model.rs` | Line 219 matches | PASS |
| H3 binding uses ::NAME not string literal | `grep "VerifyAccuracyHandler::NAME" handler.rs` inside test | Found at line 883 | PASS |
| `verify_accuracy` registered in mod.rs | `grep "VerifyAccuracyHandler::NAME" mod.rs` inside `.tool_arc(...)` | Lines 294-296 confirm `.tool_arc(VerifyAccuracyHandler::NAME, ...)` | PASS |
| `verify_accuracy` absent from integration test | `grep "verify_accuracy" workbook_integration.rs` | 0 matches — not in WORKBOOK_TOOLS | FAIL |
| reconcile.rs no toolkit/reader imports | `grep "pmcp_server_toolkit\|umya\|quick_xml\|calamine"` reconcile.rs | 0 matches in use statements | PASS |
| compare_output uses const TOL not parameter | `grep "delta <= TOL\|delta <= tol"` reconcile.rs | `delta <= TOL` (const) at line 184, not `delta <= tol` | WR-01 confirmed |

---

### Probe Execution

Not applicable — no probe-*.sh files declared for this phase.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|------------|-------------|-------------|--------|----------|
| WBVER-01 | Plan 02 | Text & boolean formula output cells render as formula-with-cached-result | SATISFIED | `write_formula_or_value` helper routes Text/Bool through formula-aware path; XML-level per-cell tests in render/mod.rs assert `<f>`+`<v>` on bracket_label (B6) and is_taxable (B7) |
| WBVER-02 | Plan 03 | `render_workbook` accepts `mode: filled \| inputs_only` | SATISFIED | RenderMode enum; mode threaded through URI, render_resource, handler; unknown mode → Err; serde(default) back-compat proven by literal pre-phase payload test; size ≤ 64 KiB proven by proptest |
| WBVER-03 | Plan 04 | `verify_accuracy` meta-tool with per-output reconciliation report | SATISFIED | reconcile.rs + VerifyAccuracyHandler wired and tested; golden: all_within_tol=true + cells_checked=7 + bracket_label/is_taxable present; D-03 unknown filter → Err; D-04 vacuous; H3 binding uses ::NAME |
| WBVER-04 | Plan 05 | No regression; ALWAYS coverage; all gates green | BLOCKED | Gates green (confirmed). Example demonstrates all 3 capabilities. Integration test does not assert verify_accuracy registration; stale "five tools" naming; REQUIREMENTS.md not updated |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/pmcp-server-toolkit/tests/workbook_integration.rs` | 7, 14, 29, 39 | Stale "five tools" / "four meta tools" naming; `verify_accuracy` absent from `WORKBOOK_TOOLS` constant | WARNING | Integration test does not assert that `verify_accuracy` is registered as the 6th tool — the additive tool's registration is only asserted at the handler unit-test level, not at the full server-builder integration level. Plan 05 acceptance criteria required updating this. |
| `.planning/REQUIREMENTS.md` | 77, 202 | WBVER-04 checkbox unchecked (`[ ]`); traceability table says "Pending" | INFO | Requirement tracking record is stale — a documentation-only inconsistency, not a behavioral defect. |

No debt markers (TBD/FIXME/XXX) found in any phase-modified files. No stub patterns in the new production code paths.

---

### Human Verification Required

None identified — all behaviors are programmatically verifiable.

---

## Resolution (post-initial-verification)

The single gap below was closed inline by the orchestrator immediately after the initial pass:
- `verify_accuracy` added to `WORKBOOK_TOOLS` (now `[&str; 7]`) in `workbook_integration.rs`; the `with_workbook_bundle_registers_all_seven_tools` and `example_server_boots_serves_seven_tools_and_shuts_down` tests now **PASS**, proving the 6th meta tool registers through the *builder* boot path (not only the handler unit test). Stale fn names/docstrings refreshed.
- WBVER-04 marked `[x]` / **Complete** in `.planning/REQUIREMENTS.md`.
- Re-ran `cargo test -p pmcp-server-toolkit --features workbook-embedded,http --test workbook_integration` → **3 passed**.

WR-01 remains an advisory follow-up (non-blocking). **Final status: passed (4/4).** The original gap analysis is retained below for the record.

## Gaps Summary (original — now resolved)

Phase 100 delivers WBVER-01 (text/bool formula-with-cached-result), WBVER-02 (inputs_only mode), and WBVER-03 (verify_accuracy meta-tool) completely. The three core behavioral goals are fully wired and tested.

WBVER-04 was **partially** complete at initial verification (now resolved per the Resolution section above):

1. **Integration test gap**: `crates/pmcp-server-toolkit/tests/workbook_integration.rs` `WORKBOOK_TOOLS` array (6 entries) omits `"verify_accuracy"`. The Plan 05 acceptance criteria explicitly required: *"Any prior 'exactly five tools'/exact-length snapshot is updated to a names-subset + additive-sixth assertion."* This was not done. The 6th tool is correctly registered in `mod.rs` and tested at the handler unit-test level, but the integration test does not confirm its presence in the served tool set at the server-builder level.

2. **Stale naming**: The integration test function (`with_workbook_bundle_registers_all_five_tools`) and module docstring ("four workbook-wide meta tools") are stale after the additive 6th tool.

3. **Requirements tracking**: REQUIREMENTS.md WBVER-04 checkbox and traceability table not updated to Complete.

**Advisory WR-01 (non-blocking)**: `compare_output` in reconcile.rs hardcodes the const `TOL` (0.01) instead of the threaded `tol` parameter. This is a latent defect currently masked because the only production caller (`handler.rs:721`) passes `reconcile::TOL` (the same 0.01 constant), so reported tolerance == grading tolerance and behavior is correct today. Documented in 100-REVIEW.md. Not a gap in this phase's goal achievement; recommended as a small follow-up.

The three fixes for WBVER-04 are small (add `"verify_accuracy"` to `WORKBOOK_TOOLS`, rename the test function, update the docstring, and mark WBVER-04 complete in REQUIREMENTS.md). They can be addressed in a single follow-up commit without re-planning.

---

_Verified: 2026-06-24T00:00:00Z_
_Verifier: Claude (gsd-verifier)_
