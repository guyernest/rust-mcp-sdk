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

## Pre-existing clippy errors in `cargo-pmcp` pentest/loadtest/deployment code

**Discovered during:** Plan 78-04 Task 3 verification (`cargo clippy -p cargo-pmcp --all-targets -- -D warnings`)

**Errors (5 total, all pre-existing on base commit `a5fd2844`):**

```
error: using `contains()` instead of `iter().any()` is more efficient
  --> cargo-pmcp/src/pentest/attacks/data_exfiltration.rs:656:13
  --> cargo-pmcp/src/pentest/attacks/data_exfiltration.rs:668:13
  --> cargo-pmcp/src/pentest/attacks/data_exfiltration.rs:672:13

error: very complex type used. Consider factoring parts into `type` definitions
  --> cargo-pmcp/src/pentest/attacks/prompt_injection.rs:660:19
  --> cargo-pmcp/src/pentest/attacks/prompt_injection.rs:694:19

error: casting to the same type is unnecessary (`u32` -> `u32`)
  --> cargo-pmcp/src/pentest/attacks/protocol_abuse.rs:563:50

error: calls to `push` immediately after creation
  --> cargo-pmcp/src/loadtest/summary.rs:58:5

error: this `if` can be collapsed into the outer `match`
  --> cargo-pmcp/src/deployment/config.rs:491:21
```

**Verification this is pre-existing:**
Stashed Plan 78-04 working-tree changes and re-ran `cargo clippy -p cargo-pmcp --all-targets -- -D warnings` against base commit `a5fd2844` — same errors reproduce. They are independent of Plan 78-04 changes.

**Why not fixed inline:**
Plan 78-04 only modifies markdown READMEs and a `///`-doc clap comment on `cargo-pmcp/src/commands/test/mod.rs::TestCommand::Apps`. None of the clippy errors above are in files modified by this plan. Fixing them would violate the executor's SCOPE BOUNDARY rule.

**Plan 78-04 verification scope:**
The `--help` change is verified end-to-end (`cargo run -p cargo-pmcp -- test apps --help | grep -q claude-desktop` passes), the build succeeds, and `cargo fmt --all -- --check` passes. The `cargo clippy -p mcp-tester --lib --tests --bins -- -D warnings` (Task 1's actual scope of change) passes.
