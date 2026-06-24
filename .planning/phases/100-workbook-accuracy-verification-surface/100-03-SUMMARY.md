---
phase: 100-workbook-accuracy-verification-surface
plan: 03
subsystem: workbook-runtime + workbook-toolkit
tags: [workbook, render, RenderMode, inputs_only, double-entry, workbook-uri, serde-default, WBVER-02]

# Dependency graph
requires:
  - phase: 100-workbook-accuracy-verification-surface
    plan: 02
    provides: write_formula_or_value shared helper (the single set_result site InputsOnly forks); render/mod.rs cell-scoped extract_sheet_xml + cell_xml test helpers
provides:
  - "pub enum RenderMode { Filled, InputsOnly } (runtime leaf, re-exported at crate root); serde rename filled/inputs_only; Default=Filled; no catch-all variant (unknown string = decode Err)"
  - "render_xlsx gains a 3rd mode param; InputsOnly writes BARE formulas (no set_result) via build_formula"
  - "workbook:// URI payload carries mode (#[serde(default)] => absent=Filled back-compat; malformed=decode Err); encode/decode/regenerate thread it"
  - "render_workbook parses+strips a render-only mode arg (unknown=Err) and advertises an optional mode enum on the RENDER schema ONLY (render_input_schema_for_manifest); never leaks into calculate/explain"
affects: [100-04, 100-05]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Render-only schema wrapper: render_input_schema_for_manifest wraps the shared input_schema_for_manifest and injects the mode prop, so calculate/explain (which call the shared fn directly) never advertise mode (advertise == accept, render-scoped)"
    - "mode parse-and-strip BEFORE validate_input: lift args[mode] out of the object (unknown -> Err), then hand the remaining {inputs,overrides} to a deny_unknown_fields DTO"
    - "serde Default governs only an ABSENT field; a PRESENT-but-unknown enum string stays a decode Err (no #[serde(other)]) — relied on at three layers (enum, URI payload, handler arg)"

key-files:
  created: []
  modified:
    - crates/pmcp-workbook-runtime/src/render/mod.rs
    - crates/pmcp-workbook-runtime/src/lib.rs
    - crates/pmcp-server-toolkit/src/workbook/render_uri.rs
    - crates/pmcp-server-toolkit/src/workbook/render_resource.rs
    - crates/pmcp-server-toolkit/src/workbook/handler.rs
    - crates/pmcp-server-toolkit/src/workbook/schema.rs

key-decisions:
  - "DEVIATION (Rule 1): rust_xlsxwriter ALWAYS structurally emits <v> for a formula cell (a bare formula defaults to <v>0</v>) and ALWAYS sets fullCalcOnLoad=1, so the plan's literal 'InputsOnly emits <f> with NO <v>' is not expressible via the writer. Implemented the ACHIEVABLE, semantically-equivalent invariant: InputsOnly writes a bare formula (no set_result) so the cell carries the writer's NEUTRAL <v>0</v> placeholder (NOT the executor's value) with no value-type attr; Filled carries the executor's cached value. Since fullCalcOnLoad=1 is always set, Excel recomputes all outputs on load — the server contributes zero output values in InputsOnly (verification guarantee holds)."
  - "render_input_schema_for_manifest is a thin wrapper, NOT a change to assemble_input_schema (which is shared by render AND explain via input_schema_for_manifest) — adding mode in assemble_input_schema would have leaked it into explain. The plan said 'add to assemble_input_schema'; the wrapper is the no-leak-correct realization of the same advertise==accept intent."
  - "build_formula helper holds the single mode branch (Filled => set_result(cached), InputsOnly => bare) so write_formula_or_value stays a flat 4-arm dispatcher under cog-25"
  - "RenderPayloadRef appends mode LAST so the existing dto/provenance encode byte order is unchanged (encode_is_deterministic stays byte-identical)"

requirements-completed: [WBVER-02]

# Metrics
duration: ~40min
completed: 2026-06-23
---

# Phase 100 Plan 03: render_workbook inputs_only Mode (WBVER-02) Summary

**Gave `render_workbook` an optional, additive, render-only `mode: filled | inputs_only` that threads a new runtime-leaf `RenderMode` enum from the tool arg, through the `workbook://` URI payload, through `regenerate`, into `render_xlsx` — producing a double-entry verification copy where formula cells are written BARE (the server contributes zero output values; Excel is the sole oracle via the always-on `fullCalcOnLoad`).**

