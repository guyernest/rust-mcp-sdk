# pmcp-workbook-server

Shape A pure-config workbook MCP server binary — point it at a compiled `bundle@version` directory and serve one named MCP tool per output table (plus infrastructure tools) with **no Rust required**.

**Status:** 0.1.0 — early access. The boot pipeline is fully implemented; the public CLI surface may still evolve.

> **Most people don't run this binary directly.** The recommended way to build and
> ship a workbook MCP server is the `cargo pmcp` CLI: you compile a governed Excel
> workbook into a bundle with `cargo pmcp workbook compile`, then either run the
> prebuilt binary against that bundle or scaffold a small extendable crate with
> `cargo pmcp new my-server --kind workbook-server`. See the user guide in
> [`cargo-pmcp/README.md`](../../cargo-pmcp/README.md).
>
> **This README is for the other path:** running the prebuilt `pmcp-workbook-server`
> binary as-is against an already-compiled bundle. The CLI scaffold ("Shape B")
> generates a small crate that uses the
> [`pmcp-server-toolkit`](../pmcp-server-toolkit) *library* with its own `main.rs`
> (extendable); this crate ("Shape A") is the standalone, no-Rust-required *binary*.
> They are siblings built on the same toolkit — the scaffold does not invoke this
> binary.

## The improvement (why this exists)

To expose a calculation workbook over the Model Context Protocol today, you hand-write a Rust binary against the SDK: wire a `ServerBuilder`, implement a tool handler for every calculation, load and version your formula model, stand up the HTTP transport, and **recompile for every formula or version change**.

`pmcp-workbook-server` collapses that into a **single input** and one binary:

- A **compiled `bundle@version` directory** (e.g. `bundles/tax-calc@1.1.0`) produced by the workbook compiler.

That bundle directory is the only thing the binary needs. Unlike its SQL sibling [`pmcp-sql-server`](../pmcp-sql-server), there is **no `config.toml` and no separate schema file** — the manifest, formulas, render pointers, and integrity lock all live inside the compiled bundle. You run one binary, ship a new bundle when the workbook changes, and never recompile the server.

It is the runnable binary built on top of the [`pmcp-server-toolkit`](../pmcp-server-toolkit) library — everything the binary does is select the bundle directory, run a fail-closed integrity gate, and register the workbook tool surface through toolkit primitives.

## What this crate is NOT

