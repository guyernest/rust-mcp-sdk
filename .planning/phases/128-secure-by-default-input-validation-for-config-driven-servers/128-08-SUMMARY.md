---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 08
subsystem: api
tags: [openapi, code-mode, path-placeholder, proptest, input-validation, http]

requires:
  - phase: 128-05
    provides: "`HttpExecutor::placeholder_rules(method, path_template, param)` — the default-implemented D4(b) seam this plan overrides — plus `ResolvedPath` and `mod query_separator`'s narrowing"
  - phase: 128-06
    provides: "`Parameter::placeholder_rules()` / `Parameter::with_rules(..)` and the OpenAPI parser that populates `pattern` / `max_length` (without which this wiring would narrow nothing)"
  - phase: 128-02
    provides: "core `validate_path_placeholder`, `validate_resolved_path`, `PlaceholderRules`, `PlaceholderRefusal`, `PLACEHOLDER_MAX_LENGTH` — left UNMODIFIED by this plan"
  - phase: 128-03
    provides: "`ServerConfig::lint()` and the `ConfigWarning` finding model this plan extends with `lint_against_spec`"
provides:
  - "`HttpCodeExecutor::with_schema(Arc<OpenApiSchema>)` — the cheap-clone builder that gives the executor the operator's parsed spec"
  - "`HttpCodeExecutor::has_schema() -> bool` — the read-only seam that makes the cross-crate wiring provable (T-128-36c)"
  - "the `HttpExecutor::placeholder_rules` OVERRIDE on `HttpCodeExecutor`: an O(1) `operation_for(template, METHOD)` lookup returning that PATH parameter's declared rules"
  - "`ServerConfig::lint_against_spec(&OpenApiSchema) -> Vec<ConfigWarning>` + `CONFIGURED_TEMPLATE_NOT_IN_SPEC` — the config-time half of the template-spelling-drift guard, emitted at startup by the binary"
  - "`crates/pmcp-server-toolkit/tests/path_placeholder_props.rs` — the D-10 ordering and percent-encoding closure as properties over generated input, plus a non-ignored smoke arm"
  - "the production wiring in `pmcp-openapi-server`'s `build_server`, applied BEFORE both executor fan-outs"
affects: [128-09, 128-10, 128-11]

# Actuals (#2632) — chars/4 over the realized diff, NOT a harness token count.
actuals:
  tokens: 6569
  tasks: 3
  commits: 7
  plan_head_before: 44064265c5e4832a63d1733f15b06ed65167b285
  # MEASURED, not narrated. `git rev-list --count 44064265..HEAD` == 6 for the six
  # code/test commits (0c0fa5ef, 47ab3ddc, 986f3694, a5168b26, eb1233ec,
  # e7989ac2); the 7th is the commit carrying THIS file plus STATE.md and
  # ROADMAP.md. It is named by role and not by hash because this figure lives
  # inside it, so writing its hash here would change that hash. 7 is what a later
  # `/gsd-verify-work` re-measure will see.
  # tokens: `git diff 44064265..HEAD -- crates/ | wc -c` == 26276, /4 == 6569.
  # The plan estimated 45000; the actual is ~15% of it. Not rounded toward the
  # estimate. The gap's cause is legible and worth recording for calibration: two
  # of the three tasks were ~40-line lookups over primitives plans 02/05/06 had
  # already built and fenced, and the third is one new test file. Plan 05 came in
  # at ~32% of its estimate for the same structural reason — a wave-6 plan that
  # only WIRES earlier waves' primitives is systematically cheaper than the
  # estimator assumes.

tech-stack:
  added: []
  patterns:
    - "A public method whose narrowing comes from a private helper: the helper carries the full cost-of-a-MISS rustdoc, the method delegates in one line and stays trivially under cog-25 (SP-4)"
    - "A `#[cfg(feature)]` / `#[cfg(not(feature))]` SIBLING-FUNCTION pair, each with its own rustdoc, rather than a `cfg` arm inside an impl — copied from `http/client.rs`'s `check_composed_path`"
    - "One test binary carrying BOTH a non-ignored smoke arm (to satisfy `REQUIRED_TEST_BINARIES`' nonzero-PASSED guard) and `#[ignore]`d `property_` arms (selected by a second, separately count-asserted invocation)"
    - "Non-vacuity of a property proven by INVERTING the implementation, capturing the shrunk counterexample, restoring byte-exact, and committing the shrunk seed with a PROVENANCE header"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/path_placeholder_props.rs
    - crates/pmcp-server-toolkit/tests/path_placeholder_props.proptest-regressions
  modified:
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/src/config.rs
    - crates/pmcp-openapi-server/src/assemble.rs
    - crates/pmcp-server-toolkit/tests/http_executor.rs
    - Makefile

key-decisions:
  - "The wiring site is `build_server` in `crates/pmcp-openapi-server/src/assemble.rs`, at line 310 — measured to precede the script-tool fan-out (:326) and the Code Mode fan-out (:344), because both surfaces share one cheap-clone executor (D-02) and a clone taken earlier would be permanently unnarrowed."
  - "`HttpCodeExecutor::has_schema()` is PUBLIC, not `#[cfg(test)]`: the wiring lives in a different crate from the field, so a test-only accessor could not prove T-128-36c from where the wiring is."
  - "The `lint()`-level guard is an ADDITIVE `ServerConfig::lint_against_spec(&OpenApiSchema)` rather than a new parameter on `lint()`: `lint()` takes only `&self` and a spec-less config must produce no findings from its own absence."
  - "The startup emission of those findings lives in `build_server` — the only production point where the config and the parsed spec are both in scope — so the guard is REACHABLE rather than a documented-but-absent method."
  - "Template canonicalization REJECTED (carried from the plan's dispositions ledger): normalizing `{alias}` to `{id}` either re-derives the exact match or guesses, and a wrong guess narrows from the WRONG parameter's declared rules."
  - "The property binary uses ONLY core's public primitives and does NOT re-derive the `?` split. It asserts the two core facts the split composes; the ACCEPT direction is already pinned where the split lives."
  - "The no-echo property asserts 'no 6-byte run of the value survives into the message' rather than the plan's literal 'no byte', which is unsatisfiable for any value against English prose. Generators draw from `[A-Z]` so that window is collision-free by construction."

