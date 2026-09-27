---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 06
subsystem: api
tags: [curated-single-call, http-client, path-placeholder, openapi-parser, config-validation, query-separator-narrowing, tdd]

# Dependency graph
requires:
  - phase: 128-02
    provides: "`validate_path_placeholder`, `validate_resolved_path`, `PlaceholderRules` (+`Default` and the three `with_*` builders), `PlaceholderRefusal` (+`Display`/`Error`), `PLACEHOLDER_MAX_LENGTH`"
  - phase: 128-03
    provides: "the six `ParamDecl` keys (`pattern`, `max_length`, `allow_slash` are the three this plan reads), `[server.validation]` + `ValidationSection`, `path_placeholder_names`, `ParamPosition`, `input-validation` in the toolkit `default`"
  - phase: 128-05
    provides: "the operator-directed `?` narrowing and the `ResolvedPath::from_checked` split this plan mirrors in its own caller, plus the 9-still-refused-row discipline"
provides:
  - "`http::schema::Parameter::{pattern, max_length, allow_slash}` — three `#[serde(default)]` fields carrying the declared narrowing to the substitution point"
  - "`http::schema::Parameter::with_rules(Option<String>, Option<u64>, bool) -> Self` — the consuming builder; `Parameter::new` keeps its three-argument signature"
  - "`http::schema::Parameter::placeholder_rules(&self) -> pmcp::server::schema_validation::PlaceholderRules<'_>` — `#[cfg(feature = \"input-validation\")]`-gated, the seam plan 08 reads"
  - "THE CURATED SURFACE'S CALL SITES: `validate_path_placeholder` at `crates/pmcp-server-toolkit/src/http/client.rs:299` and `validate_resolved_path` at `:318`/`:320`/`:321`, both reached from `substitute_path` (`:216-238`) before dispatch — SC-4 is now true on BOTH HTTP surfaces"
  - "`ConfigValidationError::MalformedPathTemplateSegment { tool, segment }` — the curated template parser's whole-segment limit, enforced at CONFIG time"
  - "`crates/pmcp-server-toolkit/tests/curated_path_injection.rs` — the two JS-engine-free CR-01 probes, the D-08 independence row, the D-11 spec row with its own accept half, and two accept controls"
  - "`http::client::placeholder_floor` (15) and `http::client::query_separator` (12) — the curated mirror of plan 05's narrowing boundary"
affects: [128-07, 128-08, 128-09, 128-10, 128-11]

# Actuals (#2632) — chars/4 over the realized diff, NOT a harness token count.
actuals:
  tokens: 20537
  tasks: 3
  commits: 6
  plan_head_before: 876110803de66c57d0010c97d7c9c77d9bdee612
  # `commits` is MEASURED with the instrument a verifier will re-run:
  # `git rev-list --count 87611080..HEAD`. It was **5** immediately before this
  # SUMMARY was written (11dbcc52, d40ba3b1, 2825edfe, 097bc94f, c41bc12b) and is
  # **6** at close-out, the sixth being the `docs(128-06)` commit that carries this
  # SUMMARY together with STATE.md and deferred-items.md. The state mutations were
  # run BEFORE this file was written specifically so the close-out figure is
  # deterministic rather than dependent on whether the SDK folds them in. 6 is what
  # a later `/gsd-verify-work` re-measure will see. HEAD is named by role and not by
  # hash because this figure lives inside it.
  # tokens: `git diff 87611080..HEAD -- crates/ Makefile | wc -c` == 82146, /4 ==
  # 20536.5. The plan estimated 60000; the actual is ~34% of it. Not rounded toward
  # the estimate — the gap is real and is recorded so later projections calibrate.

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Gate the METHOD, not just the call site, when a public accessor's RETURN TYPE comes from a feature-gated module"
    - "Two-pass substitution — render+check everything, then apply — so a refusal leaves no half-substituted path in existence"
    - "`cfg` PAIRS with a written `cfg(not(...))` half, so the unenforced shape is visible rather than implied"
    - "A schema-shape whitelist that contributes NO narrowing for any shape it does not understand, which is conservative in the safe direction"
    - "A config-time hard error where the alternative has no working interpretation; a `lint()` finding only where it does"
    - "Test modules named FOR the verify filter, so `tools::build_operation` resolves instead of selecting zero"
    - "Mutation testing per call site, reporting which rows go red for EACH of the three checks separately"

key-files:
  created:
    - crates/pmcp-server-toolkit/tests/curated_path_injection.rs
  modified:
    - crates/pmcp-server-toolkit/src/http/schema.rs
    - crates/pmcp-server-toolkit/src/http/client.rs
    - crates/pmcp-server-toolkit/src/tools.rs
    - crates/pmcp-server-toolkit/src/config.rs
    - crates/pmcp-server-toolkit/src/error.rs
    - Makefile

key-decisions:
  - "SC-4 is closed on the curated surface by FOUR real call sites, not by a claim: `validate_path_placeholder` once at `client.rs:299` (reached per path value) and `validate_resolved_path` three times inside `check_composed_path` (`:318`/`:320`/`:321`). All three checks were mutation-tested SEPARATELY and the rows that go red for each are enumerated."
  - "The inherited `?` narrowing is mirrored in the CALLER (`check_composed_path` splits at the first `?` and applies the unmodified rule set to each side), never in core. `git diff 87611080..HEAD -- src/server/schema_validation.rs` is 0 lines and plan 02's 52 strict tests are green."
  - "`substitute_path` is TWO-PASS: every value is rendered and checked before ANY replacement is applied. Stronger than the plan's ordering requirement — a refusal on the second of two placeholders means no partially-substituted path ever exists, not merely that none is returned."
  - "A missing path argument gets its OWN refusal naming the parameter, rather than leaning on the composed check's residual-brace rule. The composed refusal is param-agnostic by construction (`param: \"path segment\"`), so it cannot tell a caller WHICH declaration to supply. This is a presence check, not a second copy of the character denylist."
  - "The curated template limit is a HARD `ConfigValidationError::MalformedPathTemplateSegment`, not a `lint()` finding. The plan allowed either; a malformed segment has no working interpretation, so the only real choice was loud-at-startup versus obscure-per-call."
  - "The `allowReserved` document test was RELOCATED to `tests/curated_path_injection.rs` and STRENGTHENED from a field assertion to an end-to-end refusal with its own accept half, because the plan's own `<verify>` gate forbids that keyword's name in any non-comment line of `src/http/schema.rs` — including a test document that declares it."
  - "The OpenAPI parser narrows ONLY from a direct `ReferenceOr::Item` whose `SchemaKind` is `Type(Type::String(..))`. A `$ref`, a composition, a non-string type and the `content` form all yield floor-only rules, each with its own test."
  - "`Parameter`'s three rule FIELDS stay ungated while `placeholder_rules` is gated: gating plain `#[serde(default)]` scalars would make the struct's serialized shape feature-dependent, which is a worse break than gating an accessor that names a feature-gated type."

patterns-established:
  - "Pattern: when a gate forbids a token and a test needs to declare it, MOVE the test to a file the gate does not scope to and make the relocated test stronger — never weaken the gate and never obfuscate the token by concatenation."
  - "Pattern: mutation-test each security call SEPARATELY. Three mutations here produced three disjoint-enough red sets (10 / 10 / 1), which is what shows each check is load-bearing on its own rather than that the suite as a whole notices something."
  - "Pattern: a narrowing needs more STILL-REFUSED rows than ACCEPT rows. `http::client::query_separator` is 3 accept / 9 refuse, mirroring plan 05's curated-surface-free equivalent; 7 of the 9 are mutation-sensitive."
  - "Pattern: an ACCEPT control inside a refusal test. The D-11 row refuses a `/`-carrying value on a spec-derived `Parameter` AND accepts the identical value once `allow_slash` arrives from config, so it cannot be satisfied by refusing everything."

