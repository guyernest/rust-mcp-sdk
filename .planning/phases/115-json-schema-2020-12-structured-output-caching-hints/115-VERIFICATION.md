---
phase: 115-json-schema-2020-12-structured-output-caching-hints
verified: 2026-08-02T06:30:00Z
status: human_needed
score: 4/4 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/4
  gaps_closed:
    - "SCHM-01's round-2 BLOCKER (position-blind normalization): DATA_ONLY_KEYWORDS was tested
       against every object key regardless of whether that key sat in KEYWORD position or NAME
       position, so a $defs/properties/patternProperties/definitions/dependentSchemas entry an
       author named const/enum/default/examples was invisible to both first_legacy_dialect and
       pin_dialect_in_place. Closed by 115-14's SUBSCHEMA_MAP_KEYWORDS (5-entry) three-way member
       dispatch in both walkers. INDEPENDENTLY RE-CONFIRMED this session by direct source read
       (src/server/output_validation.rs:160-280) and by re-running the fences fresh:
       output_validation::tests 18/18 pass, including
       v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword (the fence the round-2
       report's reproduction document maps onto)."
    - "The round-2 finding that all three pre-existing defensive layers RESTATED the same defective
       rule (the unit postcondition called the crate's own detector; the property generator
       hard-coded the definition name 'Inner'; fuzz invariant 5's collector re-implemented the same
       filter) is closed by 115-15's rename-invariance metamorphic property/fuzz-invariant, DERIVED
       from the JSON Schema 2020-12 name-semantics fact rather than restated from the crate's
       keyword lists. INDEPENDENTLY RE-CONFIRMED: property_tests 20/20 pass under `--features "full
       fuzzing"` (18 under `--features full`, proving the fuzzing-gated module actually ran),
       including property_normalization_does_not_depend_on_a_subschema_map_key_name; corpus seed
       14_defs_named_default exists, is tracked (216 bytes, commit fb97b23d)."
  gaps_remaining:
    - "A NEW finding, not present in round 2: this round's own code review (115-REVIEW.md CR-01,
       timestamped AFTER the 115-14/115-15 commits it reviews) found SUBSCHEMA_MAP_KEYWORDS omits
       `dependencies` — draft-04..2019-09's own map-from-instance-property-NAME-to-subschema
       keyword, which the module's own test suite already records
       (src/server/output_validation.rs:707-712, D-115-03-C) as still honoured by jsonschema 0.49.2
       under the 2020-12 pin. INDEPENDENTLY RE-MEASURED this session through the crate's OWN
       fuzz_support seam (not the reviewer's isolated byte-for-byte copy): `dependencies.default` is
       NOT rewritten while `dependencies.Inner` IS — confirming the name-dependence CR-01 describes.
       Going one step further than the review: no v2 verdict flip was reproducible either —
       `dependencies.Inner` and `dependencies.default` both report `(Violates, Violates)` for a
       type-violating instance on the pinned jsonschema version, so no accept-everything vacuous
       validator is reachable through this position today. That is categorically different from
       both of the two prior (now-closed) rounds, which each demonstrated an actual
       `(Conforms, Conforms)` flip before being booked closed. Routed to Human Verification, not
       booked as a FAILED truth — see reasoning below and in the Human Verification section."
  regressions: []
human_verification:
  - test: "Decide the disposition of 115-REVIEW.md CR-01 (SUBSCHEMA_MAP_KEYWORDS omits `dependencies`)"
    expected: "Either (a) a further closure plan adds `dependencies` to SUBSCHEMA_MAP_KEYWORDS in
      both walkers plus the two restated mirrors, or (b) the finding is formally booked to
      deferred-items.md with a stated rationale, following the same convention already used for
      D-115-AC (WR-03) and D-115-AD (WR-04/05, IN-01/02/03) — i.e. explicit and owned-or-unowned,
      not silently absorbed."
    why_human: "This is the third time this exact requirement has been reopened over a name-position
      completeness gap in the same deny/allow-list shape, and the phase's own established pattern is
      that every review finding gets triaged (fixed or explicitly booked), never left implicit. This
      verification's own measurement differs from the reviewer's in one respect (no verdict flip
      demonstrated on either name), which is a judgment call about severity that the owner, not an
      automated re-run, should ratify given the requirement's history."
  - test: "Correct or accept contracts/mcp-protocol-sdk-v1.yaml's `output_schema_draft_pin` `formula:`
      equation head (lines 248-252), which still states an unscoped total ('NO string-valued $schema
      anywhere in s ... root or any depth' / 'EVERY such $schema') five lines above the correctly
      scoped `walk:` clause it introduces (lines 253-261) — round 3's own review WR-04, independently
      confirmed present by direct read this session"
    expected: "Either a small doc-only fix (scope the equation head to match the walk clause and the
      already-corrected invariants), or an explicit deferred-items.md entry accepting the
      inconsistency as documentation debt"
    why_human: "Cosmetic/documentation-only — does not affect compiled behavior, since the `walk:`
      clause and the `invariants:` block (which 115-14 did correct) are what a reader and
      `pmat comply check` actually consult for the traversal rule. Not a blocker on its own, but
      compounds the same 'a defensive-layer sentence overstates the code's scope' pattern this whole
      three-round closure exists to eliminate, so it should not be left silently unaddressed a third
      time either."
---

# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints Verification Report

**Phase Goal:** Schema validation moves to an explicitly-pinned Draft 2020-12, v2 `structuredContent`
accepts any JSON value (relaxing the 2.15 object-only bridge), and the list/read results carry
additive caching hints — all wasm-clean and independent enough to parallelize with the HTTP/Tasks
track.

**Verified:** 2026-08-02
**Status:** human_needed
**Re-verification:** Yes — third pass. Round 1 found the root-only normalizer BLOCKER (closed by
115-12/115-13). Round 2 found that closure's own position-blind residual BLOCKER (closed by
115-14/115-15, this round's subject). This round independently re-measures the 115-14/115-15 closure
AND weighs a fresh finding (CR-01) from this round's own code review, which landed AFTER 115-15
committed.

## Goal Achievement

**Round 2's BLOCKER is genuinely closed, independently re-confirmed — not accepted from either
SUMMARY.** `SUBSCHEMA_MAP_KEYWORDS = ["properties", "patternProperties", "$defs", "definitions",
"dependentSchemas"]` (`src/server/output_validation.rs:160-166`) is consulted first in a three-way
member dispatch in both `first_legacy_dialect_in_member` and `pin_dialect_in_member`
(`:209-227`, `:265-280`): a member whose key is in that list AND whose value is an object recurses
into every VALUE without keyword-filtering the map's own keys; the same key with a non-object
(malformed) value falls through to the ordinary walk; otherwise the `DATA_ONLY_KEYWORDS` skip applies
unchanged. This directly closes the round-2 reproduction: the `$defs.default` entry (an author-chosen
name colliding with a `DATA_ONLY_KEYWORDS` word) is now visited and rewritten. Re-run fresh this
session: `cargo test --lib --features full output_validation::tests` → **18 passed / 0 failed**,
including `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`.

**Round 2's second finding — that every fence RESTATED the code's own defective rule and so could not
catch a rule-level defect — is closed by a genuinely different mechanism, also re-confirmed.**
`property_normalization_does_not_depend_on_a_subschema_map_key_name`
(`tests/property_tests.rs`) and fuzz invariant 6 `assert_normalization_is_invariant_under_rename`
assert **rename invariance** — a metamorphic relation derived from the JSON Schema 2020-12 fact that
the keys of the five subschema-map keywords are semantically inert author-chosen names, so
normalizing an entry cannot depend on the name it is filed under. This consults no keyword list at
all and, per the SUMMARY's negative control (independently plausible from the code and cited, not
re-run — running it requires reverting `src/`), fires even when BOTH restated copies of the old rule
are also blind, which the three previous fences structurally could not do. Re-run fresh this session:
`cargo nextest run --features "full fuzzing" -E 'binary(property_tests)'` → **20 passed**, vs
**18 passed** under `--features full` alone — the pair that proves the `fuzzing`-gated module actually
ran.