patterns-established:
  - "Bounded miss-logging memo: a once-per-`(method, template)` `tracing::debug!` with a hard 64-pair cap, because a Code Mode script composes its template at RUNTIME and an unbounded memo is an unbounded allocation driven by caller input. The LOG goes quiet past the bound; enforcement never depends on the memo."
  - "A gate leg with TWO count-asserted invocations of one binary: the default run for the per-binary nonzero-PASSED guard, and a `-- --ignored property_` run for the property arms — with the second's zero-case error message naming which invariants would have stopped being asserted."
  - "Proving a mutation-sensitivity claim by naming the ROWS that go red, per mutant, and asserting the restore is byte-exact by sha256 as well as by `git diff`."

requirements-completed: [D4, SC-4]

coverage:
  - id: D1
    description: "`HttpCodeExecutor` carries `Option<Arc<OpenApiSchema>>`, gains `with_schema`, and overrides `placeholder_rules` with an O(1) `(path, METHOD)` index lookup returning that PATH parameter's declared rules. `new`'s signature is unchanged."
    requirement: "D4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/code_mode.rs#code_mode::placeholder_rules_override (10 rows: a_declared_pattern_reaches_the_rules, the_method_selects_the_operation, an_executor_with_no_schema_returns_the_default, an_unknown_path_template_returns_the_default, an_unknown_method_on_a_known_path_returns_the_default, an_unknown_parameter_name_returns_the_default, a_non_path_parameter_is_not_consulted, allow_slash_is_false_for_every_spec_derived_result, a_schema_miss_still_refuses_a_floor_denied_value, the_narrowing_refuses_a_floor_clean_value_the_spec_forbids)"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" cargo test -p pmcp-server-toolkit --features openapi-code-mode --lib code_mode:: -- --test-threads=1 (33 passed)"
        status: pass
      - kind: other
        ref: "mutation M2c — override body replaced with PlaceholderRules::default(); 3 lib rows + http_executor_spec_pattern_refuses_a_floor_clean_value go RED; restored byte-exact"
        status: pass
    human_judgment: false
  - id: D2
    description: "A template-spelling MISS is honest about its cost: the rustdoc states floor+cap RETAINED / spec narrowing LOST (never \"weakens nothing\"), a bounded once-per-pair `tracing::debug!` fires, and `ServerConfig::lint_against_spec` reports the CONFIGURED case before deploy."
    requirement: "D4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#config::lint_against_spec_tests (5 rows: reports_a_template_the_spec_does_not_declare, reports_a_method_the_spec_does_not_declare, accepts_an_exactly_declared_template, accepts_an_author_written_query_string, skips_a_tool_with_no_method_path_pair)"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/code_mode.rs#code_mode::placeholder_rules_override::a_schema_miss_still_refuses_a_floor_denied_value"
        status: pass
      - kind: other
        ref: "grep: the string \"weakens nothing\" appears nowhere in crates/pmcp-server-toolkit/src/code_mode.rs"
        status: pass
    human_judgment: false
  - id: D3
    description: "`with_schema` is wired ONCE, in `build_server`, BEFORE both executor fan-outs, sharing ONE `Arc<OpenApiSchema>` with the `api_schema` resource; never in a `#[cfg(test)]` helper."
    requirement: "D4"
    verification:
      - kind: unit
        ref: "crates/pmcp-openapi-server/src/assemble.rs#assemble::tests (narrowed_executor_attaches_a_supplied_spec, narrowed_executor_leaves_a_spec_less_executor_alone, narrowed_executor_shares_one_arc, build_server_with_a_spec_builds_and_registers_tools)"
        status: pass
      - kind: other
        ref: "measured line numbers: narrowed_executor at assemble.rs:310 < script fan-out :326 < Code Mode fan-out :344; sole workspace-src `.with_schema(` call site is assemble.rs:265, inside the non-cfg(test) helper"
        status: pass
      - kind: other
        ref: "mutation M1 — with_schema deleted from narrowed_executor; narrowed_executor_attaches_a_supplied_spec and narrowed_executor_shares_one_arc go RED; restored byte-exact"
        status: pass
    human_judgment: false
  - id: D4
    description: "The D-10 floor-then-narrow ordering and the percent-encoding closure hold over GENERATED input, and the refusal never echoes the value."
    requirement: "SC-4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/tests/path_placeholder_props.rs#property_declared_pattern_never_widens_the_floor, #property_refusal_is_closed_under_percent_encoding, #property_refusal_never_echoes_the_placeholder_value (3 passed under `-- --ignored property_`)"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/tests/path_placeholder_props.rs#path_placeholder_props_smoke_floor_refuses_cr01_payloads (1 passed on the default run)"
        status: pass
      - kind: other
        ref: "non-vacuity — core's validate_path_placeholder temporarily inverted to pattern-supersedes-floor; the ordering property FAILS and shrinks to base=\"A\", fragment_idx=0, pattern_idx=0 (the value \"A?\" against `^.*$`); core restored byte-exact, sha256 73844dc8…ee53d before and after"
        status: pass
    human_judgment: false
  - id: D5
    description: "`path_placeholder_props` is count-asserted in the gate twice — the default run for the per-binary nonzero-PASSED guard and a `-- --ignored property_` run for the three property arms."
    requirement: "SC-4"
    verification:
      - kind: integration
        ref: "RUSTFLAGS=\"\" make test-server-toolkit — exit 0, 399 tests; `✓ path_placeholder_props passed 1 tests` and `✓ path_placeholder_props property arms passed 3 tests`"
        status: pass
      - kind: other
        ref: "measured all-ignored rejection — with the smoke arm temporarily #[ignore]d, make test-server-toolkit exits 2 with \"required test binary 'path_placeholder_props' RAN but passed ZERO tests\"; restored byte-exact"
        status: pass
    human_judgment: false
  - id: D6
    description: "No regression on any consumer leg, and `src/server/schema_validation.rs` is byte-identical to the plan base."
    verification:
      - kind: integration
        ref: "RUSTFLAGS=\"\" cargo nextest run --features \"full\" --no-fail-fast — 3358 run, 3358 passed, 5 skipped, exit 0"
        status: pass
      - kind: integration
        ref: "RUSTFLAGS=\"\" make quality-gate — ALL TOYOTA WAY QUALITY CHECKS PASSED banner present, on the final tree"
        status: pass
      - kind: other
        ref: "git diff 44064265..HEAD -- src/server/schema_validation.rs — 0 lines"
        status: pass
    human_judgment: false