## Performance

- **Duration:** ~40 min (Task 1 commit -> SUMMARY)
- **Completed:** 2026-06-23
- **Tasks:** 3/3 committed (all TDD)
- **Files modified:** 6

## Accomplishments

- **Task 1 (`7356149a`):** Added `pub enum RenderMode { Filled, InputsOnly }` (serde `filled`/`inputs_only`, `Default=Filled`, no catch-all so an unknown string is a decode `Err`), re-exported at the runtime crate root. `render_xlsx` gained a 3rd `mode` param threaded `render_sheet -> write_cell -> write_computed_value -> write_formula_or_value`; the new `build_formula` helper holds the single mode branch (Filled `set_result(cached)` vs InputsOnly bare). All 13 in-crate callers pass `RenderMode::Filled` (default path byte-unchanged). 20 `render::` tests pass (+2): per-cell InputsOnly bare-formula assertion, malformed-string decode `Err`, per-mode determinism (two InputsOnly byte-equal AND two Filled byte-equal).
- **Task 2 (`9aef384b`):** `render_uri.rs` carries `mode` in all three payload structs — `RenderPayload` (DECODE) with `#[serde(default)]` so an ABSENT key defaults to Filled (pre-phase back-compat) while a PRESENT malformed value is a decode `Err`; `RenderPayloadRef` (ENCODE) appends `mode` LAST (encode determinism unchanged). `encode()` takes `mode`; `decode()` lifts it; `render_resource::regenerate` captures `decoded.mode` (Copy) and threads it into `render_xlsx`. 9 `render_uri` tests pass: round-trip carries inputs_only + asserts `< 64 KiB`; a LITERAL pre-phase payload (no `mode` key) decodes to Filled; `"mode":"bogus"` decodes to `Err`; `prop_encode_decode_identity` round-trips a generated mode + size bound; `prop_decode_total` still holds.
- **Task 3 (`f1a69b00`):** `RenderWorkbookHandler::compute` lifts+strips `mode` via the new `parse_render_mode` BEFORE `validate_input` (so `CalculateInput`'s `deny_unknown_fields` does not reject it); unknown value -> `invalid_input` `Err` (never a silent Filled, never a panic). New `schema::render_input_schema_for_manifest` wraps the shared schema builder and injects an optional top-level `mode` enum `["filled","inputs_only"]` (advertise == accept) — added ONLY on the render schema; calculate/explain keep the shared `input_schema_for_manifest` and `CalculateInput` has no `mode` field. 27 `handler` tests pass (+3): inputs_only happy path (URI decode proves InputsOnly; no-mode -> Filled), unknown mode -> isError/Err, no-leak invariant.

## Task Commits

1. **Task 1 (TDD): RenderMode enum + thread mode through render_xlsx** — `7356149a` (feat)
2. **Task 2 (TDD): carry mode through the workbook:// URI payload** — `9aef384b` (feat)
3. **Task 3 (TDD): parse mode arg + render-only schema** — `f1a69b00` (feat)

## Verification

- `cargo test -p pmcp-workbook-runtime render::` -> **20 passed** (was 18; +2 new).
- `cargo test -p pmcp-server-toolkit --features workbook-embedded --lib render_uri` -> **9 passed**.
- `cargo test ... --lib render_resource` -> **6 passed**; `... --lib workbook::handler` -> **27 passed**; full `... --lib 'workbook::'` -> **74 passed**.
- `cargo test ... --test workbook_integration --test workbook_multi_tool` -> **3 passed** (no regression).
- `make purity-check` -> **PASSED** (reader-free served cone; rust_xlsxwriter present; cargo-deny bans clean).
- `pmat analyze complexity --max-cognitive 25` -> **no violations** on render/mod.rs, handler.rs, schema.rs, render_uri.rs.
- All three commits passed the pre-commit `make quality-gate` hook (no `--no-verify`).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Writer-API constraint] InputsOnly bare formula carries the neutral `<v>0</v>` placeholder, not a literally-absent `<v>`**
- **Found during:** Task 1 (RED -> the per-cell assertion `!contains("<v>")` failed: `<c r="B5"><f>SUM(B1:B4)</f><v>0</v></c>`).
- **Issue:** The plan's behavior spec ("InputsOnly renders formula cells as bare formulas with NO cached `<v>`") is not expressible via `rust_xlsxwriter` 0.95 — its formula serializer ALWAYS emits `<f>{f}</f><v>{result}</v>` (a bare formula defaults `result` to `"0"`), and the workbook's `calcPr` is ALWAYS `fullCalcOnLoad="1"`. There is no public API to suppress the `<v>` element for a formula cell.
- **Fix:** Implemented the achievable, semantically-equivalent invariant. InputsOnly writes the bare formula (no `set_result`), so the cell carries the writer's NEUTRAL `<v>0</v>` placeholder — NOT the executor's computed value (`123`/`bracket_2`/`1` in Filled) — and no value-type attribute (`t="str"`/`t="b"`). The verification guarantee is fully preserved: the server contributes zero output values in InputsOnly, and because `fullCalcOnLoad=1` is always set, Excel recomputes every output on load (Excel is the sole oracle). The test asserts the contrast per cell: InputsOnly => `<f>` + `<v>0</v>` + no executor value + no type attr; Filled => `<f>` + the executor's cached value.
- **Files modified:** crates/pmcp-workbook-runtime/src/render/mod.rs (test + enum/render_xlsx/build_formula doc comments).
- **Commit:** `7356149a`.