requirements-completed: [D4, SC-4, SC-1]

coverage:
  - id: D1
    description: "`Parameter` carries the declared `pattern`/`max_length`/`allow_slash`, populated from `[[tools.parameters]]` for a curated tool and from a spec parameter's direct string schema for a spec-driven one; `Parameter::new` keeps its signature"
    requirement: "D4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parameter_new_defaults_the_rule_fields"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parameter_with_rules_round_trips_into_placeholder_rules"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parser_narrows_from_a_direct_string_schema"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/tools.rs#build_operation_carries_a_declared_path_pattern"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/tools.rs#build_operation_carries_a_declared_query_max_length"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/tools.rs#build_operation_leaves_rules_absent_for_an_undeclared_template_segment"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/tools.rs#build_operation_rules_reach_placeholder_rules"
        status: pass
    human_judgment: false
  - id: D2
    description: "Only a direct string-type schema narrows: a `$ref`, a non-string type, a composed schema and the `content` form all yield floor-only rules (deliberately conservative in the D-10 direction)"
    requirement: "D4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parser_yields_floor_only_rules_for_a_ref_schema"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parser_yields_floor_only_rules_for_a_non_string_schema"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parser_yields_floor_only_rules_for_a_composed_schema"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parser_yields_floor_only_rules_for_a_content_form_parameter"
        status: pass
    human_judgment: false
  - id: D3
    description: "T-128-26 / D-11: a spec cannot widen the floor. `allow_slash` is false for every spec-derived parameter, and a document declaring `allowReserved: true` still has its `/`-carrying value refused — while the identical value is accepted once the permission arrives from config"
    requirement: "D4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/schema.rs#schema_parser_leaves_allow_slash_false_for_every_spec_parameter"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/curated_path_injection.rs#curated_path_injection_spec_allow_reserved_does_not_widen_the_floor"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/tools.rs#build_operation_carries_allow_slash_from_the_config"
        status: pass
      - kind: other
        ref: "grep -v '^[[:space:]]*//' crates/pmcp-server-toolkit/src/http/schema.rs | grep -ci allowreserved  ->  0 (positive control: 'reserved' appears 3x in the file)"
        status: pass
    human_judgment: false
  - id: D4
    description: "T-128-25: the curated single-call path refuses every CR-01 payload at the substitution point, with zero upstream requests, on a build with no JS engine"
    requirement: "SC-4"
    verification:
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/curated_path_injection.rs#curated_path_injection_refuses_a_query_via_a_placeholder_value"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/curated_path_injection.rs#curated_path_injection_refuses_traversal_via_a_placeholder_value"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_a_query_separator_in_a_value"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_traversal_in_a_value"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_upper_case_encoded_traversal"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_a_value_that_is_exactly_a_denied_character"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_a_nul_byte_in_both_forms"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_an_empty_value"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_accepts_the_cap_and_refuses_one_more"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_the_second_of_two_placeholders_without_substituting"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_leaves_a_placeholder_free_template_untouched"
        status: pass
      - kind: other
        ref: "MUTATION TEST: removing the `validate_path_placeholder` call turns 10 rows red (6 lib + 4 integration); file restored byte-exact"
        status: pass
    human_judgment: false
  - id: D5
    description: "The COMPOSED check catches what no per-value check can: a composed over-cap segment, a residual `{`/`}` from a spec-shaped template, traversal written into the template literal, and everything the `?` narrowing does not relax"
    requirement: "SC-4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_a_composed_segment_over_the_cap"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_a_residual_brace_from_an_unrecognized_template"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_traversal_written_into_the_template_literal"
        status: pass
      - kind: other
        ref: "MUTATION TEST: removing the `validate_resolved_path` call turns 10 lib rows red; file restored byte-exact"
        status: pass
    human_judgment: false
  - id: D6
    description: "The inherited `?` narrowing on the curated surface: 3 ACCEPT rows and 9 STILL-REFUSED rows, so the narrowing is distinguishable from a deleted check"
    requirement: "SC-4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#query_separator (12 rows: 3 accept / 9 refuse)"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/curated_path_injection.rs#curated_path_injection_accepts_an_author_written_query_string_in_the_path"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#validate_accepts_a_path_template_carrying_an_author_written_query_string"
        status: pass
      - kind: other
        ref: "git diff 87611080..HEAD -- src/server/schema_validation.rs  ->  0 lines (core left strict); `cargo test -p pmcp --features full --lib schema_validation` 52/52"
        status: pass
    human_judgment: false
  - id: D7
    description: "T-128-27 / D-08: the placeholder cap is a module constant in core and `[server.validation] default_max_length = 0` cannot reach it"
    requirement: "D4"
    verification:
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/curated_path_injection.rs#curated_path_injection_cap_independent_of_d3"
        status: pass
    human_judgment: false
  - id: D8
    description: "T-128-29 / SC-1: the curated build gains no JS-engine edge, and the new probe binary is gated so it runs on the build it is about"
    requirement: "SC-1"
    verification:
      - kind: other
        ref: "cargo tree -p pmcp-server-toolkit --no-default-features --features http -e normal -i pmcp-code-mode  ->  'did not match any packages' (the passing outcome)"
        status: pass
      - kind: other
        ref: "cargo build -p pmcp-server-toolkit --no-default-features --features http  ->  exit 0 (the Codex-HIGH gate: an ungated `placeholder_rules` would break it)"
        status: pass
      - kind: other
        ref: "grep -v '^//' tests/curated_path_injection.rs | grep -c openapi-code-mode -> 0; raw grep -c -> 2 (the deliberately opposed pair)"
        status: pass
    human_judgment: false
  - id: D9
    description: "The curated template parser's whole-segment limit is DOCUMENTED and the unsupported shapes fail at config time"
    requirement: "D4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#validate_rejects_a_path_template_segment_with_two_brace_pairs"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#validate_rejects_a_path_template_segment_with_text_adjacent_to_a_brace_pair"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#validate_rejects_an_empty_path_template_placeholder"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#validate_rejects_an_unbalanced_path_template_brace"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#validate_accepts_whole_segment_path_template_placeholders"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/config.rs#validate_ignores_the_template_rule_for_a_tool_with_no_path"
        status: pass
    human_judgment: false
  - id: D10
    description: "T-128-28 / SC-7: no refusal message contains a byte of the rejected value or any fragment of the resolved path"
    requirement: "SC-4"
    verification:
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::assert_value_free (applied by the two CR-01 rows)"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/curated_path_injection.rs#refuse_version (asserts value-absence plus '/content', '/CUI' and the cui value)"
        status: pass
      - kind: unit
        ref: "crates/pmcp-server-toolkit/src/http/client.rs#placeholder_floor::placeholder_floor_refuses_an_absent_path_argument"
        status: pass
    human_judgment: false

# Metrics
duration: ~60min
completed: 2026-09-27
status: complete
---

# Phase 128 Plan 06: D4 on the curated single-call HTTP surface Summary

**`HttpClient::substitute_path` is now two-pass — every rendered path-placeholder value faces
`validate_path_placeholder` against its declared rules with nothing substituted yet, and only once
all of them pass are the replacements applied and the composed result checked by
`validate_resolved_path` with the inherited one-`?` exemption — so CR-01 is closed on the default,
JS-engine-free build that the change request originally missed, and SC-4 is true on BOTH HTTP
surfaces rather than one.**

