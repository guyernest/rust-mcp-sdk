# Workbook Purity Gate (Phase 91, WBRT-04)

The **purity gate** is the mechanically-provable control that guarantees the Excel
**reader** stack (`umya-spreadsheet` / `quick-xml` / `calamine`) and the JavaScript /
code-mode stack (`swc_*` / `pmcp-code-mode`) can **never** enter the *reader-free*
served crate trees:

- `pmcp-workbook-runtime` — the served runtime (writer-only `rust_xlsxwriter`)
- `pmcp-workbook-dialect` — the dialect contract (depends only on the runtime)

It is established **before any `umya` code exists** (the compiler lands in Phase 93),
so the boundary is defended from day one. This is THE phase-91 risk control
(Pitfall 1): Cargo feature unification could otherwise silently pull a reader into
the served binary.

Run it with either entrypoint (both are required by D-09 / ROADMAP Success-Criterion 3):

```bash
make purity-check     # primary, fail-closed implementation
just purity-check     # thin recipe that delegates to `make purity-check`
```

It is also part of `make quality-gate` (the local pre-commit single-source-of-truth)
and runs as a **merge-blocking** CI job wired into the org-required `gate` job.

## Three Layers

| Layer | Mechanism | What it proves |
|-------|-----------|----------------|
| **Layer 1** | `cargo tree` per-crate × per-feature: negative (reader/JS absent) + positive (`rust_xlsxwriter` present) | No reader/JS dependency resolves into either served tree under **any** feature combination, and the writer/renderer is actually wired (non-vacuous) |
| **Layer 2** | Crate-local `cargo-deny [bans]` (`crates/<crate>/deny.toml`), invoked with `--manifest-path` | A declarative ban backstop scoped to that crate's tree only — the workspace-global `deny.toml` is never touched, and Phase 93's compiler (a separate, non-dependent member) is unaffected |
| **Layer 3** | The crate split itself (delivered by plans 91-01 / 91-02) | The reader lives in a *different* crate (the future Phase 93 compiler), structurally separated from the served trees |

## Fail-Closed Design (Layer 1)

The Makefile recipe begins `set -euo pipefail` and captures **every** `cargo tree`
exit status explicitly:

```sh
tree=$(cargo tree -p $crate $feat 2>&1); status=$?
if [ $status -ne 0 ]; then echo "...failing closed"; exit 1; fi
```

A `cargo tree` invocation that fails for **any** reason — a broken `-p`, a transient
registry error, a malformed feature flag — aborts the gate as a **FAILURE**. It is
**never** read as "no banned dependency." There is deliberately **no
`cargo tree … | grep … 2>/dev/null`** swallow (the false-pass bug the cross-AI
review flagged): a failed tree piped into `grep` would exit 0 and vacuously "pass."

### Fail-closed proof

Pointing the tree command at a nonexistent package makes `cargo tree` exit
non-zero, and the recipe exits non-zero in turn:

```console
$ cargo tree -p pmcp-workbook-NONEXISTENT; echo "exit=$?"
error: package ID specification `pmcp-workbook-NONEXISTENT` did not match any packages
exit=101
```

Because the recipe captures that non-zero status and `exit 1`s on it, a broken
invocation can never be misread as a clean tree.

## Positive Writer Assertion (non-vacuous)

The positive arm asserts `rust_xlsxwriter` **is** present, scoped to
`cargo tree -p pmcp-workbook-runtime` (so another workspace member pulling the
writer cannot produce a false positive) **and** across the
`"" / --no-default-features / --all-features` matrix (so a feature combo that drops
the renderer is caught). A deleted renderer therefore cannot make the gate
vacuously pass.

## Ban Token Set

```
umya | calamine | quick-xml | swc_ | pmcp-code-mode
```

- `pmcp` is **NOT** banned (D-09 — the SDK runtime may legitimately depend on `pmcp`).
- `zip` is **NOT** banned (it enters legitimately via the writer-only
  `rust_xlsxwriter`; a *writer* is not a *reader*).

## Layer 2 Scoping (why it does not break Phase 93)

Each crate ships a minimal `[bans]`-only `deny.toml`. The gate invokes cargo-deny
scoped to that crate's manifest:

```bash
cargo deny --manifest-path crates/pmcp-workbook-runtime/Cargo.toml check --config deny.toml bans
cargo deny --manifest-path crates/pmcp-workbook-dialect/Cargo.toml  check --config deny.toml bans
```

Per cargo-deny 0.18.3 `--manifest-path` semantics, specifying a workspace member's
manifest makes **that crate the sole root** of the crate graph: only workspace
members that are dependencies of that crate are evaluated. Phase 93's
`pmcp-workbook-compiler` is a separate member and **not** a dependency of the
runtime or dialect, so its legitimate `umya` / `quick-xml` use is never evaluated
by these configs. The infra-managed **workspace-global `deny.toml` is never
edited**.

> CLI ordering note: cargo-deny 0.18.3 accepts `--config` only *after* the `check`
> subcommand and resolves it relative to the manifest directory, hence the
> `check --config deny.toml bans` ordering in the executed command (the canonical
> documented form is `--manifest-path … --config … check bans`).

The ban enforcement is **non-vacuous**: substituting a present crate
(e.g. `rust_xlsxwriter`) into a crate's ban list makes `cargo deny … check bans`
exit non-zero ("bans FAILED"), proving the ban list is actually evaluated.

## Merge-Blocking Wiring

A purity-gate CI job is advisory unless it is wired into the aggregation `gate`
job that the org ruleset requires. Enforcement requires **three** edits to
`.github/workflows/ci.yml`'s `gate` job:

1. `needs: [test, quality-gate, purity-check]`
2. `PURITY_RESULT: ${{ needs.purity-check.result }}` in `env:`
3. `|| [[ "$PURITY_RESULT" != "success" ]]` in the result-evaluation `if`

Without all three, the job runs but does not block merge.