**2. [Rule 3 - Blocking, no-leak correctness] render-only schema via a wrapper, not by editing the shared assemble_input_schema**
- **Found during:** Task 3 (reading the schema call graph).
- **Issue:** The plan said to add the `mode` property in `assemble_input_schema`, but that fn is shared by the render schema AND the explain schema (both flow through `input_schema_for_manifest`). Editing it would have leaked `mode` into the explain schema, violating the locked no-leak invariant (T-100-07) and the plan's own acceptance criterion.
- **Fix:** Added a thin `render_input_schema_for_manifest` wrapper that calls the shared builder then injects `mode`, used ONLY by `RenderWorkbookHandler::metadata`. Same advertise==accept intent, correctly render-scoped. A test asserts calculate + explain schemas do NOT carry `mode`.
- **Files modified:** crates/pmcp-server-toolkit/src/workbook/schema.rs, handler.rs.
- **Commit:** `f1a69b00`.

## Threat Surface

Threat-register dispositions held:
- **T-100-05** (forged/oversized URI w/ malicious or malformed mode): the `MAX_ENCODED_URI_LEN` size guard still runs FIRST in `decode`; `#[serde(default)]` makes an ABSENT mode Filled while a PRESENT malformed value is a decode `Err`; `prop_decode_total` (decode never panics) still passes. Provenance re-verified before render.
- **T-100-06** (unknown mode reaching the writer): `parse_render_mode` returns `Err` for any value outside `{filled, inputs_only}` BEFORE `validate_input`; `deny(panic)` upheld (`.as_object_mut`/`.remove`, no unwrap).
- **T-100-07** (mode leaking into CalculateInput / corrupting calculate/explain): mode is stripped from args before `validate_input` and is NOT on `CalculateInput` nor the calculate/explain schemas — proven by the no-leak test.
- **T-100-SC** (installs): no new packages — internal change over already-vetted serde/base64. N/a.

No new security-relevant surface beyond the plan's threat model.

## Deferred Issues

- **Pre-existing unused-import warning (out of scope):** `crates/pmcp-server-toolkit/src/code_mode.rs:557` `unused import: pmcp_code_mode::CodeExecutor`. Not in this plan's diff; not auto-fixed per the executor SCOPE BOUNDARY. (This crate is not clippy-gated by CI `make lint`, which lints only root `pmcp --features full`, so it does not block the gate.)

## Known Stubs

None.

## Notes

- NON-RELEASABLE INTERMEDIATE STATE persists from Plans 01/02: docs/constants reference `verify_accuracy` before its handler exists (lands in Plan 04). Do NOT ship the repo between Plan 01 and Plan 04 completion.

## Next

Plan 100-04 (WBVER-03): `verify_accuracy` reference reconciliation — the handler that retires the Plan-01 `verify_accuracy` placeholder and closes the H3 binding drift.

## Self-Check: PASSED

- SUMMARY.md present at the expected path.
- All 3 task commits (`7356149a`, `9aef384b`, `f1a69b00`) present in git history.
- `grep -c 'enum RenderMode' render/mod.rs` == 1; lib.rs re-exports RenderMode.
- `render_input_schema_for_manifest` present in schema.rs; `mode` field present on DecodedRender/RenderPayload/RenderPayloadRef.
