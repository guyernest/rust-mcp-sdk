---
phase: 79-cargo-pmcp-deploy-widget-pre-build-post-deploy-verification-
verified: 2026-05-03T00:00:00Z
status: passed
score: 24/24 must-haves verified
overrides_applied: 0
---

# Phase 79: cargo pmcp deploy widget pre-build + post-deploy verification — Verification Report

**Phase Goal:** Close Failure Modes A (stale widget bundle), B (Cargo cache miss for `include_str!`), and C (broken-but-live deploy reported as successful) — build half auto-detects widgets and forces cache invalidation, verify half runs warmup → check → conformance → apps after Lambda hot-swap with screaming-loud LIVE-but-broken banner on failure.

**Verified:** 2026-05-03
**Status:** PASSED — all 24 verification dimensions match the codebase. No HIGH gaps.

## Goal Achievement — Verification Dimension Matrix

### Build half (Failure Modes A + B)

| #  | Dimension                                    | Status     | Evidence (file:line)                                                        |
|----|----------------------------------------------|------------|-----------------------------------------------------------------------------|
| 1  | Widget convention `widget/`+`widgets/` ONLY  | VERIFIED   | `cargo-pmcp/src/deployment/widgets.rs:276` literal `["widget", "widgets"]`  |
| 2  | Lockfile-driven PM picker (bun>pnpm>yarn>npm) | VERIFIED  | `cargo-pmcp/src/deployment/widgets.rs:185-199`                              |
| 3  | `PMCP_WIDGET_DIRS` env-var (HIGH-C1)         | VERIFIED   | `cargo-pmcp/src/commands/deploy/mod.rs:542-546` colon-join + `set_var` ONCE |
| 4  | build.rs template splits on `:` + local fallback (HIGH-G1+C1) | VERIFIED | `cargo-pmcp/src/templates/mcp_app.rs:105-131` `split(':')` + `discover_local_widget_dirs` |
| 5  | `--no-widget-build`, `--widgets-only` flags  | VERIFIED   | `cargo-pmcp/src/commands/deploy/mod.rs:160-169`                             |
| 6  | `embedded_in_crates` explicit, not auto      | VERIFIED   | `cargo-pmcp/src/deployment/widgets.rs:102` `Vec<String>` schema field       |
| 7  | Yarn PnP `is_yarn_pnp` early-return (Codex MED) | VERIFIED | `cargo-pmcp/src/deployment/widgets.rs:326-328,398-403` `.pnp.cjs`/`.pnp.loader.mjs` |
| 8  | `build`/`install` argv-vec schema (Codex MED) | VERIFIED  | `cargo-pmcp/src/deployment/widgets.rs:85,91` `Option<Vec<String>>`          |

### Verify half (Failure Mode C)

| #  | Dimension                                    | Status     | Evidence (file:line)                                                        |
|----|----------------------------------------------|------------|-----------------------------------------------------------------------------|
| 9  | Lifecycle warmup→check→conformance→apps      | VERIFIED   | `post_deploy_tests.rs:797-812` (warmup), `:678-702` (build_run_plan ordering, gated on `widgets_present` for apps) |
| 10 | Subprocess approach via Tokio Command        | VERIFIED   | `post_deploy_tests.rs:312-319` (current_exe), `:384-388` (Tokio Command spawn) |
| 11 | JSON consumer via `serde_json::from_str`     | VERIFIED   | `post_deploy_tests.rs:421` `serde_json::from_str::<PostDeployReport>` — NO regex anywhere |
| 11b | `parse_conformance_summary` / `parse_apps_summary` removed | VERIFIED | `grep -rn` returns 0 matches across `cargo-pmcp/src/` and `crates/mcp-tester/src/` |
| 12 | Auth via env inheritance (HIGH-C2)           | VERIFIED   | `post_deploy_tests.rs:381-388` no `env_clear`, no `MCP_API_KEY` injection, no `--api-key` argv. `resolve_auth_token` does not exist (grep 0 matches). |
| 13 | Rollback hard-reject `OnFailure {Fail,Warn}` (HIGH-G2) | VERIFIED | `post_deploy_tests.rs:138` enum, `:171-184` Deserialize, `:185-194` FromStr, `commands/deploy/mod.rs:205,223-227` clap value_parser, all reject with `ROLLBACK_REJECT_MESSAGE` (`:162`) |
| 14 | Exit codes BrokenButLive=3 / InfraError=2 (HIGH-2) | VERIFIED | `post_deploy_tests.rs:622-637` enum + `:651-661` exit_code() impl |
| 15 | CI annotation `::error::` when `CI=true`     | VERIFIED   | `post_deploy_tests.rs:584-603` `emit_ci_annotation` + `write_ci_annotation`; live test output shows `::error::Deployment succeeded but post-deploy tests failed (exit code 3). Lambda revision is LIVE.` |
| 16 | Failure banner "(N/M tests/widgets passed/failed)" + IS LIVE + rollback cmd | VERIFIED | `post_deploy_tests.rs:509-528` (banner), `:543-546` noun dispatch (Apps→widgets, else→tests), `:567-577` metric format `"({passed}/{total} {noun} passed)"` and `"({failed}/{total} {noun} failed)"` |
| 17 | All 4 verify-half flags exist                | VERIFIED   | `commands/deploy/mod.rs:178-214` (`--no-post-deploy-test`, `--post-deploy-tests=`, `--on-test-failure=`, `--apps-mode=`) |
| 18 | InfraErrorKind = `{Subprocess,Timeout,AuthOrNetwork}` only | VERIFIED | `post_deploy_tests.rs:267-275` (no AuthMissing variant; only doc-comment mentions it as REMOVED) |

