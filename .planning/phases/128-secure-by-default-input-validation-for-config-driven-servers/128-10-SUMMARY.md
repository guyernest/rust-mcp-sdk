---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 10
subsystem: testing
tags: [threat-model, fuzzing, libfuzzer, proptest, sc-6, sc-7, sc-8, mutation-testing, ci-gate, redos]
status: complete

requires:
  - phase: 128-01
    provides: "`input-validation` feature; `ValidatingToolHandler` at all three handler push sites; the validator cache"
  - phase: 128-02
    provides: "`validate_input`, `render_refusal`, `check_input_schema_compiles`, `validate_path_placeholder`, `PlaceholderRules`, `safe_pointer`'s `<redacted>` token"
  - phase: 128-03
    provides: "`[server.validation]`; `ValidatingToolHandler::enforce_schema` separated from the decorator's existence"
  - phase: 128-06
    provides: "`substitute_path` / `check_composed_path`; `Parameter::placeholder_rules`"
  - phase: 128-08
    provides: "The non-vacuity technique: invert the implementation, watch the property fail, restore byte-exact"
  - phase: 128-09
    provides: "`ToolkitHooks::argument_validator_for` consulted at `tools.rs:237`; `argument_validator_seam` rows"
provides:
  - "SC-6 CLOSED: eight enforcement claims in the toolkit restated to name their enforcing function AND their condition, each backed by a MEASURED failing-on-removal acceptance row"
  - "The SC-6 sweep's METHOD, its 36-phrase candidate list verbatim, its POSITIVE CONTROL, and a per-hit disposition ledger over all 125 hits"
  - "`tests/script_tool.rs::script_tool_refuses_a_schema_violating_arg_before_the_script_runs` — the T-90-05-03 VALIDATION row (the pre-existing row asserted BINDING)"
  - "`fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` — SC-7's no-echo rule as an invariant over arbitrary bytes, with a PROVENANCE oracle"
  - "`fuzz/fuzz_targets/fuzz_placeholder_pattern_redos.rs` — the instrument pointed at RESEARCH assumption A2"
  - "Two committed hand-written seed corpora (10 + 8 seeds, 2 READMEs) plus the narrow `fuzz/.gitignore` un-ignore that makes them carryable"
  - "`tests/schema_validation_props.rs` — the ROOT-package property arm `make test-property` can actually select (3 -> 4)"
  - "`make test-fuzz-strict` — a fuzz leg that PROPAGATES a crash, chained into `quality-gate`, with all three guard branches measured"
  - "`.github/workflows/ci.yml`: nightly (NON-DEFAULT) + `cargo-fuzz` in the quality-gate job — the only option that delivers a merge gate, chosen on a measurement of the org ruleset"
  - "Both targets enrolled in `fuzz.yml`'s matrix for the deep daily campaign"
affects: [128-11]

tech-stack:
  added: []
  patterns:
    - "A threat comment names its ENFORCING FUNCTION, its CONDITION, and the acceptance row that fails when the enforcement is removed — three parts, none optional"
    - "A retired claim's wording is quoted nowhere in `src/`, so a grep gate keeps distinguishing a surviving claim from a note about one"
    - "A fuzz oracle asserts PROVENANCE (a per-case sentinel verified absent from the declaration), never string ABSENCE, which a declaration legitimately violates"
    - "A property generator may use the strict absence oracle BECAUSE it controls both sides — with the disjointness premise asserted by a non-ignored companion test, not assumed"
    - "Bound a fuzz target's reachable input space where a process-global cache would otherwise turn a correctness fuzzer into a memory-exhaustion one, and STATE the bound arithmetically"
    - "Prove a new gate leg can FAIL before chaining it: apply a probe, observe the non-zero exit, restore byte-exact"

key-files:
  created:
    - fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs
    - fuzz/fuzz_targets/fuzz_placeholder_pattern_redos.rs
    - fuzz/corpus/fuzz_input_schema_enforcement/ (10 seeds + README)
    - fuzz/corpus/fuzz_placeholder_pattern_redos/ (8 seeds + README)
    - tests/schema_validation_props.rs
    - tests/schema_validation_props.proptest-regressions
  modified:
    - crates/pmcp-server-toolkit/src/tools.rs
    - crates/pmcp-server-toolkit/src/config.rs
    - crates/pmcp-server-toolkit/tests/script_tool.rs
    - fuzz/Cargo.toml
    - fuzz/.gitignore
    - Makefile
    - .github/workflows/ci.yml
    - .github/workflows/fuzz.yml

key-decisions:
  - "The CI home is option (a) — nightly + cargo-fuzz provisioned in the quality-gate job — chosen on a MEASUREMENT, not a preference: the org ruleset 'Green Main — unified gate enforcement' requires exactly one status context, `gate`, and `fuzz.yml` is NOT a required check, so option (b) alone falls to the plan's own rejection condition and option (c) alone cannot fail in CI. Option (b) is adopted additively as the deep campaign."
  - "Nightly is installed NON-DEFAULT. `dtolnay/rust-toolchain@nightly` would silently move the quality-gate job's fmt-check / lint / test-all onto nightly clippy, whose lint set differs from stable's; CLAUDE.md names toolchain mismatch as the #1 cause of CI failures, and moving the whole gate to nightly is a far larger change than adding a fuzz leg."
  - "The plan's `fuzzing`-gated UNCACHED seam in `src/server/schema_validation.rs` was NOT added — the dispatch required a zero-line diff on that file. The cache hazard is closed instead by a BOUNDED SCHEMA PROJECTION (5760 distinct texts, measured rss 544 MB over 108k runs), while the ARBITRARY document still drives `check_input_schema_compiles`, which is public and uncached. Recorded as a deviation and a WINDOWS entry; the seam remains the durable fix."
  - "The fuzz oracle is provenance-based and therefore needs no permitted-substring allowlist — so it does not special-case `safe_pointer`'s `<redacted>` token, because the token cannot contain the sentinel. Redaction is indistinguishable from never having held the key, which is the property under test."
  - "`make test-fuzz`'s blanket `|| echo` is left EXACTLY as it is, with a breadcrumb naming `test-fuzz-strict`. Narrowing it spans 27 pre-existing targets and could turn the gate red on someone else's defect."
  - "The CHANGELOG D-15 obligation that 128-03-SUMMARY.md:471 assigned to plan 10 is discharged by REASSIGNMENT to 128-11, which carries it verbatim as a `must_haves` truth plus a machine gate. Plan 10 did not write a partial entry plan 11's Task 3 would overwrite."

requirements-completed: [SC-6, SC-7, SC-8]