## SC-4 is closed on the curated surface — named by file and line, and mutation-tested

The carried obligation for this plan was that plan 05 closed the Code Mode surface only, and that
`substitute_path` was still the bare form: `path.replace(&placeholder, &value_str)` with no
character check, no cap and no composed check.

**The call sites**, all reached from `substitute_path` (`crates/pmcp-server-toolkit/src/http/client.rs:216-238`)
before the caller can dispatch:

| File | Line | Code | Reached from |
|---|---|---|---|
| `crates/pmcp-server-toolkit/src/http/client.rs` | **299** | `pmcp::server::schema_validation::validate_path_placeholder(` | `check_placeholder_value`, called once per path value at `:228` |
| `crates/pmcp-server-toolkit/src/http/client.rs` | **318** | `None => …::validate_resolved_path(path)` | `check_composed_path`, called at `:236` — no operator-written `?` |
| `crates/pmcp-server-toolkit/src/http/client.rs` | **320** | `…::validate_resolved_path(path_part)` | same — the portion before the first `?` |
| `crates/pmcp-server-toolkit/src/http/client.rs` | **321** | `.and_then(|()| …::validate_resolved_path(query_part))` | same — the portion after it |

Measured with the comments stripped, so these are CALL SITES and not rustdoc:

```
/usr/bin/grep -v "^[[:space:]]*//" crates/pmcp-server-toolkit/src/http/client.rs \
  | /usr/bin/grep -nE "schema_validation::validate_(path_placeholder|resolved_path)\("
```
→ 4 lines, exactly the four above. The plan's raw `grep -c` readings are **8** for
`validate_resolved_path` and **4** for `validate_path_placeholder`; both are nonzero, and the
structural reading above is the one to trust.

### The rows that go red when each call is removed — THREE separate mutations

Each check was removed on its own, the suites re-run, and the file restored from a byte-exact copy
(`git diff --stat` empty, `grep -c "MUTATION TEST"` → 0, `--lib http::client` back to 41/41).

**Mutation 1 — the `validate_path_placeholder` call removed: 10 rows red** (6 lib + 4 integration).

| Row | Binary |
|---|---|
| `placeholder_floor::placeholder_floor_refuses_a_query_separator_in_a_value` | toolkit lib |
| `placeholder_floor::placeholder_floor_refuses_traversal_in_a_value` | toolkit lib |
| `placeholder_floor::placeholder_floor_refuses_upper_case_encoded_traversal` | toolkit lib |
| `placeholder_floor::placeholder_floor_refuses_a_value_failing_its_declared_pattern` | toolkit lib |
| `placeholder_floor::placeholder_floor_refuses_the_second_of_two_placeholders_without_substituting` | toolkit lib |
| `query_separator::query_separator_still_refuses_an_injected_separator_from_a_value` | toolkit lib |
| `curated_path_injection_refuses_a_query_via_a_placeholder_value` | `tests/curated_path_injection.rs` |
| `curated_path_injection_refuses_traversal_via_a_placeholder_value` | `tests/curated_path_injection.rs` |
| `curated_path_injection_cap_independent_of_d3` | `tests/curated_path_injection.rs` |
| `curated_path_injection_spec_allow_reserved_does_not_widen_the_floor` | `tests/curated_path_injection.rs` |

(`41 passed` → `35 passed; 6 failed`; `6 passed` → `2 passed; 4 failed`.)

**Mutation 2 — the `validate_resolved_path` call removed: 10 lib rows red, and ZERO integration
rows.** That second half is the honest and informative part: every integration payload is a
per-value payload, so the composed check's evidence lives entirely at the unit level — which is
exactly why the three isolating rows exist.

| Row | Why only the composed check can see it |
|---|---|
| `placeholder_floor_refuses_a_composed_segment_over_the_cap` | a 100-char template literal plus a 200-char value; each passes alone |
| `placeholder_floor_refuses_a_residual_brace_from_an_unrecognized_template` | a spec-shaped `{b}` with no supplied argument — no per-value check runs for it |
| `placeholder_floor_refuses_traversal_written_into_the_template_literal` | **the isolating row** — a literal is never floored, so nothing else can be refusing |
| `query_separator_still_refuses_traversal_in_the_path_portion` | template-level, no value involved |
| `query_separator_still_refuses_traversal_in_the_query_portion` | ditto |
| `query_separator_still_refuses_a_control_byte_in_the_query_portion` | ditto |
| `query_separator_still_refuses_an_over_cap_query_portion` | ditto |
| `query_separator_still_refuses_a_second_question_mark` | ditto |
| `query_separator_still_refuses_an_empty_query_portion` | ditto |
| `query_separator_still_refuses_a_fragment_marker` | ditto |

(`41 passed` → `31 passed; 10 failed`; integration stayed `6 passed`.)

**Mutation 3 — the `refuse_missing_path_argument` call removed: exactly 1 row red**
(`placeholder_floor_refuses_an_absent_path_argument`, `41 passed` → `40 passed; 1 failed`). Worth
recording precisely: without the dedicated refusal the composed check still refuses that request
(the residual `{id}` trips the brace rule), so the request never reaches the backend either way —
the row fails on the `contains("id")` assertion, i.e. on the message no longer naming WHICH
declaration to supply. That is the whole reason the dedicated refusal exists.

**Rows that deliberately do NOT fail under mutation 1**, stated rather than hidden:
`placeholder_floor_refuses_a_nul_byte_in_both_forms`, `…_refuses_an_empty_value`,
`…_accepts_the_cap_and_refuses_one_more` and
`query_separator_still_refuses_an_injected_traversal_from_a_value` all still pass, because the
composed check independently refuses the same payloads (a control byte anywhere, an empty segment, a
257-code-point segment, traversal). They are refused **twice over**. This is the same
double-coverage plan 05 recorded, and it is why the three isolating rows above carry the
discriminating load for the composed check.

## Core was left strict — the `?` narrowing lives in this caller

`git diff 87611080..HEAD -- src/server/schema_validation.rs` is **0 lines**, and
`cargo test -p pmcp --features full --lib schema_validation` is **52/52**. `validate_resolved_path`
still refuses a query separator anywhere for every other caller; the exemption is entirely inside
`check_composed_path` (`client.rs:314-325`), which reuses plan 05's structure verbatim:

```rust
let checked = match path.split_once('?') {
    None => …validate_resolved_path(path),
    Some((path_part, query_part)) => …validate_resolved_path(path_part)
        .and_then(|()| …validate_resolved_path(query_part)),
};
```

Because both sides meet the same rule, a second `?`, a fragment marker, an over-cap query, an empty
query portion and traversal inside the query all stay refused for free. The reasoning is encoded in
`substitute_path`'s rustdoc (`# The one narrowing`), phrased for the curated surface: a `[[tools]]`
`path` is operator-authored configuration, exactly as a Code Mode script's literal path text is
operator-authored script text, and the per-value floor already refuses `?` in literal and
percent-encoded form — so a surviving `?` can only have come from the config. Refusing `..` from
that template catches a traversal bug; refusing `?` from it rejects legitimate configuration.
**Same rule, different work.**

**The curated mirror of plan 05's boundary suite is `http::client::query_separator`: 12 rows, 3
ACCEPT / 9 STILL-REFUSED**, and 7 of the 9 are mutation-sensitive (see mutation 2). The accept rows
are an operator query string on its own, one alongside a floored placeholder
(`/content/{version}/CUI?string=x` → `/content/current/CUI?string=x`), and the Graph-style
`?$select=values` projection. `128-05-SUMMARY.md`'s Deviation 1 pre-revert claim — that authors must
migrate query values to body params — is **not** carried into any comment, rustdoc or test here.

