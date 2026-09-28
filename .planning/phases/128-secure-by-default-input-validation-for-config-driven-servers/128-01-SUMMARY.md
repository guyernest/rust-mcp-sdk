---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 01
subsystem: api
tags: [jsonschema, input-validation, json-schema-2020-12, feature-flags, makefile, wiremock, tdd]

# Dependency graph
requires:
  - phase: 115-schema-dialect-pin
    provides: "`src/server/output_validation.rs` — `normalize_schema_dialect`, `compile_2020_12`, `compile_for_era`, `cached_validator`, and the three-function split this plan reuses rather than copies"
  - phase: 090-openapi-http-backend
    provides: "`crates/pmcp-server-toolkit` `[[tools]]` synthesizer (`synthesize_inner`, `synthesize_http_inner`) and its wiremock-backed test harness"
provides:
  - "`pmcp::server::schema_validation` — the one input-validation entry point in the SDK: `validate_input`, `render_refusal`, `InputViolation`"
  - "`schema-validation` core cargo feature (D-04), with `validation` redefined as its superset `[\"schema-validation\", \"dep:garde\"]`"
  - "`crates/pmcp-server-toolkit` `input-validation = [\"pmcp/schema-validation\"]`, and ZERO `jsonschema` dependency edges of the toolkit's own"
  - "`ValidatingToolHandler` — crate-private decorator validating in BOTH `handle` and `handle_output`, wired at all three handler push sites"
  - "`crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs` — acceptance-matrix rows 8, 9, 10, 11"
  - "`make test-code-mode` and `make test-server-toolkit-code-mode` — two count-asserting gate legs chained into `test-all`"
affects: [128-02, 128-03, 128-04, 128-05, 128-06, 128-07, 128-08, 128-09, 128-10, 128-11]

# Actuals (#2632) — chars/4 over the realized diff, NOT a harness token count.
actuals:
  tokens: 14942
  tasks: 3
  commits: 4
  plan_head_before: 99ff8f0a4a02fe0a4b8bc09a99841cca1c31ba77
  # MEASURED: git rev-list --count 99ff8f0a..HEAD == 4 — the three task commits
  # (08c7c5b5 test, 9032e6ce feat, 31cccaa6 chore) plus this plan's docs commit.

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Value-free refusal rendering from `ValidationErrorKind` only (SP-1 / SC-7)"
    - "Input validator as a FOURTH sibling of the output three-function split, never a fifth branch (SP-4)"
    - "Nonzero-test-count proof per gate leg, plus per-binary `REQUIRED_TEST_BINARIES` guards (SP-2)"
    - "Superset feature redefinition so a split changes no existing consumer's graph (D-04)"

key-files:
  created:
    - src/server/schema_validation.rs
    - crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs
  modified:
    - Cargo.toml
    - Makefile
    - fuzz/Cargo.toml
    - src/server/mod.rs
    - src/server/output_validation.rs
    - crates/pmcp-server-toolkit/Cargo.toml
    - crates/pmcp-server-toolkit/src/tools.rs

key-decisions:
  - "Operator confirmed the one-way D-01 public surface verbatim as `publish-as-specified`: `validate_input`, `render_refusal` and `InputViolation` ship as `pmcp` 2.x public API, and `schema-validation` ships as a feature name."
  - "Q1 CONFIRMED — `format` enforces, via an input-only `should_validate_formats(true)` builder, so inputs get their own compile entry point and never share the output validator."
  - "Q6 CONFIRMED — a separate input validator cache keyed on schema TEXT alone, with the key accepted from the caller as `Option<&str>` so no `jsonschema` type reaches the public API."
  - "`validate_input` takes the pre-computed cache key as its THIRD parameter rather than exposing a sibling function, keeping the published surface at exactly three items."
  - "The 18 `feature = \"validation\"` -> `feature = \"schema-validation\"` cfg renames landed HERE, not in plan 02, breaking the circular wave-1/wave-2 dependency all three review lanes found."
  - "A non-compiling declared `inputSchema` refuses every call with a DETAIL-FREE `keyword: \"schema\"` violation; the compile detail is logged server-side only."
  - "`render_refusal` echoes a JSON pointer only when its first segment is in the caller-supplied `declared` allow-list, so no caller-supplied name can reach a client message even for keywords whose pointer is not `additionalProperties`."

