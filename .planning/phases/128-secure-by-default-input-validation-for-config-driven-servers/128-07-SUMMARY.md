---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 07
subsystem: testing
tags: [cargo-pmcp, cli, clap, config, toml, input-validation, lint, iam, makefile, assert_cmd]

requires:
  - phase: 128-03
    provides: "`ServerConfig::lint() -> Vec<ConfigWarning>`, the `[server.validation]` section, and the four new `ConfigValidationError` variants this command surfaces"
  - phase: 128-06
    provides: "`ConfigValidationError::MalformedPathTemplateSegment` — a hard config error this command now surfaces BEFORE deploy instead of at boot"
provides:
  - "`cargo pmcp validate config` (`ValidateCommand::Config`, `--server` / `--config`) — reads a toolkit `config.toml`, runs `ServerConfig::validate()` then `lint()`, prints every finding, exits non-zero ONLY on a read/parse/`validate()` failure"
  - "`cargo pmcp validate deploy` additionally reports the SAME `lint()` findings when a `config.toml` is discoverable beside `.pmcp/deploy.toml` — warnings only, exit-code contract untouched (D-07's literal surface)"
  - "`cargo-pmcp -> pmcp-server-toolkit` FIRST DIRECT dependency edge (`default-features = false, features = [\"input-validation\", \"http\"]`)"
  - "`cargo-pmcp/tests/validate_server_config.rs` (12 tests) wired into BOTH `test-cargo-pmcp-integration` lists, so it runs and its absence fails the leg"
affects: [128-09, 128-10, 128-11]

actuals:
  tokens: 10634
  tasks: 3
  commits: 7   # measured: git rev-list --count 7830f335..HEAD, INCLUDING this docs commit
  plan_head_before: 7830f335ea572e23faa73679a3e7310f686bc8ec

tech-stack:
  added: ["pmcp-server-toolkit (direct dep on cargo-pmcp, default-features = false, features = [\"input-validation\", \"http\"])"]
  patterns:
    - "One security rule, one implementation: a second surface PROJECTS the rule (`config.lint()`), it never re-derives it"
    - "Three-state discovery result (`Absent` / `Findings` / `Unreadable`) so a caller cannot conflate 'no such document' with 'a document that reports nothing'"
    - "A new `tests/` binary is added to BOTH the `--test` selector list and `REQUIRED_TEST_BINARIES` — the first makes it run, the second makes its absence a failure"

key-files:
  created:
    - cargo-pmcp/tests/validate_server_config.rs
  modified:
    - cargo-pmcp/Cargo.toml
    - cargo-pmcp/src/commands/validate.rs
    - Makefile

key-decisions:
  - "Q5 Route B + the literal D-07 surface: `validate config` is the PRIMARY command (clean semantics), and `validate deploy` ALSO emits the findings as warnings, so D-07's named surface is honoured without muddying a command whose contract is about IAM"
  - "`http` on the toolkit edge was decided from source, not from a probe: `ServerConfig.backend` is `#[cfg(feature = \"http\")]` and `ServerConfig` is `deny_unknown_fields`, so a non-`http` toolkit rejects every `[backend]`-carrying config"
  - "`validate.rs`'s no-rule-copy gate was re-scoped to PRODUCTION code (lines before the first `#[cfg(test)]`, comments stripped) because a TOML fixture key in a test cannot enforce anything — and the plan's literal form was unsatisfiable"
  - "The plan's three `--lib` verify filters select ZERO tests and were corrected to `--bins`: `cargo-pmcp/src/commands/validate.rs` is NOT in the lib target (`src/lib.rs` mounts only specific `#[path]` leaves)"
  - "No version moved and no pin moved — `cargo-pmcp` stays byte-identical at 0.24.3; the 0.24.3 -> 0.25.0 move and the new pin belong to plan 11's single commit (D-14)"

patterns-established:
  - "Pattern 1: a CLI reviewer surface is a PROJECTION of a library lint, asserted mechanically (zero rule literals in production code) rather than by discipline"
  - "Pattern 2: a gate-visibility edit is proven with a POSITIVE CONTROL — remove the selector, confirm the leg fails — not assumed from a green run"
  - "Pattern 3: cwd-dependent discovery is exercised only from a CHILD process, never from a unit test, when the unit leg runs without `--test-threads=1`"

requirements-completed: [D3, SC-3]

