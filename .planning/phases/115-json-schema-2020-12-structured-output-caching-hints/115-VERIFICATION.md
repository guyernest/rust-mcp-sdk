---
phase: 115-json-schema-2020-12-structured-output-caching-hints
verified: 2026-08-02T01:40:00Z
status: gaps_found
score: 3/4 must-haves verified
overrides_applied: 0
re_verification:
  previous_status: gaps_found
  previous_score: 3/4
  gaps_closed:
    - "The SPECIFIC bypass measured in the prior report — a legacy `$schema` on the `$defs.Inner`
       embedded resource (root-draft07 + embedded, `(v1,v2) = (Violates, Conforms)`) — is closed.
       Re-measured on this tree: `(Violates, Violates)`. `normalize_schema_dialect` is now
       recursive, not root-only, and 17/17 unit tests + the widened property test (19 vs 18) +
       fuzz invariant 5 + 2 new corpus seeds all pass."
  gaps_remaining:
    - "SCHM-01 is STILL not achieved. The fix 115-12 shipped is POSITION-BLIND: it applies the
       `DATA_ONLY_KEYWORDS` skip (`const`/`enum`/`default`/`examples`) to every object key
       uniformly, without distinguishing a key in KEYWORD position from a key in NAME position
       (`$defs`/`properties`/`patternProperties`/`definitions`/`dependentSchemas` map
       AUTHOR-CHOSEN NAMES to subschemas). A `$defs` (or `properties`) entry named `default`,
       `const`, `enum` or `examples` is therefore invisible to both `first_legacy_dialect` and
       `pin_dialect_in_place`, so a legacy `$schema` on such an entry survives the v2 pin and
       resolves an empty vocabulary set there — the identical vacuous-validator bypass class
       115-12 was written to close, reachable through a different document shape. Independently
       reproduced this session (see Observable Truths #1)."
    - "All three test/fuzz layers added or repaired by 115-12/115-13 restate the SAME defective
       rule (`DATA_ONLY_KEYWORDS` filtered against every map key) rather than an independent
       invariant, so none of them can detect this residual bypass — confirmed by reading
       `src/server/output_validation.rs`, `tests/property_tests.rs` and
       `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` line-by-line. The property generator and the
       fixed-example fences also still hard-code the definition name `Inner`, so the generated /
       fixed space cannot reach a colliding name either (WR-06, unaddressed — 115-13's scope was
       widening the DIALECT reached, not the NAME reached)."
    - "SCHM-01's `[x]` booking in `.planning/REQUIREMENTS.md` (~line 146) and the traceability row
       at ~line 525 ('Complete — gap closed by 115-12 + 115-13') are NOT justified by the evidence
       on this tree. The booking should be corrected — most defensibly reverted to `[~]` with an
       amended (not deleted) record — pending a further closure round."
  regressions: []
gaps:
  - truth: "SCHM-01: Schema validation runs Draft 2020-12 explicitly pinned, no `$schema` auto-detect (jsonschema 0.49), staying wasm-clean and SEP-2106-compliant"
    status: failed
    reason: >-
      Independently reproduced (not merely accepted from 115-REVIEW.md) via
      `pmcp::server::output_validation::fuzz_support::{validate_bytes, normalize_bytes}` under
      `--features "fuzzing,validation"`, zero net change to the tree. Two documents differing ONLY
      in the NAME of a `$defs` entry (`Inner` vs `default`), both carrying an embedded schema
      resource (`$id` + `$schema: draft-07` + `type: integer`), instance `{"n":"NOT-AN-INTEGER"}`:
      `$defs.Inner` (control) verdicts=(Conforms, Violates), rewritten=true;
      `$defs.default` (renamed) verdicts=(Conforms, Conforms), rewritten=false. The renamed
      document's v2 column silently drops `type: integer` — the exact vacuous-validator bypass the
      pin exists to close. Root cause: `DATA_ONLY_KEYWORDS` (`src/server/output_validation.rs:128`)
      is applied at every object node by both `first_legacy_dialect` (:149-151) and
      `pin_dialect_in_place` (:176-180) without regard to whether the enclosing key is a KEYWORD
      position or a NAME position. Also independently reproduced WR-03 (fragment-suffixed 2020-12
      URI `https://json-schema.org/draft/2020-12/schema#` misclassified as legacy and rewritten
      when it should be left alone) — a secondary, non-blocking correctness gap in the same
      normalizer, unaddressed by 115-12/115-13 (out of their stated scope).
    artifacts:
      - path: "src/server/output_validation.rs"
        issue: "first_legacy_dialect (:141-156) and pin_dialect_in_place (:165-185) filter DATA_ONLY_KEYWORDS against every object key uniformly; a $defs/properties/patternProperties/definitions/dependentSchemas entry whose AUTHOR-CHOSEN NAME collides with const/enum/default/examples is never visited by either walker. The module's own rustdoc (:25-34, :199-222) now asserts an UNCONDITIONAL 'the pin wins UNCONDITIONALLY ... across the whole DOCUMENT' claim that this reproduction falsifies."
      - path: "contracts/mcp-protocol-sdk-v1.yaml"
        issue: "output_schema_draft_pin's invariant 1 ('EVERY such declaration is normalized ... never honoured') and the NEW postcondition invariant added by 115-12 Task 3 ('after normalization no $schema string anywhere in the document ... is anything other than the Draft 2020-12 URI') are both false as shipped for a $defs/properties entry named const/enum/default/examples."
      - path: "tests/property_tests.rs"
        issue: "arb_schema_document() (widened by 115-13) generates the embedded-resource shape only under the hard-coded definition NAME 'Inner' (:982, :988, :1147-1160) — it cannot draw a colliding name, so the widened generator structurally excludes the residual bypass exactly as WR-06 describes."
      - path: "fuzz/fuzz_targets/fuzz_schema_draft_pin.rs"
        issue: "collect_dialect_declarations / strip_dialect_declarations (:305-342), the 'independent' invariant-5 walk added by 115-13, restate the identical DATA_ONLY_KEYWORDS-per-key rule (:224, :312, :335) rather than an independently-derived invariant, so invariant 5 cannot see a declaration behind a colliding $defs/properties name either — the WR-02 finding (an independently-TYPED walk that restates the same RULE catches nothing a rule defect produces) is unaddressed."
      - path: ".planning/REQUIREMENTS.md"
        issue: "SCHM-01 booked [x] (~line 146) and the traceability row (~line 525, 'Complete — gap closed by 115-12 + 115-13') both state the requirement is achieved. The booking's own evidence table (the three-row measurement, the named fences, the negative controls) is real and accurately reported for the CASES IT COVERS, but none of those cases include a colliding definition/property name, so the booking generalizes past what was actually measured — the identical process defect D-115-G was filed to prevent, recurring in a narrower form."
    missing:
      - "Position-aware traversal: distinguish keys in KEYWORD position from keys in NAME position. The review's CR-01 fix sketch (contracts-compatible, already partially present at fuzz_schema_draft_pin.rs:286 in spirit) introduces a SUBSCHEMA_MAP_KEYWORDS list (properties, patternProperties, $defs, definitions, dependentSchemas) whose VALUES are maps of author-chosen-name -> subschema; descend into every value of such a map unconditionally, and never apply DATA_ONLY_KEYWORDS to the map's own keys."
      - "Apply the same rule to BOTH first_legacy_dialect and pin_dialect_in_place, and to the independently-restated copies in tests/property_tests.rs and fuzz/fuzz_targets/fuzz_schema_draft_pin.rs — a rule defect in one restated copy is invisible to a differently-typed but rule-identical walk in another."
      - "A fixed test case (unit `normalization_cases()`, property generator, and a new corpus seed) whose $defs or properties entry is NAMED const/enum/default/examples and carries $id + a legacy $schema + a distinguishing keyword (e.g. type: integer), asserted still ENFORCED on v2 after the fix, and OBSERVED to fail before it (per this phase's own negative-control standard)."
      - "Correct SCHM-01's booking in .planning/REQUIREMENTS.md: either revert [x] to [~] with the downgrade block amended (not deleted, per this phase's established convention) recording this residual defect, or keep [x] only once the position-aware fix lands and is measured."
      - "Optionally (non-blocking on SCHM-01 itself): fix WR-03 (fragment-suffixed 2020-12 URI false-positive) while touching this normalizer again, since it was measured this session as still present."
---

# Phase 115: JSON Schema 2020-12 + Structured Output + Caching Hints Verification Report

**Phase Goal:** Adopt MCP spec `2026-07-28` era semantics for JSON Schema Draft 2020-12 output
validation (SCHM-01), non-object structured tool output (SCHM-02), and result caching hints
`ttlMs`/`cacheScope` (SCHM-03) — with the v1 (`2025-11-25`) wire behaviourally frozen.

**Verified:** 2026-08-02
**Status:** gaps_found
**Re-verification:** Yes — after gap-closure plans 115-12 and 115-13, which executed against the
prior `115-VERIFICATION.md` BLOCKER (root-only `$schema` normalization).

## Goal Achievement

**The prior BLOCKER is genuinely closed for the case it measured.** `normalize_schema_dialect` is
now recursive (not root-only); the specific three-row regression from the prior report
(`root-draft07 + embedded`, definition named `Inner`) now measures `(Violates, Violates)` instead
of `(Violates, Conforms)`, confirmed by re-running `cargo test --lib --features full
output_validation::tests` (17/17 pass) on this tree.

**A new, narrower instance of the SAME bypass class survives, and this verification independently
reproduced it — not merely accepted the framing from `115-REVIEW.md`.** The fix 115-12 shipped
filters `DATA_ONLY_KEYWORDS` (`const`, `enum`, `default`, `examples`) against every object key in
the document uniformly. That filter is correct when the key is in KEYWORD position (e.g. the
schema itself literally has a `"const": ...` keyword) but wrong when the key is in NAME position —
`$defs`, `properties`, `patternProperties`, `definitions` and `dependentSchemas` are all maps from
AUTHOR-CHOSEN NAMES to subschemas, and an author is free to name a `$defs` entry `default`. When
they do, and that entry is an `$id`-bearing embedded schema resource with a legacy `$schema`,
neither `first_legacy_dialect` nor `pin_dialect_in_place` ever visits it, so the legacy declaration
survives the v2 pin and produces the identical accept-everything sub-validator the pin exists to
prevent.

**Reproduction (this session, zero net change to the tree — throwaway `examples/_verify115_repro.rs`
written, run, and deleted; `git status --porcelain examples/` confirmed clean afterward):**

```
$defs.Inner   (control)   verdicts=Some((Conforms, Violates))  rewritten=true
$defs.default (renamed)   verdicts=Some((Conforms, Conforms))  rewritten=false
fragment-suffixed 2020-12 URI: rewritten=true (should be false)
```

This matches the measurement given in this task's `<critical_context>` exactly. Reading the source
confirms the mechanism: `DATA_ONLY_KEYWORDS.contains(&key.as_str())` is evaluated against every
map key at `src/server/output_validation.rs:150` (detector) and `:177` (rewriter) with no
distinction for whether that key sits in a `SUBSCHEMA_MAP_KEYWORDS`-shaped container. No such
container list exists anywhere in the tree (`grep -c SUBSCHEMA_MAP_KEYWORDS` → 0 across
`src/`, `tests/`, `fuzz/`).

**All three defensive layers 115-12/115-13 built or repaired share the identical blind spot,
confirmed by reading each:**

- The unit-test postcondition (`normalize_schema_dialect_changes_only_dollar_schema_keys`,
  `src/server/output_validation.rs:1174`) asserts `first_legacy_dialect(&normalized) == None` —
  the blind detector checking itself.
- `tests/property_tests.rs`'s widened `arb_schema_document()` (:982, :988) hard-codes the
  definition name `"Inner"` — it cannot draw a colliding name, so the 100-of-256 embedded-resource
  coverage 115-13 measured never exercises this shape.
- `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`'s invariant-5 collector
  (`collect_dialect_declarations`, :328-342) restates the same `DATA_ONLY_KEYWORDS`-per-key rule
  (:335) the crate's own detector uses. The module doc calls this scan "TOTAL — no skip condition"
  and "INDEPENDENT" (WR-02); it is independent in *implementation* only, not in *rule*, and a rule
  defect is exactly what it cannot catch.

**SCHM-01's `[x]` booking in `.planning/REQUIREMENTS.md` is NOT justified by the evidence on this
tree.** The booking's evidence table is honest about what it measured (the `Inner`-named case, the
17/19/13/5/… test counts, the negative controls) — but it generalizes past that measurement to
claim the requirement text ("no `$schema` auto-detect") is satisfied, which this session's
independent reproduction disproves. This is the same shape of error `D-115-G` was filed to
prevent — a requirement re-booked complete on evidence that does not cover the case that turns out
to matter — recurring in a narrower form on the very requirement `D-115-G` was about. **The booking
should be corrected**, most defensibly by reverting to `[~]` with the downgrade block amended (not
deleted, following this phase's own established convention) to record this residual defect,
pending a further gap-closure round.

**SCHM-02 and SCHM-03 remain genuinely achieved**, re-checked (not merely trusted) this session:
`output_validation.rs` and `caching.rs` were not both touched by 115-12/115-13 in a way that
affects these — `115-12`/`115-13` touched only `output_validation.rs`, `property_tests.rs`,
`fuzz_schema_draft_pin.rs`, contract/binding/planning docs — and the SCHM-02/03 test suites were
re-run fresh on this tree and pass at their previously-verified counts (see Requirements Coverage).

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | SCHM-01: Draft 2020-12 explicitly pinned, no `$schema` auto-detect, wasm-clean, SEP-2106-compliant | ✗ FAILED | The prior report's SPECIFIC bypass (`$defs.Inner`, root-only walk) is closed — re-measured `(Violates, Violates)`. A NEW instance of the same bypass class survives via a `$defs`/`properties` entry NAMED `const`/`enum`/`default`/`examples` — independently reproduced this session (`$defs.default` verdicts=`(Conforms, Conforms)`, rewritten=`false`) through the crate's own `fuzz_support::{validate_bytes, normalize_bytes}` seam. See Gaps. |
| 2 | SCHM-02: v2 `structuredContent` accepts any JSON value (scalar/array/null/object); v1 keeps object-shaped behavior | ✓ VERIFIED | `cargo nextest run --features full -E 'binary(structured_tool_output)'` → 20/20 pass (re-run this session on the post-115-13 tree); untouched by 115-12/115-13's `files_modified` |
| 3 | SCHM-03: list/read/discover results carry additive `ttlMs`/`cacheScope`, ensured on v2 and stripped on v1 at one chokepoint | ✓ VERIFIED | `binary(v2_caching_hints)` 19/19, `binary(v1_lists_golden)` 7/7, `binary(v2_schema_tripwires)` 13/13, `binary(v2_core_schema_facts)` 8/8, `binary(vendored_schema_provenance)` 6/6, `binary(phase115_contract_bindings)` 5/5 — all re-run this session, 58/58 pass; `src/types/caching.rs` was NOT in either gap-closure plan's `files_modified` |
| 4 | v1 (`2025-11-25`) wire is behaviourally frozen | ✓ VERIFIED | `v1_lists_golden` byte-identical goldens pass (re-run); `compile_for_era`'s `Era::V1` arm (`jsonschema::validator_for(schema)` verbatim) is untouched by 115-12's diff — confirmed by reading `src/server/output_validation.rs:325-333` |

**Score:** 3/4 truths verified (unchanged from the prior report's score — the SCHM-01 defect
mechanism changed, but the truth's pass/fail state did not)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `src/server/output_validation.rs` (`first_legacy_dialect`, `pin_dialect_in_place`) | Recursive, position-aware `$schema` normalization | ✗ **FUNCTIONAL DEFECT (narrower)** | Recursive — yes, closing the prior root-only defect. Position-aware — no: `DATA_ONLY_KEYWORDS` is checked against every map key regardless of whether it names a keyword or an author-chosen subschema-map entry. Confirmed by direct source read (:141-185) and by independent reproduction |
| `v2_pin_still_enforces_an_embedded_legacy_resource` (unit test) | Gate-visible regression fence for the embedded-resource bypass | ✓ VERIFIED (narrow) | Passes; fences exactly the `Inner`-named case with an OBSERVED pre-fix failure (115-12-SUMMARY.md). Does not fence a colliding-name case — no such case exists in `normalization_cases()` or as a standalone test |
| `tests/property_tests.rs` (`arb_schema_document`) | Generated space reaches the `$id`-bearing embedded-resource shape | ⚠️ PARTIAL | Reaches the shape under dialect variation (4 legacy drafts + invented URIs, measured 100/256) but NOT under name variation — the definition name is hard-coded `"Inner"` (WR-06, unaddressed) |
| `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` (invariant 5, `assert_no_legacy_dialect_survives`) | TOTAL, independently-implemented dialect-purity invariant | ⚠️ PARTIAL | Independently *coded*, not independently *derived* — restates the identical `DATA_ONLY_KEYWORDS`-per-key rule (WR-02), so it is blind to the same residual bypass as the code under test |
| `contracts/mcp-protocol-sdk-v1.yaml` (`output_schema_draft_pin`) | Invariants describing the shipped normalizer accurately | ✗ **FALSE POSTCONDITION** | The new postcondition invariant added by 115-12 Task 3 ("after normalization no `$schema` string anywhere in the document ... is anything other than the Draft 2020-12 URI") is unconditionally false as shipped — falsified by the `$defs.default` reproduction |
| `.planning/REQUIREMENTS.md` (SCHM-01 booking) | States evidence that actually supports `[x]` | ✗ **PREMATURE BOOKING** | The evidence recorded is accurate for what it measured, but the booking's conclusion ("Complete") is not supported once a colliding definition name is tried — not measured before booking |
| `src/types/tools.rs` (`CallToolResult::structured_value`) | Additive non-object structured-content constructor | ✓ VERIFIED (unchanged) | 20/20 `structured_tool_output` tests pass; file not touched by 115-12/115-13 |
| `src/types/caching.rs` (`project_caching_hints`, `CacheScope`) | Single projector for `ttlMs`/`cacheScope` | ✓ VERIFIED (unchanged) | `v2_schema_tripwires_caching_hints_are_written_in_exactly_one_place` passes; file not touched by 115-12/115-13 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `compile_2020_12` | `normalize_schema_dialect` | normalize-then-compile | WIRED (mechanically) / **DEFECT (semantically, narrower)** | Correctly threaded; the function computes a position-blind normalization, so a colliding-name embedded resource is compiled unnormalized |
| `first_legacy_dialect` | `pin_dialect_in_place` | shared traversal rule, stated once in rustdoc | WIRED but **RULE-IDENTICAL AND RULE-DEFECTIVE** | Both implement the exact same (wrong) rule, so they agree with each other and both miss the same documents — a detector/rewriter *disagreement* fence cannot catch a *shared rule* defect (this is WR-02's point, confirmed) |
| `tests/property_tests.rs::collect_dialect_declarations` (invariant 5's crate-fuzz-target mirror) | `pmcp::server::output_validation`'s rule | "independent" restatement | **NOT actually independent of the rule** | Line-by-line identical filter logic to the crate's own `DATA_ONLY_KEYWORDS` check; catches a detector/rewriter code disagreement, not a rule defect |
| `request_is_cacheable` / `inject_v2_result_envelope` / `project_caching_hints` | (SCHM-03 chokepoint) | single-projection | WIRED (unchanged) | `v2_schema_tripwires` 13/13 re-run this session, all pass; not touched by 115-12/115-13 |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SCHM-01 | 115-01, 03, 08, 09, 10, 11, 12, 13 | Draft 2020-12 pin, no `$schema` auto-detect, wasm-clean, SEP-2106 | ✗ **BLOCKED (residual defect)** | Prior BLOCKER closed for the `Inner`-named case; a narrower instance of the identical bypass class independently reproduced via a `const`/`enum`/`default`/`examples`-named `$defs`/`properties` entry. `.planning/REQUIREMENTS.md`'s `[x]` booking is premature and should be corrected |
| SCHM-02 | 115-01, 03, 04, 09, 10, 11 | v2 non-object `structuredContent`, v1 frozen | ✓ SATISFIED | 20/20 tests re-run this session; files not touched by the gap closure |
| SCHM-03 | 115-01, 02, 05, 06, 07, 08, 09, 10, 11 | Additive `ttlMs`/`cacheScope` on six cacheable results | ✓ SATISFIED | 58/58 tests across five dedicated binaries re-run this session; files not touched by the gap closure |

No orphaned requirements — `.planning/REQUIREMENTS.md`'s traceability table (line ~525-527) maps
only SCHM-01/02/03 to Phase 115, and all thirteen plans (including the two gap-closure plans)
declare `requirements: [SCHM-01]` or a subset of the three IDs. All three IDs from PLAN frontmatter
are accounted for in REQUIREMENTS.md.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `src/server/output_validation.rs` | 25-34, 199-222 | Rustdoc asserts "the pin wins UNCONDITIONALLY ... across the whole DOCUMENT" — this is false for the colliding-name case | 🛑 Blocker (the exact class of misleading-safety-claim documentation that caused the ORIGINAL gap to ship past review) | See Gaps |
| `contracts/mcp-protocol-sdk-v1.yaml` | 284-292 | The NEW postcondition invariant added by 115-12 Task 3 is stated as an unconditional total over "no `$schema` string anywhere in the document," which this session falsifies | 🛑 Blocker | `pmat comply check` and any reader are checking a claim that is not true |
| `.planning/REQUIREMENTS.md` | ~146, ~525 | SCHM-01 booked `[x]` with a large, honest evidence block whose scope does not cover the case that falsifies the requirement | 🛑 Blocker (booking correction required) | Downstream phases and future readers will trust this booking without re-deriving it |
| `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | various | Both generators restate the identical `DATA_ONLY_KEYWORDS`-per-key rule as the code under test rather than an independently-*derived* invariant (WR-02) | ⚠️ Warning | Structural — future rule defects in this normalizer will continue to slip past all three layers until the rule itself, not just its re-implementation, is independently stated |
| (phase-touched files under this closure) | — | `TBD`/`FIXME`/`XXX` scan | ℹ️ Info | Zero matches across `src/server/output_validation.rs`, `tests/property_tests.rs`, `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`, `contracts/mcp-protocol-sdk-v1.yaml`, `contracts/binding.yaml` — no debt-marker gate violation |

### Human Verification Required

None required to determine phase status — the residual finding is deterministically reproducible
and was independently reproduced in this session (not left uncertain). Recommended for the owner
regardless, since this is the second time a "the pin wins unconditionally" claim over-generalized
past what was measured:

**Decide the closure path for the residual CR-01 instance**

**Test:** Review this report's reproduction (a `$defs`/`properties` entry named `const`/`enum`/
`default`/`examples`, carrying `$id` + a legacy `$schema`) alongside `115-REVIEW.md`'s CR-01, then
decide whether to (a) accept a further closure plan implementing position-aware traversal
(`SUBSCHEMA_MAP_KEYWORDS` vs keyword position), or (b) explicitly override SCHM-01 with a
documented rationale accepting the residual risk (author-declared `outputSchema`, warn-only on
both eras, and a definition NAME colliding with one of four specific keywords is a narrow trigger
surface).
**Expected:** A recorded decision — either a new gap-closure plan or a `VERIFICATION.md` override
entry with `accepted_by` / `accepted_at`.
**Why human:** This is the second round of the same requirement being booked complete on evidence
that did not cover the case that later falsified it (`D-115-G`, recurring). A structural decision
about how much test-fence independence this requirement needs before the next booking is trusted
belongs to the owner, not to an automated re-run.

### Gaps Summary

**One BLOCKER, narrower than before but load-bearing on the same requirement text: SCHM-01 is
still not achieved.** The prior verification's specific finding (root-only `$schema` normalization)
is genuinely fixed by `115-12` — recursion now reaches every depth, the `Inner`-named embedded
resource case is enforced on v2, and 78+ tests re-run clean on this tree. But the fix shipped is
position-blind: it cannot distinguish a key that names a JSON-Schema KEYWORD from a key that names
an author-chosen SUBSCHEMA (inside `$defs`, `properties`, `patternProperties`, `definitions`, or
`dependentSchemas`). An author who names a `$defs` entry `default` — a plausible, unremarkable
choice — gets a `$schema` declaration on that entry silently ignored by the detector and rewriter
alike, reproducing the exact vacuous-validator bypass the pin exists to close.

This was independently reproduced this session through the crate's own `fuzz_support` seam with
zero net change to the tree, and confirmed at the source level: `DATA_ONLY_KEYWORDS` is checked
against every object key uniformly at both `first_legacy_dialect` and `pin_dialect_in_place`, with
no `SUBSCHEMA_MAP_KEYWORDS`-style position distinction anywhere in the tree. All three
defensive layers 115-12/115-13 built or repaired — the unit-test postcondition, the widened
property generator, and the fuzz target's "independent" invariant 5 — share the identical blind
spot, because all three restate the same rule rather than deriving the invariant independently.
This is precisely the lesson `115-REVIEW.md`'s WR-02 stated in advance and that this closure round
did not act on (WR-02 was a WARNING, not the CRITICAL, and the gap-closure plans' scope was
explicitly CR-01 only).

`.planning/REQUIREMENTS.md`'s SCHM-01 booking (`[x]`, "Complete — gap closed by 115-12 + 115-13")
is not justified by the evidence on this tree and should be corrected in the next round — this is
the same shape of premature-booking defect `D-115-G` was filed to prevent, recurring narrowly on
the requirement `D-115-G` was originally about.

SCHM-02 and SCHM-03 remain genuinely achieved and were re-checked (not merely trusted) this
session: neither `src/types/tools.rs` nor `src/types/caching.rs` was touched by the gap-closure
plans, and their dedicated test suites (58 + 20 = 78 tests) all re-run clean on this tree.

A further closure plan should implement position-aware traversal (the `SUBSCHEMA_MAP_KEYWORDS`
distinction sketched in `115-REVIEW.md` CR-01 and already informally present at
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:286` for `$ref`/`$defs` reference-keyword handling),
apply it in `first_legacy_dialect`, `pin_dialect_in_place`, and BOTH restated test/fuzz copies, add
a fixed case with a colliding definition/property name observed to fail before the fix, and only
then re-book SCHM-01.

---

_Verified: 2026-08-02_
_Verifier: Claude (gsd-verifier)_
