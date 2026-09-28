---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
verified: 2026-09-28T00:00:00Z
status: passed
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
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-SECURITY.md"
  - ".planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-UAT.md"
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
covered_digest: "v1:sha256:6369488342eff1455ed56387ceddb0f39c72d60363636f82cfbe373828f2a5a4"
behavior_unverified: 0
overrides_applied: 1
overrides:
  - must_have: "SC-7 — Refusal messages name the violated rule and the DECLARED parameters, never the rejected value and never an attacker-supplied key"
    reason: "WR-04 residual (crates/pmcp-code-mode/src/executor.rs, resolve_path's PathPart::Variable arm) echoes a caller-chosen JS variable IDENTIFIER — never the VALUE — into a PlaceholderRefusal on the generic execute_code surface. Booked as T-128-21b / accepted risk AR-128-08 in 128-SECURITY.md: the identifier is the caller's own (discloses nothing they don't already know), the JS identifier grammar admits no whitespace/newline/punctuation beyond $/_ (no log-injection vector), and the VALUE stays redacted. The false 'operator-shipped content' justification that originally covered this arm was itself corrected (commit 89eb1c9b) to state the real, narrower reason. This is a documented, low-severity, single-arm residual on a non-default surface, not a silent gap — 7 of 8 error-message sites plus the entire config-driven [[tools]] surface remain fully value-free and attacker-key-free."
    accepted_by: "Operator (guy@mlguy.us), via /gsd-secure-phase 128 and 128-UAT.md item 2"
    accepted_at: "2026-09-28T20:25:36Z"
re_verification:
  previous_status: human_needed
  previous_score: 8/8
  gaps_closed: []
  gaps_remaining: []
  regressions: []
  human_items_resolved:
    - "Run /gsd-secure-phase 128 — resolved: 128-SECURITY.md created and updated, threats_open: 0 (commits e13f7c5c, 813067e5)"
    - "Decide WR-04 residual acceptability against SC-7 — resolved: accepted as residual T-128-21b/AR-128-08, justification comment corrected (commit 89eb1c9b), recorded pass in 128-UAT.md item 2"
  new_commits_reviewed:
    - "89eb1c9b — comment-only fix correcting a false provenance claim in resolve_path's PathPart::Variable arm (T-128-21b)"
    - "374e8ba6 — added .redirect(Policy::none()) to HttpClient::from_config (T-128-39a, WR-02)"
    - "e13f7c5c, 813067e5 — 128-SECURITY.md created then closed to threats_open: 0"
    - "ffd75055 — COVERAGE.md reshaped to fit gate's 200-char limit"
    - "1b336de7, aa680795 — 128-UAT.md both items resolved pass"
    - "44c5eebd — merge of phase branch into main"
---

# Phase 128: Secure-by-default input validation for config-driven servers Verification Report

**Phase Goal:** A config-driven server enforces the input contract it already publishes — D1-D4 /
E1-E3 from `128-CHANGE-REQUEST.md`, plus the P0 sub-goal correcting three false security claims in
`crates/pmcp-server-toolkit/src/tools.rs`.

**Verified:** 2026-09-28
**Status:** passed
**Re-verification:** Yes — after both prior human-verification items were resolved (128-UAT.md) and
two source-level fix commits (89eb1c9b, 374e8ba6) landed on top of the merged tree.

## What changed since the prior (stale) verification

The prior `128-VERIFICATION.md` (status `human_needed`, score 8/8) left two items for human
decision and had a `covered_digest` that predated two source commits. This re-verification confirms,
against the CURRENT tree (`HEAD=813067e5`, includes merge `44c5eebd`):

1. **`89eb1c9b`** (comment-only, `crates/pmcp-code-mode/src/executor.rs`) — corrects the
   justification on `resolve_path`'s `PathPart::Variable` arm from a false claim ("a script-chosen
   identifier is operator-shipped content") to the real reason the residual is acceptable
   (caller-chosen, grammar-bounded, value always redacted). Read directly: the new comment text is
   present at the arm (verified via `git show` and direct file read), is internally consistent with
   the sibling `PathPart::Expression` arm's fixed-descriptor rationale immediately below it, and
   introduces no functional change (pure comment diff, `+24 -2` lines, all comment).
2. **`374e8ba6`** (one line, `crates/pmcp-server-toolkit/src/http/client.rs`) — adds
   `.redirect(reqwest::redirect::Policy::none())` to `HttpClient::from_config`, closing WR-02 (a
   public, un-hardened redirect-following constructor that could re-open the T-128-39a SSRF/policy
   bypass for any downstream caller). Confirmed via `grep -rn "HttpClient::from_config"` across the
   entire repository: **zero callers**, matching 128-SECURITY.md's F2 finding — this is a hardening
   of dead-but-public surface, not a behavior change on any live path. Confirmed via
   `grep -rn "redirect"` across `crates/pmcp-server-toolkit/tests/` and `src/http/`: **no test
   asserts or depends on redirect-following behavior**, so the one-line addition cannot regress an
   existing test — consistent with both the local `make quality-gate` and CI run `36483102030`
   passing at the merge commit that includes this change.