coverage:
  - id: D1
    description: "`cargo pmcp validate config` reads a toolkit `config.toml`, runs `ServerConfig::lint()`, prints each finding, and exits non-zero only when `ServerConfig::validate` itself fails (SC-3)"
    requirement: "SC-3"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/validate_server_config.rs#uncapped_body_string_warns_on_stderr_and_exits_zero"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/validate_server_config.rs#non_compiling_pattern_exits_non_zero_naming_the_parameter"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/validate.rs#server_config_validate_tests::uncapped_body_string_is_reported_naming_tool_and_param"
        status: pass
    human_judgment: false
  - id: D2
    description: "`cargo pmcp validate deploy` additionally warns on every uncapped body-position string and every active `[server.validation]` opt-out when a toolkit config is discoverable, without changing its own hard-error contract (D-07 / SC-3)"
    requirement: "SC-3"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/validate_server_config.rs#validate_deploy_reports_lint_findings_when_both_documents_are_present"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/validate.rs#server_config_validate_tests::validate_deploy_wildcard_allow_still_fails_with_lint_findings_present"
        status: pass
      - kind: integration
        ref: "cargo-pmcp/tests/validate_server_config.rs#validate_deploy_says_nothing_extra_without_a_toolkit_config"
        status: pass
    human_judgment: false
  - id: D3
    description: "`cargo pmcp validate config` on a config with zero tools prints a no-findings line and exits 0 — the CLI projection of the SC-3 empty-input edge"
    requirement: "SC-3"
    verification:
      - kind: integration
        ref: "cargo-pmcp/tests/validate_server_config.rs#zero_tools_config_reports_no_findings_on_stdout_and_exits_zero"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/validate.rs#server_config_validate_tests::zero_tools_config_has_no_findings_and_succeeds"
        status: pass
    human_judgment: false
  - id: D4
    description: "`cargo-pmcp` resolves `pmcp-server-toolkit` without pulling the JS engine: the dependency carries `default-features = false` so `code-mode` is not enabled"
    requirement: "D3"
    verification:
      - kind: other
        ref: "cargo tree -p cargo-pmcp -e normal -i swc_common  (package ID specification did not match any packages)"
        status: pass
    human_judgment: false
  - id: D5
    description: "A failing `cargo pmcp validate deploy` still guarantees a failing `cargo pmcp deploy`: lint findings are warnings and never flip that contract (`validate.rs:36-38`)"
    requirement: "SC-3"
    verification:
      - kind: unit
        ref: "cargo-pmcp/src/commands/validate.rs#server_config_validate_tests::validate_deploy_wildcard_allow_still_fails_with_lint_findings_present"
        status: pass
      - kind: unit
        ref: "cargo-pmcp/src/commands/validate.rs#server_config_validate_tests::validate_deploy_warns_on_a_malformed_toolkit_config_and_still_exits_zero"
        status: pass
    human_judgment: false
  - id: D6
    description: "The new integration binary is REACHED by the quality gate and its absence is a failure"
    verification:
      - kind: other
        ref: "make test-cargo-pmcp-integration  (✓ validate_server_config passed 12 tests; leg total 104 -> 116)"
        status: pass
      - kind: other
        ref: "POSITIVE CONTROL: selector removed, REQUIRED entry kept -> leg exits 2 with \"required test binary 'validate_server_config' never RAN\""
        status: pass
    human_judgment: false

duration: 118min
completed: 2026-09-27
status: complete
---

# Phase 128 Plan 07: Pre-deploy input-validation review surface Summary

**`cargo pmcp validate config` gives a reviewer every uncapped string and every active `[server.validation]` opt-out from the SAME `ServerConfig::lint()` the running server reports at startup — and `validate deploy` now emits them too, as warnings that cannot change what its failure means.**

## Performance

- **Duration:** ~118 min
- **Started:** 2026-09-27T18:35Z (HEAD `7830f335`)
- **Completed:** 2026-09-27T19:33Z
- **Tasks:** 3 of 3
- **Files modified:** 4 (1 created, 3 modified)

## Accomplishments