coverage:
  - id: D1
    description: "Eight enforcement claims in the toolkit are restated to name their enforcing function and their condition, and each is backed by an acceptance row MEASURED to go red when the enforcement is removed"
    requirement: "SC-6"
    verification:
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs#input_validation_refuses_undeclared_argument_without_contacting_upstream"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs#input_validation_refuses_absent_arguments_when_required_declared"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/script_tool.rs#script_tool_refuses_a_schema_violating_arg_before_the_script_runs"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/tools.rs#tools::argument_validator_seam::a_registered_validator_still_runs_with_enforce_input_schema_false"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/curated_path_injection.rs#curated_path_injection_refuses_traversal_via_a_placeholder_value"
        status: pass
      - kind: other
        ref: "mutation 1 (validate_input removed from check_schema) -> 3 rows red; mutation 2 (script push site undecorated) -> 1 row red, ISOLATED; mutation 3 (registry conjunct dropped) -> 1 row red; mutation 4 (check_placeholder_value neutered) -> 4 rows red. All restored shasum-byte-exact."
        status: pass
    human_judgment: false
  - id: D2
    description: "No remaining comment in `crates/pmcp-server-toolkit/src/` claims a mitigation the code does not implement, established by a reviewed sweep whose method and positive control are recorded"
    requirement: "SC-6"
    verification:
      - kind: other
        ref: "grep -rn 'enforced upstream' / 'rejected by pmcp' / 'schema-validated' over crates/pmcp-server-toolkit/src/ -> 0, 0, 0"
        status: pass
      - kind: manual_procedural
        ref: "36-phrase candidate sweep, 125 deduped comment-line hits, every hit read and dispositioned — see the ledger in this SUMMARY"
        status: pass
    human_judgment: true
    rationale: "A grep can find candidate phrasings but cannot decide whether a claim matches the code; `128-VALIDATION.md` § Manual-Only Verifications records this as manual by construction. The sweep was CALIBRATED against two known-stale claims before they were fixed (it found both) and it found a THIRD the plan did not name, which is the evidence that the method is sensitive. The judgment on the 117 unchanged hits is a human's to check."
  - id: D3
    description: "SC-7's no-echo rule holds over arbitrary bytes, not only over a dozen fixtures, and the target is PROVEN able to fail"
    requirement: "SC-7"
    verification:
      - kind: other
        ref: "cargo +nightly fuzz run fuzz_input_schema_enforcement -- -max_total_time=30 -> 108317 runs, cov 11526, 0 artifacts, exit 0"
        status: pass
      - kind: other
        ref: "NON-VACUITY: sentinel pushed into render_refusal's `declared` list -> exit 1, 'SC-7 VIOLATION: the rendered refusal echoed caller data', crash artifact written; restored byte-exact"
        status: pass
      - kind: unit
        ref: "tests/schema_validation_props.rs#property_refusal_never_echoes_input"
        status: pass
    human_judgment: false
  - id: D4
    description: "RESEARCH assumption A2 has a cheap recurring instrument pointed at it, and its residual is instrumented rather than mitigated on a hunch"
    requirement: "SC-7"
    verification:
      - kind: other
        ref: "cargo +nightly fuzz run fuzz_placeholder_pattern_redos -- -max_total_time=30 -timeout=5 -> 60751 runs, cov 14497, 0 artifacts, NO timeout, exit 0"
        status: pass
      - kind: other
        ref: "NON-VACUITY: FLOOR_DENIED swapped for a floor-ACCEPTED value -> exit 1, 'D-10 ORDERING VIOLATION'; restored byte-exact"
        status: pass
    human_judgment: true
    rationale: "A 30-second run that finds nothing is evidence of absence at that budget, not proof. The claim this deliverable makes is 'A2 now has an instrument', and whether the instrument's budget and its bounded-run cache caveat are adequate is a judgment. The deep campaign in `fuzz.yml` at `-max_total_time=300` daily is the recurring half."
  - id: D5
    description: "The ALWAYS-property leg selects something for this phase"
    requirement: "SC-8"
    verification:
      - kind: unit
        ref: "make test-property -> 3 before, 4 after; tests/schema_validation_props.rs appears in the run with `running 1 test`"
        status: pass
      - kind: unit
        ref: "tests/schema_validation_props.rs#alphabets_are_disjoint"
        status: pass
    human_judgment: false
  - id: D6
    description: "A crash in this phase's fuzz targets can fail the merge gate, and the leg was proven to propagate before being chained"
    requirement: "SC-7"
    verification:
      - kind: other
        ref: "make test-fuzz-strict with the oracle probe applied -> exit 2, 'Error: Fuzz target exited with exit status: 77'; clean re-run exit 0"
        status: pass
      - kind: other
        ref: "guard branches: present -> exit 0 with both targets run; missing + CI unset -> exit 0, RED named skip, nothing attempted; missing + CI=1 -> exit 2 with HARD FAILURE"
        status: pass
      - kind: other
        ref: "make quality-gate exit 0 with the ALL TOYOTA WAY QUALITY CHECKS PASSED banner; the strict leg observed running inside it at captured line 15842, passing at 17751"
        status: pass
    human_judgment: true
    rationale: "The CI half CANNOT be verified from here. `ci.yml`'s nightly + cargo-fuzz install has never executed on a clean GitHub runner, and the plan itself says the first CI run IS the measurement and that a `cargo fuzz build` failure there is a finding rather than a flake. A human must read that first run."

metrics:
  duration: "~3h"
  completed: 2026-09-28

actuals:
  tokens: 25670
  tasks: 3
  commits: 3
plan_head_before: 54bfaf0c343dd54ae1ebc61aeb2bb32e851fcd6b
# `commits: 3` is MEASURED as `git rev-list --count 54bfaf0c..HEAD` at SUMMARY-write
# time: caafe27e, 8fa8cbba, 9fec4900 — the three task commits. Two docs commits
# follow (this SUMMARY, then the STATE.md/ROADMAP.md metadata commit the SDK writes
# separately), so a later `/gsd-verify-work` re-measure will see **5**. Both figures
# are given because plan 09 recorded exactly this trap: the number has to be
# measured, and a single figure written mid-sequence is the count at its own write
# moment rather than at the tip. `tokens`: `git diff 54bfaf0c..HEAD | wc -c` ==
# 102683, /4 — the estimateTokens scale, NOT a harness token count.
---

# Phase 128 Plan 10: SC-6's sweep, SC-7's durable guard, and a fuzz leg that can fail Summary

**Every enforcement claim in the toolkit now names its enforcing function, its condition,
and an acceptance row measured to go red when the enforcement is removed — including a third
stale claim the plan did not name and the sweep found; SC-7's no-echo rule became a
provenance-oracle fuzz invariant over arbitrary bytes rather than a dozen fixtures; and
`make test-fuzz-strict` was PROVEN to propagate a crash (exit 2, status 77) before being
chained into a `quality-gate` job that now provisions the nightly toolchain it needs —
because the org ruleset was measured and `fuzz.yml` is not a required check.**

## Performance

- **Duration:** ~3 h
- **Tasks:** 3 of 3
- **Files:** 6 created, 8 modified (32 paths touched counting the 20 corpus files)

---

# SC-6 — the sweep, with its METHOD and its POSITIVE CONTROL

A zero-count sweep that cannot find a known-stale claim proves nothing. The two claims the
orchestrator named were used as the calibration, **before** they were fixed.

## The candidate-phrase list, VERBATIM (36 phrases)

Recorded in full so a later reviewer can judge its COVERAGE and not only its results — the
plan's first draft used three phrases and was measurably incomplete.

```
enforced upstream      rejected by pmcp       schema-validated
enforced by            enforced at            enforced in
is enforced            are enforced           enforces
validated against      validated by           validated before
checked against        checked by             checked before
refused before         refuses                rejects
guaranteed             guarantees             cannot reach
never reaches          bounded by             defence-in-depth
defense-in-depth       upstream               does not exist yet
exists yet             lands with             when the registry
not yet                will be                for now
currently              sanitiz                prevent
```