### Cross-cutting

| #  | Dimension                                    | Status     | Evidence (file:line)                                                        |
|----|----------------------------------------------|------------|-----------------------------------------------------------------------------|
| 19 | Doctor `check_widget_rerun_if_changed` distinguishes WidgetDir vs include_str! | VERIFIED | `commands/doctor.rs:228-243` regex `r#"include_str!\s*\(\s*"[^"]*widgets?/[^"]*"\s*\)"#`; WidgetDir crates skipped (no regex hit) per `:225-227` |
| 20 | Scaffold opt-in `--embed-widgets` (Codex MED) | VERIFIED | `templates/mcp_app.rs:25-66` `embed_widgets: bool` param branches main.rs + writes build.rs only when true |
| 21 | Runnable example exists                      | VERIFIED   | `cargo run -p cargo-pmcp --example widget_prebuild_demo` exits 0 — ran and printed "=== Example complete — Phase 79 build half verified end-to-end ===" |
| 22 | Fuzz target exists                           | VERIFIED   | `cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs` exists; tests both DeployConfig parse + OnFailure rollback rejection (HIGH-G2) |
| 23 | Version bumps applied                        | VERIFIED   | `cargo-pmcp/Cargo.toml:version = "0.12.0"`; `crates/mcp-tester/Cargo.toml:version = "0.6.0"`; cargo-pmcp dep pin `mcp-tester = "0.6.0"`; pmcp-server dep pin `mcp-tester = "0.6.0"`; both CHANGELOGs dated `## [0.12.0] - 2026-05-03` and `## [0.6.0] - 2026-05-03` |
| 24 | Quality gate green / build clean             | VERIFIED   | `cargo build --release -p cargo-pmcp -p mcp-tester` finished in 2m 00s with no errors. SUMMARY claim of `make quality-gate` exit 0 trusted (executor-confirmed, root-crate scoped lint per Makefile:146). |

### Test totals