- **A reviewer surface that cannot drift from the server.** `cargo pmcp validate config` parses a toolkit `config.toml`, runs `ServerConfig::validate()` (hard errors) and then `lint()` (warnings), and renders each finding. `validate.rs` holds **zero** rule literals in production code, so the CLI can only report what the toolkit decides — the Q5 route-C rejection is now mechanical rather than aspirational.
- **D-07's literal surface honoured without weakening it.** `validate deploy` discovers a `config.toml` beside `.pmcp/deploy.toml` and reports the same findings. Every branch of the new block is warnings-only, including a parse failure of the discovered document, so the documented guarantee at `validate.rs:36-38` is byte-for-byte intact. A test pins it with a wildcard-`Allow` IAM config AND lint findings present.
- **The gate actually runs the new coverage.** Both `Makefile` lists gained `validate_server_config`, and the guard was proven live by positive control, not assumed.
- **Plan-gate bugs 6, 7 and 8 of this phase found and corrected** (see Deviations) — three `--lib` verify filters that select ZERO tests and exit 0, and one acceptance criterion that was unsatisfiable as written.

## Task Commits

1. **Task 1: The dependency edge and `cargo pmcp validate config`** (TDD)
   - RED — `b9f688f3` (`test`): the edge, the `Config` variant, honest empty stubs, 10 unit tests with 5 failing on assertions for the planned behavior. `RED_EVIDENCE_OK`.
   - GREEN — `47cb6ab8` (`feat`): `render_config_lint_findings` projects `config.lint()`; `validate_server_config` discovers, parses, validates, renders. 10/10 green.
2. **Task 2: `validate deploy` emits the same findings — warnings only** (TDD)
   - RED — `c50387e5` (`test`): `DiscoveredConfigLint` three-state result, the renderer, the exit-code-contract comment at the call site, 4 new tests with 3 failing. `RED_EVIDENCE_OK`.
   - GREEN — `f791e761` (`feat`): real discovery; tests renamed `validate_deploy_*` so the leg filter covers them (`--bins validate_deploy` 4 -> 8).
3. **Task 3: Integration coverage and gate visibility** — `8f8228c3` (`test`), plus `93751156` (`style`, `cargo fmt --all` reach).

No REFACTOR commits: neither GREEN implementation had an obvious cleanup that a refactor commit would improve.

## Files Created/Modified

- `cargo-pmcp/Cargo.toml` — the first DIRECT `pmcp-server-toolkit` edge, with a `# Why:` comment recording Q5, that it is the first *direct* edge and not the first edge, why `default-features = false` is load-bearing, and why `http` is required from source evidence.
- `cargo-pmcp/src/commands/validate.rs` — `ValidateCommand::Config`; `resolve_server_config_path`, `load_server_config`, `render_config_lint_findings`, `emit_config_lint_findings`, `validate_server_config`; `DiscoveredConfigLint`, `discover_server_config_lint`, `emit_discovered_server_config_lint`; 14 unit tests.
- `cargo-pmcp/tests/validate_server_config.rs` — 12 end-to-end tests over the five fixture shapes, the `[backend]` parse regression, and all four discovery routes.
- `Makefile` — `:531` `--test validate_server_config`; `:540` the `REQUIRED_TEST_BINARIES` entry.

## Measurements the plan asked for

| Measurement | Value |
|---|---|
| Final toolkit feature list on the `cargo-pmcp` edge | `default-features = false, features = ["input-validation", "http"]`, `version = "0.1.0"` (caret; toolkit is at 0.1.3) |
| Why `http` | **Source evidence, not a probe**: `crates/pmcp-server-toolkit/src/config.rs:123` gates `ServerConfig.backend` behind `#[cfg(feature = "http")]` and `:101` carries `#[serde(deny_unknown_fields)]`, so a non-`http` toolkit rejects every `[backend]`-carrying config with `unknown field 'backend'`. Kept as a REGRESSION test (`backend_section_parses_rather_than_failing_as_an_unknown_field`), not a discovery step. |
| `[backend]`-section parse | PASSES. Asserted twice — integration (`backend_section_parses_rather_than_failing_as_an_unknown_field`, which also asserts stderr does NOT contain `unknown field`) and unit (`backend_section_parses`). |
| `cargo tree -p cargo-pmcp -e normal \| wc -l` | **1415 -> 1436 (+21 lines)** — the cost of `http`'s tree (reqwest/openapiv3/serde_yaml/base64/url were largely already unified; `pmcp/streamable-http` was already on at `Cargo.toml:68`). |
| `cargo tree -p cargo-pmcp -e normal -i swc_common` | `package ID specification 'swc_common' did not match any packages` — before AND after. No JS engine. |
| Which gate leg runs the new binary, and its count | `make test-cargo-pmcp-integration` -> `✓ validate_server_config passed 12 tests`; leg total **104 -> 116**. |
| `make test-cargo-pmcp` | **1463 -> 1477** (+14 unit tests: 10 from task 1, 4 from task 2). |
| `cargo-pmcp` `[package].version` | `0.24.3` before, `0.24.3` after — byte-identical (verified against `7830f335:cargo-pmcp/Cargo.toml`). |

