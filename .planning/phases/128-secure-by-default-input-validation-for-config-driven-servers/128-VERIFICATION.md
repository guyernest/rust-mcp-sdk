---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
verified: 2026-09-28T00:00:00Z
status: human_needed
score: 8/8 must-haves verified
covered_files:
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-01-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-01-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-02-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-02-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-03-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-03-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-04-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-04-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-05-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-05-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-06-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-06-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-07-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-07-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-08-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-08-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-09-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-09-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-10-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-10-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-11-PLAN.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-11-SUMMARY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-CHANGE-REQUEST.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-CONTEXT.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-REVIEW.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/deferred-items.md"
  - "Makefile"
  - "cargo-pmcp/src/commands/validate.rs"
  - "crates/pmcp-code-mode/src/executor.rs"
  - "crates/pmcp-code-mode/src/lib.rs"
  - "crates/pmcp-server-toolkit/examples/e05_input_validation.rs"
  - "crates/pmcp-server-toolkit/src/builder_ext.rs"
  - "crates/pmcp-server-toolkit/src/code_mode.rs"
  - "crates/pmcp-server-toolkit/src/config.rs"
  - "crates/pmcp-server-toolkit/src/http/client.rs"
  - "crates/pmcp-server-toolkit/src/policy.rs"
  - "crates/pmcp-server-toolkit/src/tools.rs"
  - "crates/pmcp-server-toolkit/tests/curated_path_injection.rs"
  - "crates/pmcp-server-toolkit/tests/http_executor.rs"
  - "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs"
  - "crates/pmcp-server-toolkit/tests/path_placeholder_props.rs"
  - "examples/s57_typed_tool_garde_validation.rs"
  - "fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs"
  - "fuzz/fuzz_targets/fuzz_placeholder_pattern_redos.rs"
  - "src/server/schema_validation.rs"
  - "src/server/typed_tool.rs"
covered_digest: "v1:sha256:5bfc09ec55a148f06aa2f3ecd7dab129e2cd54aab10142450b30627fce0444f8"
behavior_unverified: 0
overrides_applied: 0
human_verification:
  - test: "Run `/gsd-secure-phase 128` to produce 128-SECURITY.md, re-checking the Phase 83 / Phase 90 threat sign-offs for the same documented-but-absent class this phase closed, per the P0 sub-goal's own instruction (\"the Phase 83 / Phase 90 threat sign-offs are re-checked for the same class\")."
    expected: "A 128-SECURITY.md exists recording threats_open: 0 (or an explicit, accepted residual list), confirming no other Phase 83/90 threat ID still asserts a mitigation the code does not implement."
    why_human: "This is a dedicated capability-hook workflow (security review) outside this verifier's scope; the task brief explicitly says to note it as outstanding rather than run it."
  - test: "Decide whether WR-04 (crates/pmcp-code-mode/src/executor.rs:3509-3541, resolve_path's PathPart::Variable arm) is an acceptable residual against SC-7's \"never an attacker-supplied key\" wording, for the generic (non-`[[tools]] script`) `execute_code` tool where the JS variable identifier is caller-authored."
    expected: "Either an accepted-residual note (matching the `garde` bare-identifier precedent already documented in `render_garde_refusal`) or a follow-up fix routing this arm through a fixed positional descriptor like the sibling `PathPart::Expression` arm already does."
    why_human: "The code review (128-REVIEW.md WR-04) already classified this as low-impact (identifiers cannot carry whitespace, and the attacker already knows their own chosen name) and left it open by decision, not oversight. Whether that residual is acceptable for a Toyota-Way zero-tolerance-for-defects project is a human policy call, not something this verifier can resolve unilaterally."
---

# Phase 128: Secure-by-default input validation for config-driven servers Verification Report

**Phase Goal:** A config-driven server enforces the input contract it already publishes — D1-D4 /
E1-E3 from `128-CHANGE-REQUEST.md`, plus the P0 sub-goal correcting three false security claims in
`crates/pmcp-server-toolkit/src/tools.rs`.

