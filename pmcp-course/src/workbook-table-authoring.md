# Authoring Workbooks as MCP Tools: The Table Contract

> **Who this is for.** You have a business process in an Excel spreadsheet and you want
> an AI to call it correctly. In this chapter you will author a workbook so the `pmcp`
> compiler turns it into a named, typed, AI-callable MCP tool surface — using only
> visible, standard Excel Tables — and you will *preview that surface before deploying*.
>
> **Reference.** The authoritative contract is
> [`docs/design/workbook-table-authoring-contract.md`](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/design/workbook-table-authoring-contract.md).
> This chapter teaches it through the shipped `template.xlsx`.

## Learning objectives

By the end of this chapter you will be able to:

1. Identify the **four region types** in a workbook and know which two you structure.
2. Author an **Inputs Table** and one or more **output Tables** with the standard
   columns, and explain what the `value` cell does.
3. Predict the **DAG-derived per-tool input schema** for a given set of formulas.
4. Use the **`tier` dropdown** to govern which fields the AI may set.
5. Run `cargo pmcp workbook explain` to **preview the tool surface before deploy**.

## 1. The big idea: the workbook IS the contract

You already have the process in a spreadsheet. The promise of the table model is that
authoring stays *visible, standard Excel* — no hidden machinery — and the compiler
derives a tool surface an LLM can select and call on the first try.

Two ends of one pipeline:

- **You author** — name input and output regions as Excel Tables, fill realistic
  example values, pick governance from a dropdown.
- **An LLM consumes** — each output Table becomes a **named, described MCP tool** with a
  precise input schema (only the fields it uses) and an output schema.

> **Always lead with the CLI.** The command you will lean on throughout:
>
> ```sh
> cargo pmcp workbook explain your-workbook.xlsx
> ```
>
> It prints exactly what an AI will see. You are never guessing.

## 2. The four region types

| Region | Excel form | Exposed to the AI? |
|--------|-----------|--------------------|
| **Input Table** | Excel Table, `name \| value \| description \| tier` | yes — tool inputs |
| **Output Table(s)** | Excel Table, `name \| value \| description` | yes — one tool per Table |
| **Reference / lookup** | any cells/ranges (`VLOOKUP` rate cards) | no |
| **Intermediate calc** | any sheets | no |

You only ever *structure* the two declaration Tables. Calc and lookup sheets stay
free-form; the compiler walks the cross-sheet formula DAG between them.

> **Checkpoint 2.** Open a spreadsheet you already use. Circle the cells a caller would
> supply (candidate inputs) and the cells that hold final answers (candidate outputs).
> Everything else is DAG interior. You have just found your two declaration Tables.

## 3. The standard columns

### Input Table — `name | value | description | tier`

- **`name`** — the semantic key the AI uses (the served JSON key). Single source of truth.
- **`value`** — does quadruple duty: working example · type witness · unit source (number
  format) · enum source (data-validation dropdown) · the single end-to-end test input ·
  the reconciliation seed.
- **`description`** — co-located per-field description (no separate sheet → no drift).
- **`tier`** — a `{variable, strict}` dropdown. `strict` = BA-governed constant, *not*
  caller-exposed. Default `variable`.

### Output Table — `name | value | description`

- **`name`** — the served output key.
- **`value`** — the authored expected result = the reconciliation **oracle** the gate
  checks. Your example is your test.
- **`description`** — feeds the tool's `outputSchema`.
- The **Table's name is the tool name**; a **caption cell directly above** the Table is
  the **tool description**.

### What gets harvested from where

| Schema field | Source |
|--------------|--------|
| field key | `name` column cell |
| type | `value` cell type |
| unit | `value` cell number format (currency → `USD`, `%` → `rate`, date → `date`) |
| enum | data-validation list on the `value` cell |
| description | `description` column cell |
| tier / strict | `tier` column cell (inputs only) |
| test input | `value` cell |
| output oracle | output `value` cell |

> **Exercise 3.** In the template's Inputs Table, `income`'s value `100000` is formatted
> as currency and `rate`'s value `0.22` as a percent. Without running anything, write
> down the `type` and `unit` the compiler will harvest for each. (Answer: `income` →
> `number`, unit `USD`; `rate` → `number`, unit `rate`.)

## 4. One output Table → one MCP tool, with DAG-derived inputs

Each named output Table becomes one MCP tool. The lifted shape:

