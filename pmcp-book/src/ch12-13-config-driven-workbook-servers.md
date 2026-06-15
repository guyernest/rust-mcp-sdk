# Chapter 12.13: Config-Driven Workbook Servers (cargo pmcp)

The previous two chapters showed how a complete MCP server can be *described*
rather than hand-written: a SQL backend through a `config.toml` plus a schema
([Chapter 12.10](ch12-10-config-driven-sql-servers.md)), and an HTTP/REST
backend through a `config.toml` plus an OpenAPI spec
([Chapter 12.11](openapi-built-in-server.md)). This chapter is the third member
of that family, and the source of truth is something most teams already have: a
**governed Excel workbook**. You author the business logic as formulas in a
spreadsheet; the `cargo pmcp` CLI compiles it into a deterministic, verifiable
`bundle@version`; and a prebuilt binary serves that bundle as a live MCP server
exposing five workbook tools — with no user Rust at all.

After this chapter you should be able to lint a workbook against the governed
dialect, compile it into a bundle, serve that bundle over MCP, and understand
why the served server is provably free of the spreadsheet-reading and
code-execution machinery that built it.

## The Problem (Why Config, Not Code)

For the SQL and OpenAPI servers the unit of curation is a *query* or an
*endpoint*. For a great many real systems — pricing, tax, quoting, eligibility,
commission — the unit of business logic is a **spreadsheet**, and it is owned by
an analyst, not an engineer. To expose that logic over MCP the conventional way,
someone has to *transcribe* the formulas into Rust (or Python, or TypeScript),
keep the transcription in sync with the spreadsheet as the rules change, and
recompile on every edit. The transcription is where bugs and drift live: the
served system slowly stops agreeing with the workbook everyone still treats as
the source of truth.

Config-driven workbook servers remove the transcription step. The spreadsheet
*is* the program. The compiler reads the workbook, checks it against a
constrained, governed dialect, compiles the formulas, **reconciles its own
results against Excel's evaluation of the same cells**, and — only if a
fail-closed gate passes — writes a bundle. The analyst owns the logic; no one
hand-codes it.

```text
   workbook.xlsx ─► cargo pmcp workbook compile ─► bundle@version/ ─► pmcp-workbook-server ─► 5 MCP tools
   (formulas,        ingest → lint → synth →        (deterministic,      (no Rust;            calculate / explain /
    the source       compile → reconcile →           verifiable,          point at the dir)    get_manifest /
    of truth)        GATE → write                    integrity-locked)                          diff_version /
                                                                                                 render_workbook
```

The compile pipeline is fail-closed at every stage. The dialect linter rejects
constructs outside the governed subset; reconciliation rejects a compile whose
results disagree with Excel; the governance gate (on an update to an existing
bundle) blocks a write whose outputs drift past policy — **before** anything is
written to disk. The result is an artifact you can trust: recompiling the same
workbook produces a verifiable bundle, and the served binary refuses to boot on
a bundle whose integrity hashes do not match.

## Two Shapes

PMCP ships two ways to serve a compiled bundle, both built on the same
`pmcp-server-toolkit` workbook surface:

| | **Shape A — the binary** | **Shape B — the scaffold** |
|---|---|---|
| What | The prebuilt `pmcp-workbook-server` binary | A crate from `cargo pmcp new --kind workbook-server` |
| Run | `pmcp-workbook-server --bundle-dir bundles/<name>@<ver>` | `cargo run` inside the crate |
| Rust source? | None | A small generated `src/main.rs` you own |
| Best for | Zero-build point-and-serve over any compiled bundle | Shipping a workbook *with* a server crate; extending |

Unlike the SQL and OpenAPI chapters — which lead with the scaffold because it is
the deployment on-ramp — this chapter leads with **Shape A**, because the
workbook story has a clean separation: *compile* (the `cargo pmcp workbook`
verbs) and *serve* (`pmcp-workbook-server`). The binary takes exactly one
required input — a compiled bundle directory — and nothing else. Shape B, which
embeds a workbook and recompiles it on demand, is covered at the end and in the
`pmcp-workbook-server` crate README.

## Step 1: Lint the Workbook

