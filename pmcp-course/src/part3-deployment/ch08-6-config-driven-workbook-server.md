# Config-Driven Workbook Servers: From Spreadsheet to Served MCP

Earlier you hand-built calculation MCP servers in Rust — wiring a `ServerBuilder`,
writing a tool handler for every formula, loading and versioning your model, and
recompiling on every change. That is the right approach when your logic is
bespoke. But a large class of calculation servers already live in a *governed
spreadsheet*: a pricing model, a tax calculator, a quoting workbook that a domain
team owns and audits in Excel. For those, PMCP offers a config-driven path where
the **workbook is the source of truth** — you compile it into a deterministic
bundle and serve that bundle, with no per-formula Rust and no recompile when the
model changes.

This is the calculation sibling of the
[Config-Driven SQL Servers](./ch08-5-config-driven-sql-server.md) and
[Config-Driven OpenAPI Servers](../openapi-built-in-server.md) chapters. If you
have done either, the *shape* will feel familiar — the difference is the input.
Instead of a `config.toml` you author Rust-free, the input here is a governed
Excel workbook that you **compile** into a served bundle.

This chapter walks the full lifecycle: **compile → serve → customize → deploy**.

## What You'll Learn

- When a workbook-driven server beats a hand-coded one (and when it doesn't)
- How `cargo pmcp workbook compile` turns a governed Excel file into a
  deterministic `bundle@version` directory
- The two ways to serve a bundle — the prebuilt `pmcp-workbook-server` binary
  (no Rust) and a `cargo pmcp new --kind workbook-server` scaffold (extendable)
- The five workbook tools and the `workbook://` render resource you get for free
- The governance properties — fail-closed boot integrity and the reader/JS
  purity gate — that make the served binary safe to ship

## Prerequisites

```bash
# The PMCP CLI (provides `cargo pmcp workbook compile` and the scaffold)
cargo install cargo-pmcp

# The prebuilt no-Rust binary (Shape A) — optional, only if you want to
# serve a bundle without scaffolding a crate:
cargo install pmcp-workbook-server
```

You also need a **governed Excel workbook** that conforms to the workbook
dialect. The contract for that dialect lives in the
[workbook dialect spec](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/workbook-dialect-spec.md);
for this chapter you can use the committed synthetic `tax-calc` workbook the
tooling ships as a worked example.

## The Single-Source-of-Truth Model

A SQL or OpenAPI built-in server is described in a `config.toml` you write by
hand. A workbook server is different: there is **no config.toml and no separate
schema**. The manifest, the formulas, the render pointers, and an integrity lock
all live *inside the compiled bundle*, and the compiled bundle is derived
entirely from the workbook.

```text
┌───────────────────────────────────────────────────────────────────────────┐
│                    WORKBOOK-DRIVEN MCP SERVER                              │
├───────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   governed Excel workbook            cargo pmcp workbook compile            │
│   ┌──────────────────────────┐       ┌──────────────────────────────────┐  │
│   │  inputs / outputs        │  ───► │ ingest → lint → synth → parse →   │  │
│   │  formulas                │       │ compile → reconcile → GATE → write│  │
│   │  render pointers         │       └──────────────────────────────────┘  │
│   │  (the source of truth)   │                      │                       │
│   └──────────────────────────┘                      ▼                       │
│                                       bundles/<name>@<version>/             │
│                                       (deterministic, BUNDLE.lock-sealed)   │
│                                                     │                       │
│                                                     ▼                       │
│                                   pmcp-workbook-server  (Shape A binary)    │
│                                   or  --kind workbook-server  (Shape B)     │
│                                   → 5 tools + workbook:// resource          │
└───────────────────────────────────────────────────────────────────────────┘
```

The Excel reader and the JS code-mode stack run **only at compile time**. They
are mechanically absent from the served binary — a property called the
[reader/JS purity gate](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/workbook-purity-gate.md).
The thing you serve is the bundle, not the spreadsheet.

## Step 1: Compile the Workbook

The compiler is a `cargo pmcp` subcommand. Point it at a workbook and name the
workflow (the bundle name); an approver is recorded in the manifest sign-off:

```bash
cargo pmcp workbook compile tax-calc.xlsx --workflow tax-calc --approver alice
```

This runs the full pipeline — ingest, lint, synth, parse, compile, reconcile,
the fail-closed governance gate, then write — and emits a deterministic bundle
directory:

```text
bundles/
└── tax-calc@1.1.0/        # <name>@<version>; version is read FROM the workbook
    ├── BUNDLE.lock        # integrity hashes re-verified at boot
    └── ...                # manifest, compiled formulas, render pointers
```

A few properties worth internalizing:

- The bundle is **deterministic and reproducible** — the same workbook compiles
  to the same bundle.
- The **version is implicit in the path** (`@1.1.0`) and comes from the
  workbook, not a flag.
- The compile is **fail-closed**: a gate block exits with a distinct non-zero
  code, so CI can tell a governance block apart from an ordinary compile error.

You can lint a workbook on its own first if you just want feedback:

```bash
cargo pmcp workbook lint tax-calc.xlsx
```

## Step 2: Serve the Bundle — Two Ways

The only input to serving is the compiled bundle directory. There is no
`config.toml`, no schema, nothing else to author.

### Shape A — the prebuilt binary (no Rust)

If you installed `pmcp-workbook-server`, point it at the bundle directory:

```bash
pmcp-workbook-server --bundle-dir bundles/tax-calc@1.1.0
# serves over streamable HTTP on 127.0.0.1:8080 by default
```

`--bundle-dir` is the only required flag. Two optional flags matter:

| Flag           | Required | Default            | Purpose |
| -------------- | -------- | ------------------ | ------- |
| `--bundle-dir` | Yes      | —                  | Path to the compiled `bundle@version` directory. One directory = one `bundle@version`; the version is implicit in the path. |
| `--bundle-id`  | No       | —                  | Fail-closed identity assertion: the loaded bundle's id must equal this value, or the binary exits non-zero **before** registering any tool. |
| `--http`       | No       | `127.0.0.1:8080`   | Streamable-HTTP bind address (`host:port`). The loopback default means the out-of-the-box binary does not expose a public listener. |

```bash
# Assert identity (fail-closed) and bind a public address:
pmcp-workbook-server --bundle-dir bundles/tax-calc@1.1.0 \
  --bundle-id tax-calc \
  --http 0.0.0.0:9000
```

### Shape B — scaffold an extendable crate

If you want a crate you can extend — add your own transports, middleware, or
additional handlers around the workbook surface — scaffold one over the
`pmcp-server-toolkit` library:

```bash
cargo pmcp new tax-server --kind workbook-server
cd tax-server
cargo run
# prints: PMCP_WORKBOOK_SERVER_ADDR=http://… — connect your MCP client there
```

The scaffold is an ordinary Rust crate with its own `main.rs` that uses the
toolkit. It embeds a workbook (`workbook/tax-calc.xlsx`); when you edit that
workbook, recompile the embedded bundle and rerun:

```bash
cargo pmcp workbook compile
cargo run
```

Shape A and Shape B are siblings built on the same toolkit. The scaffold does
**not** invoke the prebuilt binary — it links the same library primitives the
binary uses.

## Step 3: What You Get — Five Tools and a Render Resource

Either way you serve it, a compiled bundle is exposed as five MCP tools plus one
resource — no per-tool Rust:

| Tool              | Purpose |
| ----------------- | ------- |
| `calculate`       | Run the workbook's calculation against supplied inputs |
| `explain`         | Explain how a result was derived |
| `get_manifest`    | Return the bundle manifest (identity, inputs, outputs) |
| `diff_version`    | Compare results or definitions across versions |
| `render_workbook` | Produce a render of the workbook |

The server also exposes a **`workbook://` resource** — a versioned render-pointer
URI. Its exact shape is defined in the
[workbook URI spec](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/workbook-uri-spec.md).

Connect your MCP client (or `cargo pmcp test check <addr>`) to the served
address and call `calculate` with the workbook's declared inputs; call
`get_manifest` first if you want to discover what those inputs are.

## Step 4: Customize Through the Workbook

This is the key mental shift from the SQL and OpenAPI servers. There, you
customize by editing `config.toml`. Here, **you customize by editing the
workbook** — the spreadsheet *is* the configuration.

To change a formula, add an input, or correct an output, you edit the Excel file,
then recompile:

```bash
cargo pmcp workbook compile tax-calc.xlsx --workflow tax-calc --approver alice
```

A meaningful change bumps the workbook version, so the compile writes a new
`bundles/tax-calc@<new-version>/` directory alongside the old one — prior
versions are not overwritten. Point the server at the new directory (or ship it)
to go live. Because the version is part of the bundle path, `diff_version` can
compare results across the versions you keep.

This keeps the audit story clean: domain owners who already maintain the workbook
own the server's behavior, and every served version traces back to a specific,
gated compile of a specific workbook.

## Step 5: Govern and Boot Safely

Two governance properties are worth teaching explicitly, because they are what
make it safe to hand a compiled bundle to a server you did not hand-write.

**Fail-closed boot integrity.** Before any tool is registered, the toolkit
re-verifies the bundle's `BUNDLE.lock` hashes. A tampered, incomplete, or missing
bundle fails boot with a non-zero exit — the server never comes up serving partial
or wrong tools. When you pass `--bundle-id`, the binary additionally asserts the
loaded bundle's identity *before* registering anything, and exits non-zero on a
mismatch.

**Reader/JS purity.** The Excel reader and the JS code-mode stack are
**compile-time only**. They are mechanically absent from the served binary, which
serves the fixed five-tool surface from the bundle alone. There is no spreadsheet
parser and no script executor in the served cone. The mechanism is described in
the
[reader/JS purity gate](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/workbook-purity-gate.md).

Together these mean: the only thing that can change behavior is a new, gated,
hash-sealed bundle — not a runtime input, not a smuggled script.

## Step 6: Deploy

Because the Shape B scaffold is a normal crate over the `pmcp-server-toolkit`
library, the **same deploy path** you used for the SQL and OpenAPI built-in
servers applies. Edit `[target] type` in the scaffold's `deploy.toml` (mirror the
edit into `.pmcp/deploy.toml`), then:

