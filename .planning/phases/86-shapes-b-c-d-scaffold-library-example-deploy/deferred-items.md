# Deferred Items — Phase 86

Out-of-scope discoveries logged during plan execution (SCOPE BOUNDARY rule).
These are NOT caused by this phase's changes and are NOT fixed here.

## Plan 86-01

- **Pre-existing clippy `field_reassign_with_default` (rust-1.95.0) in `crates/pmcp-server-toolkit/src/code_mode.rs:520-521`.**
  - Surfaced by `cargo clippy -p pmcp-server-toolkit --features sqlite,code-mode,http --all-targets`.
  - The file was last modified in commit `d962051e` (Plan 85-10) — NOT touched by Plan 86-01 (this plan modifies only `sql/sqlite.rs`, `lib.rs`, `Cargo.toml`).
  - Consistent with prior STATE.md decisions noting pre-existing rust-1.95.0 pedantic lints in the toolkit surface (Phase 84 Plan 08, Phase 85 Plan 03).
  - Out of scope per SCOPE BOUNDARY; left for a dedicated lint-sweep.

- **Pre-existing workspace `cargo fmt --all --check` failures in Phase 84 connector crates (committed code, not this plan).**
  - `make quality-gate` (= `cargo fmt --all -- --check`) reports reflow diffs in committed code of three crates NOT touched by Plan 86-01:
    - `crates/pmcp-toolkit-athena/src/{dev_mock.rs,lib.rs}`, `crates/pmcp-toolkit-athena/tests/integration.rs`
    - `crates/pmcp-toolkit-mysql/src/{dev_mock.rs,lib.rs}`
    - `crates/pmcp-toolkit-postgres/src/{dev_mock.rs,lib.rs}`
  - These files are committed-clean (no uncommitted edits) — the diffs are a newer-rustfmt-version reflow of Phase 84 code, surfaced workspace-wide. Plan 86-01 touches only `pmcp-server-toolkit/{src/sql/sqlite.rs,src/lib.rs,Cargo.toml}`, all of which ARE fmt-clean.
  - Out of scope per SCOPE BOUNDARY; a workspace-wide `cargo fmt --all` reflow should be done as a dedicated chore commit (or in the relevant connector phase), not silently folded into 86-01.
