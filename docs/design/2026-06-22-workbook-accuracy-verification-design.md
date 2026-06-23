# Workbook accuracy-verification surface — design

**Status:** Approved design (pre-implementation) · **Date:** 2026-06-22
**Track:** v2.3 Excel-as-Configuration MCP Servers (workbook)
**Audience:** SDK implementers + pmcp.run integration + business-analyst (BA) trial users

---

## 1. Goal

Give business analysts a way to **trust that the generated MCP server reproduces
their Excel workbook**, and to **probe it with their own inputs**, so they can
decide whether the deployed server is correct or needs a new workbook version.

Three capabilities, all extending the workbook runtime/toolkit (Approach A —
minimal additive, chosen 2026-06-22):

1. **Close the trust blind spot** — text/boolean formula outputs render as
   formula-with-cached-result (today only numeric formulas do), so Excel
   independently recomputes **all** output types on open.
2. **Inputs-only verification mode** — a `render_workbook` mode that fills inputs
   only and leaves every formula for Excel to compute from scratch (double-entry;
   the server contributes zero output values).
3. **Reference reconciliation attestation** — a `verify_accuracy` tool that
   re-runs the engine at the workbook's reference inputs and reports, per output,
   whether it matches Excel's authored value within tolerance.

## 2. Non-goals (this round)

This is a **PoC to put in front of real BA customers** and iterate from their
questions/comments. Explicitly out of scope now:

- **Arbitrary-input server-side delta** vs. Excel — impossible without a runtime
  Excel reader (the served runtime is reader-free by gate). For arbitrary inputs,
  the **downloadable formula workbook is the oracle** (Excel, in the analyst's
  hands). The server-side attestation is reference-point only.
- **The re-version loop** (analyst finds a discrepancy → uploads a fixed workbook
  → recompile → redeploy). `diff_version` already exists; wiring it to a freshly
  uploaded version on pmcp.run is a separate, later integration.
- Any change to the compiler's offline penny-reconcile / promote gate.

## 3. Background — what already ships (genchi genbutsu, 2026-06-22)

The download capability is **already built** and is a default tool; this work
*extends* it, it does not create it:

- **`render_workbook`** is one of the built-in server's default meta-tools
  (alongside `calculate`, `explain`, `get_manifest`, `diff_version`). It validates
  inputs, runs the executor, and returns a stateless `workbook://render/<b64>`
  pointer URI (inputs + provenance encoded; no server-side cache — Lambda-safe).
- **`workbook://` resource** (`render_resource.rs`) regenerates a deterministic,
  downloadable base64 `.xlsx` on every `resources/read` via
  `pmcp_workbook_runtime::render::render_xlsx`, after re-verifying provenance.
- The compiled bundle carries a full **`layout.json`** (`LayoutDescriptor`: every
  sheet/cell, formats, fills, merges, widths, hidden flags) — the writer replays
  "a copy of the original workbook, filled in," not a stripped report.
- `render_xlsx` already emits **numeric formula cells as
  `Formula::new(...).set_result(<server value>)`** — original formula + cached
  result.
- `rust_xlsxwriter` hardcodes **`<calcPr fullCalcOnLoad="1"/>`** in every file, so
  Excel **recomputes all formulas on open** and overwrites the cached results —
  the independent-verification path works today for numeric outputs.
- The runtime artifact model already carries a **per-tool reconcile oracle**:
  `Tool.oracle: BTreeMap<output_json_key, CellValue>` = Excel's authored expected
  value per output, plus a `TOL` (±0.01) tolerance mirrored from the compiler's
  `reconcile::TOL`.

Verified technical facts the design relies on:

- `rust_xlsxwriter::Formula::set_result(impl Into<String>)` — cached results are
  strings, so text/bool results are writable (`"TRUE"`/`"FALSE"` for bools).
- Executor entry: `pmcp_workbook_runtime::sheet_ir::executor::{build_dag, run}`
  → `RunResult`. The `calculate` tool already drives it; reconciliation reuses it.
- Input vs. formula cells are distinguishable (`CellLayout.formula` is
  `Some`/`None`); inputs map to the `in_*` named-range convention.

## 4. Design (Approach A)

### 4.1 Component map

| Unit | Change | Crate |
|---|---|---|
| `render/mod.rs::write_computed_value` | **Item 1**: text/bool formula cells → `Formula+set_result` | `pmcp-workbook-runtime` |
| `render/mod.rs::render_xlsx` | **Item 2**: add `RenderMode` parameter | `pmcp-workbook-runtime` |
| new `reconcile.rs` (pure fn) | **Item 3**: re-run at reference inputs, diff vs `Tool.oracle` | `pmcp-workbook-runtime` |
| `render_uri.rs` (`DecodedRender`) | **Item 2**: carry `mode` in the URI payload | `pmcp-server-toolkit` |
| `render_workbook` handler | **Item 2**: optional `mode` arg → encode into URI | `pmcp-server-toolkit` |
| `render_resource.rs::regenerate` | **Item 2**: pass `mode` into `render_xlsx` | `pmcp-server-toolkit` |
| new `verify_accuracy` handler + `mod.rs` registration | **Item 3**: 6th meta-tool | `pmcp-server-toolkit` |

All additions stay inside the reader-free boundary — no new architectural seam.

### 4.2 Item 1 — formula re-verification for all output types (writer fix)

`write_computed_value`'s `Number` arm already emits
`Formula::new(normalize_formula_for_writer(f)).set_result(...)` when
`cell.formula.is_some()`. Extend the same branch to:

- `Text(s)` → cached result = the string `s`.
- `Bool(b)` → cached result = `"TRUE"` / `"FALSE"`.