## The `Parameter::placeholder_rules` signature, for plan 08

```rust
impl Parameter {
    // unchanged, three arguments, rule fields default to "no declared narrowing"
    #[must_use] pub fn new(name: impl Into<String>, location: ParameterLocation, required: bool) -> Self;

    #[must_use] pub fn with_rules(
        mut self,
        pattern: Option<String>,
        max_length: Option<u64>,
        allow_slash: bool,
    ) -> Self;

    #[cfg(feature = "input-validation")]
    #[must_use] pub fn placeholder_rules(&self)
        -> pmcp::server::schema_validation::PlaceholderRules<'_>;
}
```

Fields: `pattern: Option<String>`, `max_length: Option<u64>`, `allow_slash: bool`, all
`#[serde(default)]` and all UNGATED. `placeholder_rules` borrows them through the three `with_*`
builders (`#[non_exhaustive]` forbids a literal) and maps a `max_length` that does not fit a `usize`
to `None`, which is safe because the always-on module cap still applies.

**`Parameter` struct-literal construction sites in the workspace: ZERO.** Measured with a positive
control so the zero is not a broken scan:

| Scan | Count |
|---|---|
| `grep -rn --include="*.rs" -E "(^|[^:[:alnum:]_])Parameter[[:space:]]*\{"` across the repo | **2** — the `pub struct Parameter {` definition and the `impl Parameter {` block, i.e. **0 real construction sites** |
| positive control: `grep -rn --include="*.rs" "Parameter::new("` | **9** |

Every construction goes through `Parameter::new`, which is why keeping its signature was sufficient
for the whole workspace to compile with three new fields.

## The curated template parser's limit — HARD ERROR, and why

Plan step 3b allowed either a `lint()` finding or a `ConfigValidationError` variant. **A hard error
was chosen:** `ConfigValidationError::MalformedPathTemplateSegment { tool, segment }`, raised by
`check_path_template_segments` (`config.rs`) from `validate_tool_parameters`.

The rule, in `is_supported_path_segment`: a `/`-delimited segment either contains no brace at all,
or is exactly one non-empty `{name}` spanning the whole segment with no further brace inside. That
refuses `/search/{a}{b}` (which `path_placeholder_names` parses as the single name `a}{b`),
`/prefix-{id}` (not recognized as carrying a placeholder at all), `/a/{}/b` and `/a/{id`.

Why a hard error rather than a warning: such a segment has **no working interpretation**. It yields
either a parameter name no `[[tools.parameters]]` entry can match, or literal braces travelling
toward the backend — and the new composed check refuses that request at call time anyway. So the
choice was only between failing loudly at startup and failing obscurely on every call. Nothing that
worked before loses a working behaviour. Contrast `UncappedStringParam`, which is `strict`-only
precisely because an uncapped free-text field DOES have a working interpretation (D-05).

The limit is also stated in prose in both places the plan requires: `Parameter::placeholder_rules`'s
rustdoc (`# The curated template parser recognizes WHOLE-SEGMENT placeholders only`) and
`substitute_path`'s `# Errors` neighbourhood (`## What counts as a placeholder here`), which also
records that a spec-derived operation IS substituted mid-segment and is not config-validated — which
is why the composed check is what closes a residual brace there.

**The tokenizer retrofit was NOT performed**, per the plan's Deferred ledger: `ToolDecl::param_position`
is contractually coupled to `path_placeholder_names`'s derivation one wave after that coupling was
established, and re-deriving names would change which `ParamDecl` matches which segment.

**Pre-flight scan, with a positive control, before making this a hard error.** No in-tree config can
be broken by it:

| Scan | Result |
|---|---|
| `grep -rn --include="*.toml" '^[[:space:]]*path[[:space:]]*='` (positive control) | **217** lines |
| … of those, containing `{` or `}` | **0** |
| … containing `?` | **0** |
| … ending in `/` | **0** |
| the single real curated HTTP tool path in TOML | `/Line/Mode/tube/Status` |
| `grep -rn --include="*.rs" 'path: Some('` (positive control) | **31** |
| … of those with braces, all whole-segment (`/lines/{line_id}/status`, `/issues/{id}/comments`, `/Line/{id}/Status`, `/repos/{owner}/issues`, `/things/{declared}`) | **13**, all supported |
| spec path templates in fixtures | `/Line/Mode/tube/Status`, `/Line/{lineId}/Disruption`, `/drives/{drive-id}/…/range(address='{address}')` — the last is mid-segment and spec-driven, so it is NOT config-validated and continues to substitute textually |

## The OpenAPI narrowing — which shapes narrow, and which deliberately do not

`declared_string_rules` (`http/schema.rs`) contributes `pattern` and `max_length` from **only** a
direct `ReferenceOr::Item` whose `SchemaKind` is `Type(Type::String(..))`. Every other shape yields
`(None, None)`, each with its own test against one `SHAPES_JSON` document:

| Parameter | Shape | Result |
|---|---|---|
| `version` | `{"type":"string","pattern":…,"maxLength":64}` | **narrows** — `pattern` + `max_length: Some(64)` |
| `cui` | `{"$ref":"#/components/schemas/Cui"}` (target declares `maxLength: 12`) | floor-only — the target is NOT read |
| `verbose` | `{"type":"boolean"}` | floor-only |
| `composed` | `{"oneOf":[…]}` | floor-only |
| `ctyped` | `content: {application/json: {schema: {maxLength: 8}}}` | floor-only — the media-type schema is NOT read |

The `content` form is the representable stand-in for the plan's "absent schema" row:
`openapiv3::ParameterData::format` is a required flattened field, so a parameter carrying neither
`schema` nor `content` does not deserialize at all and cannot be exercised. Stated in the fixture's
own doc comment rather than silently substituted.

`allow_slash` is `false` unconditionally on this path, with the D-11 reason at that line.

## Before/after test counts

| Selection | Before | After |
|---|---|---|
| `cargo test -p pmcp-server-toolkit --features http,input-validation --lib` | 251 | **298** |
| `… --lib http::schema` | 6 | **14** |
| `… --lib tools::build_operation` | — | **5** (the risky filter shape; resolves as written) |
| `… --lib http::client` | 14 | **41** |
| `… --lib http::client::placeholder_floor` | — | **15** |
| `… --lib http::client::query_separator` | — | **12** |
| `… --test curated_path_injection` | — | **6** |
| `… --test curated_path_injection … cap_independent_of_d3` (positional) | — | **1** |
| `make test-server-toolkit` | 340 | **393** |
| `make test-server-toolkit-code-mode` | 365 | **418** |
| `make test-code-mode` | 317 | **317** (unchanged) |
| `cargo test -p pmcp --features full --lib schema_validation` | 52 | **52** (unchanged — core left strict) |
| `cargo test -p pmcp-openapi-server --no-fail-fast` | 42 | **42**, 0 failed |
| `cargo nextest run --features "full" --no-fail-fast` (root) | 3358 | **3358**, 0 failed, 5 skipped |

`298 − 251 = 47` new lib tests = 8 (`http::schema`) + 5 (`tools::build_operation`) + 7 (`config`) +
27 (`http::client`: 15 + 12). `393 − 340 = 53` = those 47 plus the 6 integration probes.
`418 − 365 = 53` for the same reason. The root suite is unchanged because every new test lives in a
workspace member crate, not the root `pmcp` package.

