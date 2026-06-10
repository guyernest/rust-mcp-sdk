# Workbook Dialect Spec (v0.5.0, Phase 7)

> **The published, constrained workbook dialect contract (DIA-01).** This is the
> BA/auditor-facing "moat" document: a governed Excel workbook conforms to *this*
> dialect or it does not compile. It is published as human-readable prose **and**
> bound by an automated test to the machine `WHITELIST` const in
> `crates/pmcp-workbook-dialect/src/lib.rs` so the published contract and the
> enforced rule can never drift.
>
> Cites the architecture brief `docs/Excel-as-Configuration-Architecture-Brief.md`
> §5 (the dialect: layered sheets, function set, acyclic DAG) and §7 (cell
> taxonomy). It does not copy them — read the brief for the design rationale.

## 1. What this dialect is

A constrained subset of Excel/Open XML that a governed quoting workbook must
stay within so it can be **compiled** — offline, deny-by-default — into a
deterministic, evidence-bearing MCP tool surface. The dialect is a *moat*: a
workbook that conforms is safe to compile; a workbook outside it is refused with
a precise, located, BA-actionable lint finding (sheet + cell + rule + repair),
never silently accepted and never auto-"fixed".

The compiler reads the workbook into an **owned, `umya`-free model** (the
`WorkbookMap`) and lints over that owned model. Colour is read only as a
**lintable UX projection** — never as canonical truth (architecture brief §7).

## 2. Layered sheets (architecture brief §5)

A conforming workbook is organised into **numbered layers** by sheet-name prefix
so the compiler can reason about sheet roles structurally rather than by guessing:

| Prefix | Layer | Purpose |
|--------|-------|---------|
| `0_` | Guide | The legend: the colour ontology + human documentation |
| `1_` | Inputs | Per-quote, BA-overridable inputs |
| `2_` | Constants / governed | BA-set governed constants (prices, margins, rules) |
| `3_` | Calculation / output | Derived formulas and the quote output |

The numbered-layer convention is the ordered set the linter checks sheet names
against (`DialectRules::sheet_layer_prefixes()`).

## 3. Whitelisted function set (DIA-05)

A conforming formula may only call functions in the **whitelist**. The whitelist
is **deny-by-default**: any function token not in the set is a located
`whitelist/unsupported-fn` lint **error** (never silently accepted, never
auto-widened — see §6).

The set is a flat list of **13 first-class functions** that the lighthouse
workbook (`docs/UFH_Quote_Process_Model_Plot3.xlsx`) already authors, so it
lints clean **as-authored**. Every function below is first-class — there is no
core/widened tiering.

| Function | Category | Notes |
|----------|----------|-------|
| `IF` | whitelist | conditional |
| `VLOOKUP` | whitelist | table lookup |
| `INDEX` | whitelist | positional lookup |
| `MATCH` | whitelist | positional search |
| `SUMIF` | whitelist | conditional sum |
| `SUM` | whitelist | aggregate |
| `ROUNDUP` | whitelist | round toward +inf |
| `CEILING` | whitelist | round up to multiple |
| `IFERROR` | whitelist | error-guard the lighthouse authors |
| `ISNUMBER` | whitelist | type test the lighthouse authors |
| `SEARCH` | whitelist | substring search the lighthouse authors |
| `ROUND` | whitelist | half-up rounding the lighthouse authors |
| `TEXT` | whitelist | number formatting the lighthouse authors |

Total: **13 names**. This table is the *published* contract; the `WHITELIST`
const is the *enforced* contract. An automated binding test
(`pmcp_workbook_dialect::dialect_spec::doc_whitelist_table_matches_const`) parses
the function names out of this very table and asserts set-equality with
`WHITELIST`, so if either drifts the build fails.

The arithmetic **operators** `+ - * / ^` are part of the dialect but are checked
separately — they are not function tokens and do not appear in the whitelist.

## 4. Typed inputs and the two-layer metadata model

- **Typed inputs.** Inputs are typed (number / text / bool) so the compiled tool
  surface can carry a `outputSchema` with units and meanings (architecture brief §7).
- **Two-layer metadata (DIA-03 / DIA-04).** One logical manifest with two
  physical projections that must agree on overlap: **named ranges (+ structured
  tables)** are the BA-facing canonical surface, and a hidden **`_Manifest`
  sheet** is the technical enrichment layer (units, meanings, loop metadata, role
  classifications). Colour is linted *against* the manifest; it is never the
  source of truth. (Synthesis + round-trip of this model is Plan 04, not Plan 03.)