duration: 113 min
completed: 2026-09-27
status: complete
---

# Phase 128 Plan 08: D4(b) on the Code Mode surface — spec-declared narrowing, wired and proven

**`HttpCodeExecutor` now carries the operator's parsed OpenAPI document and narrows each `{param}` from that parameter's own declared `pattern`/`maxLength` via an O(1) `(path, METHOD)` index hit; the wiring lands in `build_server` before either surface takes its clone, a bounded once-per-pair debug log plus a new `lint_against_spec` finding make a template-spelling MISS observable instead of silent, and the floor-then-narrow ordering is pinned as a property whose non-vacuity was proven by inverting core and capturing the shrunk counterexample.**

## Performance

- **Duration:** 113 min
- **Started:** 2026-09-27T19:17Z (first commit `0c0fa5ef`)
- **Completed:** 2026-09-27T21:10Z
- **Tasks:** 3
- **Files modified:** 7 (2 created, 5 modified)

## Accomplishments

- **D4(b) closes on the Code Mode surface with the operator's real declarations.** `spec_placeholder_rules` resolves `(method, path_template)` through `OpenApiSchema::operation_for` — an O(1) hit on the `(path, METHOD)` index, which is why `method` is in the trait signature at all — finds the PATH parameter by name, and returns that `Parameter::placeholder_rules()`. It builds `PlaceholderRules` and evaluates no pattern of its own, so a placeholder `pattern` and an `inputSchema` `pattern` still resolve the whitespace shorthand through the single core regex path.
- **A MISS is documented as the real reduction it is.** The override's rustdoc says a miss RETAINS the unconditional character floor and the always-on 256-code-point cap and LOSES the spec's narrowing, and names `/users/{alias}` against a declared `/users/{id}` as the concrete way that happens. The phrase "weakens nothing" appears nowhere in the file.
- **The loss is bounded three ways, and each bound's limit is stated.** The wording; a `tracing::debug!` fired once per `(method, template)` pair with a hard 64-pair memo cap (a Code Mode script composes its template at runtime, so an unbounded memo would be an unbounded allocation driven by caller input); and `ServerConfig::lint_against_spec`, which reports a CONFIGURED `(method, path)` the spec does not declare — with its own bound spelled out, that it cannot see a runtime-composed template, which is why the debug log exists as well.
- **The wiring is in production code and provable from the crate that owns it.** `narrowed_executor` is called at `assemble.rs:310`, measured to precede the script-tool fan-out at `:326` and the Code Mode fan-out at `:344`; `has_schema()` is public so `pmcp-openapi-server`'s own tests can assert the attachment; and one `Arc<OpenApiSchema>` is shared with the `api_schema` resource rather than the document being cloned.
- **The D-10 ordering is a property, and the property is not decorative.** Inverting core to pattern-supersedes-floor makes it fail and shrink to a two-character counterexample.

## Task Commits

1. **Task 1: `HttpCodeExecutor` carries the spec and overrides `placeholder_rules`**
   - RED — `0c0fa5ef` (`test`)
   - GREEN — `47ab3ddc` (`feat`)
   - No REFACTOR commit: the cog-25 split into a free helper was part of the GREEN design, so there was no change to make afterwards. `pmat quality-gate --checks complexity` is 0 violations.
2. **Task 2: Wire the parsed spec into the executor the binary builds**
   - RED — `986f3694` (`test`)
   - GREEN — `a5168b26` (`feat`)
3. **Task 3: Property arms for the D-10 ordering and percent-encoding closure**
   - `eb1233ec` (`test`) — the binary plus the two Makefile edits. No separate RED/GREEN: see "TDD Gate Compliance" below for why, and for what stood in for RED.
4. **Follow-up:** `e7989ac2` (`docs`) — one new rustdoc warning of this plan's own making, fixed.

**Plan metadata:** the commit carrying this SUMMARY, STATE.md and ROADMAP.md.

## TDD Gate Compliance