**Verified:** 2026-09-28
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (Success Criteria SC-1..SC-8)

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| SC-1 | A config-declared tool refuses schema-violating arguments before any backend call, zero upstream requests on refusal, gated on `input-validation` not `openapi-code-mode` | ✓ VERIFIED | `enforce_input_schema`/`ValidatingToolHandler` (`tools.rs:210,263-420,941,966`) wrap all 3 handler push sites; `input-validation = ["pmcp/schema-validation"]` (`Cargo.toml:127`), independent of `openapi-code-mode`; `cargo test -p pmcp-server-toolkit --test input_validation_acceptance` = 7/7 passed, including `..._refuses_undeclared_argument_without_contacting_upstream` asserting zero wiremock requests |
| SC-2 | `ParamDecl` accepts `pattern`, `min_length`, `format`, `items`/`max_items`; non-compiling `pattern` fails config validation, not call time | ✓ VERIFIED | Fields present `config.rs:1874-1943`; `check_tool_input_schema_compiles` (`config.rs:885-896`) runs at `ServerConfig::validate()` time via `check_input_schema_compiles`, with a `not(feature)` warn-once fallback |
| SC-3 | Uncapped string surfaced by `ServerConfig::lint()` AND `cargo pmcp validate config`, `validate deploy` emits as warnings, banner states toolkit version, position-scoped 256 default, `default_max_length = 0` opt-out | ✓ VERIFIED | `lint()` (`config.rs:352`), `ValidateCommand::Config` subcommand (`cargo-pmcp/src/commands/validate.rs:45-69`), `toolkit_lint_banner()` names `pmcp_server_toolkit::VERSION` and states version-scoping explicitly (`validate.rs:760-767`, asserted by `toolkit_lint_banner_names_the_linked_toolkit_version`), `default_cap_applies` restricted to `Path`/`Query` position (`config.rs:497-536`), `DEFAULT_MAX_LENGTH: u64 = 256` (`config.rs:1069`) |
| SC-4 | Placeholder carrying `?`/`#`/`/`/`..`/percent-encoded refused on BOTH surfaces; all 5 CR-01 probes pass | ✓ VERIFIED | Curated: `check_placeholder_value`→`validate_path_placeholder`, `check_composed_path`→`validate_resolved_path` (`http/client.rs:527-578`); Code Mode: `pmcp_code_mode::{validate_path_placeholder,validate_resolved_path}` re-exported from core and called in `executor.rs:2516-2521,3010,2939` and layer-1 `resolve_path`; `cargo test -p pmcp-server-toolkit --test curated_path_injection` = 6/6, `--test http_executor --features input-validation,openapi-code-mode` = 13/13 (includes named CR-01 probe tests at lines 258-342) |
| SC-5 | `RequestPolicy` (E1) + `ArgumentValidator` (E2) registerable on server builder; `garde` runs on `TypedTool<T> where T: garde::Validate` (E3) | ✓ VERIFIED | `RequestPolicy`/`ArgumentValidator` traits + `ToolkitHooks::with_request_policy`/`with_argument_validator` (`policy.rs:241,339,451,466,480`), consumed by `ServerBuilderExt` on `pmcp::ServerBuilder` (`builder_ext.rs`); `TypedTool::new_validated` requires `T: garde::Validate<Context=()>` (`typed_tool.rs:117-194`) — retires garde's zero-reference status; ran `cargo run -p pmcp-server-toolkit --example e05_input_validation` and `cargo run --example s57_typed_tool_garde_validation --features full` live — both produced correct value-free refusals from all three layers |
| SC-6 | Three false enforcement claims in `tools.rs` corrected; no remaining comment claims a mitigation the code does not implement | ✓ VERIFIED | `tools.rs:15-21` (T-83-05-02), `:1083-1108` (T-90-03-01), `:1178-1189` (T-90-05-03) now describe the actual decorator and name their backing tests; workspace-wide grep for `enforced upstream`, `rejected by pmcp's request-validation`, `schema-validated ... BEFORE the script` returns zero hits in `src/`; 128-10-SUMMARY.md records an 8-comment sweep (more than the 3 named) with a documented method + positive control |
| SC-7 | Refusal messages name the violated rule and DECLARED parameters, never the rejected value, never an attacker-supplied key | ⚠️ PRESENT_BEHAVIOR_UNVERIFIED (partial) | Config-path (D1/D2/D3/D4, the `[[tools]]` surface) is fully proven value-free by test (`input_validation_refuses_undeclared_argument_without_contacting_upstream`, `render_refusal`/`safe_pointer`, live `e05`/`s57` example runs). **But** `128-REVIEW.md` WR-04 (open, unfixed) identifies one narrow counter-example: `pmcp-code-mode/executor.rs`'s `resolve_path` `PathPart::Variable` arm echoes the JS variable IDENTIFIER into a `PlaceholderRefusal`, and for the generic `execute_code` tool (as opposed to an operator-authored `[[tools]] script`) that identifier is attacker-chosen — reached by code, not by a passing test that disproves it. See Human Verification below. |
| SC-8 | `make quality-gate` passes; fuzz/property/unit/example coverage per CLAUDE.md ALWAYS requirements | ✓ VERIFIED | Regression gate already measured: `cargo nextest run --features full` 3363/3363 passed (per task brief). This session additionally ran, live: `make test-server-toolkit` (450 unit/integration tests + 3 property arms, all green, with the toolkit-scoped `--ignored property_` selector working as designed), `make test-fuzz-strict` (both Phase 128 fuzz targets, 11k+ exec each, no crash/timeout/artifact), and both examples (`e05_input_validation`, `s57_typed_tool_garde_validation`) ran end-to-end |