## Regression evidence

| Gate | Result |
|---|---|
| `cargo nextest run --features "full" --no-fail-fast` | **3358 run, 3358 passed (1 leaky), 5 skipped** — exactly the dispatch baseline, zero failures, no drop |
| `make quality-gate` | exit 0 **with** the `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner |
| `cargo build --all-features` | exit 0, zero `error` lines |
| `pmat quality-gate --fail-on-violation --checks complexity` | `Quality Gate: PASSED`, total violations 0 |
| `make test-server-toolkit` | 393 (unchanged) |
| `make test-server-toolkit-code-mode` | 418 (unchanged) |
| `make test-code-mode` | 317 (unchanged) |
| `cargo test -p pmcp --features full --lib schema_validation` | 52 (unchanged) |
| `cargo test -p cargo-pmcp --lib validate` | 31 (unchanged — see deviation 1) |
| `cargo test -p pmcp-openapi-server` | 42 (unchanged) |

## Decisions Made

1. **Q5 as planned: Route B plus the literal D-07 surface.** `validate config` is the primary command; `validate deploy` also emits the findings as warnings. No rule is implemented twice.
2. **`http` from source, not from a probe** — as the plan mandated, and the reason is recorded in the `Cargo.toml` comment so a future reader cannot "simplify" the feature list back.
3. **The three-state `DiscoveredConfigLint`** rather than `Result<Vec<String>>`: a pure-IAM project (`Absent`) must stay silent, while a config that exists and reports nothing (`Findings([])`) earns a `✓` line. A two-state result forces the caller to pick one behaviour for both, and the silent one is the wrong default for an existing document.
4. **The `Unreadable` warning says what the plan's deferred review note asked for.** It states that the server itself would refuse to boot on the file, which is the asymmetry that makes the warning worth reading — without turning it into an error, which would widen `validate deploy`'s contract.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Three `--lib` verify filters select ZERO tests and exit 0; corrected to `--bins`**
- **Found during:** Task 1 (and again in Task 2)
- **Issue:** The plan's gates at `:215`, `:270` and `:272` are `cargo test -p cargo-pmcp --lib validate` / `--lib validate_deploy`. MEASURED before any edit: `--lib validate_deploy` = **`running 0 tests`, exit 0** (as the orchestrator flagged), and `--lib validate` = 31 tests that have **nothing to do with this file**. Root cause: `cargo-pmcp/src/commands/validate.rs` is **not in the lib target at all** — `cargo-pmcp/src/lib.rs` mounts only specific `#[path]` leaves (`workbook_explain`, `package_kind`, `package_artifact`, …) to keep `clap`/`GlobalFlags` out of the lib, so the whole `commands` tree compiles only into the **bin** target. Every test added to `validate.rs` is therefore invisible to `--lib`, forever. Taking `--lib validate` = 31 "unchanged" as a pass would have been a green reading of a gate that measures an unrelated 31 tests.
- **Fix:** Every filtered gate was re-run as `--bins … -- --test-threads=1` and the count read. `--bins server_config_validate` 0 -> 14. `--bins validate_deploy` 4 -> **8** — and to make the plan's own (corrected) filter genuinely cover the new work, Task 2's four tests were **renamed** `deploy_*` -> `validate_deploy_*`, so a future run of that gate cannot pass while the new tests are absent.
- **Files modified:** `cargo-pmcp/src/commands/validate.rs` (test names)
- **Verification:** `--bins validate_deploy` lists all 8 by name; `make test-cargo-pmcp` 1463 -> 1477
- **Committed in:** `f791e761`

