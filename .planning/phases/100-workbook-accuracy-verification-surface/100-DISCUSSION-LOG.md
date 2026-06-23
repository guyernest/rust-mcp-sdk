# Phase 100: Workbook Accuracy-Verification Surface - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-06-22
**Phase:** 100-workbook-accuracy-verification-surface
**Areas discussed:** A1 address in report, inputs_only affordances, verify_accuracy report scope, Example/demo coverage

> Note: this phase ships an approved design doc (`docs/design/2026-06-22-workbook-accuracy-verification-design.md`, Approach A). Most implementation is locked by that doc; discussion only resolved the §7 "Open questions for BA trials" that affect this round's wire shape/deliverable.

---

## A1 address in the reconcile report

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, add `cell` field | Sheet-qualified A1 (`Sheet1!C12`) per output row; locked into WBVER-03 now, additive-safe now / breaking later | ✓ |
| Yes, A1 only (no sheet) | Bare A1 (`C12`), ambiguous in multi-sheet workbooks | |
| No, keep minimal | Ship the design's locked shape, defer to BA feedback | |

**User's choice:** Yes, add `cell` field (sheet-qualified A1).
**Notes:** Follow-up on unresolvable mappings → `cell` is `Option<String>` (nullable), rest of row still reports; panic-free. Chosen over Err-the-whole-tool and always-resolvable.

---

## inputs_only affordances

| Option | Description | Selected |
|--------|-------------|----------|
| Clean copy (design default) | Seed inputs, bare formulas, no extra formatting; least writer work; defer highlighting to BA feedback | ✓ |
| Highlight input cells | Distinct fill/format on input cells; UX win but adds scope + determinism risk | |
| Cell comments on inputs | Excel notes on input cells; riskier for byte-determinism + 64 KiB cap | |

**User's choice:** Clean copy (design default).
**Notes:** §7 q1 (highlighting) deferred to BA-trial feedback.

---

## verify_accuracy report scope

### Unknown tool-name filter

| Option | Description | Selected |
|--------|-------------|----------|
| Err (unknown tool) | Return Err listing available tool names; consistent with deny(panic)→Err | ✓ |
| Empty report | Valid report, zero tools — silently hides typos | |
| Ignore filter | Fall back to reporting all tools — surprising | |

**User's choice:** Err (unknown tool).

### Empty-oracle tool

| Option | Description | Selected |
|--------|-------------|----------|
| Include, empty outputs | `outputs: []`, all_within_tol vacuously true, 0 cells_checked; transparent | ✓ |
| Omit from report | Cleaner but tool silently missing | |
| Err | Treat as malformed bundle — too aggressive | |

**User's choice:** Include with empty outputs.

---

## Example / demo coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Extend one bundle, all three (tax) | Tax bundle (has populated oracle) demos filled / inputs_only / verify_accuracy end-to-end | ✓ |
| Loan bundle | Equivalent effort; only matters if loan shows text/bool outputs better | |
| You decide | Planner picks whichever best exercises text+bool outputs | |

**User's choice:** Extend one bundle (tax), all three capabilities.
**Notes:** Coverage flag carried into CONTEXT — tax oracle (`tax_owed`) is numeric, but WBVER-01 needs text + boolean formula outputs demonstrated; example/tests must add or adjust a fixture covering those.

---

## Claude's Discretion

- Internal helper factoring (`write_formula_or_value`), `RenderMode` enum location, how `mode` threads through `DecodedRender` — all per design doc.
- Exact `verify_accuracy` tool description wording, provided it keeps the design's honest framing (reference-point attestation; points to render_workbook where Excel is the oracle for arbitrary inputs).

## Deferred Ideas

- Highlight/comment input cells in inputs_only download (§7 q1).
- Named golden scenarios at non-reference inputs (§7 q2) — in LOCKED out-of-scope list.
- Re-version loop (diff_version ↔ pmcp.run upload).
- Arbitrary-input server-side delta vs Excel (impossible while reader-free).