**Score:** 8/8 truths present and wired; 7/8 fully behaviorally proven; 1/8 (SC-7) has a known, documented, unfixed narrow counter-example on a surface outside the config-driven `[[tools]]` path this phase's acceptance matrix targets.

### P0 sub-goal — three false security claims

✓ VERIFIED. See SC-6 row above. All three named threat IDs (T-83-05-02 at `tools.rs:15-17`,
T-90-03-01 at `tools.rs:556-557`→now `1083-1108`, T-90-05-03 at `tools.rs:615`→now `1178-1189`)
were rewritten to describe the actual, now-implemented enforcement path, with each restated claim
naming the specific test that would fail if the enforcement were removed
(`input_validation_refuses_undeclared_argument_without_contacting_upstream`,
`script_tool_refuses_a_schema_violating_arg_before_the_script_runs`). The `128-10-PLAN`/`-SUMMARY`
sweep went beyond the three named lines and corrected 8 comments total using a documented
36-phrase candidate list with a positive control, which is stronger evidence than a minimal fix of
just the three cited lines.

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `crates/pmcp-server-toolkit/src/tools.rs` — `enforce_input_schema`/`ValidatingToolHandler` | D1 runtime enforcement | ✓ VERIFIED | Wraps all 3 push sites (SQL, script, single-call HTTP); wired via `ToolkitHooks` for E2 |
| `crates/pmcp-server-toolkit/src/config.rs` — `ParamDecl`, `lint()`, position-scoped cap | D2/D3 | ✓ VERIFIED | Substantive fields + logic, not stubs; config-time pattern compile check |
| `crates/pmcp-server-toolkit/src/http/client.rs` — `substitute_path`/`check_placeholder_value`/`check_composed_path` | D4 curated surface | ✓ VERIFIED | Calls core `validate_path_placeholder`/`validate_resolved_path` directly |
| `crates/pmcp-code-mode/src/executor.rs` — `resolve_path`/`resolve_layer_two_placeholders` | D4 Code Mode surface | ✓ VERIFIED, with WR-04 residual | Same core functions via re-export; WR-04 narrows the value-free guarantee on one arm |
| `crates/pmcp-server-toolkit/src/policy.rs` — `RequestPolicy`, `ArgumentValidator`, `ToolkitHooks` | E1/E2 | ✓ VERIFIED | Traits + registry, consumed by `builder_ext.rs` and `code_mode.rs::with_request_policy` |
| `src/server/typed_tool.rs` — `TypedTool::new_validated` | E3 | ✓ VERIFIED | `garde::Validate` entry point, opt-in, tested live via `s57` example |
| `cargo-pmcp/src/commands/validate.rs` — `ValidateCommand::Config`, `toolkit_lint_banner` | SC-3 CLI surface | ✓ VERIFIED | Subcommand exists, banner states version-scoping, wired into `validate deploy` too |
| `src/server/schema_validation.rs` | D1/D4 core enforcement engine | ✓ VERIFIED | `validate_input`, `validate_path_placeholder`, `validate_resolved_path`, `render_refusal`, `safe_pointer` all present and substantive (1883 lines) |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `tools.rs` handler push sites (×3) | `ValidatingToolHandler` | `enforce_input_schema()` call | ✓ WIRED | Confirmed at lines 210, 941, 966 — "3 of 3" self-documented |
| `ValidatingToolHandler::check_schema` | `pmcp::server::schema_validation::validate_input` | direct call | ✓ WIRED | `tools.rs:389` |
| `HttpClient::substitute_path` | `pmcp::server::schema_validation::validate_path_placeholder` / `validate_resolved_path` | `check_placeholder_value`/`check_composed_path` | ✓ WIRED | `http/client.rs:527-570` |
| `pmcp-code-mode::executor::resolve_path` / `resolve_layer_two_placeholders` | same core functions | `pmcp_code_mode::{validate_path_placeholder,validate_resolved_path}` re-export | ✓ WIRED | `lib.rs:139` re-export; call sites in `executor.rs` |
| `pmcp::ServerBuilder` | `RequestPolicy`/`ArgumentValidator` | `ServerBuilderExt` trait + `ToolkitHooks` | ✓ WIRED | `builder_ext.rs`; live-run confirmed via `e05_input_validation` example |
| `cargo pmcp validate config`/`validate deploy` | `ServerConfig::lint()` | `render_config_lint_findings` | ✓ WIRED | Both subcommands call the same projection, per Q5 |

