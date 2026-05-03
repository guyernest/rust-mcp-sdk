# Deferred Items — Phase 78

Issues discovered during plan execution that are out-of-scope per SCOPE BOUNDARY (executor only auto-fixes issues directly caused by the current task's changes).

## Plan 78-07 — Pre-existing clippy errors in unrelated files

Discovered while running `cargo clippy -p cargo-pmcp --bin cargo-pmcp --all-features -- -D warnings` against worktree base `a55ab46d`. Verified pre-existing by `git stash && cargo clippy ...` — same 5 errors reproduce on a clean tree.

Files involved (none modified by Plan 78-07):
- `cargo-pmcp/src/loadtest/summary.rs:58` — `vec_init_then_push`
- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs:660` — `type_complexity`
- `cargo-pmcp/src/pentest/attacks/prompt_injection.rs:694` — `type_complexity`
- `cargo-pmcp/src/pentest/attacks/protocol_abuse.rs:563` — `unnecessary_cast`
- `cargo-pmcp/src/deployment/config.rs:491` — `collapsible_match`

These appear to be a recent clippy-version pickup (CLAUDE.md notes "Local/CI version mismatch is the #1 cause of CI failures (new clippy lints each release)"). Recommend a follow-up plan or PR to fix all 5 in a single commit.