patterns-established:
  - "Pattern: decorator-completeness — a `ToolHandler` decorator implements BOTH `handle` and `handle_output` and delegates each to the inner handler's own, so wrapping never downgrades an envelope override to the `ToolOutput::Payload` default."
  - "Pattern: feature-off is logged, never silent — `enforce_input_schema` emits a `tracing::warn!` per synthesized tool when `input-validation` is absent, so an enforcement that is off cannot read as on."
  - "Pattern: three-guard gate leg — crate-wide count + a module-scoped count + a comment-stripped `grep` pinning the feature flag, because any one of the three alone is satisfiable while the target module is uncompiled."

requirements-completed: [D1, D4, SC-1, SC-7, SC-8]

coverage:
  - id: D1
    description: "A config-declared tool refuses an undeclared argument key before any backend call, with zero upstream requests recorded"
    requirement: "D1"
    verification:
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs#input_validation_refuses_undeclared_argument_without_contacting_upstream"
        status: pass
    human_judgment: false
  - id: D2
    description: "Absent `arguments` validates as `{}`: ACCEPTED on a zero-parameter tool, REFUSED by `required` (never by `type`) on a required-param tool"
    requirement: "D1"
    verification:
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs#input_validation_accepts_absent_arguments_on_zero_param_tool"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs#input_validation_refuses_absent_arguments_when_required_declared"
        status: pass
      - kind: unit
        ref: "src/server/schema_validation.rs#schema_validation_accepts_absent_arguments_on_a_zero_parameter_tool"
        status: pass
      - kind: unit
        ref: "src/server/schema_validation.rs#schema_validation_refuses_absent_arguments_by_required_never_by_type"
        status: pass
    human_judgment: false
  - id: D3
    description: "The compliant control: a fully valid call produces exactly ONE upstream request carrying the server-side auth header"
    requirement: "SC-1"
    verification:
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs#input_validation_accepts_compliant_call_with_one_upstream_request"
        status: pass
    human_judgment: false
  - id: D4
    description: "The refusal message carries a COUNT of unknown arguments plus the DECLARED parameter names, and no byte of the rejected key or value (SC-7)"
    requirement: "SC-7"
    verification:
      - kind: unit
        ref: "src/server/schema_validation.rs#schema_validation_refuses_undeclared_argument_with_a_count_only_message"
        status: pass
      - kind: integration
        ref: "crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs#input_validation_refuses_undeclared_argument_without_contacting_upstream"
        status: pass
    human_judgment: false
  - id: D5
    description: "Refusal rendering is deterministic — the same (schema, arguments) pair renders byte-identically across repeat calls"
    verification:
      - kind: unit
        ref: "src/server/schema_validation.rs#schema_validation_renders_byte_identical_refusals_across_repeat_calls"
        status: pass
    human_judgment: false
  - id: D6
    description: "`schema-validation` stands alone as a core feature and `validation` remains a working superset; no `garde` node under the standalone build (D-04)"
    requirement: "D4"
    verification:
      - kind: other
        ref: "RUSTFLAGS=\"\" cargo build -p pmcp --no-default-features --features schema-validation"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" cargo build -p pmcp --features validation"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" cargo tree -p pmcp --no-default-features --features schema-validation -e normal -i garde  (package-not-found IS the pass)"
        status: pass
    human_judgment: false
  - id: D7
    description: "Every new gated test binary is count-asserted by a gate leg that actually enables its features (SC-8)"
    requirement: "SC-8"
    verification:
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-server-toolkit  (305 tests; input_validation_acceptance 4)"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-code-mode  (285 crate-wide; eval_semantic_regression 34; executor::-scoped 81)"
        status: pass
      - kind: other
        ref: "RUSTFLAGS=\"\" make test-server-toolkit-code-mode  (320 tests; http_executor 5)"
        status: pass
    human_judgment: false
  - id: D8
    description: "`ValidatingToolHandler` wraps the handler at all THREE push sites, confirmed by reading each push expression rather than by a count"
    requirement: "D1"
    verification: []
    human_judgment: true
    rationale: "The plan's own acceptance criterion states a `grep -c >= 3` does NOT force this, because two of the three occurrences can sit in one function. The three push expressions were read and are recorded below with line numbers; confirming that reading is a human judgment, not an automated assertion."