Before you compile, check the workbook against the governed dialect. The linter
ingests the `.xlsx`, runs every dialect rule, and reports findings:

```bash
cargo pmcp workbook lint pricing.xlsx
```

Only `Error`-severity findings block — a warnings-only report still prints its
findings but exits zero. Add `--format json` for a machine-readable report. The
governed dialect (which formulas, functions, and cell shapes are permitted, and
why the subset is constrained) is specified in the repository's
[`docs/workbook-dialect-spec.md`][dialect].

## Step 2: Compile to a Bundle

```bash
cargo pmcp workbook compile pricing.xlsx --workflow quote --approver alice
```

This runs the full pipeline — **ingest → lint → synthesize the manifest →
compile the formulas → reconcile against Excel → gate → write** — and emits a
`quote@<version>/` bundle directory. A few properties are worth calling out, all
verifiable in the repository:

- **The version comes from the workbook, not the CLI.** There is no `--version`
  flag; the bundle's version is read out of the workbook itself, so the bundle
  directory name (`<workflow>@<version>`) is a fact about the source, not a
  command-line choice.
- **`--approver` is mandatory.** Every bundle records a human approver in its
  manifest sign-off — there is no implicit git-identity fallback.
- **The gate runs before any write.** On the *first* version of a workflow there
  is no prior baseline to regress against, so the bundle is written directly. On
  an *update* to an existing workflow, the compiler builds the new bundle in
  memory, grades it against the prior accepted baseline, and on a policy block
  prints the gate decision and exits with a **distinct** gate-block exit code —
  writing nothing. A re-baseline you have explicitly reviewed is recorded with
  `--accept` (which requires `--effective-date <YYYY-MM-DD>`).

If you keep your workbook→bundle mappings in `pmcp.toml`, a bare bundle-id
(`cargo pmcp workbook compile quote`) resolves the path, output directory, and
workflow from config, and running `cargo pmcp workbook compile` with no argument
compiles every declared workbook (continue-on-error, worst-status-wins).

A sibling verb, `cargo pmcp workbook emit`, writes an **ungated** bundle for
local development and reference. It never invokes the governance gate, so it is
strictly a dev convenience — production bundles come from `compile`.

The emitted bundle is a self-describing, integrity-locked directory: a manifest,
the compiled program, the recorded version changelog, and a `BUNDLE.lock` that
hashes every member. That lock is what makes the served binary's boot gate
possible.

## Step 3: Serve the Bundle

Point the prebuilt binary at the compiled directory:

```bash
pmcp-workbook-server --bundle-dir bundles/quote@1.1.0
# serves over streamable HTTP on 127.0.0.1:8080 by default
```

The only required input is `--bundle-dir` — there is no `config.toml`, no
schema, no spec. Two optional flags refine the run:

- `--bundle-id <id>` is a **fail-closed identity assertion**. If the loaded
  bundle's recorded id does not match the value you pass, the binary exits
  non-zero **before registering any tool** — a guard against accidentally
  serving the wrong bundle from a directory that happens to exist.
- `--http <host:port>` selects the bind address. It defaults to
  `127.0.0.1:8080` — **loopback**, so the out-of-the-box binary never exposes a
  public listener until you opt in.

Point an MCP client at the address and you will see five tools plus a
`workbook://` render-pointer resource:

| Tool | What it does |
|---|---|
| `calculate` | Run the compiled workbook for a set of inputs and return the results. |
| `explain` | Return an ordered, cell-by-cell trace of how a result was computed. |
| `get_manifest` | A curated, agent-facing projection of the bundle's manifest (inputs, outputs, metadata). |
| `diff_version` | Serve the recorded previous→current delta for this workflow. |
| `render_workbook` | Return a provenance-bound `workbook://` URI pointing at the rendered output — *not* the raw `.xlsx` bytes. |

The `workbook://` pointer contract (what the URI encodes and how a client
resolves it) is specified in [`docs/workbook-uri-spec.md`][uri].

## Fail-Closed Boot Integrity

