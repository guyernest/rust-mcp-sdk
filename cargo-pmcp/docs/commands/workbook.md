# cargo pmcp workbook

Preview, lint, compile, and emit governed Excel-workbook bundles.

## Usage

```
cargo pmcp workbook <SUBCOMMAND>
```

## Description

Turn a governed Excel workbook into a deterministic `bundle@version` directory that
[`pmcp-workbook-server`](../../../crates/pmcp-workbook-server/README.md) (or a
`--kind workbook-server` scaffold) serves as MCP tools — **no Rust required**.

A business analyst authors the contract in standard Excel: name the inputs and each
result block as Excel **Tables** with columns `name | value | description | tier`.
The compiler derives the tool surface from those tables, so **each output table
becomes its own named MCP tool** (e.g. `calculate_tax`, `estimate_refund`), each
advertising a DAG-derived input schema (only the inputs that table's formulas
reference) and an emitted output schema. See the **Workbook Table Authoring**
chapter in the [pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/) for the full
authoring guide.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `explain` | Preview the MCP tool surface an AI will see — read-only, writes nothing |
| `lint`    | Lint a workbook against the dialect (cell-precise, fail-helpful) |
| `compile` | Compile to a governed, gated bundle (records an approver-bound acceptance) |
| `emit`    | Emit an UNGATED bundle for dev/reference (no approver required) |

All subcommands accept `--format <text|json>` (default `text`). Exit codes:
`0` success (or warnings-only lint), `1` error, `2` governance gate block (compile only).

---

## workbook explain

Preview the served tool surface **before** you compile or deploy. Runs the real
ingest → tool-surface projection against the `.xlsx` and prints each tool's name,
description, inputs, and outputs. Writes nothing — no bundle is produced. This is
the habit that prevents a silently-wrong deploy.

```
cargo pmcp workbook explain <WORKBOOK_PATH> [--format <text|json>]
```

### Options

| Option | Description |
|--------|-------------|
| `<WORKBOOK_PATH>` | Path to the `.xlsx` workbook (positional, required) |
| `--format <text\|json>` | `text` (default) prints a human-readable block per tool; `json` emits the tool-surface array for machines |

### Example

```
$ cargo pmcp workbook explain pricing.xlsx
tool calculate_tax
  description: Compute federal tax from income & filing
  inputs:
    income: number [USD]
    filing: string [enum: single|married]
  outputs:
    tax_owed: number
    effective_rate: number

tool estimate_refund
  description: Estimate refund given withholding
  inputs:
    income: number [USD]
    withheld: number [USD]
  outputs:
    refund: number
```

---

## workbook lint

Lint a workbook against the dialect, standalone — cell-precise, fail-helpful
diagnostics, no bundle written.

```
cargo pmcp workbook lint <WORKBOOK_PATH> [--format <text|json>]
```

---

## workbook compile

Compile a workbook into a **gated**, served bundle. The governance gate records an
approver-bound acceptance fingerprint, so the produced `bundle@version` is
provenance-stamped and tamper-evident.

```
cargo pmcp workbook compile <WORKBOOK_PATH> --workflow <WORKFLOW> --approver <NAME> [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--workflow <WORKFLOW>` | Workflow name (required for a bare workbook path) |
| `--approver <NAME>` | Approving identity recorded with the bundle (mandatory — no default) |
| `--accept` | Record a fingerprint-bound approval acceptance |
| `--effective-date <YYYY-MM-DD>` | Acceptance effective date (required when `--accept` is set) |
| `--out <PATH>` | Override the output bundle directory |
| `--format <text\|json>` | Output format (default `text`) |

---

## workbook emit

Emit an **UNGATED** bundle for dev/reference iteration — same compile pipeline, but
**no approver and no governance gate**. Use it for fast local iteration; use
`compile` for anything you serve in production.

```
cargo pmcp workbook emit <WORKBOOK_PATH> --workflow <WORKFLOW> [--out <PATH>] [--format <text|json>]
```

---

## See also

- [`pmcp-workbook-server`](../../../crates/pmcp-workbook-server/README.md) — the Shape-A binary that serves a compiled bundle
- `cargo pmcp new <name> --kind workbook-server` — scaffold a Shape-B crate over the same toolkit
- **Workbook Table Authoring** and **Config-Driven Workbook Servers** chapters in the [pmcp-book](https://paiml.github.io/rust-mcp-sdk/book/)