# Metrics
duration: ~95min
completed: 2026-09-27
status: complete
---

# Phase 128 Plan 01: Secure-by-default input validation (tracer) Summary

**One config-declared HTTP tool now refuses an undeclared argument through the real `pmcp::server::schema_validation::validate_input` seam with zero upstream requests and a count-only, value-free refusal — plus the `schema-validation` feature split, its 18-attribute cfg migration, and three gate legs that make every later verification in this phase real.**

## Performance

- **Duration:** ~95 min (continuation agent; Task 1 checkpoint was resolved before dispatch)
- **Completed:** 2026-09-27T03:46:43Z
- **Tasks:** 3 of 3 (Task 1 was the resolved decision checkpoint)
- **Files modified:** 9 (2 created, 7 modified)

## Checkpoint decision — recorded verbatim

Task 1 was a `checkpoint:decision` (gate: blocking) on the one-way D-01 public surface. The operator's
response, verbatim:

```
publish-as-specified
```

This selected the plan's recommended option: publish `pmcp::server::schema_validation::validate_input`,
`render_refusal` and `InputViolation` as `pmcp` 2.x public API, and publish `schema-validation` as a
feature name (D-01, D-04), with `validation` redefined as `["schema-validation", "dep:garde"]` so
existing `validation` / `full` / `full-v2` consumers see no change. `render_refusal` IS public, because
`pmcp-server-toolkit` is a separate crate and must not reimplement the value-free renderer.

The two items surfaced for confirmation rather than debate were **confirmed as the plan specifies them**:

- **Q1** — `format` enforces, via a dedicated input-only `should_validate_formats(true)` builder.
- **Q6** — a separate input validator cache keyed on schema text alone, with the key accepted from the
  caller as `Option<&str>` so no `jsonschema` type reaches the public API.

Surface as built — exactly three public items, none naming a `jsonschema` type
(`grep -n 'pub fn\|pub struct\|pub mod' src/server/schema_validation.rs`):

```
46:pub struct InputViolation {
89:pub fn validate_input(
274:pub fn render_refusal(violations: &[InputViolation], declared: &[&str]) -> String {
```

## Accomplishments

- **The whole input-validation architecture is proven end to end on one path.** A synthesized
  single-call HTTP tool, handed `{"cui": "C0018787", "apiKey": "secret"}` against a schema declaring
  `cui` and `version`, returns `Err` with **zero** wiremock-recorded requests, and the error names both
  declared parameters while containing neither `apiKey` nor `secret`.
- **Four acceptance rows, not one** — matrix rows 8, 9, 10 and 11, including the easy-to-conflate
  absent-`arguments` pair (accepted as `{}` on a zero-parameter tool, refused by `required` on a
  required-param tool) and the compliant control that makes the refusals meaningful.
- **`schema-validation` exists as a standalone core feature** and builds with neither `validation` nor
  `garde` present; `validation` remains a working superset, so `full`, `full-v2` and every existing
  consumer are untouched.
- **The toolkit now holds exactly zero `jsonschema` dependency edges of its own** — the parallel
  validator path D-01 rejects is gone.
- **Three gate legs are real rather than green-on-zero**: `test-server-toolkit` count-asserts the new
  binary, and `pmcp-code-mode` plus the toolkit's `openapi-code-mode` surface each got a count-asserting
  leg chained into `test-all` where previously they had none / ran zero tests.

## Task Commits

1. **Task 2 (TRACER, tdd) — RED** — `08c7c5b5` (`test`): the four acceptance rows, the four core unit
   tests, the D-04 feature split, the 18-line cfg migration, the toolkit feature forward, the decorator
   wiring and the Makefile repair — with `validate_input` a documented no-op placeholder.
2. **Task 2 (TRACER, tdd) — GREEN** — `9032e6ce` (`feat`): `compile_input_2020_12`,
   `cached_input_validator`, `effective_arguments`, `expectation`, `violation`, `render_refusal` /
   `render_one`.
