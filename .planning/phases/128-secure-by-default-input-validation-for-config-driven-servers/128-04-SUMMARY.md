---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 04
subsystem: api
tags: [garde, typed-tools, input-validation, value-free-refusal, deprecation, feature-flags, tdd, proptest]

# Dependency graph
requires:
  - phase: 128-secure-by-default-input-validation-for-config-driven-servers
    provides: "plan 01's D-04 feature split (`schema-validation`, `validation = [\"schema-validation\", \"dep:garde\"]`) and plan 02's `src/server/schema_validation.rs` — specifically `REDACTED_SEGMENT`, the token this plan reuses so a D1 and an E3 refusal read alike"
provides:
  - "`TypedTool::new_validated` / `TypedTool::new_validated_with_schema` and the `TypedSyncTool` twins — opt-in `garde` field validation with the `garde::Validate<Context = ()>` bound on the CONSTRUCTOR only, so no existing `TypedTool<T>` changes"
  - "A value-free garde refusal renderer: `render_garde_refusal` + `project_garde_path` + `project_garde_segment`, projecting caller-chosen `garde::Path` segments onto `REDACTED_SEGMENT`"
  - "`redacted_deserialize_error` — the validated path's serde refusal, classification-only, closing the value-echoing route that sits IN FRONT of garde (T-128-17a)"
  - "`garde` is no longer a zero-reference dependency, and its `derive` feature is now enabled (it is not a garde default)"
  - "`pmcp::server::validation` is `#[deprecated(since = \"2.21.0\")]` + `#[doc(hidden)]`, with its D4 harvest lineage recorded in the module doc"
  - "`tests/typed_tool_garde.rs` (19 tests incl. the E3 `property_` arm) and `tests/server_validation_deprecated.rs` (the relocated 5)"
  - "`examples/s57_typed_tool_garde_validation.rs` — SC-8's RUNNABLE example leg"
  - "Two corrected `.planning/codebase/ARCHITECTURE.md` Validation bullets that name their enforcers"
affects: [128-05, 128-06, 128-08, 128-09, 128-11]

# Actuals (#2632) — chars/4 over the realized diff, NOT a harness token count.
actuals:
  tokens: 15890
  tasks: 3
  commits: 4
  plan_head_before: e6ebe7400c9638c1f4d87cfcbd679a4d1de5bf78
  # MEASURED: `git rev-list --count e6ebe740..HEAD` == 4 at SUMMARY-write time (the
  # three task commits — one of which is a TDD pair, so 2 commits — plus this docs
  # commit). chars/4: `git diff e6ebe740..HEAD | wc -c` == 63559 -> 15890.
  # Estimate was 45000 / raw 90000; the realized diff is ~2.8x smaller, the same
  # over-estimate ratio plan 01 recorded (14942 against the same scale).

# Tech tracking
tech-stack:
  added: ["garde_derive 0.23.0 (transitive, unlocked by enabling garde's non-default `derive` feature)"]
  patterns:
    - "Bound-on-the-constructor-only: an opt-in capability added to a generic type without touching the type's or the trait impl's `where` clauses, because stable Rust has no specialization to make a bound conditional"
    - "Path PROJECTION over path copying: walk a library's path COMPONENTS (not its `Display`) and emit a segment verbatim only when its shape cannot carry caller-chosen text"
    - "Asymmetric error redaction: fix a value-echoing message only on the path that ADVERTISES value-freedom, keeping the legacy path byte-identical so the change stays additive"
    - "Relocate a deprecated module's own `#[cfg(test)] mod tests` to a root integration test with one file-scoped `#![allow(deprecated)]`, rather than taking a crate-wide suppression"

key-files:
  created:
    - tests/typed_tool_garde.rs
    - tests/server_validation_deprecated.rs
    - examples/s57_typed_tool_garde_validation.rs
  modified:
    - src/server/typed_tool.rs
    - src/server/validation.rs
    - src/server/mod.rs
    - src/server/schema_validation.rs
    - Cargo.toml
    - .planning/codebase/ARCHITECTURE.md

