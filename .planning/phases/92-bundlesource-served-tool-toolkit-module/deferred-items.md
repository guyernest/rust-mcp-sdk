# Deferred Items — Phase 92

## Out-of-scope warnings (not caused by this plan's changes)

- **`crates/pmcp-server-toolkit/src/code_mode.rs:557`** — `unused import: pmcp_code_mode::CodeExecutor`.
  Pre-existing at HEAD (`git show HEAD:crates/pmcp-server-toolkit/src/code_mode.rs` line 557 confirms it).
  Surfaces under the default `code-mode` feature; my Plan 02 changes never touch this file. The real
  CI clippy gate lints only root `pmcp` with an allow-list (the toolkit crate is not clippy-gated — see
  user MEMORY `project_rust195_clippy_gate_debt.md`), so this does not block CI. Left untouched per the
  executor scope boundary (only auto-fix issues directly caused by the current task's changes).
