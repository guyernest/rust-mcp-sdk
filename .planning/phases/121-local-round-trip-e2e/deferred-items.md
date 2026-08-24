# Phase 121 — Deferred Items

Out-of-scope discoveries logged during execution. Per the executor scope boundary,
these were NOT fixed: they are pre-existing and not caused by this phase's changes.

## D1. Pre-existing clippy lints in dependency crates (found: 121-01 Task 3)

`cargo clippy -p pmcp-openapi-server --all-targets -- -D warnings` (plan 121-01
Task 3's literal verify command) exits **101**, but every failure is in a
DEPENDENCY crate, not in `pmcp-openapi-server`:

| Crate | Lint | Location | Count |
|-------|------|----------|-------|
| `mcp-tester` | `clippy::manual_filter` | `crates/mcp-tester/src/scenario_executor.rs:653,671` | 2 (errors under `-D warnings`) |
| `pmcp-server-toolkit` | `clippy::redundant_guard` | lib | 1 (warning) |

**Proven pre-existing:** `git diff --stat f3f55f3d..HEAD` shows plan 121-01 touches
only `Makefile`, `crates/pmcp-openapi-server/Cargo.toml`, and files under
`crates/pmcp-openapi-server/tests/`. Neither dependency crate is in this plan's
`files_modified`.

**Why it was never noticed:** `make lint` is `cargo clippy --features full --lib
--tests` with **no `-p`** (`Makefile:169`), so it resolves to the root `pmcp`
package only. Neither `mcp-tester` nor `pmcp-server-toolkit` is clippy-gated by
the repo gate or by CI, so a bare `-D warnings` run on them is STRICTER than
anything that gates a merge. These lints block nothing today.

**Status after 121-01:** `pmcp-openapi-server` itself generates **zero** clippy
warnings at `--all-targets` (verified: `cargo clippy -p pmcp-openapi-server
--all-targets` exits 0 with no `pmcp-openapi-server ... generated N warnings`
line). The plan's INTENT — this crate's own code clean at a bar stricter than the
repo gate — is met and proven.

**Follow-up:** a later simplify pass may apply the two `Option::filter` rewrites
in `mcp-tester` and the `redundant_guard` fix in `pmcp-server-toolkit`. Doing so
here would have widened a costly-reversibility task into two currently-green
crates outside its declared artifacts, for no PKG-04 benefit.

## D2. `contoso_m365_parity.rs` carries duplicate `fixtures_dir` / `examples_dir` (found: 121-01 Task 3)

`crates/pmcp-openapi-server/tests/contoso_m365_parity.rs` defines its own copies of
`fixtures_dir` (line 60) and `examples_dir` (line 66), which are now ALSO available
as `pub` items in `tests/common/mod.rs`.

Deliberately NOT collapsed: plan 121-01 D-02 names only `parity_replay.rs`'s
helpers, and widening a costly-reversibility extraction to a second currently-green
file buys no PKG-04 benefit. A later simplify pass should switch
`contoso_m365_parity.rs` to `mod common;` and delete the two local copies.
