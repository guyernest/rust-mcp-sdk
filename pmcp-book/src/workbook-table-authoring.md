# Workbook Table Authoring: Your Excel Process as a Governed MCP Tool

> **Audience:** business analysts and domain experts who already have the process in
> a spreadsheet. You will learn to author an Excel workbook so the `pmcp` compiler
> turns it into a well-named, well-typed, AI-callable MCP tool surface — using nothing
> but **visible, standard Excel Tables**. No formulas-as-config, no hidden machinery.

The authoritative reference for everything below is the design contract
[`docs/design/workbook-table-authoring-contract.md`](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/design/workbook-table-authoring-contract.md).
This chapter teaches the contract from the worked example shipped as `template.xlsx`.

## The north star

**The Excel workbook IS the MCP tool contract.** You already have the process in a
spreadsheet. Authoring should be *visible, standard Excel* — you name two kinds of
region (inputs, outputs) as **Excel Tables**, fill in realistic example values, and
pick governance from a dropdown. The compiler derives a well-named, well-described,
well-typed tool surface an LLM client can select and call correctly on the first try.

Two ends of one pipeline:

- **You author** — name input and output regions as Excel Tables with standard
  columns; fill realistic example values; pick governance from a dropdown.
- **An LLM consumes** — each output Table becomes a **named, described MCP tool** with
  a precise input schema (only the fields it uses) and an output schema.

> **Lead with the CLI.** Everything in this chapter is previewable before you deploy
> anything. The single most important command you will run is:
>
> ```sh
> cargo pmcp workbook explain your-workbook.xlsx
> ```
>
> It prints exactly the tool surface an AI will see — the best guard against the
> silent-broken-deploy class. We come back to it at the end, but keep it in mind: you
> are always one command away from seeing what you built.

## The four region types

A workbook has four kinds of region. You only ever *structure* two of them.

| Region | Excel form | Purpose | Exposed to the AI? |
|--------|-----------|---------|--------------------|
| **Input Table** | named Excel Table, columns `name \| value \| description \| tier` | caller-supplied fields | yes — as tool inputs |
| **Output Table(s)** | named Excel Table, columns `name \| value \| description` | a tool's results; **table name = tool name** | yes — one MCP tool per Table |
| **Reference / lookup** | any cells/ranges (rate cards, `VLOOKUP` tables) | DAG interior data | no |
| **Intermediate calc** | any sheets | business logic (the formula DAG interior) | no |

Only the **declaration Tables** (inputs, outputs) are structured. Your calc and lookup
sheets stay free-form — the compiler walks the cross-sheet formula DAG from input-Table
value cells to output-Table value cells, so intermediate sheets are fully supported and
never exposed.

## The standard columns

### Input Table — `name | value | description | tier`

- **`name`** — the semantic key the AI calls the field by (the served JSON key). This
  is the *single source of truth* for the field's name.
