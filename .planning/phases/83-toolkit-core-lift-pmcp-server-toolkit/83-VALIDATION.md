---
phase: 83
slug: toolkit-core-lift-pmcp-server-toolkit
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-17
---

# Phase 83 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Source: see `83-RESEARCH.md` §"Validation Architecture" for the full validation matrix the planner must apply across every PLAN.md.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (workspace) + `cargo nextest` (optional) + `cargo-fuzz` for libFuzzer targets |
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

This phase's plans (~8 PLAN.md files per RESEARCH §"Phase-Split Risk Assessment") will populate this matrix during planning. Filled rows are exemplars showing the shape; the planner MUST add one row per task with a real test command derived from `83-RESEARCH.md` §"Validation Architecture".

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 83-01-01 | 01 | 1 | TKIT-01 | — | Crate skeleton compiles in workspace | unit | `cargo build -p pmcp-server-toolkit` | ❌ W0 | ⬜ pending |
| 83-02-01 | 02 | 1 | TKIT-02 | T-83-secrets | `AuthProvider` / `SecretsProvider` traits lifted, secrets never logged | unit + doctest | `cargo test -p pmcp-server-toolkit auth secrets --doc` | ❌ W0 | ⬜ pending |
| 83-03-01 | 03 | 2 | TKIT-03 | — | `StaticResourceHandler` / `StaticPromptHandler` serve from in-memory map | unit | `cargo test -p pmcp-server-toolkit static_resources static_prompts` | ❌ W0 | ⬜ pending |
| 83-04-01 | 04 | 2 | TKIT-04 / TKIT-06 | T-83-token | `HmacTokenGenerator` mint/verify round-trip + TTL + secrecy-wrapped secret | unit + doctest | `cargo test -p pmcp-server-toolkit hmac --doc` | ❌ W0 | ⬜ pending |
| 83-05-01 | 05 | 2 | TKIT-05 / TEST-02 | — | `[[tools]]` config → `ToolInfo` with parameters + annotations + cost hint | unit + property + fixture | `cargo test -p pmcp-server-toolkit tool_synth && cargo test -p pmcp-server-toolkit --test fixtures` | ❌ W0 | ⬜ pending |
| 83-06-01 | 06 | 3 | TKIT-07 / TEST-03 | T-83-policy | `[code_mode]` config wires `CodeExecutor` with policy enforcement (allow_writes=false rejects INSERT) | integration | `cargo test -p pmcp-server-toolkit --test code_mode_policy` | ❌ W0 | ⬜ pending |
| 83-07-01 | 07 | 3 | TKIT-08 | — | Prompt-body assembler combines schema text + `[[database.tables]]` (uses `SqlConnector` stub) | unit | `cargo test -p pmcp-server-toolkit prompt_assembler` | ❌ W0 | ⬜ pending |
| 83-08-01 | 08 | 4 | TKIT-09 / TKIT-10 | — | Backend-core smoke test imports public API; migration guide shim diff matches export list | integration | `cargo test -p pmcp-server-toolkit --test backend_core_smoke` | ❌ W0 | ⬜ pending |
| 83-09-01 | (cross-cutting) | 4 | TEST-02 / TEST-03 | — | Fuzz target `pmcp_server_toolkit_config_parser` survives 60 s with no panics | fuzz | `cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60` | ❌ W0 | ⬜ pending |
| 83-10-01 | (cross-cutting) | 4 | TKIT-01..10 | — | Contract YAML entry validates against public API | contract | `pmat comply check` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-server-toolkit/Cargo.toml` — new crate manifest (workspace member, version 0.1.0, MIT-OR-Apache-2.0)
- [ ] `crates/pmcp-server-toolkit/src/lib.rs` — module skeleton (`auth`, `secrets`, `resources`, `prompts`, `hmac`, `tools`, `code_mode`, `config`, `prompt_assembler`, `connector`)
- [ ] `crates/pmcp-server-toolkit/tests/fixtures/` — copy `open-images/config.toml` shape (parse-target fixture) + `imdb` / `msr-vtt` variants
- [ ] `crates/pmcp-server-toolkit/tests/code_mode_policy.rs` — integration harness for policy enforcement
- [ ] `crates/pmcp-server-toolkit/tests/backend_core_smoke.rs` — substitute for cross-repo verification (per CONTEXT.md D-03)
- [ ] `fuzz/fuzz_targets/pmcp_server_toolkit_config_parser.rs` — new libFuzzer target (cannot reuse Phase 77's target — different schema per RESEARCH §"Pitfalls")
- [ ] `contracts/binding.yaml` — extend with toolkit public API entries (or new `contracts/toolkit-v1.yaml` per RESEARCH §"Open Questions" #1; recommendation: extend)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| pmcp-run sibling-repo migration applies cleanly | TKIT-09 | pmcp-run is an external sibling repo not in this workspace (CONTEXT.md D-03). The toolkit cannot apply the diff here. | Apply `pmcp-server-toolkit-migration.patch` to a checkout of `pmcp-run`; run that repo's existing tests; verify zero source diff in the three backend cores other than `Cargo.toml` dep lines. |
| Crates.io publish dry-run succeeds | TKIT-01 | Publishing reaches an external registry; only the release tag triggers it. | `cargo publish -p pmcp-server-toolkit --dry-run` locally on the release branch before tagging. |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (crate manifest, fixtures, fuzz target, smoke harness)
- [ ] No watch-mode flags in any command (CI must be deterministic)
- [ ] Feedback latency < 60 s for quick command
- [ ] Every public type has a doctest (per RESEARCH §"Validation Architecture")
- [ ] Property test: any valid config.toml `[[tools]]` entry produces ToolInfo with non-empty schema
- [ ] Contract YAML covers every public symbol the planner exposes
- [ ] `nyquist_compliant: true` set in frontmatter after planner populates the per-task matrix

**Approval:** pending