- **Not the library.** The reusable building blocks (the bundle loader, the integrity gate, the workbook tool surface) live in [`pmcp-server-toolkit`](../pmcp-server-toolkit). This crate is the runnable binary on top of it.
- **Not the compiler.** It does not turn an Excel file into a bundle — it only *serves* an already-compiled bundle. Use `cargo pmcp workbook compile` to produce the bundle (see [Where bundles come from](#where-bundles-come-from)).
- **Not a SQL / database toolkit.** It serves pre-compiled calculation bundles, not live database queries.
- **Not a code-mode server.** The served binary contains no SWC/JS code-mode stack; it serves the per-table workbook surface (one tool per output table plus the infrastructure tools below) from the bundle alone.

## The served surface

Each output Excel **Table** in the workbook becomes its own MCP tool (the multi-tool fan-out, WBV2-04), named after the table and carrying a DAG-derived input schema — only the inputs that table's formulas actually reference. Alongside those, four fixed infrastructure tools and one resource are always served:

**Calculation tools — one per output table (dynamic):**

| Tool (example)    | Purpose                                                          |
| ----------------- | --------------------------------------------------------------- |
| `calculate_tax`   | Run one output table's calculation against supplied inputs       |
| `estimate_refund` | …another output table → another named tool, with its own inputs  |

The exact set and names come from the workbook's output tables (sanitized to the MCP tool-name charset). Preview them before deploy with `cargo pmcp workbook explain <wb.xlsx>`.

**Infrastructure tools — fixed, workbook-wide:**

| Tool              | Purpose                                                          |
| ----------------- | --------------------------------------------------------------- |
| `explain`         | Explain how a result was derived                                |
| `get_manifest`    | Return the bundle manifest (identity, inputs, outputs)          |
| `diff_version`    | Compare results or definitions across versions                  |
| `render_workbook` | Produce a render of the workbook                                |

The server also exposes a **`workbook://` resource** — a versioned render-pointer URI. See the [`workbook://` render-URI contract](../../docs/workbook-uri-spec.md) for its exact shape.

### Fail-closed boot integrity

Before any tool is registered, the toolkit re-verifies the bundle's `BUNDLE.lock` hashes. A tampered, incomplete, or missing bundle fails the boot with a non-zero exit — the server never comes up serving partial or wrong tools. If you pass `--bundle-id`, the binary additionally asserts the loaded bundle's identity *before* registering anything and exits non-zero on a mismatch.

The reader/JS purity of the served binary (no SWC/JS code-mode, no spreadsheet reader in the served cone) is mechanically enforced; see the [reader/JS purity gate](../../docs/workbook-purity-gate.md).

## Quickstart

> **Fastest path — the shipped smoke example.** This crate ships a runnable example
> that builds a server from a committed synthetic golden bundle (`tax-calc@1.1.0`)
> resolved relative to the crate, and prints the server identity. It uses zero
> customer data:
>
> ```bash
> cargo run -p pmcp-workbook-server --example workbook_server_min
> ```
>
> A successful run proves the fail-closed boot gate verified the bundle and all
> of the workbook's tools registered (one per output table, plus the infrastructure
> tools). The walkthrough below builds the same shape against a bundle of your own.

### 1. Build / install

```bash
# Build the binary.
cargo build -p pmcp-workbook-server --release

# Or install it on your PATH.
cargo install --path crates/pmcp-workbook-server
```

### 2. Run it

The only required input is `--bundle-dir`, pointing at a compiled `bundle@version` directory. The version is implicit in the directory name:

```bash
# Minimal: serve a compiled bundle over streamable HTTP.
pmcp-workbook-server --bundle-dir bundles/tax-calc@1.1.0

# Assert the bundle's identity (fail-closed: exits non-zero on a mismatch,
# before any tool is registered).
pmcp-workbook-server --bundle-dir bundles/tax-calc@1.1.0 --bundle-id tax-calc

# Override the bind address (default 127.0.0.1:8080).
pmcp-workbook-server --bundle-dir bundles/tax-calc@1.1.0 --http 0.0.0.0:9000

# Control log verbosity via RUST_LOG (the binary inits a tracing EnvFilter).
RUST_LOG=info pmcp-workbook-server --bundle-dir bundles/tax-calc@1.1.0
```

The server is served over the streamable-HTTP transport via the SDK's Tower/axum adapter. By default it binds loopback (`127.0.0.1:8080`) and restricts origins to localhost, so the out-of-the-box binary does not expose a public listener.

| Flag           | Required | Default          | Description                                                                 |
| -------------- | -------- | ---------------- | --------------------------------------------------------------------------- |
| `--bundle-dir` | Yes      | —                | Path to a compiled `bundle@version` directory (version implicit in the path) |
| `--bundle-id`  | No       | —                | Fail-closed identity assertion; exits non-zero on mismatch before serving   |
| `--http`       | No       | `127.0.0.1:8080` | Streamable-HTTP bind address (`host:port`)                                  |

**No environment-variable overrides.** Every setting is a CLI argument — there are no `env(...)` overrides on any flag. This is a deliberate difference from [`pmcp-sql-server`](../pmcp-sql-server), which accepts env interpolation in its config.

### Where bundles come from

A `bundle@version` directory is produced by compiling a governed Excel workbook with the `cargo pmcp` CLI:

```bash
cargo pmcp workbook compile pricing.xlsx --workflow quote --approver alice
```

The compile lane runs ingest → lint → synth → parse → compile → reconcile → gate → write, with the governance gate running before anything is written. The Excel dialect the compiler accepts is defined in the [governed Excel dialect contract](../../docs/workbook-dialect-spec.md). See [`cargo-pmcp/README.md`](../../cargo-pmcp/README.md) for the full CLI workflow, including `cargo pmcp new --kind workbook-server` (Shape B).

## Design context

This binary depends only on the [`pmcp-server-toolkit`](../pmcp-server-toolkit) library, built with its `workbook` and `http` features (default features off, to keep the code-mode stack out of the served binary). The single novel seam in this crate is turning `--bundle-dir` (plus the optional `--bundle-id` assertion) into a verified bundle source and registering the workbook tool surface through the toolkit — everything else mirrors the structure of [`pmcp-sql-server`](../pmcp-sql-server).