**SCHM-02, SCHM-03 and the v1 freeze are unmoved and re-confirmed fresh.** `src/types/tools.rs` and
`src/types/caching.rs` are not in either plan's `files_modified`. Re-run this session:
`cargo nextest run --features full -E 'binary(structured_tool_output) + binary(v2_caching_hints) +
binary(v1_lists_golden) + binary(v2_schema_tripwires) + binary(v2_core_schema_facts) +
binary(vendored_schema_provenance) + binary(phase115_contract_bindings)'` → **78 passed / 0 skipped**,
matching every count both SUMMARYs and the round-2 report recorded. `compile_for_era`'s `Era::V1` arm
(`jsonschema::validator_for(schema)` verbatim, `:486`) is unchanged.

**A NEW finding surfaced by this round's own code review (`115-REVIEW.md`, timestamped AFTER the
115-14/115-15 commits it covers) — independently re-measured, and NOT accepted at face value.**
CR-01 states `SUBSCHEMA_MAP_KEYWORDS` omits `dependencies`, draft-04..2019-09's own
map-from-instance-property-NAME-to-subschema keyword. The module's own test suite already records
(`src/server/output_validation.rs:707-712`, ledger `D-115-03-C`) that `jsonschema` 0.49.2 still
honours `dependencies` under the 2020-12 pin. This verification reproduced the name-dependence
directly through the crate's own `fuzz_support` seam — not the reviewer's isolated byte-for-byte copy
— with zero net change to the tree (a throwaway `examples/_verify115_dependencies_repro.rs`, run once,
deleted; `git status --porcelain examples/` confirmed clean afterward):

