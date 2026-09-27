---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 05
subsystem: api
tags: [code-mode, http-executor, path-placeholder, breaking-change, resolved-path, redaction, tdd]

# Dependency graph
requires:
  - phase: 128-01
    provides: "the `schema-validation` feature split that `pmcp-code-mode`'s `pmcp` dep now names"
  - phase: 128-02
    provides: "`validate_path_placeholder`, `validate_resolved_path`, `PlaceholderRules` (+`Default` and the three `with_*` builders), `PlaceholderRefusal` (+`Display`/`Error`), `PLACEHOLDER_MAX_LENGTH`"
provides:
  - "`pmcp_code_mode::ResolvedPath<'a>` — the D-09 migration marker AND a real proof carrier: `from_checked` is the ONLY constructor and it runs `validate_resolved_path`"
  - "`HttpExecutor::execute_request(method, ResolvedPath<'_>, body)` — the breaking contract change; resolution happens in `PlanExecutor` before dispatch"
  - "`HttpExecutor::placeholder_rules(method, path_template, param)` — the default-implemented D4(b) seam plan 08 overrides"
  - "`pmcp_code_mode::{validate_path_placeholder, validate_resolved_path, PlaceholderRules, PlaceholderRefusal, PLACEHOLDER_MAX_LENGTH}` — a re-export of the ONE core implementation (Q2)"
  - "THE FIRST REAL CALLER of `validate_resolved_path` — T-128-07a moves from a mechanism in the tree to a mitigation in effect"
  - "`crates/pmcp-server-toolkit/tests/http_executor.rs` — the three CR-01 Code Mode probes plus the FORK-2 template-literal probe, a `.`+`.` composition probe and a compliant control"
affects: [128-06, 128-08, 128-11]

# Actuals (#2632) — chars/4 over the realized diff, NOT a harness token count.
actuals:
  tokens: 21529
  tasks: 3
  commits: 6
  plan_head_before: e7aa6136b7d8edfcdecba6d7d5ce512c51cfa8a4
  # MEASURED, not narrated. `git rev-list --count e7aa6136..HEAD` == 6 at
  # close-out, composed as: 4 code commits (a939a674, 1cd4d69d, 81c39e0b,
  # 4d7856ea) + b0a17ad4 (this SUMMARY, deferred-items.md, STATE.md, ROADMAP.md
  # and WINDOWS.md in ONE commit — unlike plan 02, the SDK folded the state files
  # into the SUMMARY commit rather than writing a separate one) + the HEAD commit,
  # which is the one that corrected this very number after re-measuring. HEAD is
  # deliberately named by role and not by hash: this figure lives inside it, so
  # writing its hash here would change that hash. 6 is what a later
  # `/gsd-verify-work` re-measure will see.
  # tokens: `git diff e7aa6136..HEAD -- crates/ | wc -c` == 86114, /4 == 21528.5.
  # The plan estimated 70000; the actual is ~31% of it. Not rounded toward the
  # estimate — the gap is real and is recorded so later projections calibrate.

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "A newtype whose ONLY constructor is the check, so the invariant its rustdoc states is established rather than asserted"
    - "Two-pass substitution — render+check everything, then apply — so a refusal aborts with no half-substituted path in existence"
    - "Containment tested against the ORIGINAL template, so a brace-carrying value cannot manufacture a placeholder for a later key"
    - "Test modules named FOR the verify filter, so `executor::layer_one` resolves instead of selecting zero"
    - "Mutation testing as the evidence for a load-bearing security call: remove the call, name the rows that go red"
    - "A catch-all wiremock mock behind a zero-request assertion, so `received_requests() == 0` can only mean the request was never made"

key-files:
  created: []
  modified:
    - crates/pmcp-code-mode/Cargo.toml
    - crates/pmcp-code-mode/src/lib.rs
    - crates/pmcp-code-mode/src/executor.rs
    - crates/pmcp-code-mode/src/code_executor.rs
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/tests/http_executor.rs
    - crates/pmcp-server-toolkit/tests/http_connector_props.rs
    - crates/pmcp-openapi-server/tests/contoso_m365_code_mode.rs
    - crates/pmcp-openapi-server/tests/fixtures/contoso-m365.toml
    - crates/pmcp-openapi-server/examples/contoso_m365_min.rs
    - crates/pmcp-openapi-server/examples/contoso-m365.toml

key-decisions:
  - "`ResolvedPath::from_checked` is the ONLY constructor — no `new`, no `new_unchecked`. The plan offered a rename or a private-ctor-plus-checked pair; the stronger third option was taken: delete the unchecked route entirely. A third party cannot forge an unvalidated `ResolvedPath`, so Codex's finding (an unchecked public constructor whose rustdoc claims an invariant) is answered by removing the route rather than by documenting it away."
  - "`HttpCodeExecutor::resolve_path` DELETED; `scalar_str` RETAINED. After step (1) was removed no caller remained for `resolve_path`, and a caller-less function in a security-relevant file is a future reader's trap. `scalar_str` is still reached from step (4) — the remaining-body-as-query-params step — so its WR-03 rule and its Pitfall-5 `/// … names the KEY only` doc stay live."
  - "`validate_resolved_path` has exactly ONE textual call site and is reached from BOTH `ApiCall` arms through `ResolvedPath::from_checked`. That is stronger than the two duplicated call sites the plan's `grep -c >= 2` gate was a proxy for: a single checked constructor cannot be bypassed, whereas two copies can drift."
  - "The layer-1 floor uses `PlaceholderRules::default()` and deliberately NOT `placeholder_rules`. A layer-1 part can appear anywhere in the template, including mid-segment, so there is no OpenAPI `Parameter` to narrow from — asking `placeholder_rules` there would be asking a question with no well-defined answer."
  - "A `PathPart::Expression` refusal names a fixed positional descriptor (`path expression #N`) and never the expression body or the evaluated value. A `PathPart::Variable` refusal names its identifier, which is operator-shipped script text."
  - "The 13-site consumer migration off literal path query strings was taken rather than narrowing the composed check. The narrowing argument is sound and is recorded verbatim below so it can be overturned cheaply, but taking it unilaterally would remove a defence-in-depth layer in the phase whose identity is closing documented-but-absent gaps."