3. **Task 3 — Wave-0 gate repair** — `31cccaa6` (`chore`): `test-code-mode`,
   `test-server-toolkit-code-mode`, the `test-all` chain, and the `fuzz/Cargo.toml` comment correction.

No REFACTOR commit: the only cleanups (three clippy findings and six rustdoc intra-doc-link findings)
were caught and fixed **before** the GREEN commit, so there was no post-GREEN change to isolate.

**Plan metadata:** see the `docs(128-01)` commit that carries this SUMMARY.

## TDD Gate Compliance

`workflow.tdd_mode` is **false** in this project's config, so the machine RED gate is advisory here.
The cycle was still run and its evidence recorded.

| Gate | Commit | Evidence |
|---|---|---|
| RED | `08c7c5b5` (`test(128-01)`) | 4 tests discovered in `input_validation_acceptance`; rows 9 and 10 failed on their behaviour assertions (`absent arguments must be refused when a parameter is required: Object {"ok": Bool(true)}` and `an undeclared argument key must be refused: Object {"ok": Bool(true)}` — i.e. the call reached the backend and returned its body). Core module: 4 discovered, 3 failed on behaviour assertions. |
| GREEN | `9032e6ce` (`feat(128-01)`) | `input_validation_acceptance` 4/4; core `schema_validation` unit tests 4/4; `make test-server-toolkit` 305 tests, exit 0. |
| REFACTOR | — (not needed) | No post-GREEN behaviour-preserving change existed to commit separately. |

**Known-vacuous RED passes, stated rather than hidden.** Rows 8 and 11 (and the core
`accepts_absent_arguments_on_a_zero_parameter_tool` test) PASSED during RED, because a no-op validator
trivially accepts everything. That is not an unexpected-GREEN violation of the RED gate — those three are
the ACCEPT controls, and their passing is exactly why the refusal rows 9 and 10 carry the discriminating
load. All three would fail if the implementation over-refused, which is what they are there to catch.

**`gsd_run check tdd-red-evidence` cannot classify this run, and the reason is a tool-format limitation,
not a RED failure.** Fed the real captured output (exit 101, 4 tests, 2 named failures), the classifier
returned `INVALID_RED (zero_tests_discovered)` with `tests: 0, fail: 0`. Its TAP/node-test parsers
(`parseNodeTestSummary`, `tapFailedTestNames`) do not understand cargo libtest's
`test result: FAILED. 2 passed; 2 failed;` / `test <name> ... FAILED` format. Fabricating TAP output to
satisfy it would be exactly the false-evidence class this repo tracks, so it was not done; the captured
run output is the evidence instead. **This is a GSD-runtime gap worth reporting: the RED gate is
unusable for any Rust project.**

## The three push sites — read, not counted

The plan's acceptance criterion explicitly refuses a `grep -c >= 3` here, because two of the three
occurrences can sit inside one function. Each push expression was read; each `out.push(...)` receives the
`handler` binding that `enforce_input_schema` had just rebound on the preceding line
(`crates/pmcp-server-toolkit/src/tools.rs`):

| # | Function | Wrap | Push |
|---|---|---|---|
| 1 | `synthesize_inner` (SQL handler) | `:149` `let handler = enforce_input_schema(handler, &info, decl);` | `:150` `out.push((decl.name.clone(), info, handler));` |
| 2 | `synthesize_http_inner` (SCRIPT tool — the `build_script_tool` branch that `continue`s) | `:583` same expression | `:584` same push |
| 3 | `synthesize_http_inner` (single-call HTTP handler) | `:608` same expression | `:609` same push |

Site 2 is the one Fable measured as easy to miss: it sits before a `continue`, so wrapping only the HTTP
push at site 3 would leave every script tool unvalidated and T-90-05-03 false. Wrapping all three is what
makes the four public entry points (`synthesize_from_config`,
`synthesize_from_config_with_connector`, `synthesize_from_config_with_http_connector`,
`synthesize_from_config_with_http_connector_and_scripts`) inherit the enforcement with none forgotten.

## `ValidationErrorKind` arms implemented at tracer scope

Per the plan, two arms plus a generic fallback. Plan 02 fills the rest.

