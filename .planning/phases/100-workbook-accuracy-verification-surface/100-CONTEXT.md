# Phase 100: Workbook Accuracy-Verification Surface - Context

**Gathered:** 2026-06-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Extend the existing `render_workbook` / `workbook://` download path (Phase 92/96) with three
**additive** business-analyst trust capabilities, all inside the reader-free, stateless/Lambda-safe
boundary:

1. **WBVER-01** — text & boolean formula output cells render as formula-with-cached-result
   (`Formula::set_result`), so Excel's `fullCalcOnLoad="1"` recomputes *all* output types on open
   (today only numeric formulas do).
2. **WBVER-02** — `render_workbook` gains a `mode: filled | inputs_only` argument (default `filled`,
   additive); `inputs_only` emits bare formulas with no cached results, deterministically; unknown
   mode → `Err`; encoded `workbook://` URI stays ≤ 64 KiB.
3. **WBVER-03** — a new `verify_accuracy` meta-tool (the 6th served tool) re-runs the executor at the
   workbook's reference inputs and returns a per-output reconciliation report vs `Tool.oracle` within
   `TOL` — stateless, reader-free, making the compile-time penny-reconcile runtime-inspectable.

This is an approved-design PoC for real BA-customer trials. **The design is locked** — discussion
here only resolves the user-facing latitude the design deliberately left open (§7).

**LOCKED scope fences (from ROADMAP + design):**
- Served runtime stays READER-FREE (no `umya`/`quick-xml`/`calamine` in the served tree;
  `make purity-check` stays green).
- Server stays STATELESS/Lambda-safe (pointer-then-regenerate; no render cache/session).
- `mode` and `verify_accuracy` are ADDITIVE (do not break the existing `render_workbook`/`workbook://`
  contract or wire shapes).
- **Out of scope this round:** the re-version loop (`diff_version` ↔ freshly-uploaded workbook on
  pmcp.run), arbitrary-input server-side delta (impossible without a runtime Excel reader), and
  compile-time golden-vector capture (named scenarios at non-reference inputs).

</domain>

<decisions>
## Implementation Decisions

These resolve the design's §7 "Open questions for BA trials" for *this* round. The approved design
doc (Approach A) governs everything else — do not re-derive the component map, function signatures,
`ReconcileReport` skeleton, error handling, or purity/statelessness approach; read the design doc.

### verify_accuracy report shape (WBVER-03)
- **D-01:** Each output row carries a **sheet-qualified A1 address** in a new `cell` field
  (e.g. `"Sheet1!C12"`) — the cell whose formula produced that output. Resolves §7 q3. Adding it
  now (vs later) avoids a future breaking wire-shape change for existing clients. Row shape becomes
  `{ key, cell, server_value, oracle_value, abs_delta, within_tol }`.
- **D-02:** `cell` is **nullable** (`Option<String>` → JSON null/omitted) when an output json_key
  cannot be resolved to exactly one source cell (mapping missing/ambiguous). The rest of the row
  (deltas, `within_tol`) still reports. Panic-free: a missing mapping never blocks reconciliation.
  → **Research flag:** confirm whether each oracle/output json_key maps 1:1 to a `CellLayout`
  address; if the compiler guarantees it, `cell` will simply always be `Some`, but the type stays
  `Option` for safety.
- **D-03:** Optional tool-name filter that names a **non-existent** tool → **`Err`** listing the
  available tool names (consistent with the toolkit's `deny(panic)` → `Err` discipline). Do NOT
  silently return an empty report or ignore the filter — a typo must not read as "all good."
- **D-04:** A tool with an **empty oracle** (no authored expected values) is **included** in the
  report with `outputs: []` and `all_within_tol = true` (vacuous), contributing 0 to
  `cells_checked`. Transparent — the tool is visibly present but had nothing to attest. Do NOT omit
  it or treat it as malformed.

### inputs_only render mode (WBVER-02)
- **D-05:** `inputs_only` produces a **clean copy** — input cells seeded with the caller's values,
  formula cells written as bare formulas with no `set_result`, **no extra highlighting/formatting/
  comments**. Matches the design doc as written; keeps render byte-determinism trivial and the
  64 KiB URI cap comfortable. Highlighting/commenting the BA's input cells (§7 q1) is **deferred to
  BA feedback** — not built this round.

### Example / demo coverage (WBVER-04 ALWAYS bar)
- **D-06:** Extend **one** existing example bundle (the **tax** bundle) to demonstrate all three
  capabilities end-to-end: `render_workbook(filled)`, `render_workbook(inputs_only)`, and
  `verify_accuracy`. One cohesive narrative over known-good fixtures.
- **D-07:** **Research/coverage flag:** WBVER-01 is specifically about *text & boolean* formula
  outputs, but the tax bundle's known oracle (`tax_owed = 18241.0`) is numeric. The example (and the
  WBVER-01 unit tests) MUST exercise at least one **text** and one **boolean** formula output cell —
  add or adjust a fixture if the chosen bundle lacks them. A numeric-only demo does not prove item 1.

### Claude's Discretion
- Internal helper factoring (e.g. the design's suggested `write_formula_or_value` helper unifying
  Number/Text/Bool formula-or-literal paths), the `RenderMode` enum's exact location, and how `mode`
  threads through `DecodedRender` — all per the design doc; planner/researcher decide specifics.
