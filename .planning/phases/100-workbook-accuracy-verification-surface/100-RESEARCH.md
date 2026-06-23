# Phase 100: Workbook Accuracy-Verification Surface - Research

**Researched:** 2026-06-22
**Domain:** Rust workbook runtime/toolkit — xlsx writer, executor re-run, stateless URI codec, reference reconciliation
**Confidence:** HIGH (all findings grounded in the actual source files this phase changes)

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Each `verify_accuracy` output row carries a sheet-qualified A1 address in a new `cell` field (e.g. `"Sheet1!C12"`). Row shape becomes `{ key, cell, server_value, oracle_value, abs_delta, within_tol }`.
- **D-02:** `cell` is nullable (`Option<String>` → JSON null/omitted) when an output json_key cannot be resolved to exactly one source cell. The rest of the row still reports. Panic-free: a missing mapping never blocks reconciliation. (Research flag below: the mapping IS guaranteed 1:1 — see Architecture.)
- **D-03:** Optional tool-name filter naming a non-existent tool → `Err` listing the available tool names.
- **D-04:** A tool with an empty oracle is included with `outputs: []` and `all_within_tol = true` (vacuous), contributing 0 to `cells_checked`.
- **D-05:** `inputs_only` produces a clean copy — input cells seeded with caller values, formula cells as bare formulas with no `set_result`, no highlighting/formatting/comments.
- **D-06:** Extend the **tax** bundle (one bundle) to demonstrate all three capabilities end-to-end.
- **D-07:** Research/coverage flag — the example AND the WBVER-01 unit tests MUST exercise at least one **text** and one **boolean** formula output cell (the tax bundle's oracle is numeric-only today).

### Claude's Discretion
- Internal helper factoring (the design's `write_formula_or_value` helper unifying Number/Text/Bool formula-or-literal paths), the `RenderMode` enum's exact location, and how `mode` threads through `DecodedRender` — per the design doc; planner/researcher decide specifics.
- Exact `verify_accuracy` tool description wording, provided it keeps the design's honest framing (attests the engine matches Excel's authored values *at the reference inputs*; points BAs to `render_workbook` filled/inputs_only where Excel is the oracle for arbitrary inputs).

### Deferred Ideas (OUT OF SCOPE)
- Highlight/comment input cells in the `inputs_only` download (§7 q1).
- Named golden scenarios (compile-time captured input→output vectors) for non-reference attestation (§7 q2).
- The re-version loop (analyst finds discrepancy → uploads fixed workbook → recompile → redeploy).
- Arbitrary-input server-side delta vs Excel (impossible while runtime is reader-free).
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WBVER-01 | Text & boolean formula output cells render as formula-with-cached-result so Excel recomputes all output types on open. | `render/mod.rs::write_computed_value` already has the numeric `Formula::set_result` branch (lines 380-401, 414-443). `Formula::set_result(impl Into<String>)` accepts string results — VERIFIED below. Item 1 extends the Text/Bool arms (currently call `write_string_cell`, line 389-390) to write formula+result when `cell.formula.is_some()`. |
| WBVER-02 | `render_workbook` gains `mode: filled \| inputs_only` (default `filled`); `inputs_only` emits bare formulas, deterministically; unknown mode → `Err`; URI ≤ 64 KiB. | `render_xlsx` signature (line 227) gains a `RenderMode` param; `DecodedRender` (render_uri.rs:75) + `RenderPayload`/`RenderPayloadRef` (render_uri.rs:91-105) gain a `mode` field; `render_resource.rs::regenerate` (line 85-108) threads it into `render_xlsx`; the `render_workbook` handler (handler.rs:602-610) parses the new arg. URI size impact quantified below (~+20 bytes — trivially within 64 KiB). |
| WBVER-03 | `verify_accuracy` meta-tool re-runs the engine at reference inputs and returns a per-output reconciliation report vs `Tool.oracle` within `TOL`, stateless and reader-free. | New pure `reconcile_reference(...)` in a new `reconcile.rs` in `pmcp-workbook-runtime`. Reference inputs = manifest tier defaults (VERIFIED below — the oracle was computed at those defaults). Reuses `build_dag`/`run` (executor.rs:92, 458). New 6th meta-tool handler registered in `workbook/mod.rs:267-286`. |
| WBVER-04 | No regression to existing wire shapes; `make quality-gate` + `make purity-check` + `make doc-check` green; PMAT cog-25; ALWAYS coverage (fuzz/property/unit/example). | All additions are writer-only / pure-diff (purity gate analysis below). Existing tests must stay green; new ALWAYS coverage maps to the existing scaffolding (Validation Architecture below). |
</phase_requirements>

## Summary

Every piece this phase needs already exists and is reachable inside the reader-free boundary. The three capabilities are genuinely additive seams on top of code that already does 90% of the work:

- **Item 1 (WBVER-01)** is a small extension to one function. `write_computed_value` (render/mod.rs:371) already writes numeric formula cells as `Formula::new(...).set_result(...)` (the `write_number_cell` helper, line 414). The `Text` and `Bool` arms (lines 389-390) currently call `write_string_cell` UNCONDITIONALLY — they ignore `cell.formula`. The fix factors a `write_formula_or_value` helper so all three value types write formula+cached-result when `cell.formula.is_some()`. `rust_xlsxwriter::Formula::set_result` accepts `impl Into<String>` (cached results are strings) so text and `"TRUE"`/`"FALSE"` bool results are writable.

- **Item 2 (WBVER-02)** threads a new `RenderMode { Filled, InputsOnly }` enum from the `render_workbook` handler arg, through the `workbook://` URI payload (`DecodedRender` + the private `RenderPayload`/`RenderPayloadRef`), through `regenerate`, into `render_xlsx(layout, run, mode)`. `InputsOnly` writes formula cells as bare formulas (no `set_result`) and only seeds input/literal cells. **Plumbing gotcha:** `render_workbook` currently parses args by calling `validate_input` directly, whose DTO `CalculateInput` is `#[serde(deny_unknown_fields)]` (input.rs:47) — a `mode` key would be REJECTED. The handler must strip/parse `mode` BEFORE handing the rest to `validate_input`, and the top-level input schema (`assemble_input_schema`, schema.rs:392, `additionalProperties:false`) must gain a `mode` property.

- **Item 3 (WBVER-03)** is a new pure `reconcile_reference(...) -> ReconcileReport` in a new `reconcile.rs` in `pmcp-workbook-runtime`, plus a 6th meta-tool handler. The reference inputs are the **manifest tier defaults** — VERIFIED: the oracle values in `cell_map.json` were computed at exactly those defaults (e.g. `tax_owed: 4800` = (60000−12000)×0.10). The reconcile seeds the executor from `seed_tier_defaults`, runs it, and diffs each tool's projected outputs against `tool.oracle[key]` within `TOL` (0.01).

**Primary recommendation:** Implement in the order Item 1 → Item 2 → Item 3 (each is independent; Item 1 has zero protocol surface, Item 2 changes a function signature consumed by Item 1's tests, Item 3 is fully orthogonal). Put `RenderMode` in `pmcp-workbook-runtime::render` (it is a `render_xlsx` parameter and must cross the runtime/toolkit boundary the same way `LayoutDescriptor` does). Put `reconcile_reference` + `ReconcileReport` in a new `pmcp-workbook-runtime::reconcile` module (pure, reader-free, serde/schemars-clean like `RunResult`). Add a `text`+`bool` formula output to the tax fixture to satisfy D-07.

## Architectural Responsibility Map

| Capability | Primary Tier | Secondary Tier | Rationale |
|------------|-------------|----------------|-----------|
| Formula+cached-result for text/bool (Item 1) | `pmcp-workbook-runtime::render` (writer) | — | Pure writer concern; the only place formula cells are emitted. No protocol/toolkit change. |
| `RenderMode` enum + `render_xlsx(mode)` (Item 2 core) | `pmcp-workbook-runtime::render` | — | The mode changes how the writer emits cells; the enum is a render parameter and must be nameable by both runtime and toolkit (like `LayoutDescriptor`). |
| `mode` in URI payload + arg parsing (Item 2 plumbing) | `pmcp-server-toolkit::workbook` (render_uri, render_resource, handler, schema) | runtime (consumes the enum) | The stateless pointer-then-regenerate contract lives entirely in the toolkit; the URI is the toolkit's wire shape. |
| `reconcile_reference` + `ReconcileReport` (Item 3 core) | `pmcp-workbook-runtime::reconcile` (NEW pure module) | — | Pure diff over IR/DAG/oracle/layout; reader-free; reuses the runtime executor. Belongs beside `RunResult`/`artifact_model`. |
| `verify_accuracy` handler + registration (Item 3 surface) | `pmcp-server-toolkit::workbook` (handler, mod, schema) | runtime (calls `reconcile_reference`) | The MCP tool surface (the 6th meta-tool) is the toolkit's job, mirroring the existing 5 handlers. |
| Tax fixture text+bool output (D-07) | `pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0` | runtime test fixtures | The committed golden bundle is the example + integration substrate. |

## Standard Stack

This is an internal-only phase — **no new external dependencies**. Every crate needed is already a workspace dependency.

### Core (already present)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `rust_xlsxwriter` | (workspace-pinned) | The reader-free `.xlsx` writer; `Formula::set_result`, `write_formula`, `write_string` | Already the single deliberate purity-gate relaxation (render/mod.rs:11-15); `purity-check` asserts it is PRESENT. |
| `serde` / `serde_json` | (workspace) | Serde derives for `ReconcileReport`, the URI payload, `RenderMode` | Existing convention across the crate. |
| `schemars` | (workspace) | `JsonSchema` derive for runtime serde types (`RunResult`, `CellMap`) | `ReconcileReport` should derive `schemars::JsonSchema` to match `RunResult` (executor.rs:70) and feed `verify_accuracy`'s `outputSchema`. |
| `base64` | (workspace) | URL-safe-no-pad URI body codec (render_uri.rs) | Existing; `mode` rides inside the already-encoded JSON payload, no codec change. |
| `thiserror` | (workspace) | `RenderError` / error enums | Existing convention. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `RenderMode` enum param on `render_xlsx` | A bool `inputs_only` flag | Design §4.3 specifies an enum (`Filled`/`InputsOnly`); an enum is extensible (future modes) and reads clearly at call sites. Use the enum. |
| New `reconcile.rs` module | Adding `reconcile_reference` to `artifact_model.rs` | Keep it separate — `artifact_model.rs` is the hashing/integrity module; reconcile is a compute-diff. Design §4.1 names a new `reconcile.rs`. |

**Installation:** None — no `cargo add`.

## Package Legitimacy Audit

> Not applicable — this phase installs **zero** external packages. All dependencies are already-vetted workspace crates. slopcheck/registry verification skipped (no new package surface).

## Architecture Patterns

### System Architecture Diagram

```
                    ┌─────────────────────── pmcp-server-toolkit (workbook/) ───────────────────────┐
 MCP client         │                                                                                │
   │                │   render_workbook handler        verify_accuracy handler (NEW, 6th tool)       │
   │ tools/call     │   (handler.rs)                   (handler.rs / new file)                        │
   ├───────────────►│      │                              │                                          │
   │ render_workbook│      │ parse `mode` arg (NEW)        │ parse optional tool-name filter (D-03)   │
   │   {mode}       │      ▼                              ▼                                          │
   │                │   validate_input ──► encode ──► workbook://render/<b64>   reconcile_reference  │
   │◄───────────────┤   (strip mode first) (mode in     URI (pointer, no bytes)  (NEW, runtime call) │
   │  URI pointer   │                       payload)        │                          │             │
   │                │                                       │                          │             │
   │ resources/read │   RenderWorkbookResource              │                          │             │
   ├───────────────►│   ::regenerate (render_resource.rs)   │                          │             │
   │  (the URI)     │      │ decode (mode out of payload) ◄──┘                          │             │
   │                │      │ verify provenance / re-validate                            │             │
   │◄───────────────┤      ▼                                                            │             │
   │  base64 .xlsx  │   render_xlsx(layout, run, MODE) ◄──────────────────┐            │             │
   └────────────────┤                                                      │            │             │
                    └──────────────────────────────────────────────────── │ ────────── │ ────────────┘
                                                                           │            │
                    ┌──────────────────── pmcp-workbook-runtime (reader-free) ──────────│────────────┐
                    │                                                       │            ▼             │
                    │  render::render_xlsx(layout, run, RenderMode)         │   reconcile::            │
                    │    ├─ write_computed_value                            │   reconcile_reference(   │
                    │    │   └─ write_formula_or_value (NEW helper, Item 1) │     cell_map, manifest,  │
                    │    │       Number/Text/Bool → formula+set_result      │     ir, dag, layout, tol)│
                    │    │       (InputsOnly: bare formula, no set_result)  │     │ seed tier defaults │
                    │    └─ RenderMode { Filled, InputsOnly } (NEW enum)    │     │ run(ir,dag,seed) ──┘
                    │                                                       │     │ diff vs oracle     │
                    │  sheet_ir::executor::{build_dag, run} ──────────────► RunResult                  │
                    │  artifact_model::{Tool, Tool.oracle, CellEntry.seed_coord, TOL-mirror}           │
                    └─────────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities (file → change)

| File | Change |
|------|--------|
| `crates/pmcp-workbook-runtime/src/render/mod.rs` | Item 1: extend `write_computed_value` Text/Bool arms to formula+cached-result via a `write_formula_or_value` helper. Item 2: add `RenderMode` enum; `render_xlsx` gains a `mode` param; `InputsOnly` writes bare formulas (no `set_result`) and seeds only input/literal cells. |
| `crates/pmcp-workbook-runtime/src/reconcile.rs` | NEW: `ReconcileReport`, `ToolReport`, `OutputRow` (with the D-01 `cell: Option<String>`) + `reconcile_reference(...)`. Pure, reader-free, serde+schemars. |
| `crates/pmcp-workbook-runtime/src/lib.rs` | Re-export `RenderMode` (line ~104 beside `LayoutDescriptor`) and `reconcile::{reconcile_reference, ReconcileReport, ...}` (new `pub mod reconcile;` + `pub use`). |
| `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` | Item 2: `DecodedRender`, `RenderPayload`, `RenderPayloadRef` gain a `mode` field; `encode`/`decode` carry it; default-on-absent so OLD URIs still decode (back-compat). |
| `crates/pmcp-server-toolkit/src/workbook/render_resource.rs` | Item 2: `regenerate` passes `decoded.mode` into `render_xlsx`. |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` | Item 2: `render_workbook` parses `mode` (default `filled`, unknown → `Err`) before `validate_input`; encodes it into the URI. Item 3: NEW `VerifyAccuracyHandler`. |
| `crates/pmcp-server-toolkit/src/workbook/schema.rs` | Item 2: add `mode` to the render input schema. Item 3: `verify_accuracy_output_schema()`. |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` | Item 3: register the 6th tool; update the "five tools" doc/count → six (mod.rs:152-153, 265). |
| `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/*` | D-07: add a text + a bool formula output cell (+ re-fold the BUNDLE.lock hashes). |
| `crates/pmcp-server-toolkit/examples/workbook_table_authoring.rs` (or a new example) | D-06: demo `render_workbook(filled)`, `render_workbook(inputs_only)`, `verify_accuracy`. |

### Pattern 1: Formula-or-literal write helper (Item 1)
**What:** Unify the Number/Text/Bool formula-or-literal path into one helper that branches on `cell.formula.is_some()` AND on the render mode.
**When to use:** Inside `write_computed_value`.
**Current numeric branch (the template to generalize):**
```rust
// Source: crates/pmcp-workbook-runtime/src/render/mod.rs:414-443 (write_number_cell)
match (&cell.formula, fmt) {
    (Some(f), None) => {
        let formula = Formula::new(normalize_formula_for_writer(f)).set_result(format_number(n));
        ws.write_formula(row, col, formula).map_err(writer_err)?;
    },
    (None, None) => { ws.write_number(row, col, n).map_err(writer_err)?; },
    // ...with-format variants
}
```
The Text arm (line 389) currently does `write_string_cell(ws, row, col, s, fmt)?` with NO formula awareness. Item 1 makes it: if `cell.formula.is_some()` (and mode is `Filled`), write `Formula::new(normalize_formula_for_writer(f)).set_result(s)`; else write the string literal. Bool is identical with `set_result(if b {"TRUE"} else {"FALSE"})`.

### Pattern 2: Mode-threaded render (Item 2)
**What:** `InputsOnly` writes formula cells as `Formula::new(...)` with **no `.set_result(...)`**.
**Key determinism note:** `render_xlsx` already pins doc properties to a fixed datetime (init_workbook, line 239) — InputsOnly stays byte-deterministic because nothing cached varies. The two modes produce DIFFERENT bytes, but each mode is byte-stable across reads (the property tests must assert per-mode determinism, not cross-mode equality).
**Input-cell seeding in InputsOnly:** input cells carry `cell.formula == None` (VERIFIED in the fixture layout — `1_Inputs!B2..B5` all have `"formula": null`). The existing `write_computed_value` `_ => cell.value` fallback (line 394) and the numeric/string arms already write the seeded value for non-formula cells, so InputsOnly's "seed inputs, bare formulas for the rest" naturally falls out of: formula cell + InputsOnly → bare formula; non-formula cell → its value (unchanged from Filled).

### Pattern 3: Pure reference reconcile (Item 3)
**What:** Seed from manifest tier defaults → `run` → diff each `tool.outputs[].seed_coord` result against `tool.oracle[json_key]`.
```rust
// Conceptual shape (pure, reader-free) — lives in reconcile.rs
pub fn reconcile_reference(
    cell_map: &CellMap,
    manifest: &Manifest,
    ir: &HashMap<String, Cell>,
    dag: &Dag,
    tol: f64,
) -> Result<ReconcileReport, Box<LintFinding>> {
    // 1. Seed CellEnv from manifest tier defaults (the reference inputs).
    // 2. let run = run(ir, dag, &seed)?;   // reuse the executor (executor.rs:92)
    // 3. For each tool, for each output entry:
    //      server_value = run.computed.get(&entry.seed_coord)
    //      oracle_value = tool.oracle.get(&entry.json_key)   // BTreeMap<json_key, CellValue>
    //      cell = Some(entry.seed_coord.clone())             // D-01/D-02: always Some here (see below)
    //      abs_delta + within_tol = compare numbers within tol
}
```
**Seed source — VERIFIED:** the oracle values in `cell_map.json` are the outputs computed at the manifest tier defaults. Fixture proof: defaults are `gross_income=60000, deductions=12000` (manifest.json) → `taxable_income` oracle = `48000` = 60000−12000; `tax_owed` oracle = `4800` = 48000×0.10. So the reference inputs are exactly `seed_tier_defaults(manifest)` (the same function `validate_input` calls, input.rs:123). `reconcile_reference` should seed identically.

### Anti-Patterns to Avoid
- **Adding `mode` to `CalculateInput`:** that DTO is `deny_unknown_fields` and is reused by ALL five validating tools — adding `mode` there leaks the field into `calculate`/`explain`. Instead, parse `mode` in the render handler and pass the REMAINDER to `validate_input`.
- **Re-implementing the executor for reconcile:** reuse `run(ir, dag, seed)` (executor.rs:92). No second evaluator (mirrors handler.rs:30 `run_executor`).
- **Putting a reader in the reconcile path:** it must use only `Tool.oracle` (already in the bundle) + the executor — never re-open the source workbook.
- **Cross-mode byte equality assertions:** Filled and InputsOnly differ by design; only assert per-mode determinism.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Output json_key → A1 cell address | A new layout-walking resolver | `CellEntry.seed_coord` (artifact_model.rs:38) | It IS the sheet-qualified A1 cell key already. See D-02 resolution below. |
| Re-running the engine at reference inputs | A new evaluator | `sheet_ir::executor::run` (executor.rs:92) | Already the serve-time path. |
| Reference-input values | Recomputing or guessing | `seed_tier_defaults(manifest)` (input.rs:123) | The oracle was computed at these exact defaults — VERIFIED. |
| URI size/codec for `mode` | A new codec or URI field | Add `mode` to the existing JSON payload before base64 | `mode` is a short enum string inside the already-encoded `{dto, provenance}` JSON. |
| Mapping `CellValue` → JSON output value | Bespoke matching | `finite_output_value` (handler.rs:94) pattern | Already handles Number(finite)/Text/Bool/Empty/Error fail-closed. |

**Key insight:** The phase's single highest-value research finding is that **`CellEntry.seed_coord` already holds the sheet-qualified A1 address** — D-02's "research flag" resolves to "the mapping IS 1:1, `cell` is always `Some` in practice, but keep `Option` for safety."

## D-02 Resolution (PRIMARY RESEARCH QUESTION): output json_key → 1:1 sheet-qualified A1 cell

**Answer: YES, the mapping is guaranteed 1:1, and the address data already lives in `Tool.outputs[].seed_coord`.**

Trace:
1. `Tool.oracle` is `BTreeMap<output_json_key, CellValue>` (artifact_model.rs:72). The keys are output json_keys.
2. `Tool.outputs` is `Vec<CellEntry>` (artifact_model.rs:68). Each `CellEntry` has `json_key` AND `seed_coord` (artifact_model.rs:33-40). `seed_coord` is documented as "the fully-qualified `sheet!addr` cell key."
3. **Fixture proof** (`cell_map.json`): the `Calculate_Tax` tool's outputs are
   `{taxable_income → 3_Outputs!B2}`, `{tax_owed → 3_Outputs!B3}`, `{effective_rate → 3_Outputs!B4}`, `{marginal_rate → 3_Outputs!B5}` — and its oracle keys are exactly `{taxable_income, tax_owed, effective_rate, marginal_rate}`. One json_key ↔ one `seed_coord` ↔ one cell.
4. The reconcile already needs `tool.outputs[].seed_coord` to look up `run.computed.get(seed_coord)` for the server value (the exact pattern in `project_tool_outputs`, handler.rs:76). The same `seed_coord` IS the A1 cell address for the `cell` field.

**Conclusion for the planner:**
- The `cell` field is filled from `entry.seed_coord` (already sheet-qualified A1, e.g. `"3_Outputs!B3"`). No layout walk, no `LayoutDescriptor`/`CellLayout` lookup needed.
- An oracle key that has NO matching `Tool.outputs` entry (json_key in `oracle` but not in `outputs`) → `cell = None` (D-02 nullable safety). This is the only realistic `None` path; it indicates a malformed/skewed bundle, and the row still reports deltas.
- Keep the type `Option<String>` exactly as D-02 specifies. In conforming bundles it is always `Some`.

## D-07 Resolution (SECOND RESEARCH QUESTION): text + boolean formula outputs

**The tax bundle has NO text or boolean formula outputs today — all five outputs are numeric.** Confirmed by reading `manifest.json` (every output `"dtype": "number"`), `cell_map.json` (oracle values all `{"Number": ...}`), and `executable.ir.json` (every output formula is arithmetic). The loan bundle was not located in this tree; only the `tax-calc@1.1.0` golden fixture exists under the toolkit tests.

**What must be added (the fixture work for D-06/D-07):** extend the `tax-calc` fixture's `3_Outputs` sheet (or a new small output Table) with:
- **One text formula output** — e.g. a `bracket_label` cell whose formula is `IF(taxable_income>=40000,"bracket_2","bracket_1")` (or any text-producing formula the executor supports). Its oracle is the text value at reference inputs.
- **One boolean formula output** — e.g. an `is_taxable` cell whose formula is `taxable_income>0` (a comparison → `CellValue::Bool`). Its oracle is `true`/`false` at reference inputs.

**Concrete fixture edits required (all five artifacts must stay hash-consistent — they are integrity-locked):**
1. `manifest.json` — add the two new output `CellRole`s (`role:"output"`, `dtype:"text"` and `dtype:"bool"`).
2. `executable.ir.json` — add the two new formula cells (`"Formula": {...}`) referencing existing cells.
3. `cell_map.json` — add the two new `CellEntry` outputs to a tool's `outputs` AND their oracle values to that tool's `oracle`.
4. `layout.json` — add the two `CellLayout` cells with `formula` set (so the writer emits formula+cached-result — this is what WBVER-01 proves).
5. `BUNDLE.lock` — re-fold the per-artifact + combined hashes (the bundle loader integrity-verifies at boot; a stale lock fails the load). Use `build_bundle_lock` / `fold_evidence_hash` (artifact_model.rs:149,168) the same way the compiler emits them — OR regenerate the fixture through the compiler if a generator exists.

**Verify the executor supports the chosen text/bool formulas** before authoring them: check `semantics::apply` and `scalar_eval` support for `IF` and comparison operators. The executor returns `CellValue::Text`/`CellValue::Bool` for these (the `scalar_to_leaf`/`EvalValue` paths handle Bool/Text — executor.rs:330-338). The WBVER-01 unit test then asserts the rendered xlsx contains `<f>` (formula) AND `<v>` (cached result) for the text/bool cells.

**Recommendation:** the cleanest path is to author the new cells so their oracle is trivially derivable from the existing reference inputs, and re-fold the lock. If a fixture-regeneration tool exists in `pmcp-workbook-compiler`, prefer regenerating to hand-editing five hash-linked JSON files (hand-editing the lock is error-prone). The planner should add a Wave-0 task to locate/confirm the fixture generation path.

## Item 2 plumbing detail (WBVER-02) — the `mode` arg parse gotcha

`render_workbook`'s `compute` (handler.rs:602) calls `validate_input(args, ...)` directly. `validate_input` deserializes into `CalculateInput` which is `#[serde(deny_unknown_fields)]` (input.rs:47). **A `mode` key in the args would be rejected as `invalid_input` today.** Plumbing options for the planner (Claude's-discretion per CONTEXT):
1. In the render handler, take `args` as a `Value`, lift out `args["mode"]` (default `"filled"`, unknown → `Err`), then pass the remaining `{inputs, overrides}` object to `validate_input`. Encode the parsed `RenderMode` into the URI payload.
2. The render input schema (`assemble_input_schema`, schema.rs:392, top-level `additionalProperties:false`) must add a `mode` property `{"type":"string","enum":["filled","inputs_only"]}` so the advertised schema matches what the handler accepts (the "advertise == accept" invariant the codebase enforces everywhere, e.g. F2 override keys).

**URI size impact (quantified):** `mode` is a single short enum string (`"filled"` / `"inputs_only"`) added to the `{dto, provenance}` JSON payload before base64. Worst case ~`"mode":"inputs_only",` ≈ 24 JSON bytes → ~32 base64 bytes. `MAX_ENCODED_URI_LEN` is 64 KiB (render_uri.rs:66) and a tax payload is a few hundred bytes — the addition is ~0.05% of the cap. **No size risk.** Add a property-test assertion that a `mode`-carrying URI round-trips and stays < `MAX_ENCODED_URI_LEN`.

**Back-compat for `decode`:** make the new `mode` field default to `Filled` when ABSENT in the decoded payload (`#[serde(default)]`), so any URI minted before this phase still decodes (WBVER-04 no-regression). The existing `prop_decode_total` proptest (render_uri.rs:277) already feeds arbitrary payloads — it will continue to pass, but ADD a round-trip prop that mints with each mode.

## Common Pitfalls

### Pitfall 1: Breaking the existing `prop_decode_total` / round-trip proptests
**What goes wrong:** Adding a required `mode` field to `RenderPayload` makes old/forged payloads fail to deserialize differently than before, or makes `encode`/`decode` non-deterministic.
**How to avoid:** `#[serde(default)]` on the decode side (absent → `Filled`); keep `RenderPayloadRef` field order stable so encode stays byte-deterministic (the `encode_is_deterministic` test, render_uri.rs:205, must still pass).
**Warning signs:** `round_trip_yields_same_dto_and_provenance` or `encode_is_deterministic` start failing.

### Pitfall 2: Stale BUNDLE.lock after fixture edits
**What goes wrong:** Editing manifest/IR/cell_map/layout without re-folding `BUNDLE.lock` → `load_bundle` fails the integrity check at boot → every workbook test fails with a hash mismatch.
**Why it happens:** The bundle is integrity-locked (`build_bundle_lock`, artifact_model.rs:168; loader verifies at boot, bundle_loader.rs).
**How to avoid:** Regenerate the lock (prefer a compiler/fixture generator over hand-editing). Verify with `load_bundle` in a test before relying on it.
**Warning signs:** `golden bundle boots` expect() panics in the toolkit tests.

### Pitfall 3: PMAT cognitive complexity > 25 on the extended `write_computed_value`
**What goes wrong:** Folding mode × value-type × formula-presence × format-presence into one `match` blows past cog-25.
**Why it happens:** 3 value types × 2 modes × 2 formula states × 2 format states is a combinatorial match.
**How to avoid:** Factor the `write_formula_or_value` helper (per design §4.2) so each value type delegates to ONE shared formula-or-literal function that takes the cached-result string + mode. Keep `write_computed_value` a thin dispatcher. This is exactly the decomposition the existing code already uses (`write_number_cell`/`write_string_cell` are separate helpers).
**Warning signs:** CI `pmat quality-gate --checks complexity` flags `write_computed_value` or `render_xlsx`.

### Pitfall 4: `deny(panic)` violation in the new reconcile / handler code
**What goes wrong:** `unwrap`/`expect`/indexing in the new `reconcile.rs` or `VerifyAccuracyHandler`.
**Why it happens:** The runtime lib (`lib.rs:18`) and the toolkit workbook modules (`#![cfg_attr(not(test), deny(...))]`, e.g. render_uri.rs:36) forbid panics on the value path.
**How to avoid:** Use `?`/`ok_or_else`/`get` everywhere; D-03's unknown-filter → `Err` (not panic), D-04's empty-oracle → vacuous report (not skip). The `verify_accuracy` no-input + filter parsing must be total.
**Warning signs:** clippy `unwrap_used`/`expect_used`/`panic` denials.

### Pitfall 5: `verify_accuracy` not actually surfacing mismatches in tests
**What goes wrong:** Because a conforming bundle ONLY compiles if reconciliation passed, a happy-path reconcile is always `all_within_tol` — a test that only checks the golden gives a false sense of coverage.
**How to avoid:** The design §6 testing bar requires a **perturbed-oracle** unit test: construct a `Tool` with a deliberately-wrong oracle value and assert the row is `within_tol = false` and `all_within_tol = false`. Test the diff logic, not just the golden.
**Warning signs:** No negative-path reconcile test exists.

## Runtime State Inventory

> This is primarily an additive code phase, but it touches a committed integrity-locked fixture, so the relevant categories:

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | The committed golden bundle `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/` (5 artifacts + evidence/) is integrity-locked; D-07 edits to manifest/IR/cell_map/layout require a re-folded `BUNDLE.lock`. | Data migration: regenerate/re-fold the lock after fixture edits. |
| Live service config | None — no deployed pmcp.run state changes this round (re-version loop is OUT OF SCOPE). | None. |
| OS-registered state | None. | None — verified (no schedulers/services touched). |
| Secrets/env vars | None. | None — verified (no secret/env names referenced). |
| Build artifacts | The embedded fixture is `include_dir!`-baked into `workbook_table_authoring` example + `workbook-embedded` tests; a fixture change is picked up on rebuild (no stale artifact beyond a normal `cargo build`). | None beyond rebuild. |

**Doc/count drift to fix (string-level):** `workbook/mod.rs` says "five served tools" / "all FIVE served tools" / "register all five workbook tools" (lines 4-5, 152-153, 165-166, 265). These become **six** with `verify_accuracy`. The H3 binding test `reserved_tool_names_match_the_registered_meta_tool_names` (handler.rs:671) asserts `RESERVED_TOOL_NAMES` == the registered meta-tool NAMEs — **adding a 6th meta tool requires updating `pmcp_workbook_runtime::RESERVED_TOOL_NAMES`** (manifest_model.rs) AND this binding test, or the compiler's reserved-name gate drifts from what is registered. This is a load-bearing cross-crate constant — do not miss it.

## Code Examples

### Verified: `Formula::set_result` accepts string results (text/bool writable)
```rust
// Source: crates/pmcp-workbook-runtime/src/render/mod.rs:425 (existing numeric use)
let formula = Formula::new(normalize_formula_for_writer(f)).set_result(format_number(n));
ws.write_formula(row, col, formula).map_err(writer_err)?;
// set_result takes impl Into<String>; for Text(s) pass s, for Bool(b) pass if b {"TRUE"} else {"FALSE"}.
// Design §3 confirms: "cached results are strings, so text/bool results are writable".
```

### Verified: the existing Text/Bool arm that Item 1 must make formula-aware
```rust
// Source: crates/pmcp-workbook-runtime/src/render/mod.rs:389-390 (CURRENT — formula-blind)
Some(CellValue::Text(s)) => write_string_cell(ws, row, col, s, fmt)?,
Some(CellValue::Bool(b)) => write_string_cell(ws, row, col, &b.to_string(), fmt)?,
// Item 1: route these through a write_formula_or_value helper that checks cell.formula + mode.
```

### Verified: server-value lookup pattern reconcile reuses
```rust
// Source: crates/pmcp-server-toolkit/src/workbook/handler.rs:76-82 (project_tool_outputs)
let Some(value) = run.computed.get(&entry.seed_coord) else { /* fail closed */ };
let projected = finite_output_value(value, &entry.seed_coord, &entry.json_key)?;
// reconcile_reference uses the SAME run.computed.get(&entry.seed_coord) — and entry.seed_coord
// is also the D-01 `cell` A1 address.
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Only numeric formula cells carry cached results | All output types (after Item 1) carry formula+cached-result | This phase (WBVER-01) | Excel `fullCalcOnLoad="1"` recomputes text/bool outputs too — closes the trust blind spot. |
| `render_xlsx(layout, run)` single mode | `render_xlsx(layout, run, mode)` Filled/InputsOnly | This phase (WBVER-02) | Double-entry verification download. |
| Five served tools | Six (adds `verify_accuracy`) | This phase (WBVER-03) | Runtime-inspectable reference reconciliation. |

**Deprecated/outdated:** None — purely additive. The single `calculate` tool was already retired into per-Table tools in a PRIOR phase (handler.rs:156) — not this phase.

## Assumptions Log

| # | Claim | Section | Risk if Wrong |
|---|-------|---------|---------------|
| A1 | The executor's `semantics::apply`/`scalar_eval` support `IF(...)` returning text and comparison operators returning `Bool`, so a text/bool formula output can be authored in the fixture. | D-07 Resolution | If a chosen function is unsupported, the fixture output computes to `Error` and WBVER-01 can't be demonstrated with that formula. Mitigation: planner verifies supported functions in `semantics`/`scalar_eval` before authoring (Wave 0). |
| A2 | The reference inputs for `reconcile_reference` are exactly `seed_tier_defaults(manifest)`. | Item 3 / Pattern 3 | VERIFIED against the fixture (oracle = outputs at defaults), but if some bundle's oracle were captured at non-default inputs, reconcile would flag false mismatches. Low risk — the compiler computes oracle at defaults. Planner should confirm the compiler's oracle-capture path uses tier defaults. |
| A3 | A fixture-regeneration path exists (or hand-folding `BUNDLE.lock` via `build_bundle_lock`/`fold_evidence_hash` is feasible) for the D-07 fixture edit. | D-07 / Pitfall 2 | If no generator exists, hand-folding the lock is error-prone but doable using the runtime's hashing helpers. Planner adds a Wave-0 task to locate the generator. |
| A4 | `rust_xlsxwriter::Formula::set_result` signature is `impl Into<String>` in the workspace-pinned version. | Code Examples | Stated by design §3 ("VERIFIED technical facts") and consistent with the existing numeric `.set_result(format_number(n))` call. If the pinned version differs, the text/bool result type must be adjusted. Confirm against the locked `Cargo.lock` version. |

## Open Questions

1. **Where exactly should the text/bool formula outputs live in the fixture — extend an existing tool's outputs, or add a 3rd tool?**
   - What we know: D-06 wants one cohesive narrative; the tax bundle has two tools (`Calculate_Tax`, `Estimate_Refund`).
   - What's unclear: whether adding the text/bool outputs to `Calculate_Tax` keeps the example readable.
   - Recommendation: add them to `Calculate_Tax` (e.g. `bracket_label` text, `is_taxable` bool) so one tool demonstrates numeric + text + bool — minimal fixture churn, single tool to narrate.

2. **Is there a compiler-side fixture generator, or must `BUNDLE.lock` be hand-folded?**
   - Recommendation: Wave-0 task to grep `pmcp-workbook-compiler` for a tax-calc fixture emitter; prefer regeneration.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `cargo` / Rust toolchain | All builds/tests | ✓ (assumed — workspace) | stable | — |
| `rust_xlsxwriter` | render writer | ✓ (workspace dep, purity-check asserts present) | locked | — |
| `make` (quality-gate/purity-check/doc-check) | WBVER-04 gates | ✓ | — | — |
| `pmat` 3.15.0 | CI cog-25 gate | CI-only (per CLAUDE.md D-07) | 3.15.0 | runs in CI; not in local pre-commit |

**Missing dependencies with no fallback:** None — all-internal phase.

## Validation Architecture

> `.planning/config.json` not inspected for `nyquist_validation`; treating as enabled (absent = enabled). This phase's ALWAYS bar (CLAUDE.md) already mandates fuzz/property/unit/example, so this section maps requirements to the existing scaffolding.

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `proptest` (the "fuzz" surface is proptest-as-fuzz per existing `prop_decode_total`) |
| Config file | none — cargo workspace |
| Quick run command | `cargo test -p pmcp-workbook-runtime render::` and `cargo test -p pmcp-server-toolkit workbook::` |
| Full suite command | `make quality-gate` (fmt/clippy/build/test/audit) + `make purity-check` + `make doc-check` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WBVER-01 | text & bool formula cells → `<f>`+`<v>` in xlsx XML | unit | `cargo test -p pmcp-workbook-runtime render::` | ✅ extend render/mod.rs tests (line 632 already tests text/bool VALUES; add formula+result assertion on xlsx XML) |
| WBVER-02 | InputsOnly → formula cells have `<f>` and NO cached `<v>` | unit | `cargo test -p pmcp-workbook-runtime render::` | ❌ Wave 0 — new test in render/mod.rs |
| WBVER-02 | URI round-trips carrying `mode`; stays < MAX_ENCODED_URI_LEN | property | `cargo test -p pmcp-server-toolkit render_uri` | ✅ extend render_uri.rs proptests (line 248-288) |
| WBVER-02 | unknown mode → `Err` | unit | `cargo test -p pmcp-server-toolkit workbook::handler` | ❌ Wave 0 — new render_workbook handler test |
| WBVER-02 | per-mode render byte-determinism | property | `cargo test -p pmcp-workbook-runtime render::` | ✅ extend `render_xlsx_is_deterministic_byte_identical` (line 522) per mode |
| WBVER-03 | golden bundle → `all_within_tol`; perturbed oracle → flagged mismatch | unit | `cargo test -p pmcp-workbook-runtime reconcile` | ❌ Wave 0 — new reconcile.rs tests |
| WBVER-03 | `all_within_tol ⇔ every output within TOL` | property | `cargo test -p pmcp-workbook-runtime reconcile` | ❌ Wave 0 |
| WBVER-03 | unknown tool filter → `Err` listing tools (D-03); empty oracle → vacuous (D-04) | unit | `cargo test -p pmcp-server-toolkit workbook` | ❌ Wave 0 — new VerifyAccuracyHandler tests |
| WBVER-04 | reader-free; no wire regression | gate | `make purity-check` + existing workbook integration tests | ✅ purity-check + workbook_integration.rs |
| WBVER-04 (D-06) | example demos filled/inputs_only/verify_accuracy | example | `cargo run --example workbook_table_authoring --features workbook-embedded -p pmcp-server-toolkit` | ✅ extend (or add a new example) |

### Sampling Rate
- **Per task commit:** `cargo test -p <touched crate> <module>::`
- **Per wave merge:** full `make quality-gate`
- **Phase gate:** `make quality-gate` + `make purity-check` + `make doc-check` all green; PMAT cog-25 (CI); doctests on new public runtime fns (`reconcile_reference`, `RenderMode`).

### Wave 0 Gaps
- [ ] `render/mod.rs` tests — InputsOnly no-`<v>` assertion; text/bool formula+result `<f>`+`<v>` assertion. *(Requires inspecting produced xlsx XML — unzip the buffer; the existing tests only check the ZIP magic. A helper to extract a sheet's XML from the in-memory buffer is needed.)*
- [ ] `reconcile.rs` — new module with `ReconcileReport`/`reconcile_reference` + unit + property + perturbed-oracle tests.
- [ ] `render_uri.rs` — extend proptests for `mode` round-trip + size bound.
- [ ] `workbook/handler.rs` (or new file) — `VerifyAccuracyHandler` tests (D-03 unknown filter → Err, D-04 empty oracle vacuous).
- [ ] Fixture: text+bool formula outputs added to `tax-calc@1.1.0` with re-folded `BUNDLE.lock`.
- [ ] Update `RESERVED_TOOL_NAMES` + the H3 binding test (handler.rs:671) for the 6th tool.
- [ ] Doctests on `reconcile_reference` and `RenderMode` (CLAUDE.md: doctests on new public runtime fns).

## Security Domain

> `security_enforcement` config not located; this is an internal additive phase with an untrusted-URI surface already hardened. ASVS mapping focuses on the input/codec surface that changes.

### Applicable ASVS Categories
| ASVS Category | Applies | Standard Control |
|---------------|---------|-----------------|
| V5 Input Validation | yes | `mode` arg → `Err` on unknown value (never a panic); `verify_accuracy` filter → `Err` on unknown tool (D-03). The URI remains attacker-controlled — the existing size guard (render_uri.rs:146, FIRST) + total panic-free decode (T-92-17) must keep covering the new `mode` field. |
| V6 Cryptography | no (unchanged) | Bundle integrity hashing (`sha256_hex`/`build_bundle_lock`) is reused, not modified — but the D-07 fixture edit MUST re-fold the lock or the integrity gate fails closed. |
| V2/V3/V4 (auth/session/access) | no | No auth/session surface in this phase. |

### Known Threat Patterns for this stack
| Pattern | STRIDE | Standard Mitigation |
|---------|--------|---------------------|
| Forged/oversized `workbook://` URI carrying a malicious `mode` | Tampering/DoS | Size guard first (render_uri.rs:146); total decode; `mode` defaults to `Filled` on absent/garbage; provenance re-verified before render (render_resource.rs:91). |
| Output-forging via `mode` or reconcile path | Tampering | `verify_accuracy` is read-only (no input seeds beyond reference defaults); `render_workbook` strips `mode` then runs the unchanged `validate_input` (overrides on computed cells already rejected, input.rs:201). |
| Panic-as-DoS in new reconcile/handler code | DoS | `deny(panic/unwrap/expect)` on the value path; D-03/D-04 return values, never panics. |

## Sources

### Primary (HIGH confidence)
- `docs/design/2026-06-22-workbook-accuracy-verification-design.md` — approved Approach A design (§3 verified facts, §4 component map + item designs, §6 testing bar).
- `crates/pmcp-workbook-runtime/src/render/mod.rs` — `write_computed_value`, `write_number_cell`, `render_xlsx`, determinism + non-finite guards (read in full).
- `crates/pmcp-workbook-runtime/src/render/layout.rs` — `CellLayout.formula`/`addr`, `LayoutDescriptor` (read in full).
- `crates/pmcp-workbook-runtime/src/artifact_model.rs` — `Tool`, `Tool.oracle: BTreeMap<json_key, CellValue>`, `CellEntry.seed_coord` (read in full).
- `crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs` — `build_dag`, `run`, `RunResult` (read in full).
- `crates/pmcp-server-toolkit/src/workbook/{render_uri,render_resource,handler,input,mod}.rs` — codec, regen, the 5 handlers + registration, `validate_input`/`seed_tier_defaults`, `assemble_input_schema` (read in full / targeted).
- `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/{cell_map,manifest,executable.ir,layout}.json` — VERIFIED the oracle = outputs at tier defaults; all outputs numeric (D-07 confirmation).
- `Makefile` (purity-check:505, doc-check:417) — the reader-free gate (`PURITY_CRATES`/`PURITY_WRITER_CRATES`) and toolkit per-feature workbook purity assertion.
- `CLAUDE.md` — Toyota Way / ALWAYS coverage / cog-25 / deny-panic constraints.

### Secondary (MEDIUM confidence)
- `crates/pmcp-server-toolkit/examples/workbook_table_authoring.rs` — the existing example to extend (D-06).

### Tertiary (LOW confidence)
- The loan bundle referenced in design §6 was NOT located in this tree (only `tax-calc@1.1.0` exists under the toolkit tests) — flagged; the example/fixture work targets the tax bundle per D-06.

## Project Constraints (from CLAUDE.md)
- **Zero defects / Toyota Way**: `make quality-gate` (fmt --all, clippy pedantic+nursery, build, test, audit) must pass before any commit/PR.
- **ALWAYS coverage for every new feature**: fuzz (proptest), property, unit, AND `cargo run --example` — all four required (mapped in Validation Architecture).
- **Cognitive complexity ≤ 25 per function** (PMAT, CI-enforced 3.15.0); hard cap 50 only with `// Why:` annotated allow.
- **Zero SATD comments**; comprehensive rustdoc with examples (doctests) on new public runtime fns.
- **Tests run `--test-threads=1`** (race prevention) in CI.
- **Contract-first**: update/check contract YAML in `../provable-contracts/contracts/<crate>/` via `pmat comply check` if a contract covers these crates.
- **`deny(panic/unwrap/expect)`** on every value path in runtime + toolkit workbook modules.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; every crate already in the tree and grounded in source.
- Architecture: HIGH — every change site read directly; D-02 1:1 mapping and reference-input source both VERIFIED against the fixture.
- Pitfalls: HIGH — derived from the actual integrity-lock, deny-panic, cog-25, and `deny_unknown_fields` mechanics in the code.
- D-07 fixture path: MEDIUM — the NEED is verified; the cleanest re-fold path (generator vs hand-fold) is an open Wave-0 question.

**Research date:** 2026-06-22
**Valid until:** 2026-07-22 (stable internal code; 30 days)
