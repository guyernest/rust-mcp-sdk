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
