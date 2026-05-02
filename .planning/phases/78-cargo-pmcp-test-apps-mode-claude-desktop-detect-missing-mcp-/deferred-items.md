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