Three classes are represented deliberately: **locating** phrases (`upstream`, `enforced at`),
**asserting** phrases (`is enforced`, `guarantees`, `refuses`), and — the class that produced
the second known-stale claim — **temporal** phrases (`does not exist yet`, `lands with`,
`not yet`, `will be`, `currently`). The temporal class is the documented-but-absent defect
INVERTED, and no list built only from the first two classes can reach it.

## Method

```bash
/usr/bin/grep -rnE --include="*.rs" '^[[:space:]]*(//|///|//!)' crates/pmcp-server-toolkit/src/ \
  | /usr/bin/grep -E -- "<36-phrase alternation>" | /usr/bin/sort -t: -k1,1 -k2,2n -u
```

Restricted to COMMENT lines, because SC-6 is about comments and not about identifiers — the
unrestricted form returns 70 hits for `refuses` alone, most of them test-function names.
`--include` is **quoted** (plan 05 lost a whole gate to zsh not globbing it unquoted), every
utility is an absolute path (rtk rewrites `grep -c`), and the result is deduped so a line
matching four phrases is read once.

**Deduped result: 125 comment lines. Every one was read.**

## POSITIVE CONTROL — the calibration, run before any fix

| Phrase | Count on the PRE-FIX tree | What it caught |
|---|---|---|
| `enforced upstream` | **1** | `tools.rs:996` — the T-90-03-01 claim the orchestrator named |
| `does not exist yet` | **1** | `tools.rs:219` — the `enforce_input_schema` registry claim the orchestrator named |
| `exists yet` | **2** | the same, plus a legitimate `policy.rs:186` use |
| `lands with` | **1** | the same `tools.rs:219` paragraph |
| `schema-validated` | **3** | `tools.rs:1059`, `:1071`, `:1141` — confirming Fable's two-hit finding, plus the already-tracked one |
| `rejected by pmcp` | **1** | `tools.rs:17` — the T-83-05-02 claim |

**The sweep found BOTH known-stale claims before they were fixed, by two independent phrase
classes.** That is the calibration. It then found a **THIRD** the plan did not name — see
the ledger below — which is the evidence that the method has sensitivity beyond its
calibration set rather than having been reverse-engineered from it.

## Disposition ledger — all 125 hits

### CHANGED (8)

| # | Site (pre-fix line) | Was | Now | Failing-on-removal row |
|---|---|---|---|---|
| 1 | `tools.rs:16-17` (T-83-05-02) | "Unknown argument keys are **rejected by pmcp's request-validation path at `tools/call` time**" | Refused by `validate_input` in the `ValidatingToolHandler` decorator `enforce_input_schema` wraps — **in the toolkit, before the backend call**. States explicitly that core's `tools/call` dispatch does NOT validate arguments (D-01 defers it), names the `input-validation` feature and the `enforce_input_schema` flag as conditions. | `input_validation_refuses_undeclared_argument_without_contacting_upstream` (row 10) + row 9, with row 11 as the accept control |
| 2 | `tools.rs:219-224` | "**No validator registry exists yet (it lands with E2)**, so `has_registered_validator` is `false` today … When the registry arrives, only that one input changes." | The registry IS `ToolkitHooks`, consulted via `argument_validator_for`. The retired sentence is named as the documented-but-absent class INVERTED — in the very function that closes the gap — and the binding it named no longer exists. `:236`'s historical comment is KEPT, as instructed. | `argument_validator_seam::a_registered_validator_still_runs_with_enforce_input_schema_false` |
| 3 | `tools.rs:995-996` (T-90-03-01) | "arg injection is bounded by the object-envelope schema … **enforced upstream**" | Two named local checks: (1) the envelope via the decorator's `validate_input`; (2) every placeholder via `validate_path_placeholder` in `substitute_path` plus `validate_resolved_path` via `check_composed_path`. Both conditions named. | rows 9/10/11 for (1); `curated_path_injection_refuses_{traversal,a_query}_via_a_placeholder_value` for (2) |
| 4 | `tools.rs:1059` (T-90-05-03) | "`args` are **schema-validated** against this BEFORE the script runs" | Checked by `validate_input` inside the decorator, so the refusal precedes `handle` and therefore precedes any backend call. Names the condition, and names the binding-vs-validation distinction explicitly. | **NEW** `script_tool_refuses_a_schema_violating_arg_before_the_script_runs` |
| 5 | `tools.rs:1071` | "a script tool's `args` are **schema-validated** identically" | Checked against an identically-shaped schema by the SAME enforcer — one `enforce_input_schema` per push site, one `validate_input` inside the decorator. Points at `ScriptToolHandler::tool_info` for condition + row. | same as 4 |
| 6 | `tools.rs:1141` | "(2) Bind the **schema-validated** client args" | These args have already been checked by `validate_input` in the decorator; states that this is a fact about the DECORATOR and that `handle` performs no validation of its own. | same as 4 |
| 7 | `tools.rs:633` (T-84-03-01) | "extra keys are silently dropped — JSON-schema validation **rejects them upstream**" | **FOUND BY THE SWEEP, not by the plan's list.** Restated: the refusal is `validate_input` in the decorator, and the local filter is the only remaining layer when `additional_properties = true` — which is why the drop stays rather than becoming an assertion. | row 10 for the schema layer; the module's own `extract_params` unit tests for the filter |
| 8 | `config.rs:1019` | "Whether a declared `inputSchema` is CHECKED at `tools/call` time." — TRUE but UNNAMED | Names `validate_input` / `ValidatingToolHandler` in `crate::tools`, and says explicitly that it is not core's dispatch. | rows 9/10 (same enforcer) |

Hit 7 is the finding that matters most about the method: it sits one function-group from the
claim the plan already knew about, in the same file, and neither of the plan's two named
greps could reach it. Hit 8 was actioned under the plan's own disposition rule ("true but
unnamed → add the name") even though `config.rs` was not in `files_modified`; recorded as
Deviation 1.

### UNCHANGED (117), by disposition class

Every line below was read. The four classes:

**(a) Not an enforcement claim** — describes the local behaviour of the item it documents, or
is a test-name / historical / DoS-bound note. **51 hits.**
`code_mode.rs:` 665 782 785 1341 1342 2209 2231 2903 · `config.rs:` 10 20 167 237 294 348 427
468 1052 1368 1811 2066 2203 2243 2924 2957 3390 3613 3644 3687 · `env_ref.rs:` 51 108 ·
`error.rs:` 240 250 · `http/auth.rs:` 87 101 123 555 1263 · `http/client.rs:` 563 1012 1483 ·
`tools.rs:` 265 357 637 680 1037 1581 1841 1854 2294 2433 2450 · `workbook/error.rs:` 161 ·
`workbook/handler.rs:` 1068 1120 · `workbook/input.rs:` 45