| Task | Phase | Commit | Evidence |
|---|---|---|---|
| 1 | RED | `0c0fa5ef` | `--lib code_mode::` — 33 discovered, 30 passed, **3 FAILED**, exit 101. Target `a_declared_pattern_reaches_the_rules` failed `left: None, right: Some("^G[0-9]+$")`. `check tdd-red-evidence` → **RED_EVIDENCE_OK**. |
| 1 | GREEN | `47ab3ddc` | `--lib code_mode::` 33 passed; `--lib config::lint_against_spec_tests` 5 passed. |
| 2 | RED | `986f3694` | `-p pmcp-openapi-server --lib assemble` — 7 discovered, 5 passed, **2 FAILED**, exit 101. Target `narrowed_executor_attaches_a_supplied_spec` failed on "a supplied spec must reach the executor". **RED_EVIDENCE_OK**. |
| 2 | GREEN | `a5168b26` | `--lib assemble` 7 passed; `-p pmcp-openapi-server --no-fail-fast` 46 passed. |
| 3 | RED-equivalent | `eb1233ec` | See below — a genuine RED was impossible without violating the plan's own fence, so the implementation was INVERTED instead. |

**Two things about the RED phases a reviewer should check rather than assume.**

**(a) Both RED commits carry inert production plumbing, deliberately.** In Rust, TDD for a method that does not yet exist produces `E0599`, and `check tdd-red-evidence` classifies a compile error as `INVALID_RED` (#3770) — a syntax/load failure must not authorize GREEN. So each RED commit adds the signature and a STUB that returns the safe default (`PlaceholderRules::default()` for Task 1, the executor unchanged for Task 2), which makes the narrowing assertions fail on an ASSERTION while the miss/default/`allow_slash` rows already pass — and those rows passing in RED is itself meaningful, because they are the rows that must survive GREEN unchanged. Both stub docstrings say "STUB — the RED half" so the GREEN commit's diff is unambiguous.

**(b) `check tdd-red-evidence` cannot read cargo output, and this is a tool gap rather than a weak RED.** `gsd-core/bin/lib/tdd-red-evidence.cjs` parses **node:test TAP** (`parseNodeTestSummary` looks for `# tests`/`# pass`/`# fail`; `tapFailedTestNames` for `not ok N - <name>`). Fed raw cargo output it returns `INVALID_RED (zero_tests_discovered)` no matter how genuine the RED is — measured on both of this plan's RED runs. A line-for-line adapter (`cargo2tap.py`, kept in the session scratchpad, not committed) restates the cargo run's own `test <name> ... ok|FAILED` lines and its `test result:` summary as TAP and invents nothing; both records then classified `RED_EVIDENCE_OK`. Flagged as a handoff below: every Rust-repo `type: tdd` plan hits this.

**Task 3 had no RED, and the reason is the plan's own fence.** The properties assert behaviour plan 02 already shipped in `src/server/schema_validation.rs`, and this plan is forbidden from modifying that file (its 52 tests are the fence). A property over correct code passes on first run; there is no honest RED. What stood in for it is strictly stronger evidence and is recorded under "Non-vacuity" below: core was temporarily INVERTED, the property failed and shrank, and core was restored byte-exact.

## Non-vacuity of the D-10 ordering property

The carried obligation was explicit: show the generator reaches a floor-violating value that a permissive declared pattern would admit. Three independent pieces of evidence, weakest to strongest.

**1. By construction.** Every generated value carries a floor-denied fragment — the value is built as `base + DENIED_FRAGMENTS[i] + tail` with `base`/`tail` drawn from `[A-Z]` — and every one is paired with a declared pattern from `PERMISSIVE_PATTERNS` = `["^.*$", "^.+$", "^[\s\S]*$"]`, all of which ADMIT it. So there is no case that fails to exercise the interaction, rather than a small chance of hitting it.

**2. Asserted reachability.** Two `AtomicUsize` tallies are checked after the `proptest!` block: one counts cases that reached a floor refusal, the other counts cases that paired a floor-denied value with the MEASURED `^.*$` catch-all specifically. Both must be nonzero or the arm fails with "the property proved nothing". A generator that silently stopped producing the shape cannot pass quietly. The arm additionally asserts `refusal.rule != "pattern"` and `refusal.rule ∈ {"characterFloor", "percentEncoding"}`, and asserts the verdict is IDENTICAL with no declaration at all — which states the ordering as an equality, not only as an inequality.

**3. Shrink-reported counterexample from a deliberately inverted implementation.** `src/server/schema_validation.rs`'s `validate_path_placeholder` was temporarily changed to consult the declared pattern FIRST and return `Ok(())` on a match:

```
INVERTED:   3 property arms → 2 passed, 1 FAILED, exit 101
            property_declared_pattern_never_widens_the_floor FAILED
            minimal failing input: base = "A", tail = "", fragment_idx = 0,
                                   pattern_idx = 0, author_query = None
            i.e. the value "A?" checked against a declared `^.*$`
            path_placeholder_props_smoke_floor_refuses_cr01_payloads also FAILED
RESTORED:   sha256 73844dc8014446e85ca0da24d10d9e45ccbde988be812f8d2c811a40277ee53d
            identical before and after; `git diff 44064265..HEAD --
            src/server/schema_validation.rs` is 0 lines
```

Two characters is the whole of the D-10 hazard. The shrunk seed is committed as `path_placeholder_props.proptest-regressions` — this repo tracks sibling `*.proptest-regressions` files (the `.gitignore` entry at `:39` covers the `proptest-regressions/` DIRECTORY form only, and ten such files are already tracked) — carrying a **PROVENANCE** header stating it came from the inversion and not from a live failure, so a later reader is not misled into hunting a bug that does not exist. The same case is ALSO pinned as a deterministic assertion in the smoke arm, so deleting the seed weakens the guard but does not remove it.

## The narrowing is respected by the property

The operator narrowed the `?` rule (`Narrow the '?' rule only`), and a property that said "any path containing `?` is refused" would be false of what ships. The ordering arm therefore generates BOTH shapes and distinguishes them:

- **SHAPE A — `?` from the VALUE.** `validate_path_placeholder("v", "{base}?{tail}", &catch_all_rules)` is `Err`, with `rule` in `{characterFloor, percentEncoding}` and never `"pattern"`. An injected separator is refused by the floor, unconditionally.
- **SHAPE B — the same `?` AUTHOR-written.** The value is `[A-Z]`-only and is `Ok` under the same rules, and `validate_resolved_path("/api/{clean}")` is `Ok`. The per-value check reads the VALUE and never the template, which is simultaneously why an author's query cannot be laundered into a value refusal and why a value cannot borrow the author's exemption.
- **And the honest third fact:** `validate_resolved_path("/api/{clean}?{query}")` is `Err`. Core is strict about `?` anywhere — by design, per plan 05's instruction not to relax core — and that `Err` is precisely what the two production callers' split relaxes for an author-written separator.

The binary deliberately does NOT re-derive the split. That would be a second copy of a rule, which the plan's own prohibition forbids. The ACCEPT direction is already pinned where the split lives, and the file header names all three sets: `http::client::query_separator` (3 accept rows), `curated_path_injection.rs`'s author-query control, and `pmcp-code-mode`'s `executor::query_separator` (3 accept rows).

**Plan 05's pre-revert claim — that authors must migrate query values to body params — appears nowhere** in this plan's code, comments, rustdoc or test names. Verified by reading the three files this plan touches in `src/`.

## Mutation testing — the rows that go red, named

Two mutants, each applied, measured, and restored byte-exact (verified with `diff -q`, and with `shasum -a 256` for the core file).

**M1 — delete the `with_schema` call from `narrowed_executor`** (`assemble.rs`):

| Row | Result |
|---|---|
| `assemble::tests::narrowed_executor_attaches_a_supplied_spec` | **RED** |
| `assemble::tests::narrowed_executor_shares_one_arc` | **RED** |
| `narrowed_executor_leaves_a_spec_less_executor_alone` | green (correctly — it is the no-spec control) |
| `build_server_with_a_spec_builds_and_registers_tools` | green (correctly — the narrowing is additive, the build is not) |
| the 3 pre-existing `assemble::tests` rows | green |

`exit 101`, 5 passed / 2 failed.

**M2c — the `placeholder_rules` override's body replaced with `PlaceholderRules::default()`**, i.e. semantically "no override at all" (two earlier mutant spellings, M2 and M2b, failed to COMPILE — `E0407` and `E0794` — and a mutant that does not compile proves nothing about test sensitivity, so they were discarded rather than reported):

| Row | Binary | Result |
|---|---|---|
| `code_mode::placeholder_rules_override::a_declared_pattern_reaches_the_rules` | toolkit lib | **RED** |
| `code_mode::placeholder_rules_override::the_method_selects_the_operation` | toolkit lib | **RED** |
| `code_mode::placeholder_rules_override::the_narrowing_refuses_a_floor_clean_value_the_spec_forbids` | toolkit lib | **RED** |
| `http_executor_spec_pattern_refuses_a_floor_clean_value` | `tests/http_executor.rs` | **RED** |
| `http_executor_without_a_spec_accepts_the_same_floor_clean_value` | `tests/http_executor.rs` | green — **correctly**, and this is the point of the pair |
| the 7 other `placeholder_rules_override` rows | toolkit lib | green — they assert MISS behaviour, which the mutant preserves |
| the 11 other `http_executor` rows | `tests/http_executor.rs` | green |

Both binaries at `exit 101`. The control staying green under the mutant is what proves the narrowing — not the floor — is what refuses in the refusal row; a refusal suite whose control also went red would not distinguish "narrowing deleted" from "floor broken".

**M3 — the smoke arm temporarily `#[ignore]`d**, to exercise Codex's predicted gate failure rather than trust the prediction:

```
make test-server-toolkit → exit 2
✗ required test binary 'path_placeholder_props' RAN but passed ZERO tests —
  a #[cfg] gate turned false ... or an #[ignore] sweep landed.
```

Confirmed: an all-`#[ignore]`d binary in `REQUIRED_TEST_BINARIES` is unsatisfiable, and the non-ignored smoke arm is what makes it satisfiable. Restored byte-exact.

## Measured counts

| Command | Before | After |
|---|---|---|
| `make test-server-toolkit` | 393 | **399** (+5 `lint_against_spec_tests`, +1 property smoke arm) |
| … `✓ path_placeholder_props passed N tests` | — | **1** |
| … `✓ path_placeholder_props property arms passed N tests` | — | **3** |
| … the 4 pre-existing required binaries | 1 / 7 / 4 / 6 | **1 / 7 / 4 / 6** (unchanged) |
| `make test-server-toolkit-code-mode` | 418 | **436** (+10 override rows, +5 lint rows, +2 `http_executor` rows, +1 smoke arm) |
| … `✓ http_executor passed N tests` | 11 | **13** |
| `make test-code-mode` | 317 | **317** (unchanged) |
| `make test-openapi-server` | 42 | **46** (+4 `assemble::tests` rows) |
| `cargo test -p pmcp-openapi-server --no-fail-fast` | 42 | **46**, 0 failed |
| `make test-cargo-pmcp` | 1477 | **1477** (unchanged) |
| `cargo test -p pmcp --features full --lib schema_validation` | 52 | **52** (unchanged — the plan-02 fence) |
| `cargo nextest run --features "full" --no-fail-fast` | 3358 | **3358 run, 3358 passed**, 5 skipped, exit 0 |
| `--lib code_mode::` (openapi-code-mode) | 23 | **33** |
| `--lib config::lint_against_spec_tests` | — | **5** |
| `--lib assemble` (pmcp-openapi-server) | 3 | **7** |
| marker grep `property arm — selected by` | — | **3** |

Every arithmetic identity above closes: the toolkit legs differ from each other exactly by which features they enable (`placeholder_rules_override` needs `openapi-code-mode`, so it appears in the 436 and not in the 399).

## Verification results

| Command | Result |
|---|---|
| `RUSTFLAGS="" make quality-gate` | **`✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`** banner present. Run TWICE: once mid-plan, and again on the FINAL tree after `e7989ac2`, because a gate result on a superseded tree is not evidence about the tree being shipped. |
| `RUSTFLAGS="" cargo nextest run --features "full" --no-fail-fast` | 3358 run, **3358 passed**, 5 skipped, exit 0 |
| `RUSTFLAGS="" cargo build -p pmcp-server-toolkit --features openapi-code-mode` | exit 0, zero `error[` |
| `RUSTFLAGS="" cargo build -p pmcp-server-toolkit --no-default-features --features openapi-code-mode` | exit 0 — the `input-validation`-OFF sibling halves compile |
| `RUSTFLAGS="" cargo build -p pmcp-openapi-server` | exit 0, zero `error[` |
| `pmat quality-gate --fail-on-violation --checks complexity` | **PASSED, 0 violations** |
| `cargo clippy -p pmcp-server-toolkit --features openapi-code-mode --all-targets` | exit 0; **zero** findings naming `code_mode.rs` or `config.rs` |
| `cargo clippy -p pmcp-server-toolkit --features http,input-validation --all-targets` | exit 0; zero findings naming `path_placeholder_props.rs` |
| `cargo doc -p pmcp-server-toolkit --no-deps --features openapi-code-mode` | exit 0, **37** warnings — 38 before `e7989ac2`, which removed the one warning this plan introduced. No remaining warning names any symbol this plan added, so the crate's pre-existing baseline of 37 is unchanged. |
| `cargo doc -p pmcp-openapi-server --no-deps` | exit 0, 9 lib-doc warnings, **none** naming `narrowed_executor` / `with_schema` / `lint_against_spec` |
| SATD in the four touched source files | **0** (`grep -cE 'TODO\|FIXME\|HACK\|XXX'`) |
| `git diff 44064265..HEAD -- src/server/schema_validation.rs` | **0 lines** |
| `git ls-files --error-unmatch -- crates/pmcp-server-toolkit/tests/path_placeholder_props.rs` | exit 0 — tracked |
| sole workspace-`src` `HttpCodeExecutor::with_schema` call site | `crates/pmcp-openapi-server/src/assemble.rs:265`, inside `narrowed_executor` — **not** `#[cfg(test)]` |

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/code_mode.rs` — the `schema: Option<Arc<OpenApiSchema>>` field, `with_schema`, the public `has_schema`, the `placeholder_rules` override, `spec_placeholder_rules` (+ its `input-validation`-off sibling), `log_spec_lookup_miss`, `warn_if_narrowing_unavailable` (+ its no-op sibling), and `mod placeholder_rules_override` (10 rows).
- `crates/pmcp-server-toolkit/src/config.rs` — `ServerConfig::lint_against_spec`, the `CONFIGURED_TEMPLATE_NOT_IN_SPEC` rule constant, and `mod lint_against_spec_tests` (5 rows).
- `crates/pmcp-openapi-server/src/assemble.rs` — `narrowed_executor`, the single `Arc` wrap, the wiring at `:310` with its ordering comment, the startup emission of `lint_against_spec` findings, and 4 test rows.
- `crates/pmcp-server-toolkit/tests/http_executor.rs` — 386 → 493 lines; 11 → 13 tests. The D4(b) pair plus the `run_narrowing_probe` helper, which asserts `has_schema()` matches the configuration it claims to be in.
- `crates/pmcp-server-toolkit/tests/path_placeholder_props.rs` — **NEW**, 398 lines; 1 non-ignored + 3 `#[ignore]`d arms.
- `crates/pmcp-server-toolkit/tests/path_placeholder_props.proptest-regressions` — **NEW**, the shrunk D-10 seed with a PROVENANCE header.
- `Makefile` — `path_placeholder_props` added to `REQUIRED_TEST_BINARIES` (`:611`) and the second `--ignored property_` invocation appended to `test-server-toolkit` with its own count guard.

## Decisions Made

Recorded in `key-decisions` above. Three worth expanding:

**`has_schema()` is public and that is the point.** T-128-36c is the risk of wiring `with_schema` somewhere no test can see — a `#[cfg(test)]` helper, or after the fan-out. The field lives in `pmcp-server-toolkit`; the wiring lives in `pmcp-openapi-server`. A `#[cfg(test)] pub(crate)` accessor (the shape `inbound_token_for_test` already uses) is invisible across the crate boundary, so it could not have proven the wiring from where the wiring is. A read-only boolean over a private field exposes nothing about the document, and its rustdoc says explicitly that `false` never means "unchecked".

**`lint_against_spec` is a sibling of `lint()`, not a change to it.** `lint()` takes `&self` and is called where no spec exists; a spec-less config must produce zero findings from its own absence. Folding a spec into `lint()` would either break every existing caller or make the spec optional and the finding conditional inside a function that has no business knowing about HTTP. The new method is `#[cfg(feature = "http")]`, matching the feature that owns `OpenApiSchema`.

**The no-echo property asserts "no 6-byte run", not "no byte".** The plan's phrasing — "contains no byte of that value" — is unsatisfiable for any value at all, because the refusal is English prose and every letter of a value collides with it. Skipping length-0 and length-1 values (the plan's suggested mitigation) does not fix that; a 2-character value collides just as surely. The invariant the phrase reaches for is that no non-trivial RUN of the value survives, so the arm asserts the whole value is absent AND no 6-byte window appears. The generators draw from `[A-Z]`, which makes a 6-byte window collision-free against the refusal's lowercase prose **by construction rather than by luck** — a `[a-z0-9]` alphabet would have made the arm intermittently flaky against the message's own `%25` and its prose. The header records this reasoning so a later reader does not "tighten" it back into an unsatisfiable claim.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 — Missing Critical] `ServerConfig::lint_against_spec` required a file the plan's `files_modified` did not list**