**2. [Rule 3 - Blocking] Task 1's no-rule-copy acceptance criterion was unsatisfiable as written; re-scoped to production code**
- **Found during:** Task 1
- **Issue:** The criterion is `bash -o pipefail -c 'grep -v "^\s*//" validate.rs | grep -c "max_length"'` returning 0. Two independent defects. (a) `grep -v "^\s*//"` strips comment LINES but not a **test-fixture data line**: the `[server.validation] default_max_length = 0` opt-out fixture — which Task 1's own `<behavior>` row REQUIRES — is a TOML key in a string literal, so the scan returned **1**. This is exactly the Pitfall-5 class the dispatch warned about (plan 06's `allowReserved` gate was mutually unsatisfiable with its own mitigation for the same reason). (b) `grep -c` **exits 1 when the count is 0**, so under `-o pipefail` the passing outcome fails the command.
- **Fix:** The gate was re-scoped to what the criterion is actually protecting (T-128-32: no rule can be *enforced* twice) — the **production** portion of the file: `awk '/^#\[cfg\(test\)\]/{exit} {print}' | grep -v '^[[:space:]]*//' | grep -c 'max_length'`. Comments and test fixtures are both excluded because neither can enforce anything at runtime. **Measured: 0.** A **POSITIVE CONTROL** on the same scan returns **2** for `render_config_lint_findings`, so the scan is proven to be reading the file rather than silently matching nothing. `config.lint()` is called exactly once. Lowering the criterion to match reality was NOT done — the rescoped gate is strictly about enforcement, which is the stronger reading.
- **Files modified:** none (a gate correction)
- **Verification:** production-scope count 0; positive control 2
- **Committed in:** n/a (recorded here; the code satisfies both the old intent and the new gate)

**3. [Rule 3 - Blocking] Task 1's `cargo run -q -p cargo-pmcp --` gate cannot run: the package has three binaries**
- **Found during:** Task 1
- **Issue:** `cargo run -q -p cargo-pmcp -- pmcp validate config --help` exits non-zero with *"could not determine which binary to run … available binaries: capture_contract, cargo-pmcp, mock_test_binary"*. The gate would have failed for a reason unrelated to the subcommand.
- **Fix:** `--bin cargo-pmcp` added. The corrected command shows the new `--server` / `--config` flags, and `validate --help` lists `config  Validate a config-driven server's 'config.toml' — input-validation focus` along`workflows` and `deploy` — so the variant is wired into the dispatch, not merely added to the enum.
- **Files modified:** none (a gate correction)
- **Committed in:** n/a

**4. [Rule 3 - Blocking] The TDD RED-evidence checker cannot read cargo output**
- **Found during:** Task 1
- **Issue:** `gsd_run check tdd-red-evidence` parses **node TAP only** (`# tests N` / `# pass N` / `# fail N` + `ok N - name`). Handed a real cargo libtest run it reported `tests: 0` -> `INVALID_RED` / `zero_tests_discovered`, which would have blocked GREEN on a perfectly valid Rust RED phase. The first attempt also used the wrong record keys (`exit_code`/`failing_test` vs the required `exitCode`/`targetTest`/`output`).
- **Fix:** A small **lossless** translator (`cargo2tap.cjs`, scratchpad-only) maps each `test NAME ... ok|FAILED` line to one TAP line with the same identity and outcome, and computes the summary counts **from those lines** rather than copying the prose. Both RED phases then verified as **`RED_EVIDENCE_OK` / `target_test_failed`**: Task 1 — 10 tests, 5 pass, 5 fail; Task 2 — 14 tests, 11 pass, 3 fail. Each failure is an assertion on planned behavior, not a compile error, a zero-test discovery or a fixture crash.
- **Files modified:** none in-repo (the translator lives in the session scratchpad)
- **Committed in:** n/a

**5. [Rule 3 - Blocking] `cargo fmt -p cargo-pmcp` does not reach `tests/`**
- **Found during:** Task 3 / final gate
- **Issue:** `make quality-gate`'s `cargo fmt --all -- --check` flagged one call-site wrapping in the new integration binary that `cargo fmt -p cargo-pmcp` had not touched.
- **Fix:** `cargo fmt --all`; 12/12 re-verified green.
- **Committed in:** `93751156`

### Environment

**Untracked `cargo-pmcp/.pmcp/active-target` moved aside before any measurement**, per the recorded landmine and Task 3's own `<verify>` (which would otherwise fail on it). `git status --porcelain cargo-pmcp/.pmcp` is empty for the whole run, and it stayed empty after the integration binary ran. The file is preserved in the session scratchpad — it is the user's local state, not this plan's to delete.

---

**Total deviations:** 5 auto-fixed (5x Rule 3 - blocking). **Four of the five are verification-gate defects, not code defects** — three would have read GREEN while measuring nothing, which is the exact class this phase is repairing.
**Impact on plan:** No scope change. No new functionality beyond the plan. No version or pin moved.

