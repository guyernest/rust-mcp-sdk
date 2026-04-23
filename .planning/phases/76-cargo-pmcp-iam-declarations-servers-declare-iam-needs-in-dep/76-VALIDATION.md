---
phase: 76
slug: cargo-pmcp-iam-declarations-servers-declare-iam-needs-in-dep
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-22
---

# Phase 76 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Observable correctness signals are enumerated in `76-RESEARCH.md` → *Validation Architecture*.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (unit + integration) + `proptest` (property) + `cargo-fuzz` with `libfuzzer-sys` (fuzz) |
| **Config file** | `cargo-pmcp/Cargo.toml` + `cargo-pmcp/fuzz/Cargo.toml` (both exist today) |
| **Quick run command** | `cargo test -p cargo-pmcp iam` |
| **Full suite command** | `make quality-gate` (CI parity — fmt --all, clippy pedantic+nursery, build, test, audit, doctests) |
| **Estimated runtime** | ~30s quick; ~5-8 min for `make quality-gate` |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p cargo-pmcp iam` (the quick subset)
- **After every plan wave:** Run `cargo test -p cargo-pmcp` (full crate tests)
- **Before `/gsd-verify-work`:** `make quality-gate` must be green end-to-end
- **Max feedback latency:** 30 seconds for per-task; 8 minutes for full gate

---

## Per-Task Verification Map

> Filled by planner. Each plan task MUST map to one row.
> Column semantics: `File Exists` = ✅ if the test file/infrastructure already exists in-repo; ❌ W0 if Wave 0 must create it.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 76-01-01 | 01 (role ARN export) | 1 | PART-1 | — | Stable CFN export unblocks bolt-on stacks without name guessing | integration (CDK synth → JSON grep) | `cargo test -p cargo-pmcp test_stack_ts_emits_mcp_role_arn_export` | ❌ W0 | ⬜ pending |
| 76-02-01 | 02 (IamConfig schema) | 2 | PART-2 schema | — | TOML parses cleanly, default = empty (backward compat) | unit (serde round-trip) | `cargo test -p cargo-pmcp iam_config` | ✅ | ⬜ pending |
| 76-03-01 | 03 (translation rules) | 3 | PART-2 tables | T-76-01 (no silent privilege expansion) | read/write/readwrite emit exactly the CR-specified action lists | property (proptest) | `cargo test -p cargo-pmcp iam_translation_props` | ❌ W0 (new prop harness) | ⬜ pending |
| 76-03-02 | 03 (translation rules) | 3 | PART-2 buckets | T-76-01 | S3 object-level ARN only; bucket-level requires `[[iam.statements]]` | unit | `cargo test -p cargo-pmcp iam_bucket_translation` | ✅ | ⬜ pending |
| 76-04-01 | 04 (validator + footgun) | 4 | PART-2 validation | T-76-02 (wildcard escalation) | Allow/`*:*`/`*` hard-rejected; other rules per CR | unit | `cargo test -p cargo-pmcp iam_validate` | ✅ | ⬜ pending |
| 76-04-02 | 04 (validator) | 4 | PART-2 validation | T-76-02 | `cargo pmcp deploy` exits non-zero on invalid config before touching AWS | integration | `cargo test -p cargo-pmcp deploy_validate_gate` | ❌ W0 | ⬜ pending |
| 76-05-01 | 05 (fuzz + example + docs) | 5 | PART-2 robustness | T-76-03 (parser DoS / crash) | IamConfig parser does not panic on adversarial TOML | fuzz | `cargo +nightly fuzz run fuzz_iam_config -- -max_total_time=60` | ❌ W0 (new target) | ⬜ pending |
| 76-05-02 | 05 (example) | 5 | PART-2 DX | — | cost-coach-shaped example compiles and `cargo pmcp deploy init --dry-run` emits expected stack.ts | integration / example | `cargo run --example deploy_with_iam --manifest-path cargo-pmcp/Cargo.toml` | ❌ W0 | ⬜ pending |
| 76-05-03 | 05 (backward compat) | 5 | D-05 | — | stack.ts emitted from a no-`[iam]` config is byte-identical to pre-phase output (minus the new CfnOutput) | integration (golden file) | `cargo test -p cargo-pmcp test_backward_compat_no_iam_block` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

> **Planner note:** the table above is a reference — if the final PLAN.md structure differs (e.g., plans split differently), re-derive rows 1:1 against the actual task IDs. Do NOT lose any of the 9 signals.

---

## Wave 0 Requirements

Tests and harnesses that do not exist today and MUST be created before/during the first wave that needs them:

- [ ] `cargo-pmcp/tests/iam_stack_ts_integration.rs` — renders `create_stack_ts` for synthetic `DeployConfig` inputs, greps the emitted TypeScript string for expected PolicyStatement shapes and the McpRoleArn CfnOutput
- [ ] `cargo-pmcp/tests/iam_translation_props.rs` — proptest strategies for valid `IamConfig` (tables / buckets / statements) + property assertions on the action-set translation and resource-ARN construction
- [ ] `cargo-pmcp/tests/iam_validate.rs` — one test per validation rule in the CR (hard error + each warning class)
- [ ] `cargo-pmcp/tests/deploy_validate_gate.rs` — integration test that invokes `ValidateCommand::Deploy` on golden `.pmcp/deploy.toml` fixtures (one pass, one fail) and asserts exit behavior
- [ ] `cargo-pmcp/tests/backward_compat_stack_ts.rs` — golden-file test pinning the no-`[iam]` stack.ts output so regressions are loud (planner decides: snapshot via `insta` crate, or hand-rolled string compare)
- [ ] `cargo-pmcp/fuzz/fuzz_targets/fuzz_iam_config.rs` — new libfuzzer target for `IamConfig::from_str`/`toml::from_str` (template: existing `fuzz_config_parse.rs`)
- [ ] `cargo-pmcp/examples/deploy_with_iam.rs` — cost-coach-shaped example demonstrating `[iam.tables]` + `[iam.buckets]` + `[iam.statements]`; compiles and runs via `cargo run --example`

*Infrastructure already present (no setup needed):* `proptest` in dev-deps, `libfuzzer-sys` + existing fuzz targets in `cargo-pmcp/fuzz/`, `regex` in main deps (no new ARN parser dependency required).

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Round-trip: emitted CFN template imports into a real CDK bolt-on stack via `Fn::ImportValue` and `grantReadWriteData` succeeds against a real DynamoDB table | PART-1 end-to-end | Requires a live AWS account; not suitable for CI | Deploy a test server with `cargo pmcp deploy --target pmcp-run`, then deploy a trivial bolt-on CDK stack that imports `pmcp-${serverName}-McpRoleArn` and grants RW on a table; observe the bolt-on `grantReadWriteData` call succeeds and the Lambda can write to the table |
| Cost-coach migration: replace the env-var-based `MCP_FUNCTION_ROLE_NAME` lookup with `Fn.importValue` in cost-coach's bolt-on stack and confirm IAM grants persist across a platform-stack redeploy | PART-1 real-world unblock | Live integration with cost-coach prod; cannot CI-run | (Post-merge task for cost-coach team, tracked separately — not blocking this phase's verification) |

---

## Threat Model References

| Threat ID | Description | Mitigation |
|-----------|-------------|------------|
| T-76-01 | Silent privilege expansion — a future change to the action-set lookup maps could grant more AWS actions than the CR specifies without authors noticing | Property tests lock the exact action-set output per `read`/`write`/`readwrite` input; translation tests are golden on the full action list |
| T-76-02 | Wildcard escalation footgun — operator accidentally writes `[[iam.statements]]` with `effect=Allow` + `actions=["*"]` + `resources=["*"]` and ships it | Hard-error validation at `cargo pmcp validate` AND at `cargo pmcp deploy` entry points; refuses to synth or deploy |
| T-76-03 | Parser DoS — adversarial `.pmcp/deploy.toml` crafted to panic or hang the CLI | libfuzzer target exercises `IamConfig::from_str` / full `DeployConfig::load` path; 60-second fuzz run in quality gate |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (7 items above)
- [ ] No watch-mode flags in any test command
- [ ] Feedback latency < 30s for per-task quick command
- [ ] `nyquist_compliant: true` set in frontmatter after planner fills the per-task map

**Approval:** pending (planner to populate per-task map, then executor-time; operator signs after full gate green)