- **Found during:** Task 1, step (2b) third bullet.
- **Issue:** The plan's `must_haves.truths` require that "a CONFIGURED template that matches no spec path is a `lint()` finding an operator can fix before deploy", and the action text says to "coordinate with plan 03's `ServerConfig::lint()`". But `lint()` lives in `crates/pmcp-server-toolkit/src/config.rs`, which is absent from `files_modified`, and Task 1's `<files>` names only `code_mode.rs`. Implementing the truth without touching `config.rs` would have meant either a second `ConfigWarning` factory in `assemble.rs` (away from the other rule constants, which is how a finding vocabulary drifts) or not implementing it at all.
- **Fix:** Added `ServerConfig::lint_against_spec` and the `CONFIGURED_TEMPLATE_NOT_IN_SPEC` constant in `config.rs`, beside the existing findings, `#[cfg(feature = "http")]`-gated, plus 5 unit rows. Additive — `lint()`'s signature and behaviour are untouched, so `cargo-pmcp/src/commands/validate.rs:738` is unaffected.
- **Verification:** `--lib config::lint_against_spec_tests` 5 passed; `make test-server-toolkit` 393 → 399.
- **Committed in:** `47ab3ddc`.

**2. [Rule 2 — Missing Critical] The new lint had no consumer, which is the documented-but-absent class this phase exists to remove**