**(b) Enforcement claim that ALREADY names its enforcer** — the shape SC-6 asks for, already
satisfied. **42 hits.**
`code_mode.rs:` 455 (the `flavor`-selected validation surface) 1125 (`log_spec_lookup_miss`,
`lint_against_spec`) 1534 (`HmacTokenGenerator`) · `config.rs:` 1449 1497 1546 1606 2728 2733
(`pmcp-package` re-parsing the same config) 1872 (`format`, Phase 128 Q1) 1480 1216
(`deny_unknown_fields`) · `http/auth.rs:` 601 602 610 (`AuthConfig::malformed_env_ref_field`,
`ServerConfig::validate`) · `http/client.rs:` 168 174 186 190 194 224 229 1014 1043 (all name
`validate_path_placeholder` / `validate_resolved_path` / `check_composed_path`) · `policy.rs:`
46 85 89 103 186 559 · `tools.rs:` 447 (`ServerConfig::validate` + `build_input_schema`) 572
(structural: "no code path that builds a JSON array") · `workbook/handler.rs:` 139 178 224 783
868 · `workbook/render_uri.rs:` 16 60 62 81 158 (`decode`, T-92-14/V12) ·
`workbook/schema.rs:` 10 44 415 421 844 · `resources.rs:` 21

**(c) `upstream` / `downstream` in a NON-enforcement sense** — a backend, a dependent crate, an
env-resolution layer, an output producer. **8 hits.**
`code_mode.rs:` 1140 · `config.rs:` 1205 1219 · `error.rs:` 6 · `http/mod.rs:` 159 ·
`sql/mod.rs:` 4 5 · `workbook/schema.rs:` 411 · `prompts.rs:` 15 19 251 377 ·
`workbook/render_uri.rs:` 278 · `lib.rs:` 111 · `resources.rs:` 185 · `sql/mod.rs:` 77