key-decisions:
  - "Shape A (stored validator closure), not a new bound on the type — a `T: garde::Validate` bound on the struct or the `ToolHandler` impl would break every existing `TypedTool<T>` whose `T` does not implement it"
  - "Two validated constructors per type, not one: `new_validated` (gated on `validation` + `schema-generation`, mirrors `new`) and `new_validated_with_schema` (gated on `validation` alone, mirrors `new_with_schema`). The plan named only `new_validated` and gated it on `validation`; auto-schema generation needs `schema-generation`, so satisfying both statements requires the pair"
  - "`garde` gains `features = [\"derive\"]` — garde declares NO default features, so without it `#[derive(garde::Validate)]` does not exist and `new_validated` was a constructor no consumer could satisfy. Deliberately not `features = [\"full\"]`, which would pull regex/url/phonenumber/card-validate/idna into every `validation` build"
  - "`schema_validation::REDACTED_SEGMENT` promoted to `pub(crate)` and imported by the E3 renderer, rather than duplicating the `\"<redacted>\"` literal — a second literal could drift and make a D1 and an E3 refusal distinguishable"
  - "`garde::Path::__iter` (`#[doc(hidden)]`) is used instead of parsing `Path`'s `Display`, because `Display` joins with `.`/`[`/`]` and a caller key containing one is indistinguishable from nesting once rendered. `garde_derive` itself makes the same call; a garde release removing it is a COMPILE error here, not a silent behaviour change"
  - "The 5 unit tests inside the newly-deprecated `validation` module moved to `tests/server_validation_deprecated.rs`. Module- and fn-scoped `#[allow(deprecated)]` were both MEASURED and both failed: libtest's harness references the `#[test]`-generated consts from the crate root, outside any in-module allow"

patterns-established:
  - "Residuals stated, not implied: `render_garde_refusal`'s doc names its two leak surfaces (a `#[garde(custom(..))]` message, and an identifier-shaped caller-chosen map key) rather than claiming unconditional value-freedom"
  - "A runnable example that asserts its own claim: `s57` `assert!`s the rejected value's absence and prints the verdict, so the value-free property regresses visibly in the output rather than only in a test log"

requirements-completed: [E3, SC-5, SC-8]

coverage:
  - id: D1
    description: "A `TypedTool<T>` / `TypedSyncTool<T>` built through a `new_validated*` constructor runs `T`'s garde rules after deserialization and refuses with `Error::Validation` containing no byte of the rejected value"
    requirement: "E3"
    verification:
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_violation_refuses_and_never_echoes_the_value"
        status: pass
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_sync_violation_refuses_and_never_echoes_the_value"
        status: pass
      - kind: integration
        ref: "tests/typed_tool_garde.rs#property_typed_tool_garde_refusal_never_contains_the_input (PROPTEST_CASES=64, --ignored property_)"
        status: pass
    human_judgment: false
  - id: D2
    description: "A tool built through an EXISTING constructor behaves exactly as today — no validation runs even when `T` implements `garde::Validate`, and no bound on `T` changed on either struct or either `ToolHandler` impl"
    requirement: "E3"
    verification:
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_existing_constructor_runs_no_validation"
        status: pass
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_type_without_derive_is_unaffected"
        status: pass
      - kind: other
        ref: "git diff e6ebe740 -- src/server/typed_tool.rs | grep -E '^[+-].*(where|DeserializeOwned)' — six ADDED `where` clauses (the three new fns x two types), zero removed or modified, zero `DeserializeOwned` lines touched"
        status: pass
    human_judgment: false
  - id: D3
    description: "A caller-chosen `garde::Path` segment (a `#[garde(dive)]` map key) is redacted with the same fixed token the D1 path uses, and a key containing a `.` redacts as ONE segment rather than two identifier-shaped halves"
    requirement: "E3"
    verification:
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_map_dive_key_is_redacted_not_echoed"
        status: pass
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_dotted_map_key_is_redacted_as_one_segment"
        status: pass
    human_judgment: false
  - id: D4
    description: "The deserialization route in FRONT of garde is redacted on the validated path (wrong type, unknown enum variant, invalid string) while the unvalidated path's message stays byte-identical to today"
    requirement: "E3"
    verification:
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_validated_deserialize_redacts_a_wrong_type"
        status: pass
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_validated_deserialize_redacts_an_unknown_enum_variant"
        status: pass
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_validated_deserialize_redacts_an_invalid_string"
        status: pass
      - kind: integration
        ref: "tests/typed_tool_garde.rs#typed_tool_garde_unvalidated_deserialize_message_is_byte_identical"
        status: pass
    human_judgment: false
  - id: D5
    description: "`garde` is no longer a zero-reference dependency: `src/server/typed_tool.rs` calls `garde::Validate::validate`"
    requirement: "SC-5"
    verification:
      - kind: other
        ref: "grep -rn garde src/ | grep -c . == 60 (>0); positive control `serde_json` == 2124; cargo tree -p pmcp --features validation -e normal -i garde resolves to garde v0.23.0"
        status: pass
    human_judgment: false
  - id: D6
    description: "`pmcp::server::validation` is `#[deprecated(since = \"2.21.0\")]` + `#[doc(hidden)]`, and `make lint` / `make test-doc` / `make doc-check` all still pass"
    verification:
      - kind: other
        ref: "make lint exit 0 (\"No lint issues\"); make test-doc exit 0 (459 passed); make doc-check exit 0 (\"Zero rustdoc warnings\"); grep -c 'doc(hidden)' src/server/mod.rs == 1"
        status: pass
      - kind: integration
        ref: "tests/server_validation_deprecated.rs (5 relocated tests, all pass)"
        status: pass
    human_judgment: false
  - id: D7
    description: "`.planning/codebase/ARCHITECTURE.md`'s Validation subsection contains no claim that is false after this phase, and names `schema_validation::validate_input` and the garde constructors as the enforcers"
    verification:
      - kind: other
        ref: "grep -v '^#' ARCHITECTURE.md | grep -c 'Server can validate tool inputs before calling handler' == 0; grep -c 'auto-validate via schema' == 0; positive control 'schema_validation::validate_input' == 1"
        status: pass
    human_judgment: false
  - id: D8
    description: "SC-8's example leg is discharged by an example that RUNS, printing both an accepted result and a value-free refusal, and exiting 0"
    requirement: "SC-8"
    verification:
      - kind: manual_procedural
        ref: "cargo run -q -p pmcp --features \"full\" --example s57_typed_tool_garde_validation — exit 0, five labelled sections, both refusals value-free (full output quoted below)"
        status: pass
      - kind: other
        ref: "make test-examples exit 0 (89 examples built across 3 covered trees, 0 failures)"
        status: pass
    human_judgment: false