- **Found during:** Task 1, same step.
- **Issue:** A `lint_against_spec` nothing calls is not "a finding an operator can fix before deploy" — it is a method. `cargo pmcp validate` (plan 07) has no spec in scope, and plumbing one there is plan 07's surface, not this plan's.
- **Fix:** `build_server` emits each finding as a `tracing::warn!` at startup. That is the only production point where the config and the parsed spec are both in scope — the same fact that decided Task 2's wiring site — and the deploy log is where an operator actually reads it.
- **Verification:** `build_server_with_a_spec_builds_and_registers_tools` exercises the path; `make test-openapi-server` 42 → 46.
- **Committed in:** `986f3694` (call site) / `a5168b26`.

**3. [Rule 1 — Bug] One new rustdoc warning of this plan's own making**

- **Found during:** post-Task-3 verification.
- **Issue:** The public `placeholder_rules` override's doc used an intra-doc link to the private `spec_placeholder_rules`, producing "public documentation for `placeholder_rules` links to private item". Neither `make doc-check` nor `make lint` reaches this crate (the CLAUDE.md-recorded blind spot), so `make quality-gate` was green with the warning present.
- **Fix:** Plain backticks plus the reason, so a later reader does not restore the link.
- **Verification:** `cargo doc -p pmcp-server-toolkit --features openapi-code-mode` 38 → 37 warnings; no remaining warning names a symbol this plan added.
- **Committed in:** `e7989ac2`.