### Behavioral Spot-Checks (live-run this session)

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| Config-path D1 acceptance matrix (rows 8-11) | `cargo test -p pmcp-server-toolkit --test input_validation_acceptance --features input-validation,http` | 7 passed | ✓ PASS |
| Curated single-call CR-01 probes | `cargo test -p pmcp-server-toolkit --test curated_path_injection --features input-validation,http` | 6 passed | ✓ PASS |
| Code Mode CR-01 probes | `cargo test -p pmcp-server-toolkit --test http_executor --features input-validation,openapi-code-mode` | 13 passed | ✓ PASS |
| Toolkit full suite + gate-required binaries | `RUSTFLAGS="" make test-server-toolkit` | 450 tests passed, all 6 required binaries present with nonzero counts, +3 property arms | ✓ PASS |
| Property-based invariants | (part of above) `path_placeholder_props` `--ignored property_` | 3/3 passed (`property_declared_pattern_never_widens_the_floor`, `property_refusal_is_closed_under_percent_encoding`, `property_refusal_never_echoes_the_placeholder_value`) | ✓ PASS |
| Strict fuzz leg (SC-7 no-echo invariant + ReDoS ordering) | `RUSTFLAGS="" make test-fuzz-strict` | 11k+ execs/target, no crash/timeout/artifact | ✓ PASS |
| E1/E2/E3 three-layer example | `cargo run -p pmcp-server-toolkit --example e05_input_validation --features input-validation,http` | D1, E2, E1 all refused correctly, zero network reached on refusal | ✓ PASS |
| E3 garde example | `cargo run --example s57_typed_tool_garde_validation --features full` | Both refusals value-free (`Jane Doe DOB...`/`5000` never echoed); opt-in confirmed via plain constructor accepting the same payload | ✓ PASS |

### Requirements Coverage (D1-D4/E1-E3 + SC-1..SC-8, from PLAN frontmatter)

| Requirement | Source Plan(s) | Status | Evidence |
| --- | --- | --- | --- |
| D1 | 01, 02 | ✓ SATISFIED | See SC-1 |
| D2 | 03, 11 | ✓ SATISFIED | See SC-2 |
| D3 | 03, 07, 09, 11 | ✓ SATISFIED | See SC-3 |
| D4 | 01, 02, 05, 06, 08 | ✓ SATISFIED | See SC-4 |
| E1 | 09 | ✓ SATISFIED | See SC-5 |
| E2 | 09 | ✓ SATISFIED | See SC-5 |
| E3 | 04 | ✓ SATISFIED | See SC-5 |
| SC-1..SC-8 | all 11 plans (see coverage matrix below) | 7 ✓ / 1 partial | See truths table above |