patterns-established:
  - "Pattern: name a test MODULE for the verify filter that must resolve. `executor::layer_one` / `::composition` / `::error_does_not_echo_path` all resolve because the tests are siblings of `tests`, not children of it — three zero-selections avoided without editing a single filter."
  - "Pattern: prove a security call is load-bearing by removing it and naming the rows that go red. Five rows, enumerated below. A claim that a check 'is the mechanism' is otherwise unfalsifiable."
  - "Pattern: when a migration moves a value from one place to another, assert it ARRIVED. Two `query_param(\"$select\", \"values\")` matchers, because `path()` ignores the query string and the migration could otherwise have silently dropped the projection while the test went green."

requirements-completed: [D4, SC-4, SC-7]

# Metrics
duration: ~110min
completed: 2026-09-27
status: complete
---

# Phase 128 Plan 05: D-09 — resolve and check before dispatch Summary

**Every `HttpExecutor` implementor now receives a `ResolvedPath` whose only constructor ran
`validate_resolved_path`, both `${var}` and `{key}` contributions are floored where they are
produced, the composed path is checked in both `ApiCall` arms before dispatch, and all three
path-echoing refusal messages are gone — which is what turns `validate_resolved_path` from a
mechanism in the tree into T-128-07a's mitigation in effect.**

## The checkpoint decision, verbatim

Task 1 was a `checkpoint:decision` (gate: blocking) reached and answered before this executor ran.
The operator's response, recorded verbatim as the task's acceptance criteria require:

```
breaking-newtype
```

That is review note E option (i) and the plan's own recommendation: change
`pmcp_code_mode::HttpExecutor::execute_request`'s path parameter from `&str` to a `ResolvedPath`
newtype, and move `{key}` placeholder resolution ahead of dispatch. The operator was shown and
accepted the measured cost (11 in-tree edits, `pmcp-code-mode` 0.5.4 -> 0.6.0, three pins to move
at publish time).

**Scope of that decision.** It authorizes D-09 only. The separately-recorded
`publish-as-specified` decision belongs to plan 01's D-01/D-04 checkpoint and was correctly
refused as authority here by the prior agent.

## Performance

- **Duration:** ~110 min
- **Tasks:** 3 of 3 (Task 1 was the resolved checkpoint)
- **Files modified:** 11 (0 created), 1298 insertions / 107 deletions
- **Commits:** 4 code + 2 metadata

## T-128-07a is now LIVE — named by file and line, with the row that fails if it goes

The carried obligation for this plan was that `validate_resolved_path` existed and had **zero
callers outside its defining module**, so the top review finding of the phase was a mechanism in
the tree and not a mitigation in effect.

**The call site:**

| File | Line | Code |
|---|---|---|
| `crates/pmcp-code-mode/src/executor.rs` | **2476** | `crate::validate_resolved_path(path)?;` |

It sits inside `ResolvedPath::from_checked`, the type's **only** constructor, reached from both
`ApiCall` arms:

| File | Line | Reaching call |
|---|---|---|
| `crates/pmcp-code-mode/src/executor.rs` | **3114** | `PlanStep::ApiCall` arm — `ResolvedPath::from_checked(&resolved_path)` |
| `crates/pmcp-code-mode/src/executor.rs` | **3305** | `PlanStep::ParallelApiCalls` arm — same |

It runs on the COMPOSED path, after layer-1 `${var}` interpolation AND layer-2 `{key}`
resolution, and before `execute_request` is called.

**Tree-wide, before vs after** (`grep -rn --include="*.rs" "validate_resolved_path" src crates
cargo-pmcp examples tests`, `--include` QUOTED — an unquoted one does not glob under zsh and the
command never runs):

| | `schema_validation.rs` | `pmcp-code-mode/src/executor.rs` | `pmcp-code-mode/src/lib.rs` |
|---|---|---|---|
| before plan 05 | 17 | **0** | 0 |
| after plan 05 | 17 | **5** (1 call + 4 rustdoc) | 2 (re-export + doc) |

Positive control for that scan: `grep -c "validate_input" src/server/schema_validation.rs` -> 18.
A zero from the scan would have meant a broken scan, not an empty tree.

### The rows that fail if the call is removed — MEASURED, not asserted

`crate::validate_resolved_path(path)?;` was temporarily deleted from `from_checked` and the
suites re-run. **Exactly five rows go red:**

| Row | Binary |
|---|---|
| `http_executor_refuses_length_composed_from_two_placeholders` | `pmcp-server-toolkit` / `tests/http_executor.rs` |
| `executor::composition::composition_refuses_two_adjacent_values_over_the_cap_that_each_pass_alone` | `pmcp-code-mode` lib |
| `executor::composition::composition_refuses_traversal_assembled_from_unchecked_literal_parts` | `pmcp-code-mode` lib |
| `executor::composition::composition_refuses_a_mixed_layer_one_and_layer_two_over_cap_segment` | `pmcp-code-mode` lib |
| `executor::layer_two::layer_two_refuses_an_unsubstituted_placeholder_before_dispatch` | `pmcp-code-mode` lib |

(`256 passed` -> `252 passed; 4 failed`; `11 passed` -> `10 passed; 1 failed`.) The file was then
restored from a byte-exact copy — `git status` showed `executor.rs` unmodified afterwards, and
both suites returned to 256/256 and 11/11.

