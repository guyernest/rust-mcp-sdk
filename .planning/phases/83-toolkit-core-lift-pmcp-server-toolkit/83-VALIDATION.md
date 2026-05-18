---
phase: 83
slug: toolkit-core-lift-pmcp-server-toolkit
status: revised-from-reviews
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-17
revised: 2026-05-18
revision_driver: 83-REVIEWS.md (Gemini + Codex)
---

# Phase 83 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: see `83-RESEARCH.md` §"Validation Architecture" for the full validation matrix the planner must apply across every PLAN.md.
> Revised 2026-05-18 to add review-driven rows (R1–R10 from 83-REVIEWS.md).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (workspace) + `cargo nextest` (optional) + `cargo-fuzz` for libFuzzer targets + `trybuild` for compile-fail tests (review R5) |
| **Config file** | `crates/pmcp-server-toolkit/Cargo.toml` + `Cargo.toml` workspace, `fuzz/Cargo.toml` (root) |
| **Quick run command** | `cargo test -p pmcp-server-toolkit --lib --bins` |
| **Full suite command** | `make quality-gate` (fmt --check, clippy pedantic+nursery `-D warnings`, build, test, audit) |
| **Estimated runtime** | quick ~30 s, full ~5–8 min (workspace clippy is the long pole) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-server-toolkit --lib`
- **After every plan wave:** Run `cargo test -p pmcp-server-toolkit && cargo clippy -p pmcp-server-toolkit --all-targets -- -D warnings`
- **Before `/gsd:verify-work`:** `make quality-gate` must be green workspace-wide
- **Max feedback latency:** 60 s (quick), 8 min (full)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 83-01-01 | 01 | 1 | TKIT-01 | — | Crate skeleton compiles in workspace | unit | `cargo build -p pmcp-server-toolkit` | ❌ W0 | ⬜ pending |
| 83-02-01 | 02 | 2 | TKIT-02 | T-83-02-01 | `AuthProvider` lift consumes pmcp trait; bearer-token validated; no secrets logged | unit + doctest | `cargo test -p pmcp-server-toolkit --lib auth:: && cargo test --doc -p pmcp-server-toolkit auth` | ❌ W0 | ⬜ pending |
| 83-02-02 | 02 | 2 | TKIT-03 / TEST-03 | T-83-02-02 | `SecretsProvider::get` returns toolkit-owned `SecretValue`; **R5** trybuild compile-fail tests prove `SecretValue` lacks Debug/Clone/Serialize | unit + doctest + trybuild | `cargo test -p pmcp-server-toolkit --test trybuild` | ❌ W0 | ⬜ pending **R5/R6 ADDED** |
| 83-02-03 | 02 | 2 | TKIT-02 / TKIT-03 | — | **R3** crate-root re-exports compile (`AuthProvider`, `StaticAuthProvider`, `SecretsProvider`, `SecretValue`, `EnvSecrets`) | unit | `cargo test -p pmcp-server-toolkit --lib root_reexport_smoke` | ❌ W0 | ⬜ pending **R3 ADDED** |
| 83-02-04 | 02 | 2 | TKIT-03 | T-83-02-06 | **R6** `cargo build --no-default-features` succeeds — `SecretValue`/`SecretsProvider` are feature-independent | build | `cargo build -p pmcp-server-toolkit --no-default-features` | ❌ W0 | ⬜ pending **R6 ADDED** |
| 83-03-01 | 03 | 2 | TKIT-04 / TKIT-05 | — | `StaticResourceHandler` / `StaticPromptHandler` serve from `IndexMap`-backed in-memory map | unit | `cargo test -p pmcp-server-toolkit --lib resources prompts` | ❌ W0 | ⬜ pending |
| 83-04-01 | 04 | 2 | TKIT-01 | T-83-04-02 | `ServerConfig::from_toml` strict-parse with `deny_unknown_fields`; rejects typos at parse time | unit + doctest | `cargo test -p pmcp-server-toolkit --lib config:: && cargo test --doc -p pmcp-server-toolkit config` | ❌ W0 | ⬜ pending |
| 83-04-02 | 04 | 2 | TKIT-01 | T-83-04-06 | **R8** `ServerConfig::validate()` rejects empty `server.name`, `server.version`, tool names, table names | unit | `cargo test -p pmcp-server-toolkit --lib config::tests::validate_` | ❌ W0 | ⬜ pending **R8 ADDED** |
| 83-04-03 | 04 | 2 | TKIT-01 | T-83-04-03 | All three reference fixtures parse + validate (REF-01 superset + R8) | integration | `cargo test -p pmcp-server-toolkit --test reference_configs` | ❌ W0 | ⬜ pending |
| 83-05-01 | 05 | 3 | TKIT-07 / TEST-02 | — | `[[tools]]` config → `ToolInfo` with parameters + annotations; property test covers N entries → N tuples | unit + property + fixture | `cargo test -p pmcp-server-toolkit tool_synth && cargo test -p pmcp-server-toolkit --test tool_synthesis_props` | ❌ W0 | ⬜ pending |
| 83-06-00 | 06 | 3 | TKIT-09 | — | **R1** preflight artifact `CODE_MODE_API_NOTES.md` captures exact pmcp-code-mode API signatures BEFORE wiring code is written | doc-existence | `test -f .planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/CODE_MODE_API_NOTES.md && grep -q 'constructor signature' .planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/CODE_MODE_API_NOTES.md` | ❌ W0 | ⬜ pending **R1 ADDED** |
| 83-06-01 | 06 | 3 | TKIT-06 / TKIT-09 | T-83-06-01 | `[code_mode] allow_writes=false` rejects INSERT through synthesized validator (no `todo!()`) | integration | `cargo test -p pmcp-server-toolkit --test code_mode_wiring --features code-mode` | ❌ W0 | ⬜ pending |
| 83-06-02 | 06 | 3 | TKIT-09 | T-83-06-02 | **R9** inline `token_secret` literal rejected unless `allow_inline_token_secret_for_dev=true` | unit | `cargo test -p pmcp-server-toolkit --features code-mode --lib code_mode::tests::resolve_token_secret_inline_without_dev_flag_rejected` | ❌ W0 | ⬜ pending **R9 ADDED** |
| 83-07-01 | 07 | 4 | TKIT-10 / TEST-02 | T-83-07-05 | **R2** `SqlConnector` trait has EXACTLY 2 methods (`dialect`, `schema_text`); `execute()` + placeholder translation deferred to Phase 84 | unit | `grep -c 'async fn ' crates/pmcp-server-toolkit/src/sql/mod.rs` (must equal 1) + `! grep -q 'fn execute' crates/pmcp-server-toolkit/src/sql/mod.rs` + `! grep -q 'pub fn translate_placeholders' crates/pmcp-server-toolkit/src/sql/mod.rs` | ❌ W0 | ⬜ pending **R2 ADDED** |
| 83-07-02 | 07 | 4 | TKIT-10 | — | `assemble_code_mode_prompt` combines `schema_text()` + curated descriptions; omits curated section when tables empty | unit | `cargo test -p pmcp-server-toolkit --features code-mode --lib code_mode::tkit10_tests` | ❌ W0 | ⬜ pending |
| 83-08-01 | 08 | 5 | TKIT-08 | T-83-08-04 | Backend-core smoke test builds server from open-images fixture using every toolkit surface | integration | `cargo test -p pmcp-server-toolkit --test backend_core_smoke --features code-mode` | ❌ W0 | ⬜ pending |
| 83-08-02 | 08 | 5 | TKIT-08 | T-83-08-06 | **R3** `backend_core_minimum_imports_compile` proves every public symbol resolves at crate root | compile-only | `cargo test -p pmcp-server-toolkit --test backend_core_smoke --features code-mode backend_core_minimum_imports_compile` | ❌ W0 | ⬜ pending **R3 ADDED** |
| 83-08-03 | 08 | 5 | TKIT-08 / TEST-03 | — | **R7** `try_tools_from_config` + `try_code_mode_from_config` shipped alongside panicking variants; both doctested | doctest | `cargo test --doc -p pmcp-server-toolkit builder_ext` | ❌ W0 | ⬜ pending **R7 ADDED** |
| 83-08-04 | 08 | 5 | TKIT-08 | T-83-08-06 | **R3** example `e01_toolkit_minimal.rs` imports SOLELY from crate root — no `pmcp_server_toolkit::auth::*`, `::config::*`, etc. | grep | `! grep -E "pmcp_server_toolkit::(auth\|config\|resources\|prompts\|tools\|sql\|secrets)::" crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` | ❌ W0 | ⬜ pending **R3 ADDED** |
| 83-09-01 | 09 | 6 | TEST-02 | T-83-09-01 | Fuzz target `pmcp_server_toolkit_config_parser` survives 60 s with no panics | fuzz | `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` | ❌ W0 | ⬜ pending |
| 83-09-02 | 09 | 6 | TKIT-01..10 | T-83-09-03 | **R3** Contract YAML entries use crate-root `module_path` form; ≥10 toolkit rows | contract | `pmat comply check` + `[ $(grep -c "module_path: pmcp_server_toolkit" contracts/binding.yaml) -ge 10 ]` | ❌ W0 | ⬜ pending **R3 ADDED** |
| 83-09-03 | 09 | 6 | TKIT-08 | T-83-09-06 | **R10** Shim apply instructions honor `$PMCP_RUN_PATH` env-var override | grep | `grep -q "PMCP_RUN_PATH" .planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/shim-pmcp-run-shared.md` | ❌ W0 | ⬜ pending **R10 ADDED** |
| 83-09-04 | 09 | 6 | TKIT-01 | T-83-09-02 | `cargo publish --dry-run -p pmcp-server-toolkit --allow-dirty` succeeds; tarball excludes `.planning`/`.pmat`/`tests`/`fuzz` | publish-gate | `cargo package --list -p pmcp-server-toolkit \| ! grep -qE "^(\\.planning\|\\.pmat\|tests\|fuzz)/" && cargo publish --dry-run -p pmcp-server-toolkit --allow-dirty` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-server-toolkit/Cargo.toml` — new crate manifest (workspace member, version 0.1.0, MIT-OR-Apache-2.0); `trybuild` dev-dep added (review R5)
- [ ] `crates/pmcp-server-toolkit/src/lib.rs` — module skeleton + crate-root re-exports per D-15 (review R3)
- [ ] `crates/pmcp-server-toolkit/tests/fixtures/` — three reference config snapshots
- [ ] `crates/pmcp-server-toolkit/tests/compile_fail/` (review R5) — 3 trybuild compile-fail sources for `SecretValue`'s negative trait invariant
- [ ] `crates/pmcp-server-toolkit/tests/trybuild.rs` — trybuild harness
- [ ] `crates/pmcp-server-toolkit/tests/code_mode_wiring.rs` — integration harness for policy enforcement (renamed from `code_mode_policy.rs` per Plan 06 layout)
- [ ] `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` — substitute for cross-repo verification (per CONTEXT.md D-03)
- [ ] `crates/pmcp-server-toolkit/tests/reference_configs.rs` — REF-01 superset integration test
- [ ] `fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` — new libFuzzer target
- [ ] `contracts/binding.yaml` — extend with toolkit public API entries using crate-root `module_path` form (review R3)
- [ ] `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/CODE_MODE_API_NOTES.md` (review R1) — pmcp-code-mode preflight notes

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| pmcp-run sibling-repo migration applies cleanly | TKIT-08 | pmcp-run is an external sibling repo (CONTEXT.md D-03 / review R10). The toolkit cannot apply the diff here. | `export PMCP_RUN_PATH=<path>`; follow `shim-pmcp-run-shared.md` apply instructions; verify zero source diff in the three backend cores other than `Cargo.toml` dep lines. |
| Crates.io publish (real, not dry-run) | TKIT-01 | Publishing reaches an external registry; only the release tag triggers it. | `cargo publish -p pmcp-server-toolkit` on the release branch after CI gates pass. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (crate manifest, fixtures, fuzz target, smoke harness, CODE_MODE_API_NOTES.md per R1, trybuild scaffolding per R5)
- [ ] No watch-mode flags in any command (CI must be deterministic)
- [ ] Feedback latency < 60 s for quick command
- [ ] Every public type has a doctest (per RESEARCH §"Validation Architecture")
- [ ] Property test: any valid config.toml `[[tools]]` entry produces ToolInfo with non-empty schema
- [ ] Contract YAML covers every public symbol the planner exposes — crate-root `module_path` form per R3
- [ ] Review-driven enforcements verified: R1 (preflight), R2 (minimized trait), R3 (crate-root re-exports), R5 (trybuild), R6 (no-default-features), R7 (try_*), R8 (validate), R9 (env-only token_secret), R10 (`$PMCP_RUN_PATH`)
- [x] **R11 DEFERRED** (Codex OPTIONAL recommendation — `mcp-server-common-shim` fixture sub-crate): Plan 08's `backend_core_minimum_imports_compile` test (compile-only re-import of every public toolkit symbol from the crate root) plus `backend_core_construction_surface_smoke` (live construction across every public handler/config surface) together exercise the same D-03 import-surface invariant the shim crate would have proved, at lower coordination cost. Revisit if Plan 09's operator handoff to `pmcp-run` surfaces import-shape gaps.
- [ ] `nyquist_compliant: true` set in frontmatter after planner populates the per-task matrix

**Approval:** pending operator review (revised 2026-05-18 from 83-REVIEWS.md)