```text
Workbook → {
  inputs:  [ InputField{ key, type, unit?, enum?, description, tier } ],  // shared pool
  tools:   [ Tool{ name, description, input_keys, outputs, oracle } ]
}
```

The crucial mechanic: a tool advertises **only the inputs upstream of its output cells**
in the formula DAG. In the template:

- `calculate_tax`: `tax_owed = ROUND(income*rate - 3759, 0)` → reaches `income`.
- `estimate_refund`: `refund = ROUND(withheld - tax_owed, 0)` → reaches `withheld` and,
  through `tax_owed`, `income`.

So the two tools are **disjoint on `withheld`** — only `estimate_refund` advertises it.

> **Exercise 4.** Suppose you add a third output Table `summary` with one row
> `headline = effective_rate`. Which inputs will `summary` advertise? (Answer:
> `effective_rate = ROUND(tax_owed/income, 3)` reaches `income`, so `summary` advertises
> `income` — plus any workbook-wide governed input like `filing`.)

## 5. The shipped template, annotated

```text
0_meta (optional)         server: tax-suite   version: 1
─────────────────────────────────────────────────────────────────────
Table "Inputs"            name      | value    | description            | tier ▼
                          income    | 100000   | annual gross (USD $)   | variable
                          filing    | single ▼ | filing status          | variable   ← enum from dropdown
                          withheld  | 15000    | tax withheld YTD (USD) | variable
                          rate      | 0.22     | statutory bracket rate | strict     ← not caller-exposed
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

## 6. Governance via the tier dropdown

- **`variable`** — a normal caller-exposed input (default).
- **`strict`** — a BA-governed constant, *not* offered to the AI. In the template,
  `rate` is `strict` — you control the statutory rate, the caller does not.

Governance is a *column*, co-located with the field, so it can never drift. Provenance
(the workbook-level identity the gate verifies) is orthogonal and untouched.

> **Checkpoint 6.** If you flipped `withheld` from `variable` to `strict`, what would
> happen to `estimate_refund`'s input schema? (Answer: `withheld` would no longer be a
> caller input — `estimate_refund` would advertise only `income` and `filing`, and
> `withheld` would become a governed constant you set.)

## 7. Preview before deploy — the habit to build

Run the dry-run preview on the template:

```sh
cargo pmcp workbook explain template.xlsx
```

```text
tool calculate_tax
  description: Compute federal tax from income & filing
  inputs:
    income: number
  outputs:
    tax_owed: number
    effective_rate: number
tool estimate_refund
  description: Estimate refund given withholding
  inputs:
    income: number
    withheld: number
  outputs:
    refund: number
```

Read this the way an LLM does — as a coherent "what I do / how to call me." The preview is
projected through the same compiler path the server registers, so it can't drift from what
gets served. Note `calculate_tax` advertises only `income`, not `filing` — `filing` isn't
referenced by any tax formula, so the DAG doesn't surface it on that tool (the per-tool
minimal-input contract). If a tool name is cryptic, a description is missing, or an input is
unexpected, you fix the *Excel* and re-run. For tooling, `--format json` emits the same surface:

```sh
cargo pmcp workbook explain template.xlsx --format json
```

> **Capstone exercise.** Take a real one-page spreadsheet of your own. (1) Add an Inputs
> Table and one output Table with the standard columns. (2) Add a `tier` dropdown and
> mark one constant `strict`. (3) Run `cargo pmcp workbook explain`. (4) Confirm the
> printed tool surface reads as a coherent tool to a human — the one property only a
> human can judge. Iterate on names and captions until it does.

## A note for readers upgrading

The table model **replaces** the old named-range authoring model. You no longer
hand-author per-cell named ranges to mark inputs and outputs, nor a separate metadata
sheet to name and describe tools. Field keys, types, units, enums, descriptions,
governance, the tool name, and the tool description all now come from visible Excel
Tables and a caption cell. The old named-range model is gone — this is the one authoring
model going forward.

## Summary

- Author two declaration **Tables**: an Inputs Table and one output Table per tool.
- The `value` cell carries type, unit, enum, example, test seed, and (for outputs) the
  reconciliation oracle.
- Each output Table becomes one MCP tool with **DAG-derived** inputs.
- Govern fields with the `tier` dropdown; provenance is separate and untouched.
- Build the habit: **`cargo pmcp workbook explain` before every deploy.**