**Two rows notably do NOT fail**, and that is the honest reading the plan predicted:
`http_executor_refuses_traversal_composed_from_two_placeholders` and
`composition_refuses_two_adjacent_single_dot_placeholders` both still pass, because plan 02's
per-value single-dot rule already refuses `"."` on its own. They are refused **twice over**. That
is precisely why `composition_refuses_traversal_assembled_from_unchecked_literal_parts` exists:
it assembles `/a/../b` from three `PathPart::Literal` parts, which are deliberately NOT floored,
so **no per-value check runs at all** and only the composed check can be doing the refusing. That
is the isolating row.

## The chosen `ResolvedPath` constructor shape, and why

The plan asked for a choice and for it to be recorded: "`new_unchecked`, or a private constructor
plus a `from_checked` that runs `validate_resolved_path` itself". **A third option was taken —
strictly stronger than either.**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPath<'a>(&'a str);          // private field

impl<'a> ResolvedPath<'a> {
    pub fn from_checked(path: &'a str) -> Result<Self, PlaceholderRefusal>;  // the ONLY ctor
    #[must_use] pub fn as_str(&self) -> &'a str;
}
impl std::fmt::Display for ResolvedPath<'_> { /* writes the path */ }
```

There is **no `new` and no `new_unchecked`.** The field is private, so no downstream crate can
construct the type by any route other than the check. Consequences:

- Codex's finding — an unchecked public constructor whose rustdoc claims an invariant, i.e. the
  documented-but-absent class this phase exists to correct — is answered by **deleting the route**
  rather than by softening the doc.
- The type is a genuine proof carrier for the COMPOSED invariant, not only a migration marker.
- Its rustdoc still states honestly what it does **not** prove: that a *spec-declared* narrowing
  (an OpenAPI `pattern`/`maxLength`) was applied. That depends on the implementor's
  `placeholder_rules` override, and a default-returning implementor gets the floor and the cap but
  no narrowing. The doc says exactly that, in a `# What the value does NOT guarantee` section.
- It remains a migration marker as well: a stale implementor written against `&str` gets a
  **compile error**, which is the loudness half of D-09.

## `HttpCodeExecutor::resolve_path` — DELETED. `scalar_str` — RETAINED.

`resolve_path` had exactly one caller, step (1) of `execute_request`. With step (1) gone no caller
remained, and a caller-less resolver sitting in a security-relevant file is a trap for the next
reader — someone will wire it back. It was deleted.

`scalar_str` was kept. It is still reached from **step (4)** (`crates/pmcp-server-toolkit/src/code_mode.rs:1000`),
the remaining-body-as-query-params step, so its WR-03 rule and its
`/// Per Pitfall 5 the message names the KEY only — never the value.` doc stay live. Its rustdoc
gained a `# Scope after Phase 128 D-09` section recording that the `{path}` half moved up to
`PlanExecutor` (which applies the identical rule as `render_path_scalar` and then additionally
floors the rendered value) and that `resolve_path` was deleted rather than orphaned.

Steps (2)-(5) keep their original numbering and comments verbatim. The slot where step (1) was
now carries a comment naming Phase 128 D-09 and stating the reason: an implementor that resolved
its own path was blind by construction to what it was about to send.

## The final negative-grep expression for `NoopHttpExecutor`'s message

The plan's literal gate was
`grep -v "^\s*//" crates/pmcp-code-mode/src/code_executor.rs | grep -c "{} {}\", method, path"`,
which prints **0**. But that pattern only matches one spelling and the message uses inline
format captures, so a stronger expression was also run and is the one to carry forward:

```bash
/usr/bin/grep -v "^[[:space:]]*//" crates/pmcp-code-mode/src/code_executor.rs \
  | /usr/bin/grep -cE '\{path\}|\{\}", *method, *path'
```

**Result: 0.** It catches both the inline-capture spelling (`{path}`, which is what the file
actually used before this plan) and the positional one. Note `^[[:space:]]*//` rather than
`^\s*//`: BSD grep on this machine treats `\s` as a literal `s`, so the plan's form strips only
non-indented comment lines. Both forms print 0, so the conclusion does not depend on which grep
ran.

The structural fact behind the grep: `NoopHttpExecutor::execute_request`'s path parameter is now
named `_path` and is never formatted. Asserted by
`code_executor::tests::noop_http_executor_error_names_the_method_and_not_the_path`.

## Before/after test counts

| Selection | Before | After |
|---|---|---|
| `cargo test -p pmcp-code-mode --features js-runtime --lib` | 236 | **256** |
| `… --lib executor::` (with `--features js-runtime`) | 81 | **101** |
| `… --lib executor::layer_one` | — | **5** |
| `… --lib executor::layer_two` | — | **7** |
| `… --lib executor::composition` | — | **4** |
| `… --lib executor::error_does_not_echo_path` | — | **3** |
| `make test-code-mode` (total) | 285 | **305** |
| `cargo test -p pmcp-server-toolkit --features openapi-code-mode --test http_executor` | 5 | **11** |
| `… --features openapi-code-mode --test http_connector_props` | 5 | **5** |
| `… --features http --test http_connector_props` | 4 | **4** |
| `make test-server-toolkit-code-mode` (total) | 359 | **365** |
| `make test-server-toolkit` (total) | 340 | **340** (unchanged, as required) |
| `cargo test -p pmcp-openapi-server --no-fail-fast` | 42 | **42** (40+2-failing mid-plan; see Deviation 1) |
| `cargo nextest run --features "full" --no-fail-fast` (root) | 3358 | **3358**, 0 failed, 5 skipped |

`256 - 236 = 20` new lib tests = 19 in `executor.rs` (5 layer-1 + 7 layer-2 + 4 composition + 3
error-echo) + 1 in `code_executor.rs`. `11 - 5 = 6` new integration probes, which satisfies the
plan's "at least four higher" requirement. The root suite is unchanged because these tests live
in workspace member crates, not the root `pmcp` package.

## Hand-off items for plan 11 (the release commit)

### H1 — the `pmcp` feature requirement move (the plan's own hand-off item)

`crates/pmcp-code-mode/Cargo.toml` **line 42** (the plan says `:29`; a 13-line explanatory comment
block was added above it, so the entry moved) now reads:

```toml
pmcp = { version = ">=2.2.0", path = "../../", default-features = false, features = [
    "logging",
    "schema-validation",
] }
```

The `schema-validation` feature was added; **the version requirement was deliberately NOT moved.**
The in-tree `path` dep already carries the feature so the workspace builds, while raising the
requirement before `pmcp` is actually published at 2.21.0 would break the path-dependency version
check. **At PUBLISH time `^2.2.0` could resolve a published `pmcp` that has no `schema-validation`
feature.** Plan 11 must move it to `>=2.21.0` in the same commit that sets `pmcp`'s version to
2.21.0. This is NOT in D-13's measured list — D-13's "the `pmcp` minor needs no other downstream
repin" is correct for version compatibility and does not account for a NEW feature name being
required.

The consequence Fable flagged is accepted deliberately and belongs in the release note:
**`pmcp-code-mode` now always compiles `jsonschema`**, because the forward is unconditional.
Making it conditional would put the placeholder floor behind a non-default feature, which the
"never silently disabled by a feature flag" prohibition forbids.

### H2 — the version bump and the three pins (NOT performed here, per D-14)

Plan 11 owns every version and pin move and D-14 requires them in ONE commit. Measured current
state:

| File | Line | Current | Plan 11 must set |
|---|---|---|---|
| `crates/pmcp-code-mode/Cargo.toml` | 3 | `version = "0.5.4"` | `0.6.0` (breaking: `HttpExecutor` + `MockHttpExecutor` are shipped API) |
| `Cargo.toml` (root dev-dep) | 271 | `pmcp-code-mode = { version = "0.5.3", … }` | `0.6` |
| `crates/pmcp-server-toolkit/Cargo.toml` | 24 | `pmcp-code-mode = { version = "0.5.3", … }` | `0.6` |
| `crates/pmcp-code-mode-derive/Cargo.toml` | 27 | `pmcp-code-mode = { version = "0.5.0", … }` | `0.6` |

All three pins are semver-INCOMPATIBLE with 0.6 on a pre-1.0 line, so per CLAUDE.md's *Version
Bump Rules* the whole set moves together or not at all. The caret exception does not apply — it
covers PATCH bumps only.

### H3 — a REQUIRED rollout note that the plan did not anticipate (see Deviation 1)

**Breaking behaviour change for Code Mode script authors:** a script writing a literal query
string into its `api.get` path — `api.get("/x?$select=values")` — is now REFUSED. The correct
shape is `api.get("/x", { "$select": "values" })`. Plan 11's CHANGELOG and rollout note must carry
this alongside plan 02's trailing-slash note.

## Deviations from Plan

### 1. [Rule 3 - Blocking, SCOPE EXPANDED] `validate_resolved_path` refuses literal path query strings, breaking 13 in-tree Code Mode scripts

- **Found during:** the plan-level `make quality-gate`, and by NO gate in this plan's own verify
  list — whose scope was `pmcp-code-mode` plus `pmcp-server-toolkit`, while the third consumer of
  the contract is `pmcp-openapi-server`.
- **Issue:** plan 02's `validate_resolved_path` refuses a query separator ANYWHERE in a composed
  path (plan 02 Deviation 7, justified by T-128-07b). It cannot tell an author-written `?` from an
  injected one — that indistinguishability is the whole point of a composed check. Once this plan
  wired it before dispatch, every Code Mode script writing `?$select=values` into its `api.get`
  path started being refused with `param 'path segment' must not contain a parent-directory
  sequence, a query or fragment marker, …`.
- **Measured blast radius:** `cargo test -p pmcp-openapi-server --no-fail-fast` went **40 passed /
  2 failed** — `contoso_m365_code_mode_headline_query_returns_deterministic_set` and
  `contoso_m365_parity_through_real_binary_path`. **The default fail-fast run reported only ONE of
  the two**, so a non-`--no-fail-fast` count is a lower bound; this is the in-repo
  "`cargo test` aborts after the first failing target" lesson firing again.
  Site count: **13** `api.get` call sites across 4 files — `tests/contoso_m365_code_mode.rs` (2),
  `tests/fixtures/contoso-m365.toml` (5), `examples/contoso_m365_min.rs` (1),
  `examples/contoso-m365.toml` (5). Both double-quoted and backtick template-literal spellings.
  My FIRST scan missed all of them because it was scoped to `crates/pmcp-code-mode` and
  `crates/pmcp-server-toolkit`; the second scan missed the backtick spelling. Both were caught only
  because the scan was re-run with a **positive control** (109 `api.get(` lines total, of which 16
  contain `?`, of which 3 are this plan's own new probe payloads).
- **Fix:** the query moves from the path to a BODY param —
  `api.get("…/range(address='A2:D7')", { "$select": "values" })`. For a GET the executor already
  serializes remaining body fields as query params, so the request still reaches Graph with the
  projection (percent-encoded on the wire as `%24select`, which Graph documents). Two
  `query_param("$select", "values")` wiremock matchers were ADDED, because `path()` ignores the
  query string — without them the migration could have silently DROPPED the projection and the
  test would still have gone green. The prose lines describing the upstream
  `GET …/range(address='A2:D2')?$select=values` HTTP request are correct as written and untouched;
  only `api.get` call sites changed.
- **Verification:** `cargo test -p pmcp-openapi-server --no-fail-fast` -> **42 passed, 0 failed**;
  `make test-openapi-server` exit 0; `make quality-gate` exit 0 with the banner.
- **Committed in:** `4d7856ea`
- **REJECTED ALTERNATIVE, recorded so it can be overturned cheaply.** Narrow the composed check to
  the portion before the first `?`. The argument FOR it is genuinely sound: after both per-value
  floors (each of which refuses `?` in literal AND percent-encoded form, decode-once), a `?`
  surviving into the composed string can ONLY have come from a `PathPart::Literal` — script-authored
  text, not caller data — so the composed `?` rule guards a route that is already closed. It was
  NOT taken, for three reasons: (a) it removes a defence-in-depth layer in the phase whose whole
  identity is closing documented-but-absent gaps, and the argument depends on the per-value floor
  staying complete for all time; (b) it contradicts this plan's own `must_haves` truth that the
  composed path is checked by `validate_resolved_path`; (c) a query string in the `path` argument
  of `api.get` is a script smell when the API already has a first-class way to express query
  params. **This is the one judgment in this plan an operator may reasonably want to overturn**, and
  reverting it is one commit (`4d7856ea`) plus a split in `from_checked`'s caller.
- **Scope note, stated plainly:** `crates/pmcp-openapi-server` is NOT in this plan's
  `files_modified`. The failures are DIRECTLY caused by this plan's change, so they are in scope
  for fixing under Rule 3 rather than deferrable under the SCOPE BOUNDARY rule — those files are
  consumers of the exact contract this plan changed, not unrelated files.

### 2. [Rule 1 - Bug in a plan gate] The plan's `:364` verify command reaches 3 of 101 tests while passing its own `<fails_when>`

- **Found during:** Task 2, running the verify list literally.
- **Issue:** `RUSTFLAGS="" cargo test -p pmcp-code-mode --lib executor:: -- --test-threads=1`
  (plan `:364`) omits `--features js-runtime`. `crates/pmcp-code-mode/Cargo.toml` is `default = []`
  and `src/lib.rs` gates `pub mod executor` on `js-runtime`, so the `executor` module is not
  compiled at all. **Measured: `running 3 tests` … `85 filtered out`.** Its `<fails_when>` is
  "running 0 tests", so the gate PASSES while measuring 3 of the 101 tests it names — a
  semi-false-green of exactly the Pitfall-2 class this phase exists to close. The plan's own note
  at `:367` states the feature is required for the sibling command, so `:364` is an internal
  inconsistency rather than a considered choice.
- **Fix:** the command was run BOTH ways and both results are recorded. The corrected form —
  `--features js-runtime` — selects **101**. Later plans should use the corrected form.
- **The count was never lowered to match reality.** The real path was found.

### 3. [Plan-additive, gate-hazard avoided] Three of the plan's verify filters were at risk of selecting ZERO, and were made to resolve by NAMING THE TEST MODULES for them

- **Found during:** Task 2 test design.
- **Issue:** `executor::error_does_not_echo_path`, `executor::layer_one` and
  `executor::composition` name what looks like a function. libtest matches the FULL test path, so
  a test placed in `executor.rs`'s existing `#[cfg(test)] mod tests` would be
  `executor::tests::layer_one_…` and **none of the three filters would match** — three
  zero-selections, each exiting 0.
- **Fix:** rather than correct three filters, the new tests were placed in modules named FOR them
  — `mod layer_one`, `mod layer_two`, `mod composition`, `mod error_does_not_echo_path`, all
  **siblings** of `tests` at the `executor` module level, with a `mod d09_support` carrying the
  shared fixtures. Every filter in the plan now resolves AS WRITTEN. Measured:
  `executor::` 101, `executor::layer_one` 5, `executor::layer_two` 7, `executor::composition` 4,
  `executor::error_does_not_echo_path` 3. The `d09_support` module carries a comment explaining
  why it sits where it does, so a later tidy-up does not fold the modules into `tests` and
  silently re-break all three gates.

### 4. [Rule 1 - Bug, behaviour preserved] The `http_connector_props` prop test's premise was invalidated and was RETARGETED, not weakened

- **Found during:** Task 2 (required for the workspace to compile).
- **Issue:** `code_mode_render_scalar_rejects_object_path_value` asserted `HttpCodeExecutor`
  rejects an OBJECT `{path}` value. After D-09 that executor no longer resolves `{path}` at all,
  and `ResolvedPath::from_checked("/things/{key}")` is itself refused (residual brace) — so the
  test could not express its old premise.
- **Fix:** retargeted to the GET-QUERY leg, which is where `scalar_str` is still reached:
  `execute_request("GET", ResolvedPath::from_checked("/things")?, Some(json!({key: object_value()})))`.
  Renamed `code_mode_render_scalar_rejects_object_query_value`. **Both assertions kept intact** —
  the error still must name the key and must contain no `{`, `[` or `"`. The `{path}` half of the
  WR-03 rule is now asserted by
  `executor::layer_two::layer_two_refuses_an_object_valued_placeholder_naming_the_key_only`, and
  the module header records the split so a reader is not left wondering where the coverage went.

### 5. [Rule 1 - Bug] `http_executor_get_substitutes_path_param_and_returns_json` no longer described what it tested

- **Issue:** the test name and its `expect` message both claimed path-param substitution, which
  this executor no longer performs.
- **Fix:** renamed `http_executor_get_sends_the_resolved_path_verbatim_and_returns_json`; the
  fixture changed from a template plus a body key to a literal `/users/7`; the module header's
  first bullet rewritten. **Assertions unchanged** (`result["id"] == 7`, `result["name"] == "Ada"`)
  — adjusted, not weakened.

### 6. [Rule 3 - Blocking] `join_url(&self.base_url, &resolved_path)` became a needless borrow

- **Found during:** Task 3 (`cargo clippy -p pmcp-server-toolkit --all-targets`).
- **Issue:** with step (1) gone, `resolved_path` is a `&str`, so `&resolved_path` is a `&&str`
  immediately dereferenced — `clippy::needless_borrow`.
- **Fix:** `join_url(&self.base_url, resolved_path)`.
- **Committed in:** `81c39e0b`

### 7. [Plan-additive] Two tests beyond the plan's behaviour list

- `composition_refuses_traversal_assembled_from_unchecked_literal_parts` — the ISOLATING
  composition row. The plan's `.`+`.` row is refused twice over after plan 02's single-dot floor,
  so it cannot prove the composed check is the mechanism. This row assembles `/a/../b` from three
  `PathPart::Literal` parts, which are deliberately not floored, so no per-value check runs at all.
  It is one of the five rows the mutation test showed going red.
- `layer_one_accepts_a_literal_only_template_including_slashes` — the ACCEPT control for the
  layer-1 floor. Without it, "literals are not floored" is unfalsifiable and the floor could be
  "fixed" into refusing every legitimate multi-segment template.

### 8. [Deviation from task ORDER, not from content] The five test call-site updates landed in Task 2, not Task 3

The plan puts them in Task 3 step (2), but Task 2 changes the trait signature, so the workspace
cannot compile at Task 2's commit without them. Every commit in this repo must build. The content
is unchanged; only which commit carries it moved.

---

**Total deviations:** 8 — 1 blocking regression with expanded scope and a rejected alternative,
2 plan-gate bugs (one a semi-false-green, one a zero-selection hazard avoided by construction),
3 test retargetings that preserve or strengthen their assertions, 1 blocking clippy fix, 1
task-ordering shift, plus 2 additive tests.
**Impact on plan:** no scope creep in the plan's own crates. Deviation 1 is the only one that
expands the file set, and it is a direct consequence of the plan's own mandated change; it is
recorded with its measurement, its rejected alternative and a plan-11 rollout obligation rather
than absorbed.

## The grep gates — what they measure, honestly

The plan's `grep -c` gates count textual occurrences as a proxy for call sites, and the plan's own
instruction to "move layer-2 resolution and the composed check into free helper functions" makes
that proxy weak. Both readings are given so the claim is not a grep artifact.

| Gate | Plan's requirement | Measured | Structural reality |
|---|---|---|---|
| `grep -c validate_resolved_path` in `executor.rs` | `>= 2` | **5** | **1** call site (`:2476`), reached from **2** arms via `from_checked` (`:3114`, `:3305`). Stronger than two copies: a single checked constructor cannot be bypassed. |
| `grep -c validate_path_placeholder` in `executor.rs` | `>= 4` | **9** | **2** production call sites — `floor_layer_one_contribution` (`:2892`), reached from both layer-1 arms; `resolve_layer_two_placeholders` (`:2963`), reached from both `ApiCall` arms. Plus 4 in tests. 4 logical places, 2 textual sites, by the plan's own helper-extraction instruction. |
| path-echo wrap in `executor.rs` (comments stripped) | `== 0` | **0** | Both wraps now read `format!("{method} api call '{result_var}' failed: {e}")` / `'{temp_var}'`. |
| `NoopHttpExecutor` path format (comments stripped) | `== 0` | **0** | Parameter renamed `_path`; never formatted. |
| SATD in the four touched `pmcp-code-mode`/toolkit source files | `== 0` | **0** | — |

## Verification results

| Command | Result |
|---|---|
| `RUSTFLAGS="" cargo build -p pmcp-code-mode --features js-runtime` | exit 0, no `error[` |
| `RUSTFLAGS="" cargo build --workspace` | exit 0 |
| `… --lib` (pmcp-code-mode, js-runtime) | **256 passed**, 0 failed |
| `… --lib executor::` (js-runtime) | **101 passed** (3 without the feature — Deviation 2) |
| `… --lib executor::layer_one` / `layer_two` / `composition` / `error_does_not_echo_path` | **5 / 7 / 4 / 3**, all nonzero |
| `RUSTFLAGS="" make test-code-mode` | exit 0, **305 tests**, `executor::`-scoped leg 101 |
| `… --features openapi-code-mode --test http_executor` | **11 passed** (was 5; ≥ 4 higher ✓) |
| `… --features openapi-code-mode --test http_connector_props` | **5 passed** |
| `… --features http --test http_connector_props` | **4 passed** (nonzero ✓) |
| `RUSTFLAGS="" make test-server-toolkit-code-mode` | exit 0, **365 tests**, `http_executor` leg 11 |
| `RUSTFLAGS="" make test-server-toolkit` | exit 0, **340 tests** (unchanged; all three required binaries nonzero) |
| `RUSTFLAGS="" make test-openapi-server` | exit 0, **42 tests** |
| `RUSTFLAGS="" cargo doc -p pmcp-code-mode --no-deps` | exit 0, **0 warnings** |
| `… --no-deps --features js-runtime` | exit 0, 5 warnings — all PRE-EXISTING, see `deferred-items.md` D1 |
| `pmat quality-gate --fail-on-violation --checks complexity` | **PASSED, 0 violations** |
| `cargo clippy -p pmcp-code-mode --features js-runtime --all-targets -- -D warnings` | exit 0 |
| `cargo clippy -p pmcp-server-toolkit --features openapi-code-mode --all-targets` | 0 findings in any file this plan touched |
| `RUSTFLAGS="" cargo nextest run --features "full" --no-fail-fast` | **3358 run, 3358 passed**, 5 skipped, exit 0 |
| `RUSTFLAGS="" make quality-gate` | **exit 0 — `ALL TOYOTA WAY QUALITY CHECKS PASSED` banner present** (15066 captured lines) |
| Mutation test: `validate_resolved_path` removed | **5 rows red**, enumerated above; file restored byte-exact |
| Tracked working tree after `.pmat/` cleanup | **clean** |

## Files Created/Modified

- `crates/pmcp-code-mode/src/executor.rs` — 5040 -> **5929 lines**. New public: `ResolvedPath`
  (+`from_checked`, `as_str`, `Display`), `HttpExecutor::placeholder_rules`. Changed public:
  `HttpExecutor::execute_request`'s path parameter. New private: `refusal_to_execution_error`,
  `floor_layer_one_contribution`, `render_path_scalar`, `resolve_layer_two_placeholders`.
  `resolve_path` gained the layer-1 floor and a rustdoc stating why literals are exempt.
  `ApiCallLog::path` gained the T-128-23 disclosure note. Four new `#[cfg(test)]` sibling modules
  plus `d09_support`.
- `crates/pmcp-code-mode/src/lib.rs` — the five-symbol re-export with the Q2 doc, plus
  `ResolvedPath` added to the `executor::` re-export list.
- `crates/pmcp-code-mode/Cargo.toml` — `features = ["logging", "schema-validation"]`, version
  requirement UNCHANGED, with a comment block recording both halves and the plan-11 hand-off.
- `crates/pmcp-code-mode/src/code_executor.rs` — `NoopHttpExecutor` signature + the path dropped
  from its message + a `js-runtime`-gated regression test.
- `crates/pmcp-server-toolkit/src/code_mode.rs` — step (1) removed, `resolve_path` deleted,
  `scalar_str` retained and re-scoped in its docs, `join_url` borrow fixed.
- `crates/pmcp-server-toolkit/tests/http_executor.rs` — 165 -> **386 lines**; 5 -> 11 tests.
- `crates/pmcp-server-toolkit/tests/http_connector_props.rs` — prop test retargeted to the
  GET-query leg.
- `crates/pmcp-openapi-server/{tests/contoso_m365_code_mode.rs, tests/fixtures/contoso-m365.toml,
  examples/contoso_m365_min.rs, examples/contoso-m365.toml}` — Deviation 1's 13-site migration
  plus two `query_param` matchers.

## Task Commits

1. **Task 2 (tdd) — RED** — `a939a674` (`test`): the `ResolvedPath` surface, the trait change, the
   four implementor migrations, the five test call-site updates, and 20 new tests. Every security
   behaviour deliberately ABSENT (`from_checked` an accept-everything documented placeholder,
   layer 1 unfloored, layer 2 still in the toolkit, all three wraps still echoing the path).
   **RED evidence: 256 discovered, 19 failed, exit 101** — all 19 the new target tests failing on
   assertions about the planned behaviour, with `layer_one_accepts_a_literal_only_template…`
   passing as the accept control.
2. **Task 2 (tdd) — GREEN** — `1cd4d69d` (`feat`): the real `from_checked`, the layer-1 floor, the
   layer-2 move, the composed check in both arms, all three message fixes. **GREEN: 256/256.**
3. **Task 3** — `81c39e0b` (`feat`): step (1) removed, `resolve_path` deleted, six probes added.
4. **Deviation 1** — `4d7856ea` (`fix`): the 13-site consumer migration.

**Plan metadata:** `b0a17ad4` (`docs(128-05)`), which carries this SUMMARY, `deferred-items.md`,
STATE.md, ROADMAP.md and WINDOWS.md in ONE commit — the SDK folded them together here rather
than writing a separate state commit as it did in plan 02.

No separate REFACTOR commit: both behaviour-preserving cleanups (the clippy needless-borrow and
the helper extraction that kept both arms under cog 25) were made BEFORE their commits, so there
was no post-GREEN change to isolate.

## TDD Gate Compliance

`workflow.tdd_mode` is `false` in `.planning/config.json`, so the machine RED gate is advisory
here (same as plans 01 and 02). The cycle was still run for Task 2 and its evidence is recorded.

| Gate | Commit | Evidence |
|---|---|---|
| RED (Task 2) | `a939a674` (`test(128-05)`) | 256 discovered, 19 failed, exit 101. Named failures included all 4 `composition::*` rows, all 4 `layer_one` refusal rows, all 7 `layer_two` rows, all 3 `error_does_not_echo_path` rows, and `code_executor::tests::noop_http_executor_error_names_the_method_and_not_the_path`. |
| GREEN (Task 2) | `1cd4d69d` (`feat(128-05)`) | 256/256; clippy `-D warnings` exit 0; pmat 0 violations; `make test-code-mode` 305. |
| REFACTOR | — (not needed) | See the note above. |

**Task 3 has no meaningful RED, stated rather than fabricated.** Its six probes are integration
probes over behaviour Task 2's GREEN already implemented, so they passed on first run (11/11) and
a RED against the post-GREEN tree is not available. The informative evidence for them is the
MUTATION TEST recorded above — removing `validate_resolved_path` turns
`http_executor_refuses_length_composed_from_two_placeholders` red — which is a stronger statement
than a RED commit would have been, because it names the specific mechanism each probe depends on.

**`gsd_run check tdd-red-evidence` was not invoked.** Plans 01 and 02 measured that its TAP /
node-test parsers cannot read cargo libtest's `test result: FAILED. 237 passed; 19 failed;` format
and return `INVALID_RED (zero_tests_discovered)` on a real RED run. Fabricating TAP output to
satisfy it would be the false-evidence class this repo tracks. The captured run output is the
evidence instead. This remains a GSD-runtime gap for every Rust project.

## Known Stubs

**None.** The single RED-phase placeholder (`ResolvedPath::from_checked` accepting everything, in
`a939a674`) was replaced with the real check in the very next commit, `1cd4d69d`.
`grep -cE "TODO|FIXME|HACK|XXX"` is **0** for all four touched `pmcp-code-mode` / toolkit source
files, and no test is `#[ignore]`d.

## Behaviour Changes

Two, both user-visible, both needing a plan-11 rollout note:

1. **A literal query string in a Code Mode script's `api.get` path is now REFUSED.**
   `api.get("/x?$select=values")` -> `param 'path segment' must not contain … a query or fragment
   marker …`. The supported shape is `api.get("/x", { "$select": "values" })`; for a GET the
   executor serializes remaining body fields as query params, so the request still carries the
   projection. 13 in-tree sites migrated (Deviation 1). **This is the larger of the two and the
   plan did not anticipate it.**
2. **`HttpExecutor::execute_request`'s signature changed** (the accepted D-09 break). Breaking for
   every downstream implementor and every downstream `MockHttpExecutor` expectation constructed
   against a `{key}` template. A stale implementor gets a compile error, by design.

Plus plan 02's already-recorded trailing-slash change, which is now LIVE on the Code Mode surface
because `validate_resolved_path` has a caller: a composed path ending in `/` is refused. No
in-tree Code Mode path template ends in `/` — verified while scanning for Deviation 1.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: breaking_public_api | `crates/pmcp-code-mode/src/executor.rs` | `HttpExecutor::execute_request`'s path parameter changed `&str` -> `ResolvedPath<'_>`, and a second trait method was added. Both are shipped `pmcp-code-mode` API. There is no `cargo public-api` gate in this repo, so nothing mechanical will catch a later over-wide `pub` on `ResolvedPath` or a silent re-widening of the constructor. |
| threat_flag: enforcement_depends_on_a_caller | `crates/pmcp-server-toolkit/src/http/client.rs` | This plan closes the Code Mode surface. The CURATED single-call surface is still open: plan 06 must call `validate_path_placeholder` per value and `validate_resolved_path` at the tail of `substitute_path`. SC-4's "closed on BOTH surfaces" is not yet true. |
| threat_flag: accepted_residual | `crates/pmcp-code-mode/src/executor.rs` (`ApiCallLog::path`) | T-128-23. The internal execution log still records the resolved path and body. Mitigated only by a doc comment warning that a caller-facing surface exposing `api_calls` inherits the disclosure and must redact. Nothing mechanical enforces that. |
| threat_flag: attacker_influenced_name_in_a_refusal | `crates/pmcp-code-mode/src/executor.rs` | A layer-2 refusal names the script-chosen `{key}`, and a layer-1 `Variable` refusal names the script-chosen identifier. Retained deliberately (a nameless refusal is unactionable for the script author, and a Code Mode script is operator-shipped content), with the VALUE always redacted. The plan's Deferred ledger bounds this residual and states it becomes LIVE if a later phase ever makes scripts caller-supplied. |

## Next Phase Readiness

**Unblocked and ready:**

- **Plan 06** (curated `substitute_path`) — the pattern to copy is here: per-value
  `validate_path_placeholder`, then `validate_resolved_path` on the composed result, then dispatch.
  `PlaceholderRefusal` implements `std::error::Error`, so it wraps into `HttpConnectorError` with
  `#[from]`. **Read Deviation 1 first:** a curated `[[tools]]` HTTP tool whose `path` carries a
  literal `?` will be refused the same way, and the same migration applies.
- **Plan 08** — the seam exists and is called from both arms:
  `HttpExecutor::placeholder_rules(&self, method: &str, path_template: &str, param: &str) ->
  PlaceholderRules<'_>`, defaulting to `PlaceholderRules::default()`. `method` is present, so
  `OpenApiSchema::operator_for(path, METHOD)` can disambiguate `GET` from `DELETE` on one path.
  `path_template` is the path as it reaches layer 2 — after layer-1 `${var}` interpolation and
  before `{key}` substitution — which for a string-literal path is byte-identical to the OpenAPI
  template. Build rules through the `with_*` builders (`#[non_exhaustive]` forbids a literal).
- **Plan 11** — three hand-off items above: H1 (the `schema-validation` feature requirement move,
  `Cargo.toml:42`), H2 (0.5.4 -> 0.6.0 plus three pins, ONE commit per D-14), H3 (the breaking
  rollout note for script authors).

**Concerns to carry forward:**

- **Deviation 1's rejected alternative is the one judgment here an operator may want to overturn.**
  It is reversible in one commit. The reasoning is recorded in full so the decision can be re-made
  on the argument rather than re-derived.
- **`make quality-gate` is the only gate that found Deviation 1.** This plan's own verify list was
  scoped to two crates and the third consumer lives in a fourth. A later plan touching a shared
  contract should enumerate consumers by a POSITIVE-CONTROLLED tree-wide scan, not by the plan's
  `files_modified` list.
- **The plan's `grep -c` gates are weaker than the invariants they stand for.** See the table
  above. A later plan tightening them should count CALL SITES (comments stripped) and follow
  helper indirection, not count textual occurrences.
- **`pmcp-code-mode` now always compiles `jsonschema`.** Accepted deliberately (H1).

## Self-Check: PASSED

- All 11 modified files exist on disk; this SUMMARY and `deferred-items.md` exist.
- All four task commits are present in `git log`: `a939a674`, `1cd4d69d`, `81c39e0b`, `4d7856ea`
  (`git rev-list --count e7aa6136..HEAD` == 4 at SUMMARY-write time and 6 at close-out).
- Every `<acceptance_criteria>` row in Tasks 2 and 3 was re-run and passes; every plan-level
  `<verification>` command was re-run and its result is tabulated above.
- Task 1's acceptance criteria are met: the operator's confirmation is recorded verbatim above.
- Tracked working tree clean (`.pmat/` runtime churn restored with `git checkout -- .pmat/`, never
  committed).

---
*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Completed: 2026-09-27*