**(d) NEGATIVE claims — states that enforcement is ABSENT under a stated condition.** **4
hits**, and these are the MODEL for what SC-6 wants rather than exceptions to it:
`config.rs:912` (under `not(feature = "input-validation")` a declared `pattern` is "neither
verified here nor enforced at call time", with the reason it is not a silent skip),
`config.rs:1883` (a typo'd `format` name "silently enforces nothing"),
`config.rs:970` ("Free-form for now"), `workbook/render_resource.rs:100` (`mode` "is NOT
re-validated against the manifest", with why).

### Acceptance gates, final

| Gate | Result |
|---|---|
| `grep -rn "enforced upstream" crates/pmcp-server-toolkit/src/ \| grep -c .` | **0** |
| `grep -rn "rejected by pmcp" …` | **0** |
| `grep -rn "schema-validated" …` | **0** (was 3) |
| `grep -c "validate_input\|validate_path_placeholder" crates/pmcp-server-toolkit/src/tools.rs` | **21** (>= 4; was 11) |
| No comment claims enforcement located in core's `tools/call` dispatch | confirmed — three of the eight rewrites say explicitly that it does NOT |
| SATD in every touched file | **0** |

**The retired wordings are quoted nowhere in `src/`.** The first draft of the replacement at
`tools.rs:689` cited one verbatim and **turned the gate red on itself** (count 2, then 1) — the
cheapest possible demonstration that the gate is sensitive, and the reason the comment now
says so. The full before/after text lives in this SUMMARY instead, which is what the comment
points at.

## Mutation testing — each claim's row, measured

Applied, measured, restored, restoration `shasum`-verified against a pre-mutation copy. Zero
`MUTATION` markers remained afterwards.

| # | Mutation | Rows turned red |
|---|---|---|
| 1 | `validate_input` call removed from `ValidatingToolHandler::check_schema` | **3** — `script_tool_refuses_a_schema_violating_arg_before_the_script_runs`; `input_validation_refuses_absent_arguments_when_required_declared`; `input_validation_refuses_undeclared_argument_without_contacting_upstream` |
| 2 | decorator dropped at **push site 2 of 3 (SCRIPT) only** | **1, ISOLATED** — only the script row. The four HTTP acceptance rows stayed GREEN, which is what proves the T-90-05-03 claim has its OWN row rather than inherited coverage from the HTTP site |
| 3 | `registered_validator.is_none()` conjunct dropped from the early return | **1** — `argument_validator_seam::a_registered_validator_still_runs_with_enforce_input_schema_false` |
| 4 | `check_placeholder_value` neutered (T-90-03-01's path half) | **4** — `curated_path_injection_{cap_independent_of_d3, refuses_a_query_via_a_placeholder_value, refuses_traversal_via_a_placeholder_value, spec_allow_reserved_does_not_widen_the_floor}` |

Mutation 2 is the load-bearing one. Mutation 1 alone would have let "the script claim is
backed" pass on coverage that actually belongs to the HTTP path; isolating the push site is
what distinguishes a row for THIS claim from a row that happens to fail alongside it.

---

# SC-7 — two fuzz targets, both proven able to fail

## The oracle is PROVENANCE-based

The naive oracle ("the refusal contains no substring of the instance") rejects LEGITIMATE
output three ways: a DECLARED name is intentionally echoed on a `maxLength` violation, the
`additionalProperties` message intentionally lists the allowed names, and an independent
schema and instance can share strings by coincidence. A target that cries wolf gets its
assertion relaxed until it asserts nothing (T-128-49b).

So: a per-case sentinel (`Zq7PHI` + FNV-1a hex — a shape no template can hold), **verified
absent from the serialized schema first** (skip on collision, never fail), placed as a
DECLARED name's value, as an UNDECLARED key, and as both, asserted absent from
`render_refusal`'s output **and** from every `InputViolation::{pointer,expected}`, on BYTES
rather than `char` boundaries.

A consequence worth naming: a provenance oracle needs **no permitted-substring allowlist**, so
it does not special-case `safe_pointer`'s `<redacted>` token (read from `128-02-SUMMARY.md`,
not reinvented). The token cannot contain the sentinel, so redaction is indistinguishable from
never having held the key — which is exactly the property under test.

## The cache hazard, and the DEVIATION it forced

`validate_input` memoizes compiled validators in a process-global `Mutex<HashMap<String, …>>`
keyed on schema TEXT. Arbitrary schemas would grow it unboundedly and an OOM would be
indistinguishable from a clean run (T-128-49c). The plan's remedy was a `fuzzing`-gated
UNCACHED seam in `src/server/schema_validation.rs`; this dispatch required a **zero-line diff**
on that file. Both constraints are satisfied differently:

- the schema reaching `validate_input` is a bounded **PROJECTION** of the schema document,
  rebuilt from a fixed template table whose size is `4 x 8 x 5 x 9 x 4 = 5760` distinct
  texts — a hard cap on cache entries. The projection is chosen by the document's **own**
  declared keywords, which is what makes the hand-written seeds exercise
  `additionalProperties` / `maxLength` / `pattern` / `required` / `patternProperties` / `enum`
  / `format` / `items` for real rather than nominally;
- the **arbitrary** document still drives `check_input_schema_compiles`, which is PUBLIC and
  calls `compile_input_2020_12` directly — the uncached path. Arbitrary-schema compile
  coverage is retained at zero cache cost.

Measured **rss 544 MB over 108 317 runs** confirms the bound holds. Residual, stated in the
target header and in `WINDOWS.md` #78: a defect reachable only from a schema shape outside the
template table is out of reach.

The ReDoS target CANNOT be projected — an arbitrary pattern is the input under test — so its
cache growth is bounded by the RUN (`MAX_PATTERN_LEN` caps entry size, `-runs` /
`-max_total_time` cap the count) and its header says in as many words not to read an
unbounded-run OOM as an A2 reproduction. `WINDOWS.md` #79.

## A third invariant, not in the plan: gate/runtime AGREEMENT

When `check_input_schema_compiles` accepts a schema, `validate_input` must not then refuse
every call with the `keyword: "schema"` uncompilable violation. Two entry points onto one
compile; if they disagree, SC-2's config-time gate stops predicting runtime behaviour and its
whole value evaporates silently. Free to assert here and asserted nowhere else.

## NON-VACUITY — proven, not asserted

| Probe | Result |
|---|---|
| sentinel pushed into `render_refusal`'s `declared` list (which it echoes as the allowed-name set) | **exit 1** · `SC-7 VIOLATION: the rendered refusal echoed caller data` · crash artifact `crash-a4dc0a21…` written |
| `FLOOR_DENIED` swapped for a value the floor ACCEPTS | **exit 1** · `D-10 ORDERING VIOLATION: a declared pattern widened the unconditional floor` |
| the same probe run through **`make test-fuzz-strict`** | **exit 2** · `Error: Fuzz target exited with exit status: 77` · `make: *** [test-fuzz-strict] Error 1` |

All three restored byte-exact (`shasum -c` OK), artifacts removed, corpora pruned back to the
hand-written seeds. Clean re-runs after restoration: exit 0 for both targets and for the leg.

## Fuzz run outputs, verbatim figures

| Target | runs / 31 s | cov | ft | corpus | rss | artifacts |
|---|---|---|---|---|---|---|
| `fuzz_input_schema_enforcement` | **108 317** | 11 526 | 28 714 | 854 / 115 Kb | 544 Mb | **0** |
| `fuzz_placeholder_pattern_redos` | **60 751** | 14 497 | 45 691 | 2 038 / 105 Kb | 862 Mb | **0** |

`cargo +nightly fuzz build` exit 0 for both. `cargo +nightly fuzz list | grep -c` = **2**.
`git ls-files --error-unmatch` on both target files: exit 0.

**A2 did NOT reproduce** — no timeout, no crash artifact, over 60 751 runs seeded with four
classic catastrophic-backtracking shapes. Consistent with RESEARCH Finding 1j, and still
evidence rather than proof; T-128-48 stays `accept, instrumented`.

## The corpora — MEASURED, not assumed

`git check-ignore -v fuzz/corpus/fuzz_input_schema_enforcement` matched **`fuzz/.gitignore:7`
(`corpus/*`)**. The seeds the plan requires would have been silently untracked — a corpus the
repository cannot carry is not a corpus. Two narrow un-ignore blocks added in the shape the
three existing exceptions already use, tracking only `README.md` and `[0-9][0-9]_*`.

Confirmed with **`git add -n`**, not with `check-ignore`'s exit code, which returns 0 for a
NEGATION match too and would have read as "still ignored".

10 schema seeds (both PHI-shaped-key redaction cases, the uncompilable-pattern case, the
declared-array-index case that catches a blanket suppression) + 8 ReDoS seeds (the four
nested-quantifier shapes, the benign ACCEPT control, the D-10 ordering control). The ReDoS
corpus is the one that must be committed: `(a+)+`, `(a|aa)+`, `([0-9]+)*` and `((a)*)*` are
each a precise arrangement of nesting, essentially undiscoverable from random bytes, and they
are exactly what A2 is about.

---

# SC-8 — the root property arm

`tests/schema_validation_props.rs`, `#![cfg(feature = "validation")]`, one
`property_refusal_never_echoes_input` arm carrying the `#[ignore]` marker VERBATIM from
`tests/log_emitter.rs`.

**`make test-property`: 3 → 4**, and `tests/schema_validation_props.rs` appears in the run with
`running 1 test`. Note **3**, not the 2 RESEARCH Finding 9b recorded — re-measured before any
change as `tests/log_emitter.rs` 2 + `tests/typed_tool_garde.rs` 1. The plan's `<fails_when>`
compares against the documented 2, which a no-op would have satisfied. Corrected upward;
`WINDOWS.md` #82.

## Why this arm may use the STRICT oracle where the fuzz target may not

A property generator controls BOTH sides, so provenance is established by construction rather
than by a sentinel. The premise is **CHECKED** by a companion `alphabets_are_disjoint` test
that is deliberately NOT `#[ignore]`d, so it runs on every ordinary `cargo test`. It earned its
place twice:

1. the first generator drew caller strings from an uppercase+digit alphabet alone, and proptest
   found `key_seed=[32], bound=6` on the **first run** — a one-character key `"6"` that IS a
   substring of `"maxLength":6`. That is precisely the coincidence the fuzz target's header
   names as the third false-failure mode, **reproduced inside my own generator**. Fixed by
   prefixing every caller string with `CALLER`, i.e. by removing the coincidence rather than by
   weakening the assertion;
2. the first version of the GUARD was per-character and failed on `L`, because the keyword
   `maxLength` contains one. Corrected to a substring check — over-strict premises get relaxed
   wholesale rather than corrected, so granularity matters.

The `.proptest-regressions` file is committed (14 others already are), so the `bound=6` case is
re-run first forever.

**NON-VACUITY:** the sentinel pushed into `render_refusal`'s `declared` list made the arm FAIL
with a real refusal — `/alpha: at most 0 characters; unknown argument(s): 1; allowed: alpha,
beta, gamma, CALLERA` — confirming refusals are actually produced and the assertion fires.
Restored byte-exact.

---

# `test-fuzz-strict` and its CI home

## Why `make test-fuzz`'s blanket swallow is LEFT IN PLACE

Recorded because the plan's output spec asks for it explicitly. `make test-fuzz` pipes every
non-zero exit into `|| echo "… completed"` and invokes the PLAIN `cargo fuzz`, so a crash, a
timeout, a missing nightly toolchain and a clean run all report green — and on this repo's
`channel = "stable"` pin it reports success having fuzzed nothing.

It is unchanged except for a breadcrumb comment above it naming `test-fuzz-strict`. The
swallow spans **27 pre-existing targets**; narrowing it could turn the gate red on a
pre-existing crash in an unrelated target. That would be a genuine discovery, but it is not
this phase's scope and it would block this phase's merge on someone else's defect. All three
review lanes endorsed that call. The breadcrumb is what documents the residual instead of
inheriting it silently.

## The CI home — option (a), chosen on a MEASUREMENT

The plan offered three options and required one be implemented fully before the leg was
chained. The deciding fact, measured 2026-09-27 via the repo rulesets API:

```
ruleset "Green Main — unified gate enforcement" (org-level, active)
  rules: non_fast_forward, pull_request, required_status_checks
  REQUIRED: gate        <- exactly one context
```

and `ci.yml`'s `gate.needs` = `[test, quality-gate, purity-check, pmcp-agent-targets,
wasm32-purity, v1-severance, conformance-suite, era-matrix]`. Branch-protection
`required_status_checks.checks` is `[]`; the ruleset is the mechanism.

So **`fuzz.yml` is NOT a required check**. Option (b) alone falls to the plan's own rejection
condition ("if `fuzz.yml` is not a required check then this option does not deliver a gate at
all and must be rejected on that ground"), and option (c) alone cannot fail in CI. **Only
option (a) delivers a merge gate**, so option (a) it is — with (b) adopted additively as the
deep campaign.

Implemented:

- `ci.yml` quality-gate job gains an `Install nightly + cargo-fuzz` step, `command -v`-guarded
  like the `cargo-nextest` step, placed before `Run quality gate`;
- nightly is installed **NON-DEFAULT** (`rustup toolchain install nightly --profile minimal
  --no-self-update`, not `dtolnay/rust-toolchain@nightly`). Making it default would silently
  move that job's `fmt-check` / `lint` / `test-all` onto nightly clippy, whose lint set differs
  from stable's; CLAUDE.md names toolchain mismatch as the #1 cause of CI failures. `cargo
  +nightly` overrides `rust-toolchain.toml` explicitly, so the stable pin needs no removing the
  way `fuzz.yml` removes it;
- both targets added to `fuzz.yml`'s matrix (`-max_total_time=300`, daily + on any PR touching
  `src/**` or `fuzz/**`).

**NOT weakened**, asserted by parsing the YAML rather than by reading it: `gate.needs` still
lists all eight jobs including `quality-gate`, and the `Run PMAT quality gate (complexity
only)` step is still present in the job.

**The clean-runner smoke test is OPEN and is a human's to read.** The install step has never
executed on a GitHub runner; the plan states the first CI run IS the measurement and that a
`cargo fuzz build` failure there is a finding rather than a flake. It cannot be reproduced
locally, because a machine that already has nightly takes a different branch of the guard.

## The leg's three branches, all measured

| Branch | Exit | Behaviour |
|---|---|---|
| nightly + `cargo-fuzz` present | **0** | both targets actually run (2 × `Done N runs`) |
| missing, `CI` unset | **0** | RED named skip, **nothing attempted** |
| missing, `CI=1` | **2** | RED named skip + `CI is set — this is a HARD FAILURE, not a skip.` |

**A BUG was found and fixed while proving branch 2.** The guard was first written as two
separate recipe lines ending in `exit 0`. Make runs each recipe LINE in its own shell, so the
"skip" printed **both** banners and then ran the fuzz command anyway, failing at `cargo:
command not found` (make exit 2). A leg that turns red for the wrong reason is exactly how a
gate gets deleted (T-128-49a), so the guard and the run now share ONE shell and one exit
status, with the measurement recorded above the recipe.

Local bound is **5 s per target (~10 s total)**, not 30 s: `make quality-gate` is mandatory
before every commit, and Phase 75 D-07 already decided this exact tradeoff the same way by
keeping PMAT out of the local gate. Override with `FUZZ_STRICT_TIME=300`. Named in the
target's header comment, as the plan requires.

`grep -v '^#' Makefile | grep -c test-fuzz-strict` = **4** (>= 3): the `.PHONY`, the target, the
`quality-gate` chain, and the recipe's own progress echo. The breadcrumb prose is stripped
first, so the header cannot self-satisfy the gate.

---

# Measured verification

Every count taken through `rtk proxy cargo …` or `/usr/bin/make` (rtk rewrites `cargo test`
output even when redirected), every gate run unpiped to a file and grepped afterwards.

| Command | Baseline (128-09 / orchestrator) | After |
|---|---|---|
| `cargo nextest run --features full --no-fail-fast` | 3358 run / 3358 passed / 5 skipped | **3359 run, 3359 passed, 6 skipped**, exit 0 |
| `cargo test -p pmcp-openapi-server --no-fail-fast` | 49 passed, 2 ignored | **49 passed, 0 failed, 2 ignored** |
| `make test-server-toolkit` | 440 | **440** (unchanged) |
| `make test-server-toolkit-code-mode` | 483 | **484** |
| `make test-code-mode` | 317 | **317** (unchanged) |
| `make test-cargo-pmcp` | 1477 | **1477** (unchanged) |
| `cargo test -p pmcp --features full --lib schema_validation` | 52 | **52** (unchanged) |
| `--test script_tool` (`openapi-code-mode`, `--test-threads=1`) | 4 | **5** |
| `--test input_validation_acceptance` | 4 | **4** |
| `--test curated_path_injection` | 6 | **6** |
| `make test-property` | **3** (re-measured; RESEARCH said 2) | **4** |
| `cargo doc -p pmcp-server-toolkit` | 17 toolkit warnings / 38 aggregate lines (re-measured against the pristine tree) | **17 / 38** — did not rise |
| `make doc-check` | exit 0 | **exit 0**, zero rustdoc warnings |
| `make lint` | exit 0 | **exit 0** |
| `cargo fmt --all -- --check` | exit 0 | **exit 0** |
| `cargo build --all-features` | exit 0 | **exit 0** |
| `pmat quality-gate --fail-on-violation --checks complexity` | 0 violations | **PASSED, 0 violations** |
| `make quality-gate` | banner | **exit 0 with `ALL TOYOTA WAY QUALITY CHECKS PASSED`** in 18 041 captured lines |
| `git diff 54bfaf0c..HEAD -- src/server/schema_validation.rs` | — | **0 lines** |

**Every delta accounted for.** `+1` on `test-server-toolkit-code-mode` and on `--test
script_tool` is the new T-90-05-03 validation row (`script_tool.rs` needs `openapi-code-mode`,
which is why `test-server-toolkit` stays at 440). `+1 run` and `+1 skipped` on the root nextest
is the new `tests/schema_validation_props.rs`: `alphabets_are_disjoint` runs, the `#[ignore]`d
property arm is skipped. No pre-existing binary lost a test.

**`src/server/schema_validation.rs` diff = 0 lines** — the fifth consecutive plan to meet its
goals without touching the file plan 02 owns.

## The toolkit-rustdoc figure, and a correction to the target

The orchestrator's brief gave 36 as the ceiling. Measured apples-to-apples by restoring the
pristine files, re-running `cargo doc -p pmcp-server-toolkit`, then restoring mine: the
pre-change figures are **17 warnings for `pmcp-server-toolkit` (lib doc)** and **38 aggregate
`^warning` lines** across the three crates it documents. Both figures are unchanged after.

A first draft took the toolkit figure to **20**: the public module doc used intra-doc links
`[`ValidatingToolHandler`]` and `[`enforce_input_schema`]` to two **private** items, which
rustdoc reports as "public documentation … links to private item". De-linked to plain code
spans; back to 17. Caught only because the count was measured on both sides rather than
compared against a remembered number.

## File-health advisory — not caused here

`make quality-gate`'s non-blocking report reads `CRITICAL: 24 files >2000 lines`. Every file
this plan touched was **already** over 2000 before the plan (`tools.rs` 2617, `config.rs` 3849,
`Makefile` 2585) or is far under it (`script_tool.rs` 397, `schema_validation_props.rs` 248, the
two fuzz targets 447 and 238). **This plan crossed no threshold.** The five advisory `✗`
sub-checks (File Health, CB-200 TDG, CB-1204, CB-1208, CB-1308) are the pre-existing set plan
09 verified, and the gate exits 0 with them present.

---

# Deviations from Plan

### 1. [Rule 3 — Blocking] The `fuzzing`-gated UNCACHED seam was not added; a bounded projection replaces it

- **Found during:** Task 2, before writing a line of the target.
- **Issue:** The plan's `key_links` and a `must_haves` truth both require a `fuzzing`-gated
  uncached seam **in `src/server/schema_validation.rs`**. The dispatch's success criteria
  require `git diff` on that file to be **0 lines**, with the reasoning given (plan 02 owns it;
  its 52 tests are the fence; four consecutive plans met their goals without touching it). The
  two are mutually unsatisfiable. There is no zero-diff route to the seam: `compile_input_2020_12`
  and `cached_input_validator` are module-private, and a sibling module cannot reach them.
- **Fix:** the caller's constraint was treated as binding and the HAZARD was closed by a
  different mechanism — a bounded schema projection (5760 distinct texts, arithmetic in the
  target header) plus routing the arbitrary document through the already-public, already-uncached
  `check_input_schema_compiles`. Measured rss 544 MB over 108 317 runs.
- **Cost, stated:** the target's SCHEMA side is not fully arbitrary, so a defect reachable only
  from a shape outside the template table is out of reach. The INSTANCE side — where SC-7 lives —
  is fully arbitrary.
- **Files:** `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs`
- **Recorded:** `WINDOWS.md` #78 and #79, and the target's own header. **Handed to whichever
  plan is permitted to touch `schema_validation.rs`.**
- **Committed in:** `8fa8cbba`

### 2. [Rule 2 — Missing critical] A THIRD stale claim, found by the sweep, and a fourth file

- **Found during:** Task 1, running the sweep the plan asked for.
- **Issue:** `tools.rs:633` (T-84-03-01) read "extra keys are silently dropped — JSON-schema
  validation **rejects them upstream**" — the same defect class as the claim the plan named, one
  function-group away in the same file, reachable by NEITHER of the plan's two named greps. And
  `config.rs:1019` was TRUE but UNNAMED, which the plan's own disposition rule says to fix by
  adding the name.
- **Fix:** both restated. `crates/pmcp-server-toolkit/src/config.rs` joined the modified set
  even though `files_modified` did not list it — the sweep's scope is
  `crates/pmcp-server-toolkit/src/`, and leaving a hit unfixed because its file was not
  anticipated would be the sweep reporting a problem and declining to close it.
- **Committed in:** `caafe27e`

### 3. [Rule 1 — Bug] The `test-fuzz-strict` guard did not actually skip

- **Found during:** Task 3, proving the guard's branches rather than assuming them.
- **Issue:** The guard was two recipe lines each ending in `exit 0`. Make runs each line in its
  own shell, so `exit 0` ended only that line successfully and make proceeded — the "skip"
  printed both RED banners and then ran the fuzz command, failing at `cargo: command not found`
  (make exit 2). A leg that turns red for the wrong reason is exactly how a gate gets deleted.
- **Fix:** guard and run share ONE backslash-joined shell and one exit status. Measurement
  recorded above the recipe so the form is not reverted.
- **Verification:** all three branches re-measured — 0 / 0 (nothing attempted) / 2.
- **Committed in:** `9fec4900`

### 4. [Rule 1 — Bug in my own generator] The property arm's caller alphabet was not disjoint enough

- **Found during:** Task 3, on the property arm's FIRST run.
- **Issue:** caller strings drawn from an uppercase+digit alphabet alone; proptest found
  `key_seed=[32], bound=6` — the one-character key `"6"`, which IS a substring of
  `"maxLength":6`. The strict oracle reported a coincidence as a leak. This is the exact
  false-failure mode the fuzz target's own header warns about, reproduced inside my generator.
- **Fix:** a fixed `CALLER` prefix, i.e. the coincidence removed by construction rather than the
  assertion weakened; plus a non-`#[ignore]`d `alphabets_are_disjoint` companion asserting the
  premise over every `(shape, bound, closed)` the generator draws from. That guard then caught a
  second error of my own — it was per-character and failed on `L` from `maxLength` — corrected to
  a substring check.
- **Verification:** arm passes at `PROPTEST_CASES=256` and at the leg's 1000; the regressions
  file pins the `bound=6` case permanently.
- **Committed in:** `9fec4900`

### 5. [Rule 1 — Bug in a plan gate] RESEARCH Finding 9b's property baseline was one short

- `make test-property` selects **3** on the pristine tree (`log_emitter` 2 + `typed_tool_garde`
  1), not the 2 RESEARCH recorded. The plan's `<fails_when>` compares against "the two tests
  RESEARCH Finding 9b measured", which a no-op change would have satisfied.
- Corrected **upward**. Recorded in `WINDOWS.md` #82.

### 6. [Rule 3 — Blocking] Three rustdoc warnings introduced and removed

- Intra-doc links from the PUBLIC `tools` module doc to two PRIVATE items took the toolkit's
  rustdoc count 17 → 20. De-linked to plain code spans. Caught only because the baseline was
  measured on the pristine tree rather than recalled.
- **Committed in:** `caafe27e`

### 7. [Rule 2 — Missing critical] A third fuzz invariant the plan did not specify

- Gate/runtime AGREEMENT: `check_input_schema_compiles` accepting a schema that `validate_input`
  then reports as uncompilable would make SC-2's config-time gate stop predicting runtime
  behaviour. Two entry points onto one compile, free to assert here, asserted nowhere else.
- **Committed in:** `8fa8cbba`

### 8. [Rule 3 — Blocking, environment] Disk exhaustion mid-verification

- **Found during:** plan-level verification. `/System/Volumes/Data` fell to **2.8 GiB** free.
  Note the orchestrator's brief said ~57 GiB; the measured figure at dispatch was **27 GiB**, and
  the fuzz release builds (ASAN) plus nextest plus `target/doc` consumed the rest.
- **Fix:** the documented remedy — `rm -rf target` (not `cargo clean`), keeping `fuzz/target`
  (4.2 GiB, expensive to rebuild and needed by the strict leg). 44 GiB recovered; full rebuild
  and the entire verification suite re-run afterwards from cold.
- **Files:** none (build artifacts only).

---

**Total deviations:** 8 — 1 plan-instruction conflict resolved in favour of the caller's
constraint with the hazard closed differently, 2 missing-critical additions (a third stale
claim + a fourth file; a third fuzz invariant), 3 real bugs in my own work caught by probing
rather than assuming (the Makefile guard, the property generator twice, the rustdoc links), 1
plan-gate baseline correction, 1 environmental.

**Impact:** no scope creep. Deviation 1 is the only one that changes what a must-have means, and
it does so by closing the hazard the must-have exists for, with an arithmetic bound and a
measured rss, while recording the residual in three places. Deviations 3 and 4 are the phase's
own thesis applied to this plan's output: a gate nobody has watched fail is not a gate, and the
fix for a false positive is to remove the coincidence, never to relax the assertion.

---

# Issues Encountered

- **`gsd_run check tdd-red-evidence` still cannot read cargo output.** Fifth consecutive plan in
  this phase to record it: it parses node-test TAP and returns
  `INVALID_RED (zero_tests_discovered)` for a genuine Rust RED. `workflow.tdd_mode` is absent
  from `.planning/config.json` so nothing is blocked; both `tdd="true"` tasks are evidenced by
  captured cargo / libFuzzer output instead. No TAP was fabricated. `WINDOWS.md` #81.
- **rtk output interception.** Every count was taken through `rtk proxy cargo …` or
  `/usr/bin/make`, and every gate run unpiped to a file then grepped.
- **zsh does not glob unquoted `--include=*.rs`.** Quoted in every sweep invocation; plan 05 lost
  a whole gate to this.
- **`git check-ignore`'s exit code is not the answer to "is this trackable".** It returns 0 for a
  NEGATION match. `git add -n` is.
- **`.pmat/` runtime churn** restored with `git checkout -- .pmat/` after each `pmat` run, never
  committed.
- **libFuzzer writes discovered units into `fuzz/corpus/<target>/`.** Each run added hundreds to
  thousands; pruned back to the hand-written `[0-9][0-9]_*` seeds plus `README.md` before every
  commit, so the tracked corpus is exactly the seeds.

---

# Known Stubs

None. No stub, placeholder, `TODO`, `FIXME`, `HACK`, `XXX`, skipped test or unexplained
`#[ignore]` was added. Three deliberate NON-stubs, named so a reader does not mistake them:

1. `property_refusal_never_echoes_input`'s `#[ignore]` is the **mechanism** by which
   `make test-property`'s `--ignored property_` selector reaches it, not a disabled test. It is
   run by that leg on every `make quality-gate`.
2. `test-fuzz-strict`'s nightly skip is a conditional skip, and the reason a conditional skip is
   acceptable in this one place is written out above the recipe. It is loud, it names both
   targets, and it is a HARD FAILURE under `CI`.
3. The bounded schema projection in `fuzz_input_schema_enforcement` is a recorded scope decision
   with its arithmetic and its residual, not an unfinished generator. See Deviation 1.

# Threat Flags

None new. Every file changed here is test, fuzz, build-config or comment; no production code
path was added or altered, and `cargo build --all-features` plus the full root suite confirm no
behaviour change. The one surface that might read as new — `ci.yml` installing a nightly
toolchain — is provisioning for an existing gate and is deliberately NON-DEFAULT so no other
check changes toolchain.

All nine threat rows addressed: **T-128-45** (mitigated — eight claims restated, four mutations),
**T-128-46** (mitigated — the 36-phrase sweep with its positive control and a 125-hit
disposition ledger), **T-128-47** (mitigated — the provenance-oracle fuzz invariant, proven able
to fail), **T-128-48** (accepted, now INSTRUMENTED — A2 did not reproduce over 60 751 runs
seeded with four nested-quantifier shapes; a timeout is a finding to file),
**T-128-49** (mitigated — `test-fuzz-strict` propagates, proven at exit 2 / status 77, chained
into `quality-gate`; `make test-fuzz`'s swallow left with a breadcrumb),
**T-128-49a** (mitigated — the CI home decided on a measured ruleset and provisioned in the same
change; the clean-runner smoke test is the one open half), **T-128-49b** (mitigated — the
provenance oracle plus a header paragraph recording why it must not be "simplified" back; my own
property generator then reproduced the false-failure mode, which is the evidence the concern was
real), **T-128-49c** (mitigated differently than planned — see Deviation 1; rss 544 MB over 108k
runs is the measurement).

---

# Handoffs to 128-11

**Version and pin moves — NONE made here, as instructed.** D-14 requires them in one commit and
plan 11 owns every one. What this plan adds:

- **`pmcp` (root)** — a new test binary and a new fuzz registration only. **No public API
  added**, so no bump is owed on this plan's account. `src/` is untouched except that it is not:
  `src/server/schema_validation.rs` diff is **0 lines** and no other `src/` file changed.
- **`pmcp-server-toolkit`** — comment-only changes to `src/tools.rs` and `src/config.rs`, plus
  one new integration test row. **No public API change, no bump owed on this plan's account.**
  (Plan 09's MINOR-bump obligation stands independently.)
- **`pmcp-fuzz`** — `publish = false`; two `[[bin]]` stanzas, no dependency added.

**The CHANGELOG D-15 obligation.** `128-03-SUMMARY.md:471` assigned it to **plan 10**:
"the CHANGELOG must name the D-15 forward incompatibility for **both** the six `ParamDecl` keys
and the `[server.validation]` section, because all three of `ParamDecl`, `ServerSection` and
`ServerConfig` carry `deny_unknown_fields`". It is **discharged by reassignment, not dropped**:
`128-11-PLAN.md`'s `must_haves` carries it verbatim (line 48), `CHANGELOG.md` is in that plan's
`files_modified`, its Task 3 writes the whole release entry, and it has a machine gate
(`grep -c "server.validation" CHANGELOG.md`). Plan 10 deliberately wrote no partial entry for a
release whose twelve version numbers it is forbidden to set — plan 11's Task 3 would have
overwritten it. **Verify it lands there.** `WINDOWS.md` #80.

**Two residuals for a future plan, not for 128-11 unless it wants them:**

1. the `fuzzing`-gated UNCACHED seam in `src/server/schema_validation.rs` (Deviation 1). Adding
   it lets `fuzz_input_schema_enforcement` drop the projection and go fully arbitrary on the
   schema side, and lets `fuzz_placeholder_pattern_redos` stop being bounded-by-the-run;
2. `make test-fuzz`'s blanket `|| echo` across 27 targets. The breadcrumb names
   `test-fuzz-strict` as the leg that propagates; narrowing the swallow is a standalone piece of
   work whose first act will be discovering whether any of the 27 currently crashes.

**One thing 128-11 should read before shipping:** `ci.yml`'s nightly + `cargo-fuzz` install has
never run on a GitHub runner. The first CI run IS the measurement. If `cargo install cargo-fuzz`
or `cargo +nightly fuzz build` fails there, that is a finding — and the correct response is to
fix the provisioning, not to relax `test-fuzz-strict`'s CI branch, because relaxing it returns
the leg to being a gate that cannot fail.

---

# Self-Check: PASSED

**Files created — all present on disk:**
`fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` (447 lines),
`fuzz/fuzz_targets/fuzz_placeholder_pattern_redos.rs` (238),
`tests/schema_validation_props.rs` (248), `tests/schema_validation_props.proptest-regressions`,
`fuzz/corpus/fuzz_input_schema_enforcement/` (10 seeds + README),
`fuzz/corpus/fuzz_placeholder_pattern_redos/` (8 seeds + README).
`git ls-files --error-unmatch` on all three `.rs` files: **exit 0**.

**Commits — all resolve in `git log`:** `caafe27e` (Task 1), `8fa8cbba` (Task 2), `9fec4900`
(Task 3). `git rev-list --count 54bfaf0c..HEAD` = **3**.

**Every `<acceptance_criteria>` row in all three tasks re-run and passing**, and every
plan-level `<verification>` command re-run and tabulated above — including the corrected
`make test-property` baseline and the corrected toolkit-rustdoc measurement.

**Tracked working tree clean** apart from the pre-existing untracked directories present at
dispatch; `.pmat/` churn restored; fuzz artifacts directories empty; fuzz corpora pruned to the
hand-written seeds.

**`make quality-gate` captured output contains `ALL TOYOTA WAY QUALITY CHECKS PASSED`** (18 041
lines), with the strict fuzz leg observed running inside it at line 15842 and passing at 17751.

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-28*