```
dependencies.Inner    rewritten=true  verdicts=Some((Violates, Violates))
dependencies.default  rewritten=false verdicts=Some((Violates, Violates))
dependencies.const    rewritten=false verdicts=Some((Violates, Violates))
components.Inner    rewritten=true   (an arbitrary vendor container, for contrast — WR-04's older, broader concern)
components.default  rewritten=false
```

The name-dependence is real and confirmed: `dependencies.default`'s legacy `$schema` is left
unrewritten while `dependencies.Inner`'s is rewritten, exactly as CR-01 measured. **But going one step
further than the review, which explicitly stated it could not demonstrate this either: no v2 verdict
flip is reproducible at this position on the pinned jsonschema version.** Both `dependencies.Inner`
(rewritten) and `dependencies.default` (not rewritten) report `(Violates, Violates)` for a
type-violating instance — the `type: integer` constraint is enforced identically whether or not the
declaration was normalized. This is categorically different from the reproductions that reopened
SCHM-01 twice before (round 1: `root-draft07 + embedded` measured `(Violates, Conforms)`; round 2:
`$defs.default` measured `(Conforms, Conforms)` against the control's `(Conforms, Violates)`) — both
of those were demonstrated accept-everything bypasses. This one is not. A plausible mechanism (stated
here as reasoning, not as an established library-internals fact): `jsonschema`'s embedded-resource
(`$id`-boundary) discovery walk for 2020-12 compilation appears scoped to keywords the 2020-12
core/applicator vocabulary actually defines as subschema-bearing, and `dependencies` was removed from
that vocabulary in 2020-12 (replaced by `dependentSchemas`) — so a `$schema` sitting inside a
`dependencies` entry is not treated by the library as establishing a dialect boundary at all, unlike
the five keywords `SUBSCHEMA_MAP_KEYWORDS` already covers. If that reasoning is right, the module's
`# Why the walk is position-aware` claim that rewriting is "deliberately a SUPERSET of what
`jsonschema` honours" (`:384-387`) still holds in the security-relevant sense — the normalizer omits a
position, but that position is not one the library treats as a dialect declaration either — though the
sentence is not literally provable without pinning that library-internal behavior, which is exactly
the caution the review itself raised. Given the demonstrated absence of a bypass, and given the bar
this requirement has been held to across two real reopenings is a **demonstrated** bypass, this
verification does NOT reopen SCHM-01 to FAILED a third time on CR-01 alone. It is routed to Human
Verification instead, because (a) the finding is real and not yet triaged into `deferred-items.md`
the way this phase's own convention requires, and (b) the security reasoning above rests on unpinned
library-internal behavior, which is precisely the condition the module's own rustdoc already warns is
fragile.

A second, narrower documentation-only inconsistency was also independently confirmed this session:
`contracts/mcp-protocol-sdk-v1.yaml`'s `output_schema_draft_pin` `formula:` block still states an
unscoped total ("NO string-valued $schema anywhere in s ... root or any depth", "EVERY such
$schema") at lines 248-252, five lines above the correctly position-scoped `walk:` clause 115-14
introduced at lines 253-261. The `invariants:` list (what 115-14's own must-have named) IS correctly
scoped; the `formula:` equation head was missed. This is this round's review WR-04 (a different
finding from round 2's WR-04, both files reused the ID); it does not affect compiled behavior — the
`walk:` clause and `invariants:` are what a reader or `pmat comply check` would actually consult — but
it is the same "a defensive-layer sentence overstates the code's actual scope" pattern this whole
three-round closure exists to eliminate, so it is listed under Human Verification rather than silently
passed over.

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SCHM-01: Draft 2020-12 explicitly pinned, no `$schema` auto-detect, wasm-clean, SEP-2106-compliant | ✓ VERIFIED | Round-2 BLOCKER (position-blind traversal on the 5 known subschema-map keywords) closed and independently re-confirmed: 18/18 `output_validation::tests`, 20/20 `property_tests` (`--features "full fuzzing"`), fuzz target position-aware (source-read confirmed), corpus seed `14_defs_named_default` tracked (216 bytes). `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` → exit 0, re-run this session. `v2_schema_tripwires` 13/13 (SEP-2106, re-run in the combined-78 run below). **CR-01 residual (`dependencies` omission) independently confirmed as a real name-dependence defect but NOT as a demonstrated bypass** — see Gaps/Human Verification. Not reopened to FAILED on that basis alone |
| 2 | SCHM-02: v2 `structuredContent` accepts any JSON value; v1 keeps object-shaped behavior | ✓ VERIFIED | `binary(structured_tool_output)` 20/20, re-run this session; `src/types/tools.rs` untouched by 115-14/115-15 |
| 3 | SCHM-03: list/read/discover results carry additive `ttlMs`/`cacheScope`, ensured on v2 and stripped on v1 at one chokepoint | ✓ VERIFIED | `v2_caching_hints` 19/19, `v1_lists_golden` 7/7, `v2_schema_tripwires` 13/13, `v2_core_schema_facts` 8/8, `vendored_schema_provenance` 6/6, `phase115_contract_bindings` 5/5 — 78/78 combined, re-run this session; `src/types/caching.rs` untouched by 115-14/115-15 |
| 4 | v1 (`2025-11-25`) wire is behaviourally frozen | ✓ VERIFIED | `v1_lists_golden` byte-identical goldens pass (re-run); `compile_for_era`'s `Era::V1` arm (`jsonschema::validator_for(schema)` verbatim, `:486`) confirmed unchanged by direct source read |

**Score:** 4/4 truths verified. Status is `human_needed`, not `passed`, because of the two items in
Human Verification Required below — neither is a failed truth, but neither is silently clean either.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/output_validation.rs` (`SUBSCHEMA_MAP_KEYWORDS`, both `*_in_member` dispatchers) | Position-aware traversal over the five JSON-Schema-defined subschema-map keywords | ✓ VERIFIED (with a noted, non-blocking completeness gap) | 5-entry list confirmed by direct read (`:160-166`); both walkers' three-way dispatch confirmed (`:209-227`, `:265-280`); `dependencies` (a 6th, legacy-but-still-honoured keyword) is NOT in the list — independently re-measured as name-dependent but not verdict-flipping (see CR-01 above) |
| `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword` (unit test) | Gate-visible regression fence for the colliding-name bypass | ✓ VERIFIED | Present, passes (18/18 in `output_validation::tests`); SUMMARY records it was OBSERVED to fail pre-fix (16 passed / 2 failed) |
| `tests/property_tests.rs` (`arb_definition_name`, `arb_container`, rename-invariance property) | Generator can draw colliding names; independent invariant that a rule defect cannot satisfy | ✓ VERIFIED | `SUBSCHEMA_MAP_KEYWORDS` mirror present (6×), `arb_definition_name`/`arb_container` present (10×), hard-coded `/$defs/Inner/$schema` pointer absent (WR-06 from round 2 discharged) — all confirmed by grep this session; 20/20 pass |
| `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` (invariant 6, position-aware collector) | Independent rename-invariance fence in the fuzz target; both restated copies onto the shipped rule | ✓ VERIFIED | `SUBSCHEMA_MAP_KEYWORDS` present (7×), `assert_normalization_is_invariant_under_rename` present (3×) — confirmed by grep this session; build/replay/campaign results cited from `115-15-SUMMARY.md` (not re-run — nightly toolchain, ~5 min campaign; no reason to distrust the recorded exit-0/empty-artifacts evidence) |
| `fuzz/corpus/fuzz_schema_draft_pin/14_defs_named_default` | Colliding-name seed, committed | ✓ VERIFIED | Present, tracked (`git log` shows commit `fb97b23d`), 216 bytes — matches SUMMARY exactly |
| `contracts/mcp-protocol-sdk-v1.yaml` (`output_schema_draft_pin`) | Invariants describing the shipped normalizer's actual scope | ⚠️ PARTIAL | `invariants:` block (postcondition, invariant 1) correctly scoped to "SCHEMA POSITION" with the five named keywords — confirmed by direct read. The `formula:` equation head (lines 248-252) still states an UNSCOPED total, five lines above the correctly scoped `walk:` clause — a documentation-only inconsistency, independently confirmed present (round-3 review WR-04); routed to Human Verification |
| `contracts/binding.yaml` | `115-14 POSITION CORRECTION` notes, all bindings `status: implemented` | ✓ VERIFIED | 5 occurrences of the correction note (3 corrections + 2 extracted-helper bindings); anchored `status: planned` returns 0 (the one text match is a prose comment, not a YAML field) — confirmed by grep this session |
| `.planning/REQUIREMENTS.md` (SCHM-01 booking) | States evidence covering the colliding-name case, written after the gate ran | ✓ VERIFIED | `[x]`, new block above the `115-13` block (amend-not-delete preserved — `REOPENED` still appears exactly once), traceability row updated to name `115-14`/`115-15` — confirmed by direct read |
| `.planning/ROADMAP.md` | Plan-progress bookkeeping only; phase marker untouched | ✓ VERIFIED | `115-14-PLAN.md`/`115-15-PLAN.md` both `[x]`, `15 plans` count present, Phase 115 marker still `[~]` (correctly left for this verification to score) — confirmed by direct read |
| `.planning/phases/.../deferred-items.md` | Every review finding triaged (fixed or explicitly booked) | ⚠️ PARTIAL | `D-115-AC` through `D-115-AG` present, whole-ID duplicate check clean — all correctly booked for round-2's review. This round's OWN review (`115-REVIEW.md`, CR-01 and the round-3 WR/IN findings) postdates 115-15 and has NOT yet been triaged into this ledger — a process gap, not a code gap; routed to Human Verification |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `first_legacy_dialect_in_member` / `pin_dialect_in_member` | `SUBSCHEMA_MAP_KEYWORDS` | member-key dispatch | WIRED | Both walkers consult the same 5-entry list first; confirmed identical dispatch shape by direct read |
| `tests/property_tests.rs` / `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | the shipped position-aware rule | mirrored `SUBSCHEMA_MAP_KEYWORDS` constants | WIRED | Both restated copies now match the shipped 5-entry list (grep-confirmed); this closes the false-positive window `115-14-SUMMARY.md` named |
| `property_normalization_does_not_depend_on_a_subschema_map_key_name` / fuzz invariant 6 | `normalize_bytes` | rename-then-compare-subtrees | WIRED | Derived from a spec fact, not from either keyword list; confirmed present and passing (property side, this session) |
| `.planning/REQUIREMENTS.md` SCHM-01 | the measured post-fix verdicts | booking written after `make quality-gate`/`pmat quality-gate` ran | WIRED | Task 3 of `115-15` is explicitly gated on the earlier tasks' evidence; confirmed by reading the booking text and cross-checking counts against tests re-run this session |
| `src/server/output_validation.rs:707-712` (D-115-03-C) | `SUBSCHEMA_MAP_KEYWORDS` | "jsonschema still honours `dependencies`" vs. the list's omission of it | ⚠️ INTERNAL INCONSISTENCY (documentation, not a demonstrated bypass) | The module's own test comment records `dependencies` as a live keyword for `jsonschema` 0.49.2, but `SUBSCHEMA_MAP_KEYWORDS` does not include it — see CR-01 discussion above |

### Data-Flow Trace (Level 4)

Not applicable — this phase's artifacts are validation/normalization logic and test/fuzz harnesses,
not UI components rendering fetched state. The equivalent question ("does the normalizer's behavior
actually reach the compiled validator") is answered directly by the `(v1, v2)` verdict measurements
throughout this report and by `compile_2020_12` calling `normalize_schema_dialect` before
`jsonschema::draft202012::new` (confirmed by direct read, `:449-467`).

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Position-aware traversal enforces the round-2 colliding-name case | `cargo test --lib --features full output_validation::tests -- --test-threads=1` | 18 passed / 0 failed | ✓ PASS |
| Rename-invariance property (derived fence) exists and passes | `cargo nextest run --features "full fuzzing" -E 'binary(property_tests)' --test-threads=1` | 20 passed (vs 18 under `--features full`) | ✓ PASS |
| SCHM-02/SCHM-03 unregressed | `cargo nextest run --features full -E 'binary(structured_tool_output) + binary(v2_caching_hints) + binary(v1_lists_golden) + binary(v2_schema_tripwires) + binary(v2_core_schema_facts) + binary(vendored_schema_provenance) + binary(phase115_contract_bindings)' --test-threads=1` | 78 passed / 0 skipped | ✓ PASS |
| wasm-clean | `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"` | exit 0 | ✓ PASS |
| CR-01 name-dependence, independently reproduced | throwaway `examples/_verify115_dependencies_repro.rs` via `fuzz_support::{normalize_bytes, validate_bytes}` (written, run, deleted; `git status --porcelain examples/` clean after) | `dependencies.Inner rewritten=true`, `dependencies.default rewritten=false`, both `verdicts=(Violates, Violates)` — no verdict flip | ✓ PASS (confirms name-dependence; confirms NO demonstrated bypass) |
| Ledger hygiene | `grep -o '^## D-115-[A-Z0-9]\{1,2\}' deferred-items.md \| sort \| uniq -d` | empty | ✓ PASS |
| Debt markers | `grep -n -E "TBD\|FIXME\|XXX"` over the 5 touched files | no matches | ✓ PASS |
| Commits exist | `git log --oneline -1` for `f8692f1d`, `07bfdd52`, `2bf4d637`, `43246c19`, `fb97b23d`, `d666fffa` | all found | ✓ PASS |

### Probe Execution

SKIPPED — no `scripts/*/tests/probe-*.sh` files exist in this repository and neither this round's
plans nor its verification criteria reference probe-based verification. This phase's runnable
evidence is the cargo test/nextest/wasm-build commands above, all independently re-run this session.

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SCHM-01 | 115-01, 03, 08, 09, 10, 11, 12, 13, 14, 15 | Draft 2020-12 pin, no `$schema` auto-detect, wasm-clean, SEP-2106 | ✓ SATISFIED | Round-2 BLOCKER closed and re-confirmed; CR-01 residual confirmed real but not a demonstrated bypass — routed to Human Verification rather than blocking |
| SCHM-02 | 115-01, 03, 04, 09, 10, 11 | v2 non-object `structuredContent`, v1 frozen | ✓ SATISFIED | 20/20 re-run this session; files untouched by 115-14/115-15 |
| SCHM-03 | 115-01, 02, 05, 06, 07, 08, 09, 10, 11 | Additive `ttlMs`/`cacheScope` on six cacheable results | ✓ SATISFIED | 58/58 (of the 78 combined) re-run this session; files untouched by 115-14/115-15 |

No orphaned requirements: `.planning/REQUIREMENTS.md`'s traceability table (lines 640-642) maps only
SCHM-01/02/03 to Phase 115, and all fifteen plans (including the four gap-closure plans) declare
`requirements: [SCHM-01, SCHM-02, SCHM-03]` or a subset. All three IDs are accounted for.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/server/output_validation.rs` | 160-166 | `SUBSCHEMA_MAP_KEYWORDS` omits `dependencies`, which the same file's own test comment (`:707-712`) records as still honoured by `jsonschema` under the 2020-12 pin | ⚠️ Warning (real, independently confirmed name-dependence; no demonstrated verdict flip) | See CR-01 discussion; routed to Human Verification |
| `contracts/mcp-protocol-sdk-v1.yaml` | 248-252 vs 253-261 | The `formula:` equation head states an unscoped total five lines above the correctly position-scoped `walk:` clause it introduces | ⚠️ Warning (documentation-only; `invariants:`, the field 115-14's must-have actually named, is correctly scoped) | Round-3 review WR-04; routed to Human Verification |
| `.planning/phases/.../deferred-items.md` | — | This round's own code review (`CR-01` and the round-3 `WR-01..WR-06`, `IN-01..IN-03`) has not yet been triaged into the ledger — it postdates the plans it reviews | ⚠️ Warning (process, not code) | Breaks this phase's own established "every finding gets fixed or explicitly booked" convention for the first time in three rounds |
| (phase-touched files this round) | — | `TBD`/`FIXME`/`XXX` scan over `src/server/output_validation.rs`, `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, `contracts/mcp-protocol-sdk-v1.yaml`, `contracts/binding.yaml` | ℹ️ Info | Zero matches — no debt-marker gate violation |

### Human Verification Required

Neither item below is a failed truth. Both are real, independently-confirmed findings from this
round's own code review that have not yet been formally triaged (fixed or explicitly deferred), which
breaks this phase's own established convention for the first time across three verification rounds.

### 1. Decide the disposition of CR-01 (`SUBSCHEMA_MAP_KEYWORDS` omits `dependencies`)

**Test:** Review this report's independent reproduction (through the crate's own `fuzz_support` seam,
not an isolated copy) alongside `115-REVIEW.md`'s CR-01, then decide whether to (a) accept a further
closure plan that adds `dependencies` to `SUBSCHEMA_MAP_KEYWORDS` in both walkers and both restated
mirrors, closing the completeness gap outright, or (b) formally book the finding to
`deferred-items.md` — following the exact convention already used for `D-115-AC` (WR-03, also
declined with a stated, reasoned rationale) — recording that no verdict flip was demonstrated on the
pinned `jsonschema` version and that the risk is therefore judged acceptable pending a future
`jsonschema` upgrade or further investigation of its resource-discovery internals.
**Expected:** A recorded decision — either a new gap-closure plan, or a `deferred-items.md` entry with
an owner or an explicit "unowned" marker, matching this phase's own established bar for every other
review finding.
**Why human:** This is the third time this requirement has been reopened over a name-position
completeness gap in the same deny/allow-list shape. This verification's own measurement diverges from
the reviewer's characterization in one material respect (no verdict flip demonstrated on either name,
where both prior reopenings DID demonstrate one) — a severity judgment the owner should ratify given
the requirement's history, not one an automated re-run should silently resolve either way.

### 2. Correct or accept the contract's unscoped equation head

**Test:** Review `contracts/mcp-protocol-sdk-v1.yaml` lines 248-261 — the `formula:` block's equation
head still reads as an unscoped total ("NO string-valued $schema anywhere in s... root or any depth" /
"EVERY such $schema") immediately above the correctly position-scoped `walk:` clause 115-14
introduced.
**Expected:** Either a small doc-only edit bringing the equation head into line with the `walk:`
clause and the already-corrected `invariants:` block, or an explicit acknowledgment that this is
accepted documentation debt.
**Why human:** Does not affect compiled behavior — the `walk:` clause and `invariants:` block are what
a reader or `pmat comply check` actually consult, and both ARE correctly scoped. But it is the exact
"a defensive-layer sentence overstates the code's actual scope" pattern this whole three-round closure
exists to eliminate, so leaving it silently unaddressed a third time is worth a deliberate decision
rather than an accident.

### Gaps Summary

**No BLOCKER this round.** Round 2's position-blind BLOCKER is genuinely closed and independently
re-confirmed (not accepted from either SUMMARY): `SUBSCHEMA_MAP_KEYWORDS`'s three-way member dispatch
in both walkers correctly reaches all five JSON-Schema-defined subschema-map keywords regardless of
the author-chosen name filed under them, and the fences that would have missed a rule-level defect
(round 2's own finding) are now backed by a rename-invariance metamorphic relation derived from the
spec rather than restated from the code. 18/18 unit tests, 20/20 property tests, 78/78 combined
SCHM-02/03 tests and a clean wasm build were all re-run fresh this session and match every number both
SUMMARYs and the prior VERIFICATION.md recorded.

**One new, real, but non-blocking finding surfaced by this round's own code review.** `115-REVIEW.md`
CR-01 — `SUBSCHEMA_MAP_KEYWORDS` omits `dependencies` — was independently re-measured through the
crate's own seam and confirmed as genuine name-dependent normalization behavior. Unlike the two prior
reopenings of this requirement, no v2 verdict flip (accept-everything vacuous validator) is
reproducible at this position: `dependencies.Inner` and `dependencies.default` both enforce `type`
identically on the pinned `jsonschema` version. Given the bar this requirement has been held to across
two real, demonstrated reopenings, this verification does not reopen SCHM-01 to FAILED on CR-01 alone.
It is instead routed to Human Verification, together with a smaller, documentation-only contract
inconsistency (this round's WR-04), because this phase's own established convention — every review
finding gets fixed or explicitly booked to `deferred-items.md` — has not yet been applied to this
round's own review, which landed after the plans it covers.

**SCHM-02 and SCHM-03 remain genuinely achieved**, re-checked (not merely trusted) this session: files
for both are untouched by 115-14/115-15's diff, and their dedicated test suites (78 tests combined)
all re-run clean on this tree.

---

_Verified: 2026-08-02_
_Verifier: Claude (gsd-verifier)_