| Arm | Rendering | Note |
|---|---|---|
| `AdditionalProperties { unexpected }` | `format!("unknown argument(s): {}", unexpected.len())` | `unexpected` is read **only** through `.len()` — it is the caller's own key list and may itself be sensitive. |
| `Required { property }` | `` format!("`{}` is required", property.as_str().unwrap_or("<unnamed>")) `` | `property` is a `serde_json::Value` in 0.49.2, so `as_str()` strips the quotes `Value`'s `Display` would add. |
| `_` | `"does not match the declared schema"` | Value-free fallback; a keyword with no safe rendering yet still REFUSES. |

`jsonschema::ValidationError`'s `Display` is never called anywhere in the new module, and
`output_validation.rs`'s `format!("{} (at {})", e, e.instance_path())` anti-analog is not reused.
`instance_path()` appears exactly once in the new module (`:161`), assigned to
`InputViolation::pointer` — never inside a `format!` that also formats the error with `{}`.

**Note on `e.kind` vs `e.kind()`, measured.** The plan records that an earlier draft claimed the field
form is `E0615` and that Fable measured this as wrong. Checked against
`jsonschema-0.49.2/src/error.rs` this session: the `kind` field lives on `ValidationErrorParts` (`:402`),
not on `ValidationError`, whose `repr` is private; `ValidationError::kind()` is the method (`:445`). So on
`ValidationError` **only the method form compiles**, which matches RESEARCH Finding 1c rather than the
plan's correction of it. `e.kind()` is used.

## Measured gate-leg test counts

| Leg | Crate-wide total | Per-binary / scoped guards |
|---|---|---|
| `make test-server-toolkit` | **305** | `env_ref_grammar_parity` 1, `base_url_expansion` 7, `input_validation_acceptance` **4** |
| `make test-code-mode` | **285** | `eval_semantic_regression` 34, `executor::`-scoped selection **81** |
| `make test-server-toolkit-code-mode` | **320** | `http_executor` **5** |

**The SP-2 hole, measured on this tree rather than assumed.** With `--features http` alone (the leg's
pre-change flags) the toolkit total is **301** and `input_validation_acceptance.rs` prints its
`Running tests/…` target line and then **runs zero tests**. With `http,input-validation` the total is
**305** — exactly the four new rows, which also proves no pre-existing toolkit binary lost tests.

**The T-128-04a hole, measured.** `cargo test -p pmcp-code-mode --lib executor::` selects **3** tests
with default features and **81** with `--features js-runtime`. So an `executor::`-scoped nonzero-count
assertion is *not* sufficient on its own — three unrelated tests whose path happens to contain
`executor::` keep it green while `pub mod executor` is uncompiled. The leg therefore carries three
guards: the crate-wide count, the scoped count, and a comment-stripped `grep -c js-runtime` over the
Makefile that pins the feature so a later edit cannot drop it silently. That 3-vs-81 measurement is
recorded in the leg's own header comment.

## Files Created/Modified

- `src/server/schema_validation.rs` **(new, 416 lines)** — the one input-validation entry point:
  `InputViolation`, `validate_input`, `render_refusal` (public); `render_one`, `effective_arguments`,
  `violation`, `expectation`, `compile_input_2020_12`, `cached_input_validator` (private); four unit tests.
- `crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs` **(new, 241 lines)** — acceptance
  rows 8/9/10/11, all four using `received_requests()` rather than `.expect(0)`.
- `Cargo.toml` — `schema-validation = ["dep:jsonschema"]`; `validation = ["schema-validation",
  "dep:garde"]`. `full` (`:280`) and `full-v2` (`:295`) untouched. The load-bearing
  `jsonschema = { version = "0.49", optional = true, default-features = false }` line survives
  verbatim — the only `jsonschema` line in this file's diff is the feature line.
- `src/server/output_validation.rs` — all **18** lines carrying `feature = "validation"` rewritten to
  `feature = "schema-validation"` with every surrounding form preserved (`:106` `not(...)`, `:668`/`:811`
  `all(...)` with `fuzzing`, `:932` `all(test, ...)`, plus the `:662` doc mention);
  `normalize_schema_dialect` widened to `pub(in crate::server)`; one rationale comment added. No function
  signature, `where` clause, `match` arm or in-body statement changed; `compile_for_era`'s `Era::V1` /
  `Era::V2` arms have **zero** diff lines; `normalize_schema_dialect`, `compile_2020_12`,
  `compile_for_era` and `cached_validator` all still exist as separate `fn` items.