Before the server registers a single tool, the toolkit's boot gate **recomputes
the bundle's `BUNDLE.lock` hashes and re-verifies them**. A bundle that has been
tampered with, partially copied, or is missing a member fails to load — the
server returns an error and never boots on an unverified bundle. The
`--bundle-id` check (when supplied) layers on top of this: the integrity load is
the real security boundary, and the id assertion is an operator-convenience
guard that runs against the same fail-closed load.

This is the property that lets you treat a bundle as a trustworthy unit: it is
not "a directory of files the server hopes are correct," it is an artifact whose
integrity is verified at every boot.

## The Served Server Is Reader-Free

The single most important governance property of this design is what the served
binary does **not** contain. Reading an `.xlsx`, parsing formulas, and running
the JavaScript code-mode machinery used during compilation are **compile-time
only** — none of that stack is linked into `pmcp-workbook-server`. The binary
depends solely on the toolkit's workbook *boot* surface (load a bundle, verify
it, register the five tools), and a mechanical **purity gate** enforces that the
heavyweight reader and code-execution cones never reach the served binary.

In practice this means the deployed attack surface is small and bounded: the
server evaluates a *pre-compiled, verified* program against inputs, and that is
all it can do. It cannot be coerced into reading an arbitrary spreadsheet or
executing arbitrary code, because the code paths that could do so are not present
in the shipped artifact. The purity gate and what it excludes are documented in
the repository's [`docs/workbook-purity-gate.md`][purity].

## Shape B: Scaffold a Workbook Server Crate

When you want to *ship a workbook together with a server crate* — versioned in
your own repository, extendable, and deployable through the same
`cargo pmcp deploy` lifecycle as the SQL and OpenAPI scaffolds — generate the
Shape B crate:

```bash
cargo pmcp new my-workbook-server --kind workbook-server
cd my-workbook-server
cargo run
# prints PMCP_WORKBOOK_SERVER_ADDR=http://… — connect your MCP client (5 tools)
```

The scaffold emits a single runnable crate with a deliberately small,
generated `src/main.rs` (drift-locked to the toolkit's
`workbook_server_http.rs` example) and an embedded demo workbook. When you edit
the workbook, recompile its bundle in place and rerun:

```bash
cargo pmcp workbook compile      # recompile the embedded workbook's bundle
cargo run                        # serve the freshly compiled bundle
```

Pick **Shape A** when you already have a compiled bundle and just want to serve
it with zero build; pick **Shape B** when the workbook lives in your project and
you want a crate you own, can extend, and can deploy. Both serve the identical
five tools over the identical boot gate.

## What You Built

You now have a workbook MCP server that:

- treats a **governed Excel workbook as the single source of truth** — the logic
  is authored by analysts as formulas, not transcribed into Rust,
- compiles that workbook into a **deterministic, integrity-locked
  `bundle@version`** through a fail-closed pipeline that reconciles against Excel
  and gates updates before any write,
- serves the bundle through five MCP tools with **no user Rust** (Shape A) or a
  small crate you own (Shape B),
- **re-verifies the bundle's integrity at every boot**, refusing to serve a
  tampered or incomplete bundle, and
- ships a **reader-free, code-execution-free served cone** — mechanically gated —
  so the deployed attack surface is bounded to evaluating a pre-compiled,
  verified program.

For the standalone no-Rust binary form, see the `pmcp-workbook-server` crate
README; for the SQL and OpenAPI siblings in this config-driven family, see
[Chapter 12.10](ch12-10-config-driven-sql-servers.md) and
[Chapter 12.11](openapi-built-in-server.md); and for the end-to-end narrative of
turning a spreadsheet into a secure MCP server, see the repository's
[`docs/articles/spreadsheet-to-mcp-server.md`][article].

[dialect]: https://github.com/paiml/rust-mcp-sdk/blob/main/docs/workbook-dialect-spec.md
[uri]: https://github.com/paiml/rust-mcp-sdk/blob/main/docs/workbook-uri-spec.md
[purity]: https://github.com/paiml/rust-mcp-sdk/blob/main/docs/workbook-purity-gate.md
[article]: https://github.com/paiml/rust-mcp-sdk/blob/main/docs/articles/spreadsheet-to-mcp-server.md