```bash
cargo pmcp validate deploy        # pre-flight checks, no cloud calls yet
cargo pmcp deploy
cargo pmcp deploy outputs         # deployed endpoint URL
```

The deploy bundles your assets — here, the compiled `bundle@version` directory —
and ships them with the crate, the same way the SQL server bundles its
`config.toml`/`schema.sql`. For the per-target mechanics, lean on the existing
chapters rather than re-learning them here:

- [Deploying to AWS Lambda](./ch08-aws-lambda.md)
- [Deploying to Cloudflare Workers](./ch09-cloudflare.md)
- The [SQL server deploy section](./ch08-5-config-driven-sql-server.md#step-4-deploy-to-aws-lambda)
  for the `validate deploy` → `deploy` → `outputs` flow

<!-- VERIFY: which specific deploy targets (aws-lambda / cloud-run / cloudflare) are wired and tested for the --kind workbook-server scaffold, vs. inheriting the generic toolkit deploy path -->

Workbooks add no deploy mechanics of their own beyond shipping the bundle as an
asset, so there is nothing workbook-specific to configure on the target.

## When to Use This (and When Not To)

| Use workbook-driven | Hand-code instead (Chapter 3) |
|---|---|
| The model already lives in a governed spreadsheet | Logic is bespoke Rust, not a calc model |
| Domain owners maintain it in Excel | Engineers own every formula in code |
| You need deterministic, versioned, gated bundles | You need custom transports / middleware |
| Pure calculation (`calculate` / `explain` / `diff`) | The backend isn't a calculation workbook |

For bespoke calculation logic, or anything that isn't a governed workbook, the
hand-coded approach from Chapter 3 remains the right tool.

## Exercise: Compile, Serve, and Version a Workbook

**Goal:** take a governed workbook through the full compile → serve → version
lifecycle.

1. Compile the `tax-calc` workbook with
   `cargo pmcp workbook compile tax-calc.xlsx --workflow tax-calc --approver <you>`
   and confirm a `bundles/tax-calc@<version>/` directory is written.
2. Serve it with `pmcp-workbook-server --bundle-dir bundles/tax-calc@<version>`
   and call `get_manifest`, then `calculate`, from your MCP client.
3. Pass a deliberately wrong `--bundle-id` and confirm the binary exits non-zero
   **before** any tool registers (fail-closed identity).
4. Edit the workbook to change a formula, bump its version, and recompile.
   Confirm a *new* `bundle@<new-version>/` directory appears beside the old one,
   and that `diff_version` can compare the two.
5. **Stretch:** scaffold the same workbook as a crate with
   `cargo pmcp new tax-server --kind workbook-server`, run it locally, then
   edit `deploy.toml` and run `cargo pmcp validate deploy`.

**Success criteria:** the compile is deterministic and gated; the served binary
exposes the five tools and the `workbook://` resource; a mismatched `--bundle-id`
fails closed; and a workbook edit produces a new, comparable bundle version.

## Key Takeaways

- Workbook-driven servers make the **governed Excel workbook the source of
  truth**. You `cargo pmcp workbook compile` it into a deterministic,
  hash-sealed `bundle@version` directory and serve *that* — no per-formula Rust,
  no recompile of the server when the model changes.
- There are two ways to serve: the prebuilt `pmcp-workbook-server` binary
  (`--bundle-dir`, no Rust) and a `cargo pmcp new --kind workbook-server`
  scaffold (an extendable crate over `pmcp-server-toolkit`).
- Every bundle is served as five tools — `calculate`, `explain`, `get_manifest`,
  `diff_version`, `render_workbook` — plus a `workbook://` render resource.
- Governance is built in: the boot gate re-verifies `BUNDLE.lock` before serving,
  `--bundle-id` is an optional fail-closed identity assertion, and the Excel
  reader and JS code-mode are compile-time only — absent from the served binary.
- You customize by editing the workbook and recompiling, not by editing config;
  and you deploy through the same `cargo pmcp deploy` path as the SQL and OpenAPI
  built-in servers.

For the conceptual companion to this lab, see the narrative
[Spreadsheet to MCP Server](https://github.com/paiml/rust-mcp-sdk/blob/main/docs/articles/spreadsheet-to-mcp-server.md)
article and the crate
[README](https://github.com/paiml/rust-mcp-sdk/blob/main/crates/pmcp-workbook-server/README.md).