## Issues Encountered

- **The `non_compiling_pattern` integration assertion was initially vacuous.** `stderr(contains("id"))` would pass on almost any message. Tightened to require the tool name (`lookup`), the key (`pattern`) and the parameter (`id`) — substance, not formatting. The real message is `[[tools]] 'lookup' has a declared parameter schema that does not compile at /properties/id/pattern: "([unclosed" is not a "regex"`.
- **A transient `variants 'Absent' and 'Unreadable' are never constructed` warning in Task 2's RED commit**, inherent to a Rust RED phase with an honest stub. Gone in GREEN (`f791e761`). The one remaining `never constructed` warning in that leg is pre-existing (`PayloadLibrary`, `src/pentest/payloads/mod.rs`) and out of scope.
- **Disk at 99% capacity** (18 GiB free on `/System/Volumes/Data`). No link failures occurred; recorded because it is the first thing to check if a later run fails at link time.

## Hand-offs to plan 11 (release ledger) — NOT satisfied here, per D-14

1. **An eighth `pmcp-server-toolkit` pin.** `cargo-pmcp/Cargo.toml` now carries `pmcp-server-toolkit = { version = "0.1.0", … }` directly. It moves on every future toolkit release.
2. **A ninth, pre-existing and still unrecorded pin.** `cargo-pmcp/Cargo.toml:75` `pmcp-workbook-compiler = "0.1.3"` is a SECOND `cargo-pmcp` line that moves on every toolkit release (the compiler pins the toolkit at `crates/pmcp-workbook-compiler/Cargo.toml:113`). Plan 11 Task 2 section (B) carries it.
3. **`cargo-pmcp` 0.24.3 -> 0.25.0.** A new subcommand is a feature, so a minor bump, taking D-13's "~11 crate versions" to **twelve**. This plan changed **nothing** in `[package].version`.
4. **CLAUDE.md item 15a records `cargo-pmcp` at 0.23.0; it is at 0.24.3.** Plan 11 corrects that ledger line.
5. **The ROADMAP/SC-3 wording deviation.** SC-3 and the ROADMAP name `cargo pmcp validate deploy` verbatim; the PRIMARY surface shipped here is `cargo pmcp validate config`, with `validate deploy` also emitting the findings. Plan 11 records this in the rollout note and amends the ROADMAP success criterion.
6. **Publish-order note (no action needed, recorded so it is not "fixed").** The new edge is a **normal** dependency, not a dev-dependency, and `pmcp-server-toolkit` (release.yml item 5) publishes well BEFORE `cargo-pmcp` (item 15a), so the CR-01 path-only constraint documented at `crates/pmcp-openapi-server/Cargo.toml:112-119` does **not** apply. `scripts/check-release-coverage.sh` passes (`all 25 publishable workspace members have a publish step`).

## Flagged assumption carried forward — NOT closed

**SC-3 · unclassified: the mixed-version case.** A config that lints clean under the toolkit `cargo-pmcp` was BUILT against may lint dirty under the toolkit the deployed server RUNS. This plan pins both surfaces to ONE implementation at build time, which makes them agree for a **same-version pair** and says nothing about a mixed pair. The plan explicitly says not to treat the pinning as a closure, and it is not treated as one. What the operator is owed (a toolkit-version banner in the lint output? a hard error on mismatch?) remains a product question for a human. Still open.

## Known Stubs

None. `render_config_lint_findings` and `validate_server_config` were stubs only inside the RED commits (`b9f688f3`, `c50387e5`) and are fully implemented in the paired GREEN commits.

## Threat Flags

None. The plan's `<threat_model>` covers the whole surface this plan touches (T-128-30 through T-128-34); no new network endpoint, auth path, file-access pattern or schema change at a trust boundary was introduced. The one new file-read is an operator-supplied `config.toml` that the toolkit already parses at boot, read here with the same parser.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- Plan 09 (startup log) can call `ServerConfig::validation_report()` knowing the CLI's projection of `lint()` is already proven end to end; the two surfaces should agree by construction.
- Plan 11 has the six hand-offs above, itemised, and must move **all** of them in its single D-14 commit.
- Nothing is blocked by this plan.

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-27*

## Self-Check: PASSED

All 5 named files exist on disk; all 6 task commits resolve in `git log --all`.
