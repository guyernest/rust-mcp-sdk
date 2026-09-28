---
phase: "128"
slug: "secure-by-default-input-validation-for-config-driven-servers"
# status lifecycle: draft (seeded by plan-phase) → validated (set by validate-phase §6)
# audit-milestone §5.5 distinguishes NOT-VALIDATED (draft) from PARTIAL (validated + nyquist_compliant: false) (#2117)
status: draft
nyquist_compliant: false
wave_0_complete: false
created: "2026-09-26"
---

# Phase 128 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Seeded from `128-RESEARCH.md` § Validation Architecture (lines 2280–2356).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `libtest` + `cargo nextest`; `proptest 1.7` (property), `libfuzzer-sys 0.4` (fuzz), `wiremock 0.6` + `tokio 1` (integration) |
| **Config file** | none — the Makefile *is* the config. Root `Makefile:244-253, 791-814, 917-920`; toolkit `Makefile:585-616` |
| **Quick run command** | core: `cargo test --lib --features "full" <filter>` · toolkit: `cargo test -p pmcp-server-toolkit --features http,input-validation --test <binary> -- --test-threads=1` · code-mode: `cargo test -p pmcp-code-mode --lib` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | quick ~10–60s per crate; `make quality-gate` several minutes |

⚠ `--test-threads=1` is mandatory per CLAUDE.md (race-condition prevention) and is already baked into `make test-server-toolkit`.

⚠ Prefix commands with `RUSTFLAGS=""` where the local environment sets it — `project_local_gate_blind_spots` records that a stray `RUSTFLAGS` diverges the local gate from CI.

---

## Sampling Rate

