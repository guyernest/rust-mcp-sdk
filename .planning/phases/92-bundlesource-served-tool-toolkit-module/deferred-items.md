# Deferred Items — Phase 92

## Out-of-scope warnings (not caused by this plan's changes)

- **`crates/pmcp-server-toolkit/src/code_mode.rs:557`** — `unused import: pmcp_code_mode::CodeExecutor`.
  Pre-existing at HEAD (`git show HEAD:crates/pmcp-server-toolkit/src/code_mode.rs` line 557 confirms it).
  Surfaces under the default `code-mode` feature; my Plan 02 changes never touch this file. The real
  CI clippy gate lints only root `pmcp` with an allow-list (the toolkit crate is not clippy-gated — see
  user MEMORY `project_rust195_clippy_gate_debt.md`), so this does not block CI. Left untouched per the
  executor scope boundary (only auto-fix issues directly caused by the current task's changes).
## [92-04] Pre-existing rustfmt drift in Plan-02 test-support files

- **Files:** `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs`, `tests/support/tamper.rs`, `tests/fixture_byte_stability.rs`
- **Discovered during:** Plan 92-04 Task 1 (`cargo fmt -p pmcp-server-toolkit` reformatted them).
- **Issue:** These Plan-02 fixture/tamper helpers were committed with a slightly older rustfmt formatting (multi-line fn args / `assert!` wrapping). A current-toolchain `cargo fmt --all --check` flags them. They are NOT touched by Plan 04 and the change is whitespace-only.
- **Disposition:** Out of scope (executor scope boundary — not caused by Plan 04's changes). Reverted to keep Plan 04 commits scoped. Should be picked up by a workspace-wide `cargo fmt --all` in the next plan that runs `make quality-gate` (Plan 05 wires the builder-ext + purity gate + example, which runs the full gate).