## 5. The refuse-set (DIA-02, D-07 / D-08)

A conforming workbook contains **none** of the following. Each is a precise,
located lint **error** carrying a BA-actionable repair, collected all at once
(the linter never stops at the first finding):

| Rule id | What is refused | Why | Owned `WorkbookMap` field read |
|---------|-----------------|-----|--------------------------------|
| `structure/macro` | a macro-bearing workbook (`.xlsm` / VBA) | the compiler never executes anything; macros are unverifiable code | `wb.has_macros` |
| `structure/external-link` | a formula referencing another workbook (`[1]Sheet1!…`, `[Book.xlsx]…`) | pulls unseen values from outside the governed file | `wb.external_links` (+ raised during ingest) |
| `structure/hidden-sheet` | a `Hidden` **or** `VeryHidden` sheet | conceals real role/value from the BA and the compiler | `SheetRecord.state` |
| `structure/hidden-row` | a hidden row | conceals located content | `SheetRecord.hidden_rows` |
| `formula/array` | a legacy CSE array formula or a dynamic-array spill | non-scalar semantics the v1 compiler does not model | `CellRecord.formula_kind` (`Array` / `DynamicArray`) |
| `whitelist/unsupported-fn` | a function token outside the §3 whitelist | smuggles unsupported semantics | `CellRecord.formula` (token scan vs `WHITELIST`) |
| `manifest/range-out-of-bounds` | a defined name targeting a sheet outside the layered set | a named range pointing somewhere unexpected | `wb.defined_names` |
| `role/conditional-formatting` | conditional formatting covering a role cell (D-08) | CF colour is *evaluated*, not stored; the compiler refuses the role rather than evaluate CF | `SheetRecord.cf_ranges` |
| `role/merged-cell` | a merge covering a role cell | a role cell must be a single addressable cell | `SheetRecord.merges` |

**Colour ontology (lint-only).** The dialect reads blue font `FF0000FF` → input,
green fill `FFE2EFDA` → constant, yellow fill → assumption, default-font formula
cell → formula. These are *evidence labels* used to propose roles and to lint
colour against the manifest — never canonical (architecture brief §7).

## 6. Phase-7-enforced vs declared-for-Phase-9

To avoid overclaiming, every dialect rule is marked as either **ENFORCED in
Phase 7** (the linter actually checks it over the owned `WorkbookMap`) or
**DECLARED but deferred** (part of the published dialect, but not checked until a
later phase builds the machinery).

### Enforced in Phase 7

- The full **refuse-set** of §5 (structural + role + formula-kind), over the owned
  `WorkbookMap` — the linter never reaches back into `umya`.
- The **whitelist token scan** (§3) — deny-by-default, located, no auto-widening.
- The **two-layer overlap** consistency check and **colour-vs-manifest** lint
  (DIA-03 / DIA-04) land with manifest synthesis (Plan 04, same phase).

### Declared but deferred (NOT enforced in Phase 7)

- **Acyclic dependency DAG.** The architecture brief §5 requires the formula
  graph to be an acyclic DAG. **Phase 7 does NOT reconstruct a dependency DAG and
  does NOT check acyclicity** — that requires the formula parser + DAG
  reconstruction, which is **Phase 9**. The dialect *declares* the requirement;
  Phase 9 enforces it.
- **Full formula parsing / AST.** Phase 7 uses a lightweight *token* scan for the
  whitelist check (§3), not a lexer or parser. The Phase-9 parser supersedes it.

### Known Phase-7 limitation — whitelist string-literal false positive

The whitelist scan is a **token approximation**, not a lexer. A function name
that appears *inside a quoted string literal* — e.g. `TEXT(x,"0")` containing a
literal `"SUM("` inside the quotes, or a cell whose text content is `"OFFSET("` —
can be mistaken for a function call. The Phase-7 scanner mitigates the common case
by skipping over quoted-string regions, but a fully robust resolution (a real
lexer that tracks string vs formula context) is **Phase 9**. This is an accepted
Phase-7 approximation, documented here so it is not mistaken for a defect.

The scanner also strips a leading `_xlfn.` future-function prefix before
comparison, so `_xlfn.CONCAT(` is compared as `CONCAT`.

---

*Bound to `crates/pmcp-workbook-dialect/src/lib.rs` `WHITELIST` by
`pmcp_workbook_dialect::dialect_spec::doc_whitelist_table_matches_const`. Cites
`docs/Excel-as-Configuration-Architecture-Brief.md` §5 / §7.*