| Crate         | Claimed | Observed | Status   |
|---------------|---------|----------|----------|
| cargo-pmcp    | 1064    | **1064** (17 suites, 26.69s) | VERIFIED |
| mcp-tester    | 205     | **205** (9 suites, 6.02s)    | VERIFIED |

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/mcp-tester/src/post_deploy_report.rs` | `PostDeployReport` struct + schema_version="1" + outcome enum + FailureDetail | VERIFIED | 10.9K, struct at line 55, schema_version field line 84, default "1" line 97 |
| `crates/mcp-tester/src/lib.rs` | `pub use post_deploy_report::*` re-export | VERIFIED | line 52 (`pub mod`), lines 65-66 (`pub use`) |
| `cargo-pmcp/src/deployment/widgets.rs` | schema + run_widget_build + is_yarn_pnp + argv_to_cmd_args | VERIFIED | 31.0K — all symbols present at expected line ranges |
| `cargo-pmcp/src/deployment/post_deploy_tests.rs` | OnFailure + ROLLBACK_REJECT_MESSAGE + spawn_test_subprocess + parse_subprocess_result + format_failure_banner + emit_ci_annotation + OrchestrationFailure::BrokenButLive | VERIFIED | 38.2K — all symbols present, no regex parsers, no resolve_auth_token, no AuthMissing |
| `cargo-pmcp/src/deployment/config.rs` | `widgets: WidgetsConfig` + `post_deploy_tests: Option<PostDeployTestsConfig>` with `#[serde(default)]` | VERIFIED | lines 41-49, both fields with `skip_serializing_if` byte-identity guards |
| `cargo-pmcp/src/commands/deploy/mod.rs` | 6 new flags + Step 2.5 widget hook + post-deploy verifier hook | VERIFIED | flags lines 161-214, `pre_build_widgets_and_set_env` line 522, `run_post_deploy_tests` invocation line 916 |
| `cargo-pmcp/src/commands/doctor.rs` | `check_widget_rerun_if_changed` distinguishing WidgetDir vs include_str! | VERIFIED | line 228, registered as a doctor check at line 27 |
| `cargo-pmcp/src/templates/mcp_app.rs` | `--embed-widgets` opt-in; default WidgetDir; conditional build.rs writing | VERIFIED | lines 25, 32-33, 62-66 |
| `cargo-pmcp/Cargo.toml` | version 0.12.0 | VERIFIED | line 2 |
| `crates/mcp-tester/Cargo.toml` | version 0.6.0 | VERIFIED | line 2 |
| `cargo-pmcp/CHANGELOG.md` | `## [0.12.0] - 2026-05-03` entry | VERIFIED | header present |
| `crates/mcp-tester/CHANGELOG.md` | `## [0.6.0] - 2026-05-03` entry | VERIFIED | header present |
| `cargo-pmcp/examples/widget_prebuild_demo.rs` | Runnable example | VERIFIED | runs cleanly, exits 0 |
| `cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs` | Fuzz TOML + OnFailure rollback path | VERIFIED | both paths exercised |

## Test Surface Verification

| Test File                                          | Status     |
|---------------------------------------------------|------------|
| `cargo-pmcp/tests/widgets_config.rs`              | VERIFIED   |
| `cargo-pmcp/tests/post_deploy_tests_config.rs`    | VERIFIED   |
| `cargo-pmcp/tests/widgets_orchestrator.rs`        | VERIFIED   |
| `cargo-pmcp/tests/post_deploy_orchestrator.rs`    | VERIFIED   |
| `cargo-pmcp/tests/deploy_post_deploy_flags.rs`    | VERIFIED   |
| `cargo-pmcp/tests/test_format_json.rs`            | VERIFIED   |
| `cargo-pmcp/tests/cli_acceptance.rs`              | VERIFIED   (rollback rejection test at line 186, REQ-79-18 verbatim test at line 154) |

## Key Link Verification (Goal-Backward Wiring)

| From | To | Via | Status |
|------|----|----|--------|
| `commands/deploy/mod.rs` | `widgets::run_widget_build` | `pre_build_widgets_and_set_env` → loop over detect_widgets → set PMCP_WIDGET_DIRS env | WIRED (line 535) |
| `commands/deploy/mod.rs` | `post_deploy_tests::run_post_deploy_tests` | `if !no_post_deploy_test` branch | WIRED (line 916) |
| `post_deploy_tests::spawn_test_subprocess` | `mcp_tester::PostDeployReport` | `serde_json::from_str::<PostDeployReport>(stdout)` | WIRED (line 421) |
| `templates/mcp_app.rs::generate_build_rs` | `PMCP_WIDGET_DIRS` env var contract | template splits `:`, falls back to local discovery | WIRED (lines 107-113) |
| `commands/doctor.rs::check_widget_rerun_if_changed` | regex matching `include_str!\s*\(\s*"[^"]*widgets?/[^"]*"\s*\)` | scan_workspace_for_pattern | WIRED (line 229) |
| clap `--on-test-failure` parser | `OnFailure::FromStr` (rollback hard-reject) | `parse_on_test_failure_flag` value_parser delegates to FromStr | WIRED (mod.rs:205,223-227) |

## Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| cargo-pmcp test suite passes (1064) | `cargo test -p cargo-pmcp -- --test-threads=1` | `1064 passed (17 suites, 26.69s)` | PASS |
| mcp-tester test suite passes (205) | `cargo test -p mcp-tester -- --test-threads=1`  | `205 passed (9 suites, 6.02s)`     | PASS |
| Example runs end-to-end             | `cargo run -p cargo-pmcp --example widget_prebuild_demo` | exits 0, prints "=== Example complete ===" | PASS |
| Release build clean                 | `cargo build --release -p cargo-pmcp -p mcp-tester` | finished in 2m 00s, no errors | PASS |
| CI annotation emits live (test side effect proves runtime path) | (observed during cargo test run) | stderr printed `::error::Deployment succeeded but post-deploy tests failed (exit code 3). Lambda revision is LIVE. To roll back: cargo pmcp deploy rollback --target prod` | PASS |

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `cargo-pmcp/src/deployment/mod.rs` | 21-23 | `pub use post_deploy_tests::{...}` flagged as `unused imports` by clippy in cargo-pmcp test build | INFO  | Cargo-pmcp clippy emits 2 warnings ("unused imports") on the post_deploy_tests re-exports. The root-crate `make lint` does NOT cover cargo-pmcp (it targets `pmcp` only), so quality-gate exits 0 as the SUMMARY claims. Recommend cleanup in a follow-up: either consume these symbols downstream or remove the re-exports. NOT a phase blocker. |

No SATD comments, no TODO/FIXME placeholders, no `return null`/empty stubs, no hardcoded empty data flowing to user output. Banner formatter renders real data sourced from `PostDeployReport` JSON.

## Requirements Coverage

All 18 REQ-79-* requirements (REQ-79-01 through REQ-79-18) verified satisfied via the dimension matrix above. Per the SUMMARY mapping table, every REQ has matching code at the cited file:line locations.

## Override Suggestions

None needed. All dimensions PASSED.

## Phase Self-Check

- [x] All 24 verification dimensions VERIFIED
- [x] All 14 required artifacts exist with substantive content (verified by direct grep + line citations)
- [x] All 7 test files exist
- [x] All 6 key links wired
- [x] Test totals match SUMMARY claims (1064 cargo-pmcp + 205 mcp-tester)
- [x] Example runs end-to-end exit 0
- [x] Release build clean
- [x] No regex parsing of test output (HIGH-1 closed)
- [x] No `resolve_auth_token` / `MCP_API_KEY` injection (HIGH-C2 closed)
- [x] `OnFailure` rejects "rollback" at both Deserialize AND FromStr AND clap value_parser (HIGH-G2 closed)
- [x] `OrchestrationFailure::BrokenButLive` exit_code 3 distinct from InfraError exit_code 2 (HIGH-2 closed)
- [x] `PMCP_WIDGET_DIRS` colon-list set ONCE for multi-widget cache invalidation (HIGH-C1 closed)
- [x] `build.rs` template falls back to local discovery when env unset (HIGH-G1 closed)
- [x] All 6 REVISION 3 HIGH supersessions confirmed in code
- [x] All 3 REVISION 3 MEDIUM polish items confirmed (scaffold opt-in, argv-vec, Yarn PnP)

## VERIFICATION PASSED

No HIGH gaps. All 24 verification dimensions match the contract from `79-CONTEXT.md` (including the binding "Review-Driven Supersessions (2026-05-03)" header). Phase 79 is goal-complete — Failure Modes A, B, and C are closed by code that exists, is substantive, is wired, and produces real data flow end-to-end. Test totals match the SUMMARY claim. Example runs. Independent release build is clean.

One INFO-level cleanup opportunity: cargo-pmcp's `deployment/mod.rs:21-23` re-exports of `AppsMode`/`FailureRecipe`/etc. are flagged by `cargo clippy -p cargo-pmcp` as unused. This does NOT block quality-gate (root-crate lint scope) and does NOT block phase closeout. Recommend a follow-up small cleanup commit to either drop the re-exports or consume them at a downstream call site.

Phase 79 is ready for closeout. Per the SUMMARY's recommended next step, the release can proceed: tag `v0.12.0` for cargo-pmcp + `v0.6.0` for mcp-tester.

---

_Verified: 2026-05-03_
_Verifier: Claude (gsd-verifier)_