- **`value`** — does quadruple duty. It is simultaneously your working example, the
  **type witness** (a number cell → `number`, text → `string`), the **unit source**
  (the cell's number format: currency → `USD`, `%` → `rate`, a date format → `date`),
  the **enum source** (a data-validation dropdown freezes a closed list), the single
  **end-to-end test input**, and the **reconciliation seed**.
- **`description`** — the per-field description, co-located with the field so it can
  never drift from a separate metadata sheet.
- **`tier`** — governance, via a `{variable, strict}` dropdown. `strict` means a
  BA-governed constant that is **not** caller-exposed (a bracket rate you control, not
  something the AI may set). Default is `variable`.

### Output Table — `name | value | description`

- **`name`** — the served output key.
- **`value`** — the **authored expected result**. This is the reconciliation *oracle*
  the governance gate checks: if your formulas ever stop producing this value, the
  build fails. Your example is your test.
- **`description`** — the per-output description (feeds the tool's `outputSchema`).
- The **Table's name is the tool name**; a **caption cell directly above the Table**
  holds the **tool description**.

### How each schema field is harvested

| Schema field | Source |
|--------------|--------|
| field key | the `name` column cell |
| type (`number`/`string`/`boolean`) | the `value` cell's type |
| unit | the `value` cell's **number format** (currency → `USD`, `%` → `rate`, date → `date`) |
| enum domain | a **data-validation list** on the `value` cell |
| description | the `description` column cell |
| tier / strict | the `tier` column cell (input Tables only) |
| example / test input | the `value` cell |
| output expected (oracle) | the output `value` cell |

The `tier` dropdown **dogfoods** the enum-from-dropdown mechanism — the template teaches
the pattern by using it for its own governance column.

## One output Table → one MCP tool

**Each named output Table becomes one MCP tool.** A single output value is just the
N=1 case — there is no special path. Multiple output Tables in one workbook produce
multiple tools (different business paths, intermediate steps), which is strictly better
for LLM tool-selection than one generic `calculate` with a `mode` flag.

The compiler lifts the workbook into this shape:

```text
Workbook → {
  inputs:  [ InputField{ key, type, unit?, enum?, description, tier } ],  // shared pool
  tools:   [ Tool{
              name,            // = output Table name  (→ MCP tool name)
              description,     // = the caption above the output Table
              input_keys,      // DERIVED from the formula DAG (see below)
              outputs,         // [ OutputField{ key, type, unit?, description } ]
              oracle,          // { <output key>: <expected value> } — per-tool gate
            } ]
}
```

Each tool emits an MCP `inputSchema` (its derived inputs, fully typed) **and** an
`outputSchema` (its outputs → `structuredContent`), matching the SDK's
`TypedToolWithOutput` pattern.

### Per-tool inputs are DAG-derived

This is the key ergonomic win. A tool advertises **only the inputs that are upstream of
its output Table's cells** in the formula DAG. The compiler already has the graph;
reachability gives each tool a precise, minimal schema.

Concretely, in the shipped template:

- `calculate_tax`'s `tax_owed = ROUND(income*rate - 3759, 0)` reaches **`income`**.
- `estimate_refund`'s `refund = ROUND(withheld - tax_owed, 0)` reaches **`withheld`**
  *and* (through `tax_owed`) `income`.

So the two tools have **disjoint** input sets on `withheld` — only `estimate_refund`
advertises it. An LLM sees exactly the inputs each tool needs, nothing more.

Edge cases the compiler handles for you:

- an input reachable only through a constant path is **excluded**;
- an input that feeds *no* tool raises a `"feeds no tool"` lint;
- a shared intermediate feeding multiple tools contributes to each tool's own union.

## The shipped template, annotated

The single `template.xlsx` is your starting point, this documentation's worked example,
and the honest reference fixture, all at once. Here is its annotated structure:

```text
0_meta (optional)         server: tax-suite   version: 1
─────────────────────────────────────────────────────────────────────
Table "Inputs"            name      | value    | description            | tier ▼
                          income    | 100000   | annual gross (USD $)   | variable
                          filing    | single ▼ | filing status          | variable   ← enum from dropdown
                          withheld  | 15000    | tax withheld YTD (USD) | variable
                          rate      | 0.22     | statutory bracket rate | strict     ← not caller-exposed
                                                  └ value: type+unit+example+enum+test seed
        │ (formula DAG, cross-sheet)            ▲
        ▼                                        │ VLOOKUP(ref_brackets)
   ┌─ calc sheets + lookup/reference regions (DAG interior, not exposed) ─┐
        │                                        │
        ▼                                        ▼
"Calculate_Tax"  ← caption: "Compute federal tax from income & filing"   [TOOL]
Table            name           | value | description
                 tax_owed       | 18241 | federal tax liability (USD)
                 effective_rate | 0.182 | effective tax rate (%)
                   inputs (DAG-derived): income, filing

"Estimate_Refund" ← caption: "Estimate refund given withholding"         [TOOL]
Table            name    | value | description
                 refund  | -3241 | estimated refund (neg = owed)
                   inputs (DAG-derived): income, filing, withheld
```

Four region types to learn; two tools emitted; each tool's inputs derived
automatically; the whole contract *and* its end-to-end test case authored in visible
Excel Tables.

## Governance via the tier dropdown

Per-field governance lives in the `tier` column, picked from a `{variable, strict}`
dropdown:

- **`variable`** — a normal caller-exposed input (the default).
- **`strict`** — a BA-governed constant. It is *not* offered to the AI as an input; you
  control it. In the template, `rate` is `strict` — the statutory bracket rate is yours
  to set, not the caller's.

Because governance is a column on the Inputs Table (not a separate config artifact),
it is "fill in the standard form" — business-friendly, co-located, and impossible to
drift from the field it governs. Provenance (the workbook-level identity the gate
verifies) is orthogonal and untouched by any of this.

## Fail helpful, preview before deploy

The compiler's row linting names the **exact cell or row** on any problem: a blank
`name`, a duplicate key, a value-less row, an input that feeds no tool, an output Table
with no caption (a missing tool description), or a tool name that cannot map to the MCP
charset.

And before you deploy anything, run the dry-run preview:

```sh
cargo pmcp workbook explain template.xlsx
```

```text
tool calculate_tax
  description: Compute federal tax from income & filing
  inputs:
    filing: string [enum: single|married]
    income: number [USD]
  outputs:
    tax_owed: number
    effective_rate: number
tool estimate_refund
  description: Estimate refund given withholding
  inputs:
    filing: string [enum: single|married]
    income: number [USD]
    withheld: number [USD]
  outputs:
    refund: number
```

This is **exactly the tool surface an AI will see**: the tool names, their descriptions
(your captions), and the per-tool input/output schemas (DAG-derived, typed, with units
and enums). Read it as a coherent "what I do / how to call me" — that is what an LLM
reads. For tooling, `--format json` emits the same surface as JSON:

```sh
cargo pmcp workbook explain template.xlsx --format json
```

The template plus the preview together replace the old invisible-failure mode entirely:
you can *see* your tool surface before a single client ever connects.

## A note for readers upgrading

If you used an earlier version of `pmcp`, the table model **replaces** the old
named-range authoring model. You no longer hand-author per-cell named ranges to mark
inputs and outputs, nor a separate metadata sheet to name and describe tools.
Everything — field keys, types, units, enums, descriptions, governance, the tool name,
and the tool description — now comes from visible Excel Tables and a caption cell. The
old named-range model is gone; this is the one authoring model going forward.

## Summary

- Author two kinds of region as **Excel Tables**: an **Inputs** Table
  (`name | value | description | tier`) and one **output Table per tool**
  (`name | value | description`, with a caption above it as the tool description).
- The `value` cell does the heavy lifting — type, unit, enum, example, test seed, and
  (for outputs) the reconciliation oracle.
- Each output Table becomes one MCP tool; its inputs are **DAG-derived** so each tool
  advertises exactly what it needs.
- Governance is a `tier` dropdown; provenance is orthogonal and untouched.
- Run `cargo pmcp workbook explain <file>` to **preview the exact AI-facing tool
  surface before you deploy** — text by default, `--format json` for tooling.

Your Excel process becomes a governed, AI-callable tool — authored entirely in visible,
standard Excel.