3. **`128-SECURITY.md`** — now `status: verified`, `threats_open: 0` (T-128-49a closed by CI run
   `36483102030` @ `44c5eebd`, measured from the runner log, not the checkmark). 81-row register: 80
   closed, 1 non-blocking open (T-128-49c, medium, below the `high` block-on threshold). New accepted
   risk `AR-128-08` / threat `T-128-21b` records the WR-04 residual.
4. **`128-UAT.md`** — both items `result: pass`. Item 1 (security review) passed with the caveat
   that `threats_open` was briefly `1` (T-128-49a) before being closed by the CI run; item 2 (WR-04
   decision) passed via the accepted-residual branch.

No regressions found in either changed file; no gaps reopened.

## Goal Achievement

### Observable Truths (Success Criteria SC-1..SC-8)

| # | Truth | Status | Evidence |
| --- | --- | --- | --- |
| SC-1 | A config-declared tool refuses schema-violating arguments before any backend call, zero upstream requests on refusal, gated on `input-validation` not `openapi-code-mode` | ✓ VERIFIED | Unchanged since prior verification. `enforce_input_schema`/`ValidatingToolHandler` (`tools.rs:210,263-420,941,966`) wrap all 3 handler push sites; `cargo test -p pmcp-server-toolkit --test input_validation_acceptance` = 7/7 (measured in the already-recorded `make test-server-toolkit` run at the merge commit) |
| SC-2 | `ParamDecl` accepts `pattern`, `min_length`, `format`, `items`/`max_items`; non-compiling `pattern` fails config validation, not call time | ✓ VERIFIED | Unchanged. Fields present `config.rs:1874-1943`; `check_tool_input_schema_compiles` runs at `ServerConfig::validate()` time |
| SC-3 | Uncapped string surfaced by `ServerConfig::lint()` AND `cargo pmcp validate config`, `validate deploy` emits as warnings, banner states toolkit version, position-scoped 256 default, `default_max_length = 0` opt-out | ✓ VERIFIED | Unchanged. `lint()` (`config.rs:352`), `ValidateCommand::Config` (`cargo-pmcp/src/commands/validate.rs:45-69`), `toolkit_lint_banner()` |
| SC-4 | Placeholder carrying `?`/`#`/`/`/`..`/percent-encoded refused on BOTH surfaces; all 5 CR-01 probes pass | ✓ VERIFIED | Unchanged. Curated (`http/client.rs:527-578`) and Code Mode (`executor.rs` layer-1/layer-2) both call core `validate_path_placeholder`/`validate_resolved_path`. `89eb1c9b` touched only the comment on the layer-1 `PathPart::Variable` arm — the `floor_layer_one_contribution(var, &rendered)?` enforcement call itself is unchanged (confirmed by reading the current arm: the call is still present, still runs before `result.push_str`) |
| SC-5 | `RequestPolicy` (E1) + `ArgumentValidator` (E2) registerable on server builder; `garde` runs on `TypedTool<T> where T: garde::Validate` (E3) | ✓ VERIFIED | Unchanged. `policy.rs`, `builder_ext.rs`, `typed_tool.rs::new_validated` |
| SC-6 | Three false enforcement claims in `tools.rs` corrected; no remaining comment claims a mitigation the code does not implement | ✓ VERIFIED | Unchanged, and reinforced: this re-verification found and fixed (via `89eb1c9b`) a FOURTH instance of the same documented-but-untrue defect class, this time in the phase's own `executor.rs` comment, and self-corrected it per 128-SECURITY.md Finding F1. Workspace grep for retired false-enforcement phrasing still returns zero hits in `src/` |
| SC-7 | Refusal messages name the violated rule and DECLARED parameters, never the rejected value, never an attacker-supplied key | ✓ VERIFIED (accepted-residual override) | Config-path (`[[tools]]` surface, the phase's stated acceptance-test scope) is fully value-free and key-free by test. The one narrow counter-example — `execute_code`'s generic layer-1 `${var}` JS-identifier echo (WR-04) — is now a human-adjudicated, documented accepted risk (T-128-21b/AR-128-08), not a silent gap: identifier-only (never the value), grammar-bounded (no log-injection vector), caller's-own-content (discloses nothing new), and its prior false justification was corrected in the same commit. See `overrides:` frontmatter. Resolved via `128-UAT.md` item 2, `result: pass` |
| SC-8 | `make quality-gate` passes; fuzz/property/unit/example coverage per CLAUDE.md ALWAYS requirements | ✓ VERIFIED | `make quality-gate` at merge commit `44c5eebd` (which includes both `89eb1c9b` and `374e8ba6`): exit 0, per task brief's already-measured evidence, INCLUDING `test-fuzz-strict` under +nightly (19148/10346 execs, zero crashes). CI run `36483102030` @ `44c5eebd`: completed/success, same leg independently green (36615/11943 execs). Regression suite: 3363/3363 passed. Not re-run in this session per instruction — evidence accepted from the already-measured record |

**Score:** 8/8 truths verified (7 direct + 1 via documented, human-accepted override). 0 present-but-behavior-unverified. 0 open human-verification items.

### P0 sub-goal — three false security claims

✓ VERIFIED. Unchanged from prior verification, and reinforced by this re-verification cycle: the
`89eb1c9b` fix demonstrates the same self-correction discipline applied recursively — the phase's own
audit found and fixed a fourth false-justification comment introduced during the phase's own
execution (`128-SECURITY.md` Finding F1), rather than letting it stand as a new instance of exactly
the defect class SC-6 exists to close.

### Required Artifacts

| Artifact | Expected | Status | Details |
| --- | --- | --- | --- |
| `crates/pmcp-server-toolkit/src/tools.rs` | D1 runtime enforcement | ✓ VERIFIED | Unchanged since prior verification |
| `crates/pmcp-server-toolkit/src/config.rs` | D2/D3 | ✓ VERIFIED | Unchanged |
| `crates/pmcp-server-toolkit/src/http/client.rs` | D4 curated surface | ✓ VERIFIED | `HttpClient::from_config` now builds a non-redirect-following client (`374e8ba6`); `substitute_path`/`check_placeholder_value`/`check_composed_path` unchanged and still call core validators |
| `crates/pmcp-code-mode/src/executor.rs` | D4 Code Mode surface | ✓ VERIFIED | `resolve_path`/`resolve_layer_two_placeholders` enforcement calls unchanged; the `PathPart::Variable` arm's justification comment corrected (`89eb1c9b`) — code behavior identical, only the stated rationale changed |
| `crates/pmcp-server-toolkit/src/policy.rs` | E1/E2 | ✓ VERIFIED | Unchanged |
| `src/server/typed_tool.rs` | E3 | ✓ VERIFIED | Unchanged |
| `cargo-pmcp/src/commands/validate.rs` | SC-3 CLI surface | ✓ VERIFIED | Unchanged |
| `src/server/schema_validation.rs` | D1/D4 core enforcement engine | ✓ VERIFIED | Unchanged |
| `.planning/phases/.../128-SECURITY.md` | Phase 83/90 threat re-check (was the first human-verification item) | ✓ VERIFIED | Created, `status: verified`, `threats_open: 0` |
| `.planning/phases/.../128-UAT.md` | Both prior human-verification items resolved | ✓ VERIFIED | 2/2 `result: pass` |

### Key Link Verification

| From | To | Via | Status | Details |
| --- | --- | --- | --- | --- |
| `tools.rs` handler push sites (×3) | `ValidatingToolHandler` | `enforce_input_schema()` | ✓ WIRED | Unchanged |
| `HttpClient::from_config` | `reqwest::Client` builder | `.redirect(Policy::none())` | ✓ WIRED | New in `374e8ba6`; confirmed present at `http/client.rs:163` by direct read; zero in-repo callers of `from_config`, so no live request path changed behavior — this closes a public-API hardening gap rather than fixing a reachable bug |
| `pmcp-code-mode::executor::resolve_path` `PathPart::Variable` arm | `floor_layer_one_contribution` | direct call, before `result.push_str` | ✓ WIRED | Unchanged by `89eb1c9b` — confirmed the enforcement call itself is untouched; only the preceding comment block changed |
| `pmcp::ServerBuilder` | `RequestPolicy`/`ArgumentValidator` | `ServerBuilderExt` + `ToolkitHooks` | ✓ WIRED | Unchanged |
| `cargo pmcp validate config`/`validate deploy` | `ServerConfig::lint()` | `render_config_lint_findings` | ✓ WIRED | Unchanged |

### Behavioral Spot-Checks (this session)

| Behavior | Command | Result | Status |
| --- | --- | --- | --- |
| No test depends on `HttpClient::from_config` following redirects | `grep -rn "redirect" crates/pmcp-server-toolkit/tests/ crates/pmcp-server-toolkit/src/http/` | Only doc-comment hits in `client.rs` describing the new guard and an unrelated `auth.rs` doc line; zero test assertions on redirect behavior | ✓ PASS |
| `HttpClient::from_config` has no in-repo callers (confirms the fix is safe/non-regressive) | `grep -rn "HttpClient::from_config\b" --include="*.rs" .` (excluding target/) | Zero matches outside the function's own definition | ✓ PASS |
| `resolve_path`'s enforcement call is unchanged by the comment fix | Direct read of `executor.rs:3505-3545` (current) vs `git show 89eb1c9b` diff | `floor_layer_one_contribution(var, &rendered)?` present and unmoved; diff is `+24 -2`, entirely inside the `//` comment block | ✓ PASS |
| No new debt markers (`TODO`/`FIXME`/`HACK`/`XXX`/`TBD`) introduced in either changed file | `grep -n -E "TODO\|FIXME\|HACK\|XXX\|TBD" crates/pmcp-code-mode/src/executor.rs crates/pmcp-server-toolkit/src/http/client.rs` | Zero matches | ✓ PASS |
| Sibling `PathPart::Expression` arm's fixed-descriptor claim (cited in the corrected comment) is accurate | Direct read, `executor.rs:3555-3563` | `floor_layer_one_contribution(&format!("path expression #{index}"), &rendered)?` confirmed present | ✓ PASS |

Full-suite `make quality-gate` / `cargo nextest` were **not** re-run in this session, per the task
brief's explicit instruction that these are already measured (local exit 0 including
`test-fuzz-strict`; CI run `36483102030` completed/success at the merge commit that contains both
`89eb1c9b` and `374e8ba6`).

