# Deferred items — Phase 120

Out-of-scope findings surfaced during execution. Logged, not fixed (executor
scope boundary: only auto-fix issues directly caused by the current task).

## 9 pre-existing failures in the `cargo-pmcp` BIN test target

`cargo test -p cargo-pmcp --bins` -> `845 passed; 9 failed`.

The project's own gate (`make test-cargo-pmcp`, Makefile:284) runs
`cargo test -p cargo-pmcp --lib` ONLY, so the bin target's ~854 tests are not
gated locally and these failures predate this phase. `--lib` is green
(465 passed).

Failing tests, none in a file this phase touched:

| Test | File |
|---|---|
| `configure::resolver::tests::resolve_target_returns_target_source_for_target_fields` | `src/commands/configure/resolver.rs:625` |
| `deploy::manifest_resolution_tests::guard_init_root_fires_when_not_cwd_and_no_deploy_toml` | `src/commands/deploy/mod.rs:1990` |
| `doctor::tests::doctor_widget_check_*` (5 tests) | `src/commands/doctor.rs:378` |
| `aws_lambda::artifact::tests::fetch_builtin_binary_rejects_corrupt_cache` | `src/deployment/targets/aws_lambda/artifact.rs:1027` |
| `aws_lambda::artifact::tests::fetch_builtin_binary_uses_cache_without_network_on_hit` | `src/deployment/targets/aws_lambda/artifact.rs:1000` |

All are runtime filesystem / download-stub / cwd-dependent failures
("No such file or directory", "stub has no entry for <url>"), not type or
API errors — the 0.2.0 API break is compile-time-coupled, so a regression
from it would surface as a build failure, and the build is green.

**Suggested follow-up:** either fix these tests or widen
`make test-cargo-pmcp` to cover the bin target. Leaving both as-is means
854 tests are shipping unwatched.

## Release-ledger prose still naming the 0.1 line (Phase 124's half)

Per the Task 1 decision (option-a), Phase 124 keeps the release/publish
ledger. These markdown references still describe `pmcp-package = "0.1"` and
are release-ledger or historical-design text, not in-repo emitters:

- `CLAUDE.md:252,258,267,274` — publish-order prose for items 13/13a/14/15.
- `crates/pmcp-cfn-renderer/tests/goldens/README.md:109` — publish-ordering note.
- `docs/design/agents-teams-sdk-extraction-plan.md:95,130` — historical plan text.
- `docs/superpowers/plans/2026-07-21-cfn-renderer-extraction.md` — historical plan text.

`crates/pmcp-package/README.md` was NOT deferred — it is the published
crate's own user-facing doc and would have shipped inside 0.2.0 telling users
to depend on `0.1`, so this plan updated it.