- Exact `verify_accuracy` tool description wording, provided it keeps the design's **honest framing**:
  it attests the engine matches Excel's authored values *at the reference inputs* (makes the
  compile-time penny-reconcile inspectable), and points BAs to `render_workbook` filled/inputs_only
  where Excel is the oracle for arbitrary inputs.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Approved design (governs this phase)
- `docs/design/2026-06-22-workbook-accuracy-verification-design.md` — Approach A, approved
  2026-06-22. Component map (§4.1), item-by-item design (§4.2 writer fix, §4.3 inputs-only mode,
  §4.4 `verify_accuracy` + `ReconcileReport`), cross-cutting guarantees (§4.5), requirement IDs (§5),
  testing bar (§6), and the §7 open questions this CONTEXT resolves.

### Source files this phase changes (genchi genbutsu targets)
- `crates/pmcp-workbook-runtime/src/render/mod.rs` — `write_computed_value` /
  `write_formula_or_value` (item 1) and `render_xlsx` (gains `RenderMode` param, item 2).
- `crates/pmcp-workbook-runtime/src/artifact_model.rs` — `Tool.oracle:
  BTreeMap<output_json_key, CellValue>` and `TOL`; the reconcile attestation source of truth.
- `crates/pmcp-server-toolkit/src/workbook/render_uri.rs` — `DecodedRender` gains a `mode` field;
  `MAX_ENCODED_URI_LEN` (64 KiB) cap.
- `crates/pmcp-server-toolkit/src/workbook/render_resource.rs` — `regenerate` passes `mode` into
  `render_xlsx`.
- `crates/pmcp-server-toolkit/src/workbook/mod.rs` — registers the served tools; the "five tools"
  count/docs become six (adds `verify_accuracy`).

### Roadmap / requirements
- `.planning/ROADMAP.md` § "Phase 100: Workbook Accuracy-Verification Surface (BA Trust Tools)" —
  goal, success criteria, LOCKED scope fences.
- `.planning/REQUIREMENTS.md` — WBVER-01..04 (lines ~74–77, traceability ~199–202).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pmcp_workbook_runtime::sheet_ir::executor::{build_dag, run}` → `RunResult` — already driven by the
  `calculate` tool; `verify_accuracy`'s reference reconciliation reuses it (no new engine).
- `render_xlsx` already emits numeric formula cells as `Formula::new(...).set_result(...)` — item 1
  extends the same branch to Text/Bool; item 2 adds a no-`set_result` path.
- `Tool.oracle` (Excel's authored expected value per output) + `TOL` (±0.01, mirrored from
  compiler `reconcile::TOL`) — already in the artifact model; `verify_accuracy` diffs against it.
- `CellLayout.formula` (`Some`/`None`) distinguishes formula vs input cells; inputs map to the
  `in_*` named-range convention — used by `inputs_only` seeding and by the `cell` A1 resolution.

### Established Patterns
- Stateless pointer-then-regenerate: `render_workbook` returns a `workbook://render/<b64>` URI; the
  resource recomputes bytes per `resources/read`. `mode` must ride inside that payload so
  `regenerate` reproduces the chosen artifact.
- Toolkit `deny(panic)` → all malformed input (bad mode, oversized URI, unknown filter) surfaces as
  `Err`, never a panic.
- `rust_xlsxwriter` hardcodes `<calcPr fullCalcOnLoad="1"/>` — Excel recomputes & overwrites cached
  results on open; this is what makes formula+cached-result independently verifiable.

### Integration Points
- New `verify_accuracy` handler registers in `workbook/mod.rs` alongside the existing five tools.
- New pure `reconcile_reference(...) -> ReconcileReport` lives in a new `reconcile.rs` in
  `pmcp-workbook-runtime` (reader-free, pure diff).

</code_context>

<specifics>
## Specific Ideas

- `ReconcileReport` (per design §4.4, with D-01 added):
  ```text
  ReconcileReport {
    tolerance: f64,
    all_within_tol: bool,
    cells_checked: u32,
    tools: [ { tool, all_within_tol,
               outputs: [ { key, cell: Option<String>, server_value, oracle_value,
                            abs_delta, within_tol } ] } ],
  }
  ```
- `verify_accuracy` takes no required inputs; optional tool-name filter (D-03 governs the miss case).
- Honest framing required in the tool description (see Claude's Discretion above).

</specifics>

<deferred>
## Deferred Ideas

Design §7 questions intentionally left for BA-trial feedback (NOT this round):
- **Highlight/comment input cells** in the `inputs_only` download (§7 q1) — D-05 ships a clean copy;
  revisit if BAs ask for it.
- **Named golden scenarios** (compile-time captured input→output vectors) so `verify_accuracy` can
  attest at chosen *non-reference* inputs (§7 q2) — explicitly in the LOCKED out-of-scope list.
- **The re-version loop** — analyst finds a discrepancy → uploads fixed workbook → recompile →
  redeploy (`diff_version` ↔ pmcp.run upload). Separate later integration.
- **Arbitrary-input server-side delta** vs Excel — impossible while the runtime is reader-free; the
  downloadable formula workbook (Excel in the BA's hands) is the oracle for arbitrary inputs.

None of these are scope for Phase 100.

</deferred>

---

*Phase: 100-workbook-accuracy-verification-surface*
*Context gathered: 2026-06-22*