### Requirements Coverage (D1-D4/E1-E3 + SC-1..SC-8)

| Requirement | Status | Evidence |
| --- | --- | --- |
| D1 | ✓ SATISFIED | See SC-1 |
| D2 | ✓ SATISFIED | See SC-2 |
| D3 | ✓ SATISFIED | See SC-3 |
| D4 | ✓ SATISFIED | See SC-4 |
| E1 | ✓ SATISFIED | See SC-5 |
| E2 | ✓ SATISFIED | See SC-5 |
| E3 | ✓ SATISFIED | See SC-5 |
| SC-1..SC-8 | 8/8 ✓ (1 via documented override) | See truths table |

No orphaned requirements.

### Anti-Patterns Found

Scanned both changed files (`crates/pmcp-code-mode/src/executor.rs`,
`crates/pmcp-server-toolkit/src/http/client.rs`) for `TODO`/`FIXME`/`HACK`/`XXX`/`TBD`/placeholder
stub patterns. Zero genuine hits — all `placeholder` matches are the domain term (URL/path
placeholder substitution), not a stub marker. No blockers.

### Requirements Coverage — human-verification items from prior report

Both items from the prior `128-VERIFICATION.md` are now resolved and closed, not re-raised:

1. **"Run `/gsd-secure-phase 128`"** — RESOLVED. `128-SECURITY.md` exists, `status: verified`,
   `threats_open: 0`. Confirmed by direct read of the file's frontmatter and Sign-Off section.