- `src/server/mod.rs` — `#[cfg(feature = "schema-validation")] pub mod schema_validation;` placed
  adjacent to the `output_validation` pair; `output_validation`'s cfg-pair structure untouched.
- `crates/pmcp-server-toolkit/Cargo.toml` — `input-validation = ["pmcp/schema-validation"]`; the crate's
  own optional `jsonschema` dependency **deleted** (`grep -c '^jsonschema'` -> 0). `default` still
  `["code-mode"]` — turning `input-validation` on by default is plan 03's task.
- `crates/pmcp-server-toolkit/src/tools.rs` — `enforce_input_schema` + `ValidatingToolHandler` (with
  `handle`, `metadata` AND `handle_output`), wired at three push sites.
- `Makefile` — `test-server-toolkit` gains `input-validation` and the third `REQUIRED_TEST_BINARIES`
  entry; two new legs; `test-all` chain.
- `fuzz/Cargo.toml` — feature-rationale comment now names `schema-validation`; the `features` list is
  unchanged (`validation` is the superset and still reaches the seam).

## Decisions Made

- **`validate_input` takes the pre-computed cache key as a third parameter** rather than exposing a
  key-taking sibling. Q6 permitted either; the parameter form keeps the operator-confirmed published
  surface at exactly three items instead of four.
- **A non-compiling declared schema tells the client nothing but "the tool's declared inputSchema is not
  a valid JSON Schema"**, with the compile detail emitted via `tracing::warn!`. The detail is
  author-supplied schema text and carries no caller data, so this is conservatism rather than necessity —
  but it also makes the refusal trivially deterministic.
- **`render_refusal` will echo a JSON pointer only when its first segment is in `declared`.** The plan
  required value-freedom; this generalizes it to name-freedom for every keyword, not just
  `additionalProperties`, so a future arm (e.g. `unevaluatedProperties`, or a schema whose
  `additionalProperties` is a subschema rather than `false`) cannot leak a caller-supplied key through
  the pointer.
- **A disagreement between `is_valid` and `iter_errors` REFUSES** rather than falling through. An
  enforcement that cannot describe a failure must still enforce it.
- **`schema_validation` is NOT `#[cfg(not(target_arch = "wasm32"))]`-gated**, matching its direct analog
  `output_validation` rather than PATTERNS' suggestion to copy `pub mod validation`'s target gate. The
  `jsonschema` dependency carries `default-features = false` specifically so it is wasm-clean, and
  `make wasm-build` uses `--no-default-features --features wasm`, which enables neither
  `schema-validation` nor `validation` — so the module is absent from the wasm build by feature, and a
  target gate would add nothing but a second reason. Recorded because it is a deliberate departure from
  PATTERNS.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Three clippy findings in the new module under the real gate**
- **Found during:** Task 2 (GREEN, before commit)
- **Issue:** `make lint` (pedantic + nursery, which is STRICTER than a bare
  `cargo clippy -- -D warnings`) rejected three patterns: `needless_pass_by_value` on
  `fn violation(e: jsonschema::ValidationError<'_>)`, `or_fun_call` on
  `expectation(&e).unwrap_or((…, GENERIC_MISMATCH.to_string()))`, and `map_unwrap_or` on the
  determinism test's `.map(…).unwrap_or_else(…)` over a `Result`.
- **Fix:** `violation` now takes `&ValidationError`; `unwrap_or` -> `unwrap_or_else`; the determinism test
  uses a small `refuse()` closure with `expect_err` instead of the `map`/`unwrap_or_else` chain.
- **Files modified:** `src/server/schema_validation.rs`
- **Verification:** `make lint` exit 0, zero issues.
- **Committed in:** `9032e6ce` (Task 2 GREEN commit — fixed before the commit, so no separate commit).