**Every filtered verify command was confirmed to select a NONZERO count, and no count was lowered to
match reality.** The one shape the prompt flagged as risky — `tools::build_operation`, which reads
like a function name — was made to resolve by NAMING THE TEST MODULE for it (a sibling of `tests` at
the `tools` module level), exactly as plan 05 did for `executor::layer_one`. Its 5 selected tests
prove the filter resolves; had the tests been placed in `mod tests` the path would have been
`tools::tests::build_operation_…` and the filter would have selected zero while exiting 0.

## Files Created/Modified

- `crates/pmcp-server-toolkit/src/http/schema.rs` — 436 → **~700 lines**. `Parameter` gained three
  `#[serde(default)]` fields, `with_rules`, and the `input-validation`-gated `placeholder_rules`
  (whose rustdoc carries D-10, D-11 and the whole-segment limit). `convert_parameter` restructured
  into one construction path so the D-11 `false` is written once. New private
  `declared_string_rules`. New gated `use pmcp::server::schema_validation::PlaceholderRules`. Eight
  new tests plus the `SHAPES_JSON` fixture.
- `crates/pmcp-server-toolkit/src/http/client.rs` — 709 → **~1000 lines**. `substitute_path` is
  two-pass and carries ~60 lines of new rustdoc. New private `cfg` PAIRS:
  `check_placeholder_value`, `check_composed_path`, `refuse_missing_path_argument`, plus
  `refusal_to_backend_error`. `Parameter` added to the module's imports. Three new `#[cfg(test)]`
  sibling modules: `d4_support`, `placeholder_floor` (15), `query_separator` (12).
- `crates/pmcp-server-toolkit/src/tools.rs` — `build_operation`'s two loops populate the three rule
  fields; the path loop looks the `ParamDecl` up BY NAME. New `#[cfg(all(test, feature = "http"))]
  mod build_operation` (5).
- `crates/pmcp-server-toolkit/src/config.rs` — new private `check_path_template_segments` and
  `is_supported_path_segment`, called from `validate_tool_parameters`; 7 new tests.
- `crates/pmcp-server-toolkit/src/error.rs` — `ConfigValidationError::MalformedPathTemplateSegment`
  (the enum is `#[non_exhaustive]`, so this is not a breaking change).
- `crates/pmcp-server-toolkit/tests/curated_path_injection.rs` — **NEW**, 6 tests.
- `Makefile` — `curated_path_injection` added to `test-server-toolkit`'s `REQUIRED_TEST_BINARIES`
  (`:603`), plus a comment above the target recording why its gate omits `openapi-code-mode`.

## Task Commits

1. **Task 1 (tdd) — RED** — `11dbcc52` (`test`): the three `Parameter` fields, `with_rules`, the
   gated `placeholder_rules`, the `MalformedPathTemplateSegment` variant, and 20 tests with every
   target behaviour ABSENT. **RED evidence: 271 discovered, 9 failed, exit 101** — the four
   config-template refusals, the spec-narrowing row, and all four `build_operation` population rows.
   The accept controls and the four floor-only rows passed, as accept controls do.
2. **Task 1 (tdd) — GREEN** — `d40ba3b1` (`feat`): `declared_string_rules`, the restructured
   `convert_parameter`, `build_operation`'s two populated loops, and the config-time template check.
   **GREEN: 271/271.**
3. **Task 2 (tdd) — RED** — `2825edfe` (`test`): 27 tests in the two new sibling modules, with
   `substitute_path` unchanged. **RED evidence: `--lib http::client` 41 discovered, 22 failed,
   exit 101**, with the 5 accept controls passing.
4. **Task 2 (tdd) — GREEN** — `097bc94f` (`feat`): the two-pass `substitute_path`, the three `cfg`
   pairs, `refusal_to_backend_error`, and the rustdoc. **GREEN: lib 298/298.**
5. **Task 3** — `c41bc12b` (`test`): `tests/curated_path_injection.rs` and the Makefile entry.

**Plan metadata:** the `docs(128-06)` commit carrying this SUMMARY, `deferred-items.md` and
STATE.md. The state mutations were run BEFORE this file was written so the close-out commit count is
deterministic.

No separate REFACTOR commit: every behaviour-preserving cleanup (formatting, the raw-string
`r##"…"##` fix, the test-module import fix) was made BEFORE the commit it belongs to, so there was
no post-GREEN change to isolate.

## TDD Gate Compliance

`workflow.tdd_mode` is absent from `.planning/config.json`, so the machine RED gate is advisory here
(same as plans 01, 02 and 05). The cycle was still run for Tasks 1 and 2 and its evidence is
recorded.

| Gate | Commit | Evidence |
|---|---|---|
| RED (Task 1) | `11dbcc52` (`test(128-06)`) | 271 discovered, 9 failed, exit 101. Named failures: `validate_rejects_a_path_template_segment_with_two_brace_pairs`, `…_with_text_adjacent_to_a_brace_pair`, `validate_rejects_an_empty_path_template_placeholder`, `validate_rejects_an_unbalanced_path_template_brace`, `schema_parser_narrows_from_a_direct_string_schema`, and all four `build_operation` population rows. |
| GREEN (Task 1) | `d40ba3b1` (`feat(128-06)`) | 271/271; `--lib http::schema` 14; `--lib tools::build_operation` 5; `cargo build --workspace` exit 0 with zero `error[`; `--no-default-features --features http` exit 0; `make lint` exit 0; `make test-server-toolkit` 360. |
| RED (Task 2) | `2825edfe` (`test(128-06)`) | `--lib http::client` 41 discovered, 22 failed, exit 101 — all 22 the target refusals, with the 3 `query_separator` accepts, the declared-pattern accept and the placeholder-free row passing. |
| GREEN (Task 2) | `097bc94f` (`feat(128-06)`) | lib 298/298; `--no-default-features --features http` exit 0; `cargo tree -i pmcp-code-mode` finds no package; pmat 0 violations; clippy clean in every touched file; `pmcp-openapi-server` 42/42. |
| REFACTOR | — (not needed) | See the note above. |

**Task 3 has no meaningful RED, stated rather than fabricated.** Its six probes are integration
probes over behaviour Task 2's GREEN already implemented, so all six passed on first run and a RED
against the post-GREEN tree is not available. The informative evidence for them is the MUTATION TEST
above — four of the six go red when the per-value call is removed — which is a stronger statement
than a RED commit would have been, because it names the specific mechanism each probe depends on.
The two that do not go red under any mutation are the ACCEPT controls, which is correct.

**`gsd_run check tdd-red-evidence` was not invoked.** Plans 01, 02 and 05 measured that its
TAP/node-test parsers cannot read cargo libtest's `test result: FAILED. 262 passed; 9 failed;`
format and return `INVALID_RED (zero_tests_discovered)` on a real RED run. Fabricating TAP output to
satisfy it would be the false-evidence class this repo tracks. The captured run output is the
evidence instead. This remains a GSD-runtime gap for every Rust project.

## Deviations from Plan

### 1. [Rule 1 - Bug in a plan gate] Task 1's `allowReserved` gate and its own T-128-26 test placement are mutually unsatisfiable

- **Found during:** Task 1 test design, before writing the test.
- **Issue:** the gate is
  `grep -v "^\s*//" crates/pmcp-server-toolkit/src/http/schema.rs | grep -ci "allowreserved"`, with
  `<fails_when>` "the printed count is not 0", and its `<fails_when>` prose says "comments are
  stripped first so the explanatory prose that MUST mention the keyword cannot trip this gate".
  **That is not what either grep dialect does.** BSD grep on this machine treats `\s` as a literal
  `s`, so `^\s*//` strips only NON-indented `//` lines; and even under GNU grep, `^\s*//` strips
  comment lines but **not** a YAML/JSON test document line such as `          allowReserved: true`.
  So T-128-26's own mitigation — "a unit test that parses a document declaring `allowReserved: true`"
  — is a non-comment line in that file and would make the gate print `1`.