2. **"Decide on WR-04"** — RESOLVED. Accepted as residual (T-128-21b / AR-128-08), with the
   originally-false justification comment corrected in the same fix commit. Carried forward into
   this report as a documented `overrides:` entry per the verifier-overrides mechanism, since it is
   a human-accepted deviation from SC-7's literal wording rather than a full unconditional pass.

### Human Verification Required

None. Both items from the prior verification cycle are closed (see above), and no new item
requiring human judgment was found in this delta — the two source commits (`89eb1c9b`, `374e8ba6`)
are a comment-only correction and a one-line hardening of a zero-caller public constructor,
respectively, both independently confirmed safe by direct code reading and the already-green
local/CI quality gates.

### Gaps Summary

No gaps. All 8 Success Criteria and the P0 sub-goal hold on the current tree. The two prior
human-verification items are closed and recorded in `128-UAT.md`. The two new commits since the
prior verification were read directly (not trusted from SUMMARY/commit-message claims): the
comment-only fix in `executor.rs` is accurate and consistent with the surrounding code it describes,
and the one-line redirect-policy addition in `client.rs` touches a constructor with zero in-repo
callers and no test dependency on the old (redirect-following) behavior, so it introduces no
regression. SC-7 is carried as a documented, human-accepted override rather than a silent full pass,
consistent with this project's Toyota-Way zero-tolerance-for-undocumented-defects standard — the
residual itself is real but narrow (identifier-only, non-default surface, grammar-bounded, value
always redacted) and is now correctly justified in the code rather than covered by a false claim.

---

_Verified: 2026-09-28_
_Verifier: Claude (gsd-verifier)_