No orphaned requirements: every D1-D4/E1-E3 and SC-1..SC-8 ID declared across the 11 plans' frontmatter maps to at least one plan, and every plan's declared requirements are accounted for in this report.

### Anti-Patterns Found

Scanned the covered implementation files (see `covered_files`) for `TODO`/`FIXME`/`HACK`/`XXX`/`TBD`/placeholder patterns. None found in the phase's own new/modified source. The workspace-wide grep for retired false-enforcement phrasing (`enforced upstream`, `rejected by pmcp's request-validation`, `schema-validated ... BEFORE the script`) returns zero hits in `src/` — confirming SC-6's negative claim, not just its positive one.

Two residuals carried forward from `128-REVIEW.md` (code review, `status: issues_found`, 4 criticals all fixed + 11 warnings all open) are worth naming explicitly per the task brief's instruction to assess whether any warning undermines a Success Criterion:

- **WR-04** (open, unfixed) — directly touches SC-7's letter on one narrow surface (`execute_code`'s generic layer-1 `${var}` path variable identifier). Classified WARNING by the reviewer (bounded impact: identifiers cannot carry whitespace, attacker already knows their own chosen name), but it is a real, reachable counter-example to "never an attacker-supplied key" as literally stated. Routed to human verification below rather than silently counted as a full SC-7 pass.
- **WR-08** (open, unfixed) — under `PMCP_QUIET`, `cargo pmcp validate config` prints nothing at all (not even findings) and exits 0, so the "surfaced by `cargo pmcp validate config`" claim in SC-3 does not hold in that one non-default mode. In the DEFAULT invocation (`PMCP_QUIET` unset, the common case) findings print to stderr and the summary correctly reflects the finding count, so SC-3's default-path claim holds; only the opt-in quiet mode is affected. Not routed to human verification because it degrades an opt-in flag's behavior rather than the criterion's stated default surface, but it is recorded here for completeness per the task brief.

All other WR items (01, 02, 03, 05, 06, 07, 09, 10, 11) were reviewed and assessed not to invalidate any SC's literal wording — most are false-positive refusals of legitimate input (fail-closed, not a security gap) or code-quality/maintainability concerns (e.g. WR-11's fragile string-comparison severity derivation). WR-10 is additionally noted as likely already fixed as a side effect of the CR-02 commit ("Also fixes WR-10" per commit `5c37285c`'s message), though no dedicated commit or test title confirms it independently — a residual worth a follow-up check but not gating this phase's goal.

### Human Verification Required

1. **Run `/gsd-secure-phase 128`** to produce `128-SECURITY.md` and re-check the Phase 83/Phase 90 threat sign-offs for the documented-but-absent class this phase's P0 sub-goal exists to close, as the phase's own goal text requires ("the Phase 83 / Phase 90 threat sign-offs are re-checked for the same class"). This is outside this verifier's scope (a dedicated capability-hook workflow) and has not yet been run for this phase.
2. **Decide on WR-04** — whether the `execute_code` tool's layer-1 path-variable-identifier echo is an acceptable residual against SC-7, matching the precedent already accepted for `garde`'s bare-identifier map key, or whether it should be fixed (swap to a fixed positional descriptor, matching the sibling `PathPart::Expression` arm) before this phase is considered fully closed.

### Gaps Summary

No BLOCKER-level gaps. All 8 Success Criteria and the P0 sub-goal are backed by real, substantive, wired implementation — verified by reading the actual enforcement code (not SUMMARY claims) and by running the phase's own tests, property arms, fuzz targets, and two live examples in this session, all green. The four code-review CRITICAL findings (CR-01..CR-04) each have a landed fix commit with regression tests that were independently confirmed present in the diffs. The one item keeping this phase out of a clean `passed` is SC-7, which is 100% proven on the config-driven `[[tools]]` surface (the phase's stated acceptance-test scope) but has one open, reviewer-acknowledged, low-severity counter-example on the separate `execute_code` generic-script surface — routed to human decision rather than silently waived or treated as a blocking regression, consistent with the Toyota-Way zero-tolerance standard this project holds itself to.

---

_Verified: 2026-09-28_
_Verifier: Claude (gsd-verifier)_