# Metrics
duration: 44 min
completed: 2026-09-27
status: complete
---

# Phase 128 Plan 04: E3 garde validation on typed tools + D-03 deprecation Summary

**Opt-in `garde` field validation on `TypedTool`/`TypedSyncTool` with a value-free refusal whose path segments are projected, not copied — plus the third dead validation path deprecated and hidden, and the codebase map's two false enforcement claims corrected.**

## Performance

- **Duration:** 44 min
- **Started:** 2026-09-27T05:03:32Z
- **Completed:** 2026-09-27T05:47:47Z
- **Tasks:** 3 (one TDD, so 4 commits)
- **Files modified:** 9 (3 created, 6 modified)

## Accomplishments

- `TypedTool::new_validated` / `new_validated_with_schema` and the `TypedSyncTool` twins run `T`'s garde rules after deserialization. The `garde::Validate<Context = ()>` bound sits on the constructors ONLY — not on either struct, not on either `ToolHandler` impl — so nothing changes for an existing `TypedTool<T>`, including one whose `T` already implements `Validate`.
- The refusal is rebuilt from `Report::iter()` and its `garde::Path` is **projected**, not copied: a segment is emitted verbatim only when it is identifier-shaped or a base-10 index, and every other segment becomes `schema_validation::REDACTED_SEGMENT` — the same `"<redacted>"` token the D1 path uses, now `pub(crate)` and imported rather than re-declared.
- The value-echoing route that sits **in front of** garde is closed on the validated path (T-128-17a): a validated tool's deserialization failure renders `serde_json`'s CLASSIFICATION, never its `Display`. The unvalidated path is byte-identical to today, asserted against `serde_json`'s own error text.
- `garde` is no longer a zero-reference dependency, and its **non-default `derive` feature** is now enabled — without it `#[derive(garde::Validate)]` does not exist, which would have made `new_validated` unsatisfiable.
- `pmcp::server::validation` is `#[deprecated(since = "2.21.0")]` + `#[doc(hidden)]`, its module doc records exactly what the D4 harvest took from `validate_safe_path` and what it did not, and all 11 doctests carry a hidden `# #![allow(deprecated)]`.
- `ARCHITECTURE.md`'s two false Validation bullets now name their enforcers and state plainly that core dispatch does NOT schema-check inputs.
- `examples/s57_typed_tool_garde_validation.rs` discharges SC-8's example leg by RUNNING, and asserts its own value-free claim so a regression shows in the output.