- **Fix:** the keyword is used in NO line of `src/http/schema.rs`, comment or otherwise. The D-11
  parser comment names D-11 and describes the keyword by its meaning ("the one that grants latitude
  over reserved characters in a serialization") with a cross-reference to
  `crate::config::ParamDecl::allow_slash`, whose doc (plan 03's, in a different file the gate does
  not scope to) names the keyword in full and states the rule. The document-declaring test was
  **relocated** to `tests/curated_path_injection.rs` and **strengthened** from a field assertion to
  an end-to-end refusal that drives the parsed `Parameter` through the real connector with a
  `/`-carrying value, plus an ACCEPT half proving the identical value succeeds once `allow_slash`
  arrives from config. `src/http/schema.rs` keeps a field-level row
  (`schema_parser_leaves_allow_slash_false_for_every_spec_parameter`) over all five shapes.
- **Explicitly NOT done:** obfuscating the token by concatenation (`concat!("allow","Reserved")`),
  which is the evadable-by-syntax class plan 02 Deviation 1 flagged; and weakening the gate.
- **Verification:** the gate prints **0** in BOTH grep dialects (the plan's literal `^\s*//` form
  and a POSIX-correct `^[[:space:]]*//` form), with a positive control — `grep -ci "reserved"` on
  the same file is **3**, so the scan is live and a real `allowReserved` would be found. The gate's
  pipeline exits **1** when the count is 0, because `grep -c` exits 1 on no match; the
  `<fails_when>` reads on the printed count, which is 0.
- **Files modified:** `crates/pmcp-server-toolkit/src/http/schema.rs`,
  `crates/pmcp-server-toolkit/tests/curated_path_injection.rs`
- **Committed in:** `11dbcc52` / `d40ba3b1` / `c41bc12b`

### 2. [Rule 1 - Bug in a plan gate] Task 2's `%2e` acceptance criterion now returns 1, and it is a TEST PAYLOAD

- **Found during:** Task 2 verification.
- **Issue:** the criterion is
  `grep -rn "%2e\|%2E" crates/pmcp-server-toolkit/src/ | grep -c .` returns 0. It returns **1**.
  The single hit is `crates/pmcp-server-toolkit/src/http/client.rs:751`,
  `substitute_one("/content/{version}/CUI", "version", "a%2E%2Eb")` — the upper-case-hex
  decode-once TEST PAYLOAD the plan's own behaviour list demands. It is not a denylist copy.
- **Fix:** the criterion was NOT satisfied by removing the test or renaming the payload. The
  measurement is recorded with the exact hit, and the REAL guard is the one the plan itself names as
  stronger (Fable's annotated caveat): the four structural call sites into
  `pmcp::server::schema_validation`, enumerated with line numbers above and measured
  comments-stripped, plus the three mutation tests. `grep -c` over core as a positive control
  returns **9**, so the denylist demonstrably lives in exactly one place.
- **A future plan tightening this gate** should exclude `#[cfg(test)]` regions or scan for the
  *implementation* shapes (`denied_byte`, a byte-array literal) rather than the encoded spelling.
- **Files modified:** none (a measurement correction).

### 3. [Rule 2 - Missing critical] The missing-path-argument case got its OWN refusal, not only the composed check

- **Found during:** Task 2 implementation.
- **Issue:** the plan's dispositions ledger routes this case through `validate_resolved_path`'s
  residual-brace rule ("one mechanism for both surfaces rather than a second bespoke check"), while
  its behaviour row and its action text both require the refusal to **name the parameter**. Those
  cannot both hold: `validate_resolved_path` is param-agnostic by construction — plan 02 fixed
  `param: "path segment"` on every composed refusal, because a composition belongs to no single
  parameter.
- **Fix:** a dedicated `refuse_missing_path_argument` (a `cfg` pair) raises
  `param '<name>' is a declared path parameter and must be supplied`, and
  `validate_resolved_path` is STILL called at the tail as the backstop for a residual brace arriving
  from any other route — a spec-derived template with an unrecognized placeholder, which config
  validation cannot reach. This is not a second copy of the character denylist; it is a presence
  check.
- **Verification:** mutation 3 above — removing it turns exactly 1 row red, and the record states
  that the composed check still refuses the same request (so nothing reaches the backend either
  way); what is lost is the message naming the declaration.
- **Committed in:** `097bc94f`

### 4. [Rule 2 - Missing critical] `substitute_path` is TWO-PASS rather than check-then-replace per iteration

- **Found during:** Task 2 implementation.
- **Issue:** the plan asks for the check "between `render_scalar` returning the rendered string and
  `path.replace(...)`", which a single-pass loop satisfies — but its own behaviour row asks that a
  refusal on the second of two placeholders leave no partially-substituted URL "anywhere".
  Single-pass leaves one in a local that is then dropped.
- **Fix:** pass 1 renders and checks every contribution into a `Vec`; pass 2 applies them and checks
  the composed result. Nothing is substituted until every value has passed, which is plan 05's
  established two-pass pattern and is strictly stronger than the ordering the plan states.
- **Verification:** `placeholder_floor_refuses_the_second_of_two_placeholders_without_substituting`
  additionally asserts `/a/ok/b/` appears nowhere in the error text.
- **Committed in:** `097bc94f`

### 5. [Plan step ambiguity RESOLVED, recorded] Task 1 step 3b's config-time finding is a HARD ERROR, not a `lint()` finding

- **Issue:** the plan offers "a `lint()` finding (or, coordinate with plan 03 and add a
  `ConfigValidationError` variant)" and requires the choice be recorded.
- **Resolution:** a hard `ConfigValidationError::MalformedPathTemplateSegment`. The reasoning, the
  exact rule, and the positive-controlled pre-flight scan proving no in-tree config is affected are
  in "The curated template parser's limit" above. `ConfigValidationError` is `#[non_exhaustive]`, so
  the variant is not a breaking change.
- **Committed in:** `d40ba3b1`

### 6. [Plan-additive] Twenty tests beyond the plan's behaviour lists

- `placeholder_floor_refuses_traversal_written_into_the_template_literal` — **the isolating row for
  the composed check.** Every per-value payload the plan lists is refused twice over once both
  checks are wired, so none of them can prove the composed check is the mechanism. A template
  literal is never floored, so only the composed check can be refusing this one.
- `placeholder_floor_refuses_a_residual_brace_from_an_unrecognized_template` and
  `placeholder_floor_refuses_a_value_failing_its_declared_pattern` — the spec-derived residual-brace
  route, and the D-10 narrowing direction (the plan's list has the accept half only).
- **`mod query_separator`, 12 rows** — the curated mirror the prompt's inherited-narrowing section
  requires: 3 ACCEPT and 9 STILL-REFUSED, the latter deliberately outnumbering the former.
- **`config.rs`, 3 accept controls** — `validate_accepts_whole_segment_path_template_placeholders`,
  `validate_accepts_a_path_template_carrying_an_author_written_query_string` (the curated half of the
  inherited narrowing, at config level) and `validate_ignores_the_template_rule_for_a_tool_with_no_path`.
  Without them the four refusal rows are satisfiable by refusing every template.
- **`curated_path_injection`, 2 rows beyond the plan's four** — the relocated D-11 row (Deviation 1)
  and `…_accepts_an_author_written_query_string_in_the_path`, which proves the narrowing end to end
  and asserts the operator's query survives to the wire.
- Two `schema.rs` floor-only rows beyond the plan's three (`composed`, `content`), because the plan
  names "a composed schema" in prose but lists only reference / non-string / absent.

---

**Total deviations:** 6 — 2 plan-gate bugs (one mutually unsatisfiable with its own threat
mitigation, one whose zero-assertion now counts a test payload), 2 missing-critical additions that
make a stated behaviour row actually hold, 1 recorded resolution of an explicit plan choice, plus 20
additive tests.
**Impact on plan:** no scope creep. Every file touched is in the plan's `files_modified` list plus
`src/error.rs` (required for the `MalformedPathTemplateSegment` variant the plan's own step 3b
names). One `must_haves` truth is delivered slightly differently than written — the truth about
`substitute_path` calling `validate_resolved_path` "after its last `replace`" holds, but on each side
of an operator-written `?` rather than across it, which is the narrowing the prompt hands down and
not a choice made here.

## Threat mitigations — status after this plan

| Threat | Mitigation | Asserted by |
|---|---|---|
| T-128-25 (curated `substitute_path` injection) | `validate_path_placeholder` on every rendered value before any `replace`, one core implementation | 11 lib rows + 2 integration CR-01 rows; mutation 1 turns 10 red |
| T-128-26 (a spec's reserved-expansion keyword widening the floor) | the parser leaves `allow_slash` false unconditionally, with D-11 named at that line | `schema_parser_leaves_allow_slash_false_for_every_spec_parameter` + the end-to-end `curated_path_injection_spec_allow_reserved_does_not_widen_the_floor` (with its accept half) |
| T-128-27 (`default_max_length = 0` disabling the cap) | the cap is a module constant in core, unreachable from config | `curated_path_injection_cap_independent_of_d3` |
| T-128-28 (refusal messages disclosing input) | `PlaceholderRefusal`'s value-free `Display` forwarded verbatim; the missing-arg message names the param only | `assert_value_free` on the CR-01 rows, `refuse_version`'s path-fragment assertions, `placeholder_floor_refuses_an_absent_path_argument` |
| T-128-29 (the curated build gaining a JS-engine edge) | the new binary is gated on `http` + `input-validation` only | `cargo tree -i pmcp-code-mode` under `--no-default-features --features http` finds no package; the two opposed grep gates |

## Known Stubs

**None.** No `#[ignore]`d test, and `grep -cE "TODO|FIXME|HACK|XXX"` is **0** for all five touched
source files (`http/schema.rs`, `http/client.rs`, `tools.rs`, `config.rs`, `error.rs`) and for
`tests/curated_path_injection.rs`. Nothing was appended to `.planning/WINDOWS.md`, because this plan
produced no stub, no skipped test and no unrun `<verify>` — every filtered verify command was run
and confirmed to select a nonzero count.

## Behaviour Changes

1. **A curated single-call tool's path-placeholder value is now checked before substitution.**
   Refused, with zero upstream requests: a path separator (unless the parameter declares
   `allow_slash`), parent-directory traversal, `?`, `#`, a backslash, any ASCII control byte or NUL
   — in literal or percent-encoded form, decode-once — a bare single dot, an empty value, a value
   over 256 code points, and a value failing a declared `pattern` or `max_length`.
2. **The composed path is checked before dispatch**, on each side of ONE operator-written `?`. This
   makes plan 02's trailing-slash and doubled-slash refusals LIVE on the curated surface, and
   refuses traversal or an over-cap segment written into a `[[tools]]` `path` or an OpenAPI path
   template. **An operator-written query string in a `[[tools]]` `path` is NOT refused** — that is
   the inherited narrowing. A second `?`, a dangling `?`, and a `#` are refused. Scanned: no
   in-tree curated path or spec template is affected (table above).
3. **A declared path parameter with no supplied argument is now an error**, naming the parameter,
   instead of the literal `{name}` travelling toward the backend.
4. **A `[[tools]]` `path` whose segment is not a supported placeholder shape now fails
   `ServerConfig::validate`** — so a server with such a tool refuses to boot rather than sending
   literal braces on every call. No in-tree config is affected.
5. **`http::schema::Parameter` gained three public `#[serde(default)]` fields**, so its serialized
   form carries three new keys. Deserialization of pre-Phase-128 data is unaffected.

All of (1)–(4) are gated on `input-validation`, which is in the toolkit's `default`, so an
unenforced build is an explicit opt-out.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: new_public_api | `crates/pmcp-server-toolkit/src/http/schema.rs` | Three new public `Parameter` fields, `with_rules`, and the gated `placeholder_rules`. There is no `cargo public-api` gate in this repo, so nothing mechanical will catch a later over-wide `pub` here or a change to `placeholder_rules`'s gating. |
| threat_flag: behaviour_change_can_refuse_a_previously_accepted_config | `crates/pmcp-server-toolkit/src/config.rs` | `MalformedPathTemplateSegment` makes `validate()` refuse a config that previously booted. Proven safe for every in-tree config by the positive-controlled scan above, but a THIRD-PARTY config with `/prefix-{id}` will now fail to boot. It was silently broken before; plan 11's rollout note must say so. |
| threat_flag: enforcement_depends_on_a_feature | `crates/pmcp-server-toolkit/src/http/client.rs` | All three checks are `#[cfg(feature = "input-validation")]`. The `cfg(not(...))` halves are written and documented, and `input-validation` is in `default`, but a consumer with `default-features = false` that does not opt back in ships the pre-Phase-128 behaviour. Plan 03's drift test covers the three in-tree such consumers; nothing covers an out-of-tree one. |
| threat_flag: accepted_residual | `crates/pmcp-server-toolkit/src/http/client.rs` | A placeholder value that is itself `{other}` where `other` is ALSO a supplied path parameter can duplicate that other (already floored) value into its own slot. Not an escalation — the caller could place that value there directly, and the value is floored either way — and if `other` is NOT supplied the composed check refuses the residual brace. Recorded rather than closed, because closing it means replacing `String::replace` with a single-pass scan, which would change how spec-derived mid-segment placeholders resolve. |

## Verification results

| Command | Result |
|---|---|
| `RUSTFLAGS="" cargo test -p pmcp-server-toolkit --features http,input-validation --lib` | **298 passed**, 0 failed |
| `… --lib http::schema` | **14 passed** (nonzero — the filter resolves) |
| `… --lib tools::build_operation` | **5 passed** (nonzero — the risky filter shape resolves) |
| `… --lib http::client` | **41 passed** |
| `… --lib http::client::placeholder_floor` / `::query_separator` | **15 / 12** |
| `… --test curated_path_injection` | **6 passed** |
| `… --test curated_path_injection … cap_independent_of_d3` | **1 passed** (the D-08 row is present and correctly named) |
| `RUSTFLAGS="" cargo build --workspace` | exit 0, **0** lines containing `error[` |
| `RUSTFLAGS="" cargo build -p pmcp-server-toolkit --no-default-features --features http` | exit 0 — the Codex-HIGH gate |
| `cargo tree -p pmcp-server-toolkit --no-default-features --features http -e normal -i pmcp-code-mode` | "did not match any packages" — **the passing outcome** (SC-1) |
| `grep -v "^\s*//" src/http/schema.rs \| grep -ci allowreserved` | **0** (and 0 with a POSIX-correct strip; positive control `grep -ci reserved` → 3) |
| `grep -v "^//" tests/curated_path_injection.rs \| grep -c openapi-code-mode` | **0** (and 0 with a POSIX-correct strip) |
| `grep -c openapi-code-mode tests/curated_path_injection.rs` | **2** — nonzero, as the opposed gate requires |
| `git ls-files --error-unmatch -- tests/curated_path_injection.rs` | exit 0 — tracked |
| `RUSTFLAGS="" make test-server-toolkit` | exit 0, **393 tests**; `env_ref_grammar_parity` 1, `base_url_expansion` 7, `input_validation_acceptance` 4, `curated_path_injection` **6** — no previously-required binary regressed |
| `RUSTFLAGS="" make test-server-toolkit-code-mode` | exit 0, **418 tests**, `http_executor` leg 11 |
| `RUSTFLAGS="" make test-code-mode` | exit 0, **317 tests** (unchanged) |
| `RUSTFLAGS="" cargo test -p pmcp --features full --lib schema_validation` | **52 passed** — core left strict |
| `git diff 87611080..HEAD -- src/server/schema_validation.rs` | **0 lines** |
| `RUSTFLAGS="" cargo test -p pmcp-openapi-server --no-fail-fast` | **42 passed, 0 failed** |
| `RUSTFLAGS="" cargo nextest run --features "full" --no-fail-fast` | **3358 run, 3358 passed**, 5 skipped, exit 0 |
| `RUSTFLAGS="" make lint` | exit 0, "No lint issues" |
| `RUSTFLAGS="" make doc-check` | exit 0, "Zero rustdoc warnings" |
| `RUSTFLAGS="" cargo doc -p pmcp-server-toolkit --no-deps --features http,input-validation` | exit 0; **17 warning locations, all pre-existing** and none in new code (enumerated and cross-checked) |
| `cargo clippy -p pmcp-server-toolkit --features http,input-validation --all-targets` | exit 0; **0** findings in any of the six touched files |
| `pmat quality-gate --fail-on-violation --checks complexity` | **PASSED, 0 violations** |
| `RUSTFLAGS="" make quality-gate` | **exit 0 — `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner present** (15180 captured lines) |
| Mutation tests (3, one per check) | **10 / 10 / 1** rows red; file restored byte-exact (`git diff --stat` empty, `grep -c "MUTATION TEST"` → 0, suite back to 41/41) |
| SATD in the six touched files | **0** |
| Tracked working tree after `.pmat/` cleanup | **clean** |

## Issues Encountered

- **rtk output interception, again.** Every measurement in this SUMMARY was taken through an
  absolute binary path (`/usr/bin/grep`, `/usr/bin/wc`, `/usr/bin/git`, `/usr/bin/make`,
  `/Users/guy/.cargo/bin/cargo`) and long gates were run unpiped to a file, then grepped.
- **`grep -c` exits 1 when the count is 0**, so a `bash -o pipefail` gate whose passing outcome is
  `0` exits non-zero. Both Task 1's `allowreserved` gate and Task 3's comment-stripped
  `openapi-code-mode` gate have that shape. Their `<fails_when>` clauses read on the PRINTED COUNT,
  which is what was checked; a future plan writing such a gate should say so explicitly or append
  `|| true`.
- **A raw string carrying an OpenAPI `$ref` needs `r##"…"##`.** The first draft of `SHAPES_JSON`
  used `r#"…"#` and the `"#/components/schemas/Cui"` value closed the literal, producing three
  confusing `unknown prefix` parse errors. Noted in a comment at both fixture sites.
- **`.pmat/` runtime churn.** `pmat quality-gate` modifies four tracked `.pmat/` files; all restored
  with `git checkout -- .pmat/` rather than committed, exactly as plans 01, 02 and 05 did.

## Next Phase Readiness

**Unblocked and ready:**

- **Plan 07** (`cargo pmcp validate config`) — `ConfigValidationError::MalformedPathTemplateSegment`
  is a new `validate()` failure the CLI will surface. It carries `tool` and `segment`, both
  author-written config, so both are safe to print to a config author.
- **Plan 08** (the `placeholder_rules` seam) — the curated half is
  `Parameter::placeholder_rules(&self) -> PlaceholderRules<'_>`, signature quoted verbatim above.
  Plan 05's Code Mode half is `HttpExecutor::placeholder_rules(&self, method, path_template, param)`.
  Both build rules through the `with_*` builders. `Parameter` is already populated from BOTH the
  config and the spec, so plan 08's `OpenApiSchema::operator_for(path, METHOD)` lookup on the Code
  Mode side has an already-working precedent to read from on this side.
- **Plan 09** (the startup log) — no new knob was added; `ValidationSection` is unchanged.
- **Plan 11** (the release commit) — see the rollout items below.

**Rollout items plan 11 must carry** (in addition to plan 02's trailing-slash note and plan 05's H1–H3):

- **R1.** The curated single-call surface now refuses injected path-placeholder values. Same list as
  plan 05's H3, on the `[[tools]]`/spec path rather than the script path, and **with the same
  narrowing**: an operator-written query string in a `[[tools]]` `path` keeps working. Do NOT tell
  operators to move query values out of the path.
- **R2.** A missing argument for a declared path parameter is now an error rather than a literal
  `{name}` on the wire.
- **R3. A `[[tools]]` `path` with a segment that is not exactly one whole-segment `{name}` now fails
  to BOOT** (`MalformedPathTemplateSegment`). No in-tree config is affected, but a third-party
  config with `/prefix-{id}` or `/search/{a}{b}` will stop starting — it was silently broken before,
  sending literal braces upstream. This is the loudest behaviour change in this plan and belongs in
  the CHANGELOG's breaking section.
- **R4.** `http::schema::Parameter` gained three public `#[serde(default)]` fields and two methods.
  Additive; `Parameter::new`'s signature is unchanged and the workspace has zero struct-literal
  construction sites.

**Concerns to carry forward:**

- **The composed check's evidence is unit-level only.** Mutation 2 turned zero integration rows red,
  because every integration payload is per-value. If a later plan adds integration coverage for the
  composed mechanism, the shapes to use are the three isolating rows named above.
- **Two plan gates were weaker or self-contradictory than their intent** (Deviations 1 and 2). Both
  are recorded with the measurement and the stronger replacement instrument.
- **The `{other}`-valued placeholder residual** is recorded as a threat flag above rather than
  closed; closing it means replacing `String::replace` with a single-pass scan, which would change
  how spec-derived mid-segment placeholders resolve.
- **`crates/pmcp-server-toolkit` has 17 pre-existing rustdoc warning locations no gate sees.** This
  plan added none (each was cross-checked against its line) and one further pre-existing
  `unused_imports` warning is logged in `deferred-items.md` as D4.

## Self-Check: PASSED

- All six modified/created files exist on disk, and this SUMMARY and `deferred-items.md` exist.
- All five code commits are present in `git log`: `11dbcc52`, `d40ba3b1`, `2825edfe`, `097bc94f`,
  `c41bc12b` (`git rev-list --count 87611080..HEAD` == 5 before this metadata commit).
- Every `<acceptance_criteria>` row in all three tasks was re-run and passes, except the one
  recorded as Deviation 2, whose real measurement and stronger replacement instrument are both
  recorded rather than the criterion being silently restated.
- Every plan-level `<verification>` command was re-run and its result is tabulated above.
- Every filtered verify command was confirmed to select a NONZERO count; no count was lowered.
- The inherited narrowing is mirrored: 3 accept / 9 refuse at unit level plus 2 accept rows at
  config and integration level, and `128-05-SUMMARY.md`'s superseded "migrate to body params" claim
  is carried nowhere.
- Core left strict: `git diff 87611080..HEAD -- src/server/schema_validation.rs` is 0 lines and its
  52 tests are green.
- Three mutation tests run and the file restored byte-exact.
- Tracked working tree clean (`.pmat/` runtime churn restored, never committed).

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-27*