- **After every task commit:** the narrowest relevant command from the map below, plus `make fmt-check` and `make lint`. The pre-commit hook enforces the Toyota Way gate, so a commit that cannot pass `make quality-gate` cannot land — task granularity must respect that.
- **After every plan wave:** `make test-server-toolkit && make test-unit && cargo test -p pmcp-code-mode`, plus `make lint` and `./scripts/check-release-coverage.sh`.
- **Before `/gsd-verify-work`:** full `make quality-gate` green, plus a manual `cargo run --example s57_typed_tool_garde_validation` (CLAUDE.md's ALWAYS requirement — `make test-examples` only *builds*).
- **Max feedback latency:** ~60 seconds for the per-task command.

---

## Per-Task Verification Map

Task IDs are assigned when plans are written; this table carries the requirement→command
binding the planner must project onto tasks. `validate-phase` fills the Task ID / Plan / Wave
columns after execution.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | D1 / SC-1 | T-83-05-02 | Schema-violating args refused before any backend call; 0 upstream requests | integration | `cargo test -p pmcp-server-toolkit --features http,input-validation --test input_validation_acceptance -- --test-threads=1` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D1 | — | Missing `arguments` on a zero-param tool ACCEPTED as `{}` | integration | same binary, `-- arguments_missing_zero_param` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D1 | — | Missing `arguments` on a required-param tool REFUSED by `required` | integration | same binary, `-- arguments_missing_required` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D1 | T-83-05-02 | `additionalProperties:false` honoured, not re-added | unit | `cargo test --lib --features full schema_validation::` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D2 / SC-2 | — | `ParamDecl` round-trips `pattern`/`min_length`/`format`/`items`/`max_items` | unit | `cargo test -p pmcp-server-toolkit --lib config::` | ✅ extend | ⬜ pending |
| TBD | TBD | TBD | D2 / SC-2 | — | A non-compiling `pattern` fails **config** validation, not call time | unit | `cargo test -p pmcp-server-toolkit --lib config::validate_rejects_uncompilable_pattern` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D2 | — | `items` emitted in object form (array form fails the 2020-12 pin) | unit | `cargo test -p pmcp-server-toolkit --lib tools::build_param_property` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D3 / D-05 / D-06 | CR-02 | Path/query string over 256 refused; body string over 256 warns only | integration | `--test input_validation_acceptance -- position_scoped_cap` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D3 / SC-3 / D-07 | — | Uncapped body-position string surfaced by config lint | unit | `cargo test -p pmcp-server-toolkit --lib config::lint_` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D4(a) / SC-4 | CR-01 | `?`,`#`,`/`,`..` + percent-encoded refused — **Code Mode** (3 probes) | integration | `cargo test -p pmcp-server-toolkit --features openapi-code-mode --test http_executor -- --test-threads=1` | ✅ extend | ⬜ pending |
| TBD | TBD | TBD | D4(a) / SC-4 | CR-01 | Same — **curated single-call**, no JS engine (2 probes) | integration | `cargo test -p pmcp-server-toolkit --features http,input-validation --test curated_path_injection -- --test-threads=1` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D4(b) / D-10 | — | Spec `pattern` narrows on top of the floor; `^.*$` does not disable it | property | `--test path_placeholder_props -- --ignored property_` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D4(c) / D-08 | CR-01 | Placeholder cap holds even with `default_max_length = 0` | integration | `--test curated_path_injection -- cap_independent_of_d3` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D-09 | CR-01 | Every `HttpExecutor` receives an already-resolved path | unit | `cargo test -p pmcp-code-mode --lib executor::` | ✅ extend | ⬜ pending |
| TBD | TBD | TBD | D-09 / SC-7 | — | `PlanExecutor` error wrap no longer echoes `resolved_path` | unit | `cargo test -p pmcp-code-mode --lib executor::error_does_not_echo_path` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | E1 / SC-5 / D-12 | — | `RequestPolicy` refuses, 0 upstream requests, runs before auth is applied | integration | `--test request_policy -- --test-threads=1` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | E2 / SC-5 | — | Per-tool `ArgumentValidator` runs after D1 | integration | `--test request_policy -- argument_validator` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | E3 / SC-5 | — | `#[garde(length(max = 10))]`, 11 chars → `Error::Validation`, value not echoed | unit | `cargo test --lib --features full typed_tool::garde` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | SC-6 | T-83-05-02 / T-90-03-01 / T-90-05-03 | Three `tools.rs` comments corrected; no toolkit comment claims an unimplemented mitigation | manual + grep | see Manual-Only Verifications below | ✅ | ⬜ pending |
| TBD | TBD | TBD | SC-7 | — | No refusal message contains any byte of the rejected value or key | property + fuzz | `--ignored property_refusal_never_echoes_input` and `cargo fuzz run fuzz_input_schema_enforcement` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | SC-8 | — | Gate green; fuzz/property/unit/example all present | gate | `make quality-gate` | ✅ | ⬜ pending |
| TBD | TBD | TBD | D-13 / D-14 | — | Publish ledger stays coherent across the ~11-crate bump | gate | `./scripts/check-release-coverage.sh` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs` — gated `#![cfg(all(feature = "http", feature = "input-validation"))]`; covers D1's six rows
- [ ] `crates/pmcp-server-toolkit/tests/curated_path_injection.rs` — gated **without** `openapi-code-mode`; the two JS-engine-free CR-01 rows and D-08
- [ ] `crates/pmcp-server-toolkit/tests/request_policy.rs` — E1 + E2
- [ ] `crates/pmcp-server-toolkit/tests/path_placeholder_props.rs` — proptest arms
- [ ] `tests/typed_tool_garde.rs` (root) — E3
- [ ] `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` + its `[[bin]]` stanza in `fuzz/Cargo.toml`
- [ ] **Makefile edits — the gap that makes every row above real:**
  - `Makefile:587` — add `input-validation` to `test-server-toolkit`'s `--features http` (without it a new gated test file compiles to `running 0 tests` and the gate goes green on zero coverage; `Makefile:576-583` documents this exact measurement)
  - `Makefile:596` — add the new binaries to `REQUIRED_TEST_BINARIES`
  - add a `test-code-mode` leg — `pmcp-code-mode` has **no** `quality-gate` leg today (`test-all` at `Makefile:1481` omits it), so D-09's tests would otherwise be gate-invisible
- [ ] **Property-test naming:** every new `property_*` fn must carry `#[ignore = "property arm — selected by \`make test-property\` (--ignored property_)"]` verbatim (pattern at `tests/log_emitter.rs:169`), or `make test-property` selects nothing. `tests/property_tests.rs` has 19 property fns and zero `#[ignore]`, so today that leg measures 2 tests.
- [ ] Framework install: none required.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| No remaining toolkit comment claims an unimplemented mitigation | SC-6 | A comment's truth is not machine-checkable — a grep can find candidate phrasings but cannot decide whether the claim matches the code | `grep -rn "enforced upstream\|schema-validated\|rejected by pmcp" crates/pmcp-server-toolkit/src/`, then read each hit and confirm the named enforcement exists and is covered by a test that fails when the enforcement is removed |
| Example demonstrates the feature end to end | SC-8 (CLAUDE.md ALWAYS) | `make test-examples` only *builds* examples; it never runs them | `cargo run --example s57_typed_tool_garde_validation` and read the output |
| Any new `T-*` threat comment names its enforcing function | SC-6 / threat-model discipline | The discipline rule is about provenance, not syntax | For each new `T-*`, confirm it names the enforcing function and points at an acceptance row that fails when the enforcement is removed |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s for the per-task command
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