## Task Commits

1. **Task 1 (tdd) — RED** — `c1e80bf7` (`test(128-04)`): `tests/typed_tool_garde.rs` (19 tests) plus the published surface as documented RED placeholders (`deserialize_args` emitting today's message on both paths, `run_garde` accepting everything), and the `garde` `derive` feature fix. **RED evidence: 19 discovered, 11 failed, 1 ignored, exit 101.**
2. **Task 1 (tdd) — GREEN** — `747aa678` (`feat(128-04)`): the real garde call, the path projection, the redacted deserialization error, and the corrected handler comments. **18 passed / 0 failed / 1 ignored.**
3. **Task 2** — `9e809952` (`docs(128-04)`): the deprecation + hide, the module-doc harvest record, the 11 doctest allows, the test relocation, and both `ARCHITECTURE.md` bullets.
4. **Task 3** — `8775da9c` (`feat(128-04)`): `s57` + its `[[example]]` stanza.

**Plan metadata:** this commit (`docs(128-04)`).

No REFACTOR commit: `pmat quality-gate --checks complexity` reported 0 violations on the GREEN body and every new function is under 15 lines, so there was nothing to clean up. Committing an empty refactor would have been theatre.

## Files Created/Modified

- `src/server/typed_tool.rs` — the `validator` field on both types, four new constructors, `with_garde_validator`, `deserialize_args` (cfg-split), `run_garde`, and the five free helpers (`legacy_deserialize_error`, `redacted_deserialize_error`, `render_garde_refusal`, `project_garde_path`, `project_garde_segment`, `is_identifier_shaped`, `is_base10_index`).
- `src/server/schema_validation.rs` — `REDACTED_SEGMENT` promoted to `pub(crate)` with the reason recorded on it. Nothing else touched; plan 02's scoped `output_validation.rs` assertion is untouched.
- `src/server/mod.rs` — `pub mod validation` gains `#[deprecated]` + `#[doc(hidden)]` and an expanded outer doc.
- `src/server/validation.rs` — module doc + 11 doctest allow lines; the `#[cfg(test)] mod tests` removed (relocated, see Deviations). **No function body changed.**
- `Cargo.toml` — `garde` gains `features = ["derive"]` (with the reason and the explicit rejection of `["full"]`); the `s57` `[[example]]` stanza after `s56`.
- `tests/typed_tool_garde.rs` — NEW, 19 tests, `#![cfg(feature = "validation")]`.
- `tests/server_validation_deprecated.rs` — NEW, the 5 relocated tests under one file-scoped allow.
- `examples/s57_typed_tool_garde_validation.rs` — NEW.
- `.planning/codebase/ARCHITECTURE.md` — both Validation bullets rewritten.

## The final `new_validated` signatures (verbatim, as the plan's `<output>` requires)

`TypedTool<T, F>` (`src/server/typed_tool.rs:167` and `:180`):

```rust
    #[cfg(all(feature = "validation", feature = "schema-generation"))]
    pub fn new_validated(name: impl Into<String>, handler: F) -> Self
    where
        T: JsonSchema + garde::Validate<Context = ()>,
    {
        Self::new(name, handler).with_garde_validator()
    }

    #[cfg(feature = "validation")]
    pub fn new_validated_with_schema(name: impl Into<String>, schema: Value, handler: F) -> Self
    where
        T: garde::Validate<Context = ()>,
    {
        Self::new_with_schema(name, schema, handler).with_garde_validator()
    }
```

`TypedSyncTool<T, F>` (`:649` and `:660`) carries the identical pair, differing only in the enclosing type. The stored field on both is

```rust
    #[cfg(feature = "validation")]
    validator: Option<GardeValidator<T>>,
// where
type GardeValidator<T> = Box<dyn Fn(&T) -> std::result::Result<(), garde::Report> + Send + Sync>;
```

## RESEARCH assumption A1 — MEASURED verbatim

A1 assumed: *"`RUSTFLAGS="-D warnings"` does not reach rustdoc's doctest compilation, so the doctests inside `validation.rs` will warn rather than fail under `make test-doc`."*

The prophylactic and the measurement were kept SEPARATE, exactly as the plan requires, because a pass with the allow lines in place would have answered nothing. The 11 hidden `# #![allow(deprecated)]` lines were applied first (Gemini's recommendation), then **temporarily removed** so the leg could be measured against the real question, then restored.

**Measurement with the deprecation applied and the allow lines REMOVED:**

```
$ make test-doc
RUSTFLAGS="-D warnings" cargo test --doc --features "full"
test result: ok. 459 passed; 0 failed; 80 ignored; 0 measured; 0 filtered out; finished in 25.00s
✓ All doctests passed
exit 0
```

Zero `error:` lines and **zero `warning:` lines** in the whole run — the deprecation diagnostic is not merely non-fatal under `cargo test --doc`, it is not surfaced at all. **A1 was correct:** `-D warnings` does not reach rustdoc's doctest compilation on rust 1.98.0. The allow lines were nonetheless restored and shipped, because A1 being true on *this* toolchain is not a guarantee for the next one, and the lines cost nothing.

**Shipped state, allow lines restored:** `make test-doc` exit 0, `459 passed; 0 failed; 80 ignored`.

## The prose sweep for `server::validation` — measured, as predicted

`grep -rn "server::validation"` over `docs/`, `CRATE-README.md`, `README.md` and `pmcp-book/`: **zero hits in all four**, matching the planner's own measurement. Positive controls on the identical scopes (`TypedTool`) returned hits in every one, so the scans were live rather than silently matching nothing.

A whole-workspace sweep over tracked files confirms `128-PATTERNS.md` § Corrections: the only non-planning `server::validation` references are the 11 doctest lines inside `validation.rs` itself. `examples/s19_wasm_typed_tools.rs` and `src/server/wasm_typed_tool.rs` reference the *different inline* `wasm_typed_tool::validation` module, and `s19` is byte-for-byte unmodified (`git diff --stat` empty).

## The `s57` example's printed output (verbatim, as the plan's `<output>` requires)

```
$ cargo run -q -p pmcp --features "full" --example s57_typed_tool_garde_validation
=== 1. The declared schema `new_validated` still generates ===
    properties: {"title":{"description":"Report title. At most 16 characters.","type":"string","minLength":1,"maxLength":16},"rows":{"description":"How many rows to include. 1..=100.","type":"integer","format":"uint32","minimum":1,"maximum":100}}

=== 2. ACCEPTED — a conforming call ===
    handler result: {"title":"Q3 revenue","rows":25,"status":"created"}

=== 3. REFUSED — the title violates its declared maximum ===
    refusal: Validation error: Invalid arguments for tool 'create_report': /title: length is greater than 16
    rejected value was: "Jane Doe DOB 1970-01-01 SSN 123-45-6789"
    -> the rejected value does NOT appear in the refusal above: value-free ✓
    -> the refusal names the declared field and bound instead

=== 4. REFUSED — the row count is out of its declared range ===
    refusal: Validation error: Invalid arguments for tool 'create_report': /rows: greater than 100
    -> "5000" does NOT appear in the refusal above: value-free ✓

=== 5. E3 is OPT-IN — the plain constructor accepts the same payload ===
    handler result: {"title":"Jane Doe DOB 1970-01-01 SSN 123-45-6789","rows":5000}
    -> no rule ran, so no existing `TypedTool` user's behavior changed

Done. A refusal is expected output here, so this process exits 0.
exit 0
```

Incidental finding worth recording: **`schemars` reads the `garde` attributes too.** The generated `inputSchema` in section 1 already carries `minLength`/`maxLength`/`minimum`/`maximum` derived from the `#[garde(...)]` rules, so a single declaration is both published to clients in `tools/list` and enforced server-side. Nobody planned this and no requirement depends on it, but it means an E3 tool's declared rules are discoverable, not just enforced.

## The redaction token, recorded as the plan asks

`"<redacted>"` — `schema_validation::REDACTED_SEGMENT`, promoted from private to `pub(crate)` and **imported** by the E3 renderer rather than duplicated as a second literal. A duplicated literal could drift, and a client able to tell a D1 refusal from an E3 refusal learns which enforcement path rejected it.

The integration test pins the wire text with a deliberate string literal (`const REDACTED_SEGMENT: &str = "<redacted>";` in `tests/typed_tool_garde.rs`), because the const is crate-private and an integration test asserting the literal is what makes a future change to the token a test failure rather than a silent change in what clients see.

## Decisions Made

1. **Two validated constructors per type, not one.** The plan said "add `pub fn new_validated(...)`, gated `#[cfg(feature = "validation")]`" — but auto-generating the schema (what `new` does) needs `schema-generation`. Satisfying both halves requires the pair: `new_validated` gated on `all(validation, schema-generation)` mirroring `new`, and `new_validated_with_schema` gated on `validation` alone mirroring `new_with_schema`. The acceptance criterion ("`new_validated` exists on both types and carries the bound on the constructor only") holds.
2. **`garde::Path::__iter` over parsing `Display`.** `Display` joins components with `.` and `[`/`]`, so a key like `ssn.value` renders indistinguishably from nesting and a lossy split yields two identifier-shaped halves — both of which the projection would then emit verbatim. `__iter` gives exact component boundaries. It is `#[doc(hidden)]`, which is a real fragility, mitigated by the fact that `garde_derive` itself makes the identical call and that its removal is a compile error here rather than a silent behaviour change. `tests/typed_tool_garde.rs#typed_tool_garde_dotted_map_key_is_redacted_as_one_segment` is the regression fence.
3. **Identifier SHAPE, not identifier membership.** The projection cannot enumerate `T`'s declared field names at runtime, so its test is "does this segment have the shape of a Rust identifier". This is the plan's rule taken literally, and it leaves a residual documented below.
4. **The `derive` feature, not `full`.** Enabling garde's `full` would pull `regex`, `url`, `phonenumber`, `card-validate` and `idna` into every `validation` build for rules E3 does not promise. A consumer wanting `#[garde(pattern(..))]` enables the matching garde feature in its own manifest.

## Residual leak surfaces — stated, not implied

`render_garde_refusal`'s doc comment names both, rather than claiming unconditional value-freedom:

1. **`#[garde(custom(..))]` supplies its own message**, copied verbatim. A custom validator must not put the rejected value in it; `pmcp` cannot inspect a closure's text.
2. **A caller-chosen map key that is ITSELF a bare identifier survives the projection.** A `#[garde(dive)]` `HashMap` key of `ssn` is indistinguishable from a field named `ssn`. Keys carrying anything else — a space, a dot, a hyphen, a leading digit — are redacted, which is what the two dive tests assert. Recorded in `.planning/WINDOWS.md` (kind: `deviation`, phase 128) so it is visible at ship time; the escape hatch for a tool whose map keys are themselves sensitive is to validate inside the handler body.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `garde`'s `derive` feature was not enabled, so `#[derive(garde::Validate)]` did not exist**

- **Found during:** Task 1 (first compile of `tests/typed_tool_garde.rs`)
- **Issue:** `garde = { version = "0.23", optional = true }` — and garde declares **no** `default` feature set at all, so `derive = ["dep:garde_derive"]` was off. Measured: `error: cannot find derive macro Validate in this scope` × 4 plus `cannot find attribute garde in this scope`, and then `the trait bound TitleArgs: Validate is not satisfied`. Without this, `new_validated` is a constructor no consumer can satisfy and SC-5 is unreachable — E3 would have shipped as another declared-but-unusable surface, which is the exact defect class this phase exists to close.
- **Fix:** `features = ["derive"]` on the existing dependency, with a comment recording why and why NOT `["full"]`. This is a feature flag on an already-declared dependency, not a new package install, so the package-legitimacy checkpoint does not apply. `Cargo.lock` is gitignored in this repo, so the `garde_derive 0.23.0` addition is not a tracked change.
- **Files modified:** `Cargo.toml`
- **Verification:** `cargo tree -p pmcp --features validation -e normal -i garde` resolves; the test binary compiles and the suite passes.
- **Committed in:** `c1e80bf7` (RED commit)

**2. [Rule 3 - Blocking] `#[deprecated]` on `pub mod validation` broke `make lint` through the module's OWN unit tests — a cost `128-PATTERNS.md` § Corrections did not predict**

- **Found during:** Task 2
- **Issue:** PATTERNS § Corrections was **right** that `examples/s19_wasm_typed_tools.rs` does not break (it never fired). But it cleared `s19` without looking at `src/server/validation.rs`'s own `#[cfg(test)] mod tests`. `#[deprecated]` on a module makes every item inside it deprecated, including the `const` that `#[test]` expands to — and libtest's harness references those consts **from the crate root**. `make lint` failed with 5 × `error: use of deprecated constant server::validation::tests::test_*` under `-D warnings`.
- **Fix, after measuring the alternatives rather than guessing:**
  - `#[allow(deprecated)]` on `mod tests` — **measured, FAILED** (identical 5 errors).
  - `#[allow(deprecated)]` on each `#[test]` fn — **measured, FAILED** (identical 5 errors). Both fail because the use site is outside the module.
  - A crate-wide `#![cfg_attr(test, allow(deprecated))]` — **rejected without measuring**: it would mask genuine deprecation findings across ~2000 unit tests to fix 5.
  - Dropping the deprecation — forbidden by the plan's acceptance criterion.
  - **Chosen:** the 5 tests moved verbatim to `tests/server_validation_deprecated.rs` with one file-scoped `#![allow(deprecated)]`. That file exists precisely to exercise a deprecated API, so the allow is honest and narrow, every other target still treats `deprecated` as an error, and the file is booked for deletion alongside the module at 3.0. Total test count is unaffected (5 lib tests → 5 integration tests) and all 5 pass.
- **Acceptance-criterion note:** this makes the criterion "`git diff src/server/validation.rs` shows doc-comment lines only" false as written — the diff also removes the test module. **No function body changed**, which is the criterion's intent. Flagging it rather than quietly satisfying the letter.
- **Files modified:** `src/server/validation.rs`, `tests/server_validation_deprecated.rs`
- **Verification:** `make lint` exit 0 ("No lint issues"); `cargo test --test server_validation_deprecated` 5 passed.
- **Committed in:** `9e809952`

**3. [Rule 3 - Blocking] `src/server/schema_validation.rs` needed a one-line visibility change, and it is not in the plan's `files_modified`**

- **Found during:** Task 1 (GREEN)
- **Issue:** The plan requires the E3 refusal to use "the same fixed redaction token `schema_validation::safe_pointer` uses". `REDACTED_SEGMENT` was a private const, so the only alternatives were duplicating the `"<redacted>"` literal (which can drift, defeating the requirement) or widening its visibility.
- **Fix:** `const REDACTED_SEGMENT` → `pub(crate) const`, with the reason recorded on the const itself. No other line in that file changed, and plan 02's deliberately-scoped `output_validation.rs` assertion is untouched.
- **Files modified:** `src/server/schema_validation.rs` (6 lines, all the const's doc + its visibility)
- **Verification:** the dive tests assert the shared literal; `make quality-gate` passes.
- **Committed in:** `747aa678`

**4. [Rule 2 - Missing Critical] The RED commit needed two temporary `#[allow(clippy::unnecessary_wraps)]` attributes**

- **Found during:** Task 1 (RED)
- **Issue:** A RED placeholder `run_garde` returning `Ok(())` unconditionally trips `clippy::unnecessary_wraps`, and `make lint` is `-D warnings`, so the RED commit could not have been made without either the allow or a fabricated body.
- **Fix:** the allow, with a doc line saying it is part of the placeholder and goes away with it. Both were removed in the GREEN commit.
- **Files modified:** `src/server/typed_tool.rs`
- **Verification:** `grep -n "unnecessary_wraps" src/server/typed_tool.rs` returns nothing at HEAD.
- **Committed in:** added `c1e80bf7`, removed `747aa678`

---

**Total deviations:** 4 auto-fixed (3 blocking, 1 missing-critical). **Zero Rule 4 escalations.**
**Impact on plan:** all four were necessary to make the plan's own acceptance criteria satisfiable. Deviation 1 is the difference between E3 shipping usable and shipping as another declared-but-unreachable surface. Deviation 2 corrects an incomplete PATTERNS correction and is the only one that changes a plan artifact's shape (the test relocation). No scope creep: nothing was added that the plan did not ask for.

## Issues Encountered

- **The `garde::Path` → safe-text problem has no clean API.** `Path` exposes only `len`/`is_empty`/`join`/`Display` publicly; the component iterator is `#[doc(hidden)]`. Resolved by using the hidden iterator and documenting the choice, its precedent (`garde_derive` does the same), and the fact that its removal is a compile error. The alternative — parsing `Display` — was rejected after working out that it silently splits a dotted caller key into two identifier-shaped halves, which is a leak; `typed_tool_garde_dotted_map_key_is_redacted_as_one_segment` is the fence that keeps that decision from being quietly reversed.
- **`.pmat/` cache churn.** `make quality-gate` rewrites five tracked `.pmat/` files, including a 21 MB → 64 MB binary `context.db`. These are regenerated tool state, not this plan's work, so they were restored with a path-scoped `git checkout -- .pmat/` rather than committed. The tracked tree is clean at SUMMARY-write time.

## Verification Results

| Leg | Result |
|---|---|
| `cargo test --features full --test typed_tool_garde -- --test-threads=1` | 19 discovered, **18 passed, 0 failed, 1 ignored** |
| `PROPTEST_CASES=64 ... --test typed_tool_garde -- --ignored property_` | **1 selected, 1 passed** (nonzero count asserted) |
| `cargo test --features full --lib typed_tool` | 10 passed (nonzero count asserted) |
| `cargo test --features full --test server_validation_deprecated` | 5 passed |
| `cargo test --features full --test docs04_examples_run` | 3 passed (the example staleness guard, nonzero) |
| `cargo tree -p pmcp --features validation -e normal -i garde` | resolves: `garde v0.23.0 └── pmcp v2.20.4` |
| `grep -rn garde src/ \| grep -c .` | **60** (was 0 — SC-5 discharged). Positive control `serde_json`: 2124 |
| `pmat quality-gate --fail-on-violation --checks complexity` | exit 0, **0 violations** |
| `make lint` | exit 0, "✓ No lint issues" |
| `make test-doc` | exit 0, 459 passed (and see the A1 measurement above) |
| `make doc-check` | exit 0, "✓ Zero rustdoc warnings" |
| `make test-examples` | exit 0, **89 examples built across 3 covered trees, 0 failures** |
| `cargo run --example s57_typed_tool_garde_validation --features full` | exit 0, both outcomes printed (output quoted above) |
| **`cargo nextest run --features full --no-fail-fast`** | **3358 tests run: 3358 passed (1 leaky), 5 skipped** — up from the 3340/4 baseline by exactly the 18 new non-ignored tests and 1 new ignored arm |
| **`make quality-gate`** | **exit 0, banner present: `✅ ALL TOYOTA WAY QUALITY CHECKS PASSED`** (15171-line captured run, unpiped to a file per the rtk hazard) |

All measurement commands were invoked by absolute path (`/usr/bin/grep`, `/usr/bin/wc`, `/usr/bin/make`, `/usr/bin/git`) per the rtk output-corruption hazard, and every zero-count scan was paired with a positive control on the identical scope.

## Known Stubs

None. Both RED-phase placeholders (`deserialize_args`, `run_garde` in `c1e80bf7`) were replaced by real bodies in `747aa678`; `grep -n "RED placeholder" src/server/typed_tool.rs` returns nothing at HEAD. The two documented residuals above are stated design boundaries with a named escape hatch, not unwired code — the second is recorded in `.planning/WINDOWS.md` so it stays visible at ship time.

## Threat Flags

None. Every surface this plan adds is covered by a register row (T-128-16, T-128-17, T-128-17a, T-128-17b, T-128-18, T-128-19). No new network endpoint, auth path, file access pattern or schema change at a trust boundary.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- **E3 is complete and opt-in.** Plan 09's `ArgumentValidator` remains the escape hatch for a context-carrying rule (`Context = ()` is an explicit and documented scope boundary of `new_validated`).
- **Plans depending on `REDACTED_SEGMENT`** now have it as `pub(crate)`; a third consumer needs no further change.
- **Deferred and named, so a later phase does not re-derive it:** core dispatch does not schema-check tool inputs. `ARCHITECTURE.md` now records the four unguarded sites (`src/server/mod.rs:2590`, `:2820`, `src/server/core.rs:1085`, `:1270`) and names `core.rs:1014-1023` — the whole-object byte-size cap — as the natural neighbour for the wiring.
- **A release note is owed but NOT taken here:** `#[deprecated(since = "2.21.0")]` names a version the manifest has not reached (`2.20.4`). That is correct as an intent marker, but whoever cuts 2.21.0 should confirm the `since` matches, and the `garde` `derive` feature addition belongs in the CHANGELOG as a (non-breaking, additive) dependency change.

## Self-Check: PASSED

- `tests/typed_tool_garde.rs` — FOUND
- `tests/server_validation_deprecated.rs` — FOUND
- `examples/s57_typed_tool_garde_validation.rs` — FOUND (and `git ls-files --error-unmatch` exit 0)
- `c1e80bf7`, `747aa678`, `9e809952`, `8775da9c` — all four FOUND in `git log`
- `git rev-list --count e6ebe740..HEAD` == 4, matching `actuals.commits`
- Tracked working tree clean before this commit
- Every task's `<acceptance_criteria>` re-run and passing; every plan-level `<verification>` leg run and recorded in the table above

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-27*