**4. [Deviation from plan TEXT, not from intent] The RESEARCH probe table has SIX payloads, not five**

- **Found during:** Task 3.
- **Issue:** The plan says the smoke arm "asserts the five CR-01 payloads". RESEARCH Finding 5b's measured list is six: `2026AA?string=x`, `current/../../search/current`, `%2e%2e%2f`, `%3Fstring%3Dx`, `a%00b`, `a#frag`.
- **Fix:** All six are asserted, and the constant's doc says why the count differs from the plan's. **The count was not lowered to match the plan** — the measurement is the reason the ordering is the way it is, so asserting a subset would weaken the arm to match a typo.
- **Verification:** the smoke arm passes; a seventh assertion pins the shrunk `"A?"` case.
- **Committed in:** `eb1233ec`.

**5. [Deviation from plan TEXT] `PERMISSIVE_PATTERNS` is three patterns, not "a few realistic spec patterns"**

- **Found during:** Task 3.
- **Issue:** The plan asks for a pattern set including "the permissive catch-all and a few realistic spec patterns". A REALISTIC pattern (`^[a-z]+$`, `^C[0-9]+$`) would REFUSE the generated floor-violating value on its own, so `refusal.rule` could legitimately be `"pattern"` and the arm's central assertion (`rule != "pattern"`) would fail spuriously — the property would be flaky rather than wrong.
- **Fix:** The set is restricted to three patterns that all provably ADMIT every generated value (`^.*$`, `^.+$`, `^[\s\S]*$`), which is what makes "the FLOOR refused it, not the pattern" a sound assertion. Realistic narrowing patterns are exercised where they belong — as fixtures with known-matching and known-violating values, in `mod placeholder_rules_override` and `tests/http_executor.rs`.
- **Verification:** 3 property arms pass; the inversion still kills the ordering arm, so the restriction did not weaken it.
- **Committed in:** `eb1233ec`.

**6. [Deviation from plan STRUCTURE] Task 3 has one commit, not a RED/GREEN pair**

Reasoned in full under "TDD Gate Compliance". A property over already-correct, explicitly-fenced code cannot be RED without modifying the fence.

---

**Total deviations:** 6 — 2 missing-critical (Rule 2), 1 bug (Rule 1), 3 plan-text/structure corrections.
**Impact on plan:** no scope creep outside the phase's own crates. Deviations 1 and 2 add one method and one startup log required by a `must_haves` truth; 4 and 5 correct plan text in the direction of a STRONGER assertion, never a weaker one.

## Issues Encountered

1. **`check tdd-red-evidence` cannot classify a cargo run.** Root-caused to node:test-TAP-only parsing, resolved with a line-for-line adapter. Full detail under "TDD Gate Compliance (b)". Not a defect in this plan's RED phases; a handoff.

