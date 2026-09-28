# Phase 128 — deferred items (out of scope discoveries)

Logged per the executor's SCOPE BOUNDARY rule: these were surfaced while running this
phase's gates but are NOT caused by this phase's changes. They are recorded rather than
fixed.

## From plan 05 (2026-09-27)

### D1 — five pre-existing rustdoc `broken_intra_doc_links` warnings in `pmcp-code-mode`

`RUSTFLAGS="" cargo doc -p pmcp-code-mode --no-deps --features js-runtime` emits 5 warnings,
exit 0:

| Warning | Site |
|---|---|
| unresolved link to `HttpExecutor` | `crates/pmcp-code-mode/src/code_executor.rs:114` |
| unresolved link to `ExecutionConfig` | `crates/pmcp-code-mode/src/code_executor.rs:118` |
| unresolved link to `SdkExecutor` | `crates/pmcp-code-mode/src/code_executor.rs:165` |
| unresolved link to `validate_javascript_code_async` | `crates/pmcp-code-mode/src/validation.rs` |
| unresolved link to `validate_graphql_query_async` | `crates/pmcp-code-mode/src/validation.rs` |

**Proven pre-existing**, not introduced by plan 05:
`git show e7aa6136:crates/pmcp-code-mode/src/code_executor.rs | grep -n 'Adapter bridging'`
returns the identical three lines at the identical line numbers (114, 118, 165), and
`git diff --name-only e7aa6136 -- crates/pmcp-code-mode/src/validation.rs` is empty.

**Why no gate sees them:** `make doc-check` scopes to the root `pmcp` package only (the
CLAUDE.md-recorded blind spot), and `cargo doc -p pmcp-code-mode --no-deps` *without*
`--features js-runtime` emits **0** warnings — the items those links name live behind that
feature. A crate-scoped, feature-on `cargo doc` is the instrument that finds them.

### D2 — seven clippy findings in `crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs`

Surfaced by `cargo clippy -p pmcp-server-toolkit --features openapi-code-mode --all-targets`
(lines 8, 10, 11, 13, 14, 16, 17). That file belongs to plan 03 and plan 05 did not touch it.
Not CI-gated: per the in-repo memory note, `make lint` lints only the root `pmcp` package with
a generous allow-list, so a bare `-D warnings` run on the toolkit is STRICTER than CI.
`make quality-gate` is green.

### D3 — two clippy `too_many_arguments` errors in `pmcp-workbook-runtime`

`cargo clippy -p pmcp-server-toolkit --features openapi-code-mode --all-targets -- -D warnings`
exits 101 on `pmcp-workbook-runtime` (a transitive dependency), "this function has too many
arguments (8/7)", twice. `git diff --name-only e7aa6136 | grep -c workbook` is **0** — plan 05
never touched that crate. Same non-CI-gated class as D2.

## From plan 06 (2026-09-27)

### D4 — one `unused_imports` warning in `crates/pmcp-server-toolkit/src/workbook/render_resource.rs`

`RUSTFLAGS="" cargo build --workspace` emits
`warning: unused import: pmcp_workbook_runtime::RenderMode` at
`crates/pmcp-server-toolkit/src/workbook/render_resource.rs:43`, exit 0.

**Proven pre-existing**, not introduced by plan 06:
`git diff --name-only 87611080..HEAD | grep -c workbook` is **0** — plan 06 touched
`src/http/schema.rs`, `src/http/client.rs`, `src/tools.rs`, `src/config.rs`, `src/error.rs`,
`tests/curated_path_injection.rs` and the `Makefile`, none of which is in `src/workbook/`.

**Why no gate sees it:** the same CLAUDE.md-recorded blind spot as D2/D3 — `make lint` scopes
to the root `pmcp` package, and a plain `cargo build --workspace` warning is not
`-D warnings`. `make quality-gate` is green.