**2. [Rule 3 - Blocking] Six rustdoc intra-doc-link errors under `make doc-check`**
- **Found during:** Task 2 (GREEN, before commit)
- **Issue:** `RUSTDOCFLAGS="-D warnings"` rejected the new docs. (a) The `///` doc on
  `pub mod schema_validation;` in `mod.rs` linked `[`output_validation`]`, which is `pub(crate)` —
  `rustdoc::private_intra_doc_links`. (b) Every `[`…`]` link in `schema_validation.rs`'s module-level
  `//!` block resolved in the PARENT module's scope, not the module's own: rustdoc reported
  `no item named output_validation in module pmcp` for `[`super::output_validation`]`, proving the scope
  was `crate::server` rather than `crate::server::schema_validation`. The cause is the module carrying
  BOTH an outer `///` doc at its declaration site and inner `//!` docs.
- **Fix:** de-linked to plain backticks in the module-level prose and in the `mod.rs` declaration doc.
  Item-level links (e.g. `InputViolation::pointer`'s reference to `render_refusal`) resolve correctly and
  were left as links.
- **Files modified:** `src/server/schema_validation.rs`, `src/server/mod.rs`
- **Verification:** `make doc-check` exit 0, zero rustdoc warnings.
- **Committed in:** `9032e6ce`.

**3. [Rule 3 - Blocking] A backtick inside a double-quoted shell string in the new Makefile leg**
- **Found during:** Task 3
- **Issue:** the `test-code-mode` failure message contained `` `pub mod executor` `` inside a
  double-quoted `echo`, where sh treats backticks as command substitution — the recipe would have tried
  to execute `pub mod executor` on the failure path.
- **Fix:** removed the backticks from that message. A scan confirmed every remaining backtick in the new
  Makefile region sits in a `#` comment, where make does not pass it to a shell.
- **Files modified:** `Makefile`
- **Verification:** `make test-code-mode` exit 0; `make -n test-all` resolves.
- **Committed in:** `31cccaa6`.

**4. [CLAUDE.md-driven] Corrected a stale doc phrase the plan's carried findings flagged**
- **Found during:** Task 2
- **Issue:** `src/server/mod.rs` carried the phrase "no-op unless the `validation` feature is enabled"
  **twice** — once as a `//` comment (`:72-73`) and once as the `///` doc on the `fuzzing` arm (`:85-86`).
  Both go stale the moment D-04 splits the feature. The dispatch brief named "that one doc line".
- **Fix:** corrected both, since fixing one would leave a stale twin. Doc wording only; the `fuzzing` cfg
  structure is untouched.
- **Files modified:** `src/server/mod.rs`
- **Verification:** `make doc-check` exit 0; no cfg diff on the `output_validation` declaration pair.
- **Committed in:** `08c7c5b5`.

---

**Total deviations:** 4 auto-fixed (3 blocking, 1 documentation correctness).
**Impact on plan:** No scope creep. Three were gate failures that had to be cleared to commit at all;
the fourth was explicitly authorized by the dispatch brief and widened only to its identical twin.

## Pre-existing toolkit tests: zero fixture corrections required

The plan anticipated that existing tests invoking a synthesized handler directly would now pass through
D1 and might need fixture corrections. **None did.** Every such call already supplies only declared
parameters:

- `crates/pmcp-server-toolkit/tests/http_connector_props.rs` (~`:149`) — passes exactly its two declared
  params; runs in the `http,input-validation` leg and passes.
- `crates/pmcp-server-toolkit/tests/script_tool.rs` (`:102`, `:172`),
  `tests/code_mode_tools.rs` (`:148`/`:195`/`:223`), `tests/script_tool_engine_parity.rs` — all
  `#![cfg(feature = "openapi-code-mode")]`, so they run in the new
  `test-server-toolkit-code-mode` leg, which does NOT enable `input-validation`; the decorator is the
  identity function there.

Measured: `make test-server-toolkit` 305 vs a 301 baseline under `--features http` alone — the delta is
exactly the four new rows, so nothing pre-existing changed count or outcome.

## Issues Encountered

- **rtk output interception corrupted several measurements.** `grep -c`, `wc -l`, `tail` and redirected
  `make` output were all rewritten or truncated by the rtk proxy hook, at one point producing a 51-line
  file for a 475-line run. Every measurement in this SUMMARY was re-taken through `rtk proxy <cmd>` or an
  absolute binary path (`/usr/bin/grep`) and cross-checked in Python. This matches the recorded
  "rtk proxy corrupts `git diff`/`gh pr checks`" memory; the blast radius is wider than that note
  suggests.
- **`gsd_run check tdd-red-evidence` is unusable for Rust** — see § TDD Gate Compliance.

## Known Stubs

None. The RED-phase no-op `validate_input` was replaced in the very next commit (`9032e6ce`); no stub,
placeholder, `TODO`/`FIXME`/`HACK`/`XXX`, or skipped test remains in either new file
(`! grep -rn "TODO\|FIXME\|HACK\|XXX" …` exits 0).

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: stale_threat_comment | `crates/pmcp-server-toolkit/src/tools.rs` (`HttpToolHandler::handle`) | The T-90-03-01 comment claims the object envelope is "enforced upstream". As of this plan it IS enforced — but by `ValidatingToolHandler`/`validate_input` in THIS module, not upstream. The comment now names the wrong enforcer, which is the SC-6 defect class. Deliberately NOT edited here: SC-6's three-comment sweep is owned by a later plan, and touching it would collide on this file. |
| threat_flag: new_public_api | `src/server/schema_validation.rs` | Three new `pmcp` 2.x public items and one new feature name, all operator-confirmed one-way (D-01/D-04). There is no `cargo public-api` gate in this repo, so nothing mechanical will catch a later over-wide `pub` in this module. |

## Deferred / out-of-scope, recorded

- **`.pmat/context.db` churn.** Running the CLAUDE.md-mandated
  `pmat quality-gate --fail-on-violation --checks complexity` modified the tracked `.pmat/context.db` and
  deleted the tracked `.pmat/context.db-shm` / `-wal` sqlite sidecars. **Restored to HEAD** rather than
  committed (`git checkout -- .pmat/...`), so the working tree is clean and pmat state is exactly as
  committed — it is tool runtime state, not this plan's output, and the tracked-sidecar question predates
  this phase.
- **`--features validation` gates in `src/server/typed_tool.rs`** are plan 04's (garde/E3); the
  zero-assertion on the old string is deliberately scoped to `src/server/output_validation.rs` alone so
  it cannot contradict that wave peer.
- **Property and fuzz arms** for `schema_validation` are not in this plan (`128-08` owns the durable
  `(schema, arguments)` fuzz guard for T-128-02). CLAUDE.md's ALWAYS set is satisfied at unit +
  integration level here; the fuzz/property/example legs land with the plans that own them.

## Next Phase Readiness

**Unblocked and ready:**
- **Plan 02** — extends `schema_validation.rs`'s `expectation` arms and its tests. Its Task 3 no longer
  owns the 18 cfg renames (they landed here); the tripwire-fence and Makefile halves are unaffected.
- **Plan 03** — can add `input-validation` to the toolkit's `default` and handle the two
  `default-features = false` consumers; the feature edge and the opt-out warning are in place.
- **Plans 04–11** — the `schema-validation` / `validation` superset relationship is stable, and the three
  count-asserting gate legs exist for every later binary to be pinned into.

**Concerns to carry forward:**
- Turning `input-validation` on by DEFAULT (plan 03) will activate the decorator for the
  `openapi-code-mode` test surface too. Those fixtures pass declared-only arguments today, but they have
  never been run WITH the decorator active — plan 03 should run `make test-server-toolkit-code-mode`
  with `input-validation` added before assuming they are clean.
- `format` now enforces for inputs (Q1). No config in-tree declares `format` yet (D2 adds the field in
  plan 02/03), so nothing changed behaviourally — but the first config to declare one will start
  refusing, which is the intent and should be stated in the docs deliverable.

## Self-Check: PASSED

All named artifacts exist on disk and all three task commits are present in the git history:
`src/server/schema_validation.rs`, `crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs`,
this SUMMARY; commits `08c7c5b5`, `9032e6ce`, `31cccaa6`. Both new source files are tracked by git
(`git ls-files --error-unmatch` exit 0).

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-27*
