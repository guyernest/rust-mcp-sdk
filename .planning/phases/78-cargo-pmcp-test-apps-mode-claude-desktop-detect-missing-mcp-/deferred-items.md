# Phase 78 Deferred Items

Out-of-scope discoveries logged during plan execution. Per SCOPE BOUNDARY rule
(executor execution_flow.deviation_rules), these are pre-existing issues not
caused by the current task and are NOT fixed inline.

## Pre-existing clippy error in `crates/mcp-tester/examples/render_ui.rs`

**Discovered during:** Plan 78-01 Task 1 verification (`cargo clippy -p mcp-tester --all-targets -- -D warnings`)

**Error:**

```
error: you seem to want to iterate on a map's keys
  --> crates/mcp-tester/examples/render_ui.rs:88:34
   |
88 |     for (tool_name, _ui_info) in tool_uis {
   |                                  ^^^^^^^^
   |
   = note: `-D clippy::for-kv-map` implied by `-D warnings`
```

**Verification this is pre-existing:**
Stashed Plan 78-01 changes and re-ran `cargo clippy -p mcp-tester --all-targets -- -D warnings` against base commit `78a844e8` — same error reproduced. The error is independent of Plan 78-01 changes.

**Fix (recommended for a follow-up commit):**

```rust
// In crates/mcp-tester/examples/render_ui.rs around line 88:
for tool_name in tool_uis.keys() {
    // ... use _ui_info via tool_uis[tool_name] if needed, or just iterate keys
}
```

**Why not fixed inline:**
The error sits in an example that Plan 78-01 does not modify. Fixing it would
violate the executor's SCOPE BOUNDARY rule (only auto-fix issues directly
caused by the current task's changes). The Plan 78-01 verification scope is
the `app_validator` module and its consumers, which are clippy-clean after
this plan's `#[allow(dead_code)]` annotations.

## Pre-existing clippy errors in `cargo-pmcp` pentest/loadtest/deployment modules

**Discovered during:** Plan 78-02 Task 1 verification AND Plan 78-04 Task 3 verification — both ran `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` and surfaced the same 5 pre-existing errors. Consolidated here for traceability.

**Errors (unrelated files):**

- `cargo-pmcp/src/pentest/attacks/data_exfiltration.rs:656,668,672` — `clippy::manual_contains` (3x: `iter().any(|i| *i == "literal")` → `.contains(&"literal")`)
- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs:660,694` — `clippy::type_complexity` (2x: complex type used; consider type alias)
- `cargo-pmcp/src/pentest/attacks/protocol_abuse.rs:563` — `clippy::unnecessary_cast` (1x: `u32 -> u32`)
- `cargo-pmcp/src/loadtest/summary.rs:58` — `clippy::vec_init_then_push` (1x: push immediately after creation)
- `cargo-pmcp/src/deployment/config.rs:491` — `clippy::collapsible_match` (1x)

**Verification this is pre-existing:** Stashed plan changes (separately for both 78-02 and 78-04) and re-ran `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` against base commit `a5fd2844` — same errors reproduced. They are independent of any Phase 78 plan changes.

**Why not fixed inline:** Out-of-scope per executor SCOPE BOUNDARY rule. Plan 78-02 modifies `cargo-pmcp/src/commands/test/apps.rs`, `cargo-pmcp/tests/apps_helpers.rs`, `cargo-pmcp/tests/cli_acceptance.rs`, `cargo-pmcp/Cargo.toml`. Plan 78-04 modifies markdown READMEs and a `///`-doc clap comment on `cargo-pmcp/src/commands/test/mod.rs::TestCommand::Apps`. None of the clippy errors above are in files touched by either plan.

**Plan 78-02 / 78-04 verification scope:** Both plans scope `cargo clippy -p cargo-pmcp` runs to `--lib --tests --bins` (excluding examples + unrelated pentest code). The new code in `apps.rs`, `apps_helpers.rs`, `cli_acceptance.rs`, `report.rs`, and `mod.rs` is clippy-clean.

## Worktree-only: `fuzz/Cargo.toml` workspace-collision pre-existing issue (Plan 78-03)

**Discovered during:** Plan 78-03 Task 3 verification — `cd fuzz && cargo build --bin app_widget_scanner` and `cargo build --manifest-path fuzz/Cargo.toml --bin <any_target>` both fail with:

```
error: current package believes it's in a workspace when it's not:
current:   /Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/worktrees/agent-aa0813d2c50896b88/fuzz/Cargo.toml
workspace: /Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml
```

**Verification this is pre-existing and worktree-environmental:** The error reproduces for ALL existing fuzz targets (`protocol_parsing`, `auth_flows`, etc.) when invoked from inside this worktree (`/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/worktrees/agent-aa0813d2c50896b88/fuzz/`). Cargo's workspace walker resolves UPWARD past the worktree root and finds the parent repository's `Cargo.toml` (`/Users/guy/Development/mcp/sdk/rust-mcp-sdk/Cargo.toml`) before stopping, because the worktree is nested inside the parent repository's working tree. The parent repo's `[workspace] exclude` does NOT include the worktree-specific `.claude/worktrees/agent-.../fuzz` path.

**Why not fixed inline:** Adding `[workspace]` to `fuzz/Cargo.toml` would persist into the upstream merge and is unnecessary in non-worktree environments (CI runs `cargo fuzz build` directly from the upstream repo root, where the parent `Cargo.toml`'s `exclude = ["fuzz", ...]` correctly excludes the fuzz crate). The pre-existing fuzz targets demonstrably build in CI and on direct repo checkouts; the failure is exclusively a worktree-nesting artifact.

**Verification of new fuzz target's correctness (without `cargo build`):**
1. The file `fuzz/fuzz_targets/app_widget_scanner.rs` matches the plan-mandated structure (`#![no_main]`, `fuzz_target!`, three-element tuple, `mcp_tester::AppValidator::validate_widgets`).
2. The fuzz target's API surface (`AppValidator::new`, `validate_widgets` with `&[(String, String, String)]`) is the IDENTICAL surface exercised by the seven passing integration tests in `crates/mcp-tester/tests/app_validator_widgets.rs` and the two passing property tests in `crates/mcp-tester/tests/property_tests.rs`. Therefore a non-worktree `cargo fuzz build app_widget_scanner` will succeed when CI or a direct-repo invocation runs.
3. `cargo fuzz list --fuzz-dir fuzz` from this worktree returns `app_widget_scanner` in the bin list (confirms the `[[bin]]` registration is correct).

**Recommended follow-up:** None — this is a worktree-specific testing-environment limitation. CI and post-merge `cargo fuzz build` will work normally because the parent repository's `Cargo.toml` excludes the upstream `fuzz/` directory correctly.