2. **Two mutant spellings failed to COMPILE before M2c worked.** Deleting the override outright left the impl block naming a non-trait method (`E0407`); adding a turbofish to keep the helper "used" hit `E0794` (explicit lifetime args on a late-bound parameter). A mutant that does not compile is evidence of nothing, so both were discarded rather than reported as kills. Recorded because the temptation to report a compile failure as a mutation kill is exactly how mutation testing becomes theatre.

3. **`cargo nextest` reported 2 failures that were not mine.** `docs04_examples_run::doc_review_team_runs_to_completion` and `…::s50_standalone_vs_sampled_runs_to_completion` — the staleness-guard example binaries that FAIL rather than skip when absent from `target/`. Building `-p pmcp-team-servers --example doc_review_team --features runtime` and `-p pmcp-agent --example s50_standalone_vs_sampled` cleared both with **no code change**, and the re-run is 3358/3358. A reader who saw only the first run would have attributed them to this plan.

4. **The first `make quality-gate` ran against a superseded tree.** It was launched before `e7989ac2` (the rustdoc fix) landed. Rather than reason that a doc comment cannot affect a gate, the gate was re-run on the final tree; the banner in the verification table is from that second run. A gate result on a tree you are not shipping is not evidence about the tree you are.

## Broken-windows ledger

**No entry appended.** No stub, no `t.skip`, no unrun `<verify>`, no `TODO`/`FIXME`. Every `<verify>` command in the plan was run and is recorded above.

One item is deliberately NOT a ledger entry and is flagged here instead so the judgment is visible: the `#[cfg(not(feature = "input-validation"))]` siblings of `spec_placeholder_rules` and `warn_if_narrowing_unavailable` are **compiled** (proven by the `--no-default-features --features openapi-code-mode` build) but not **test-exercised**. That mirrors the established shape in `src/http/client.rs`, whose three `input-validation`-off halves are likewise compile-only, and the off-half is the strictly-safer direction (it returns the floored-and-capped default). Turning it into a ledger entry would block `/gsd-ship` on a pattern the crate already ships four instances of.

## Concerns to carry forward

- **`with_schema` is a builder, and a builder can be skipped.** Exactly one production call site exists today and the SUMMARY pins its line number relative to both fan-outs, but nothing in the type system prevents a future `HttpCodeExecutor` construction site from omitting it. The cheapest future guard would be a test asserting the count of `.with_schema(` call sites in `crates/*/src/`, or a constructor that takes `Option<Arc<OpenApiSchema>>` positionally when the next breaking window opens.
- **The lookup is exact-template, by decision.** Canonicalization was rejected with reasons (it can narrow from the wrong parameter). The debug log and the config lint bound the cost, but a Code Mode script that composes `/users/{alias}` at runtime against a spec declaring `/users/{id}` still loses the narrowing with only a `debug!` to show it. That is the honest residual and it is stated in the rustdoc rather than papered over.
- **`make test-server-toolkit` now runs `cargo test` twice.** The second invocation recompiles nothing (same feature set, same binary) but does re-run the three property arms at 256 cases each — measured at ~0.09s, so the cost is noise. If a future plan adds expensive property arms to this binary, that is where to look first.

## Hand-offs to plan 128-11 (do NOT act on these here — D-14 requires ONE commit)

Added to the nine already recorded by earlier plans:

- **H-08a — `pmcp-server-toolkit` gains public API and needs a MINOR bump.** New public items: `HttpCodeExecutor::with_schema`, `HttpCodeExecutor::has_schema`, `ServerConfig::lint_against_spec`, `config::CONFIGURED_TEMPLATE_NOT_IN_SPEC`. All additive — no existing signature changed, and `HttpCodeExecutor::new`'s signature is unchanged by design (verified: `cargo build -p pmcp-openapi-server` exit 0 with every pre-existing construction site untouched). No pin needs to move: `pmcp-openapi-server` reaches the toolkit by path.
- **H-08b — no new feature edge and no new dependency.** `spec_placeholder_rules` is gated on the EXISTING `openapi-code-mode` + `input-validation` pair. Deliberately NOT added: `input-validation` to `openapi-code-mode`'s feature list. The off-half is safe (floor and cap still run in `PlanExecutor` over `pmcp-code-mode`'s unconditional `pmcp/schema-validation`) and widening a published feature's list is 128-11's call, not this plan's.
- **H-08c — not a version item, but a runtime-wide gap.** `gsd_run check tdd-red-evidence` parses node:test TAP only and returns `INVALID_RED (zero_tests_discovered)` for any cargo run. Every `type: tdd` plan in this Rust repo will hit it. Worth raising upstream rather than each plan re-inventing an adapter.

## Next Phase Readiness

**D4(b) is complete on both HTTP surfaces.** The curated surface landed in plan 06; the Code Mode surface lands here, wired to the operator's real spec through the binary's only production construction path.

Ready for:

- **128-09 / 128-10** — `ServerConfig::validation_report()` still has no production consumer, so the once-at-startup enforcement log remains that plan's work. `build_server` is now proven to be the right place to put it: this plan established that it is where the config and the spec meet, and it now already emits one findings stream there. `lint_against_spec` is available to join it.
- **128-10's root-package property arm** (`tests/schema_validation_props.rs`) — this binary's header names it as the arm `make test-property` can actually select, since that leg is root-scoped (SP-3). The three arms here are reached by `make test-server-toolkit`'s second invocation instead.
- **128-11** — the three hand-offs above.

No blockers.

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-27*

## Self-Check: PASSED

All seven named files exist on disk. All six task/follow-up commit hashes resolve in
`git log --all`. `src/server/schema_validation.rs` is byte-identical to the plan base
(`git diff 44064265..HEAD` on it = 0 lines). `make quality-gate` printed the
`ALL TOYOTA WAY QUALITY CHECKS PASSED` banner on the final tree.