Factor a `write_formula_or_value` helper so all three value types share one
formula-or-literal path. Non-formula cells, error/empty results, and the
non-finite-number guard are unchanged. **Effect:** every formula output — numeric,
textual, boolean — carries its original formula, so `fullCalcOnLoad="1"` makes
Excel recompute all of them. No protocol change.

### 4.3 Item 2 — inputs-only render mode (double-entry)

New `RenderMode { Filled, InputsOnly }` (runtime). `render_xlsx(layout, run, mode)`:

- **Filled** (default) — today's behavior (formulas + server's cached results).
- **InputsOnly** — formula cells written as **bare formulas, no `set_result`**;
  only input + literal cells carry values, seeded with the **caller's** inputs
  from `run`. The server contributes zero output values; Excel computes every
  output from scratch. Determinism preserved (fixed doc properties; nothing
  cached to vary).

Plumbing: `mode` joins the input DTO + provenance in the `workbook://` payload
(`DecodedRender` gains a field), so the stateless `regenerate` reproduces the
chosen artifact. `render_workbook` gains an optional
`mode: "filled" | "inputs_only"` argument (default `"filled"`; unknown value →
`Err`, never a panic). The encoded URI must stay within the existing 64 KiB cap.

### 4.4 Item 3 — `verify_accuracy` meta-tool (reference reconciliation)

New pure runtime fn `reconcile_reference(cell_map, &layout, tol) -> ReconcileReport`:
seed the executor with the workbook's **reference input values** (input cells'
captured values), `run`, project each tool's outputs, and compare to
`tool.oracle[key]` within `TOL`.

```text
ReconcileReport {
  tolerance: f64,
  all_within_tol: bool,
  cells_checked: u32,
  tools: [ { tool: String, all_within_tol: bool,
             outputs: [ { key, server_value, oracle_value, abs_delta, within_tol } ] } ],
}
```

The `verify_accuracy` tool (no required inputs; optional tool-name filter) returns
this as structured JSON.

**Honest framing (in the tool description):** it attests the engine matches
Excel's authored values *at the reference inputs* — i.e., it makes the
compile-time penny-reconcile **inspectable at runtime**. It will not surface new
discrepancies (the bundle only compiled because reconciliation passed); its value
is transparency the BA/pmcp.run can query rather than trust blindly. For arbitrary
inputs, the description points to `render_workbook` (filled / inputs_only) where
Excel is the oracle.

### 4.5 Cross-cutting guarantees

- **Purity:** all additions are writer-only / pure-diff; `make purity-check` stays
  green (no `umya`/`quick-xml`/`calamine` in the served tree).
- **Stateless / Lambda-safe:** `verify_accuracy` computes from the in-memory
  bundle; render stays pointer-then-regenerate (no cache/session).
- **Back-compat:** `mode` is optional (default `filled`) → existing
  `render_workbook` callers unaffected; `verify_accuracy` is purely additive. The
  "five served tools" doc/count in `mod.rs` becomes six.
- **Panic-free:** toolkit `deny(panic)` upheld — malformed mode / oversized URI /
  bad address all surface as `Err`.

## 5. Proposed requirement IDs

| ID | Requirement |
|---|---|
| `WBVER-01` | Text & boolean formula output cells render as formula-with-cached-result, so Excel recomputes all output types on open (item 1). |
| `WBVER-02` | `render_workbook` accepts `mode: filled \| inputs_only` (default `filled`); `inputs_only` emits bare formulas with no cached results, deterministically; unknown mode → `Err`; URI ≤ 64 KiB (item 2). |
| `WBVER-03` | A `verify_accuracy` meta-tool re-runs the engine at reference inputs and returns a per-output reconciliation report vs. `Tool.oracle` within `TOL`, stateless and reader-free (item 3). |
| `WBVER-04` | No regression to existing `render_workbook`/`workbook://` behavior or wire shapes; `make quality-gate` + `make purity-check` + `make doc-check` green; PMAT cog-25; ALWAYS coverage (fuzz/property/unit/example). |

## 6. Testing (project ALWAYS bar)

- **Unit:** text & bool formula cells → assert `<f>`+`<v>` in the produced xlsx
  XML; InputsOnly → formula cells have `<f>` and **no** cached `<v>`; reconcile
  over a known bundle → `all_within_tol`, and a perturbed oracle → a flagged
  mismatch.
- **Property:** `all_within_tol ⇔ every output within TOL`; render byte-determinism
  for both modes; `workbook://` URI round-trips carrying `mode`.
- **Fuzz:** extend the existing `render_uri` decode fuzz target with the `mode`
  field.
- **Example:** extend a workbook example to demonstrate `render_workbook(filled)`,
  `render_workbook(inputs_only)`, and `verify_accuracy` over the loan/tax bundle.
- **Gates:** `make quality-gate` + `make purity-check` + `make doc-check` green;
  PMAT cog-25; doctests on the new public runtime fns.

## 7. Open questions for BA trials (feedback-driven next iteration)

These intentionally stay open; the first customer PoCs should answer them:

- Do BAs expect the inputs-only download to **highlight** which cells are theirs to
  fill vs. computed (e.g., comment/format the input cells), or is a clean copy
  better?
- Is the reference-point attestation enough, or do BAs want **named golden
  scenarios** (compile-time captured input→output vectors) so `verify_accuracy`
  can attest at chosen non-reference inputs too?
- Should `verify_accuracy` surface the **per-cell A1 address** alongside the output
  key for analysts who think in spreadsheet terms?
- How should the re-version loop feel to a BA (item deferred) once they find a
  discrepancy?
