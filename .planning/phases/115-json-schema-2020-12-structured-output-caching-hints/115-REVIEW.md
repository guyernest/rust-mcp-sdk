---
phase: 115-json-schema-2020-12-structured-output-caching-hints
reviewed: 2026-08-02T04:55:48Z
depth: standard
scope: gap-closure round 2 (plans 115-14 and 115-15) — NOT full-phase coverage
diff_base: fc674e40
supersedes_note: >-
  This file replaces the gap-closure round-1 review (plans 115-12 and 115-13) at
  the same path. That pass is preserved in git history at 695a7123; its findings
  (CR-01, WR-01..WR-07, IN-01..IN-03) are not re-litigated here. Where a round-1
  finding is only PARTIALLY closed by round 2, the residue is booked below as a
  NEW finding against the round-2 change, with the round-1 ID named for
  traceability. Round 1 in turn replaced the 2026-08-01 full-phase review
  (preserved at c478e75a).
files_reviewed: 7
files_reviewed_list:
  - src/server/output_validation.rs
  - tests/property_tests.rs
  - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
  - fuzz/corpus/fuzz_schema_draft_pin/14_defs_named_default
  - fuzz/corpus/fuzz_schema_draft_pin/README.md
  - contracts/mcp-protocol-sdk-v1.yaml
  - contracts/binding.yaml
findings:
  critical: 1
  warning: 6
  info: 3
  total: 10
status: issues_found
---

# Phase 115: Code Review Report (GAP-CLOSURE ROUND 2)

**Reviewed:** 2026-08-02T04:55:48Z
**Depth:** standard
**Files Reviewed:** 7
**Status:** issues_found

## Scope

**This is a GAP-CLOSURE review of plans 115-14 and 115-15 only.** It is not
full-phase coverage. Phase 115 received a 38-file full-phase review on
2026-08-01 (preserved at `c478e75a`) and a 6-file gap-closure review of plans
115-12/115-13 on 2026-08-02 (preserved at `695a7123`). This pass covers only the
seven files changed between `fc674e40` and `b94ff70f`. A later reader must not
read this document as coverage of the other ~31 files in the phase.

Findings from the two superseded passes are not repeated. Round-1 `WR-03` (the
fragment-suffixed `2020-12#` URI misclassified as legacy) and round-1 `WR-04`
(`DATA_ONLY_KEYWORDS` omits OpenAPI's singular `example`) were verified to still
be present on this tree and remain open under their round-1 IDs.

## Summary

The 115-14 position fix is real and it works on the shape it was written for. I
reproduced the shipped walk verbatim against `jsonschema` 0.49.2 in an isolated
crate and measured the control pair the plan claims:

```
$defs.Inner   (control)   rewritten=true   (v1,v2) = (Conforms, Violates)
$defs.default (colliding) rewritten=true   (v1,v2) = (Conforms, Violates)
```

Both now enforce. All 23 `output_validation` unit tests pass under
`--features "full fuzzing"`, both `schema_dialect_normalization_properties`
proptests pass, and `pmat quality-gate --fail-on-violation --checks complexity`
(pmat 3.15.0) reports `Total violations: 0` on this tree. The detector and the
rewriter agree on all five (key-class × value-kind) combinations reachable
today, the rename-invariance property and fuzz invariant 6 are both genuinely
name-blind (they consult no keyword list), and neither produces a false positive
against the shipped rule — I traced the `{"properties": {"$schema": "…draft-07…"}}`
false-positive shape through all three restated copies and it is now handled
correctly in each.

**The fix is scoped to a five-entry allow-list, and the allow-list is
incomplete.** `SUBSCHEMA_MAP_KEYWORDS` omits `dependencies` — draft-07's own
map-from-property-NAME-to-subschema keyword, which this very module records
(`src/server/output_validation.rs:707-712`, D-115-03-C) as still honoured by
`jsonschema` 0.49.2 under the 2020-12 pin. Measured on this tree with the shipped
walk:

```
$ref -> #/dependencies/Inner     rewritten=true     <- normalized
$ref -> #/dependencies/default   rewritten=false    <- NOT normalized, no warn
$ref -> #/components/Inner       rewritten=true
$ref -> #/components/default     rewritten=false
```

Normalization is therefore **still name-dependent**: renaming a `dependencies`
entry from `Inner` to `default` flips `Cow::Owned` to `Cow::Borrowed`, the legacy
declaration survives, and `compile_2020_12`'s `tracing::warn!` — the only D-02
diagnostic a tool author gets — silently does not fire. That is the identical
category error 115-14 exists to close, one keyword over, and **both** of the new
fences 115-15 added are structurally blind to it, because both enumerate the same
five-entry list whose incompleteness is the defect: the property generator's
`arb_container()` draws three of the five, and fuzz invariant 6 iterates the fuzz
copy of `SUBSCHEMA_MAP_KEYWORDS`. Booked as CR-01.

The secondary theme is that this round shipped three literal copies of two
keyword lists across `src/`, `tests/` and `fuzz/`, documented in each file as
"the mirror is REQUIRED", with **no gate that they stay in sync** — and left
several of the same-file claims the round was booked to correct (round-1 `WR-02`)
still standing three hundred lines from their own correction.

## Critical Issues

### CR-01: `SUBSCHEMA_MAP_KEYWORDS` omits `dependencies`, so the 115-14 name-position bypass survives one keyword over — and both new fences structurally cannot reach it

**File:** `src/server/output_validation.rs:160-166` (definition),
`209-227` / `265-280` (the two dispatches);
`tests/property_tests.rs:960-966`, `1034-1036`;
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:272-278`, `577-601`;
`contracts/mcp-protocol-sdk-v1.yaml:253-261`, `270-280`

**Issue:**

`SUBSCHEMA_MAP_KEYWORDS` lists `properties`, `patternProperties`, `$defs`,
`definitions`, `dependentSchemas`. Every other object key falls to the ordinary
walk, which applies `DATA_ONLY_KEYWORDS` to the key — including keys that are
**author-chosen names, not keywords**. `dependencies` is exactly such a container:
in draft-04 through 2019-09 its value is a map from an instance-property NAME to
a subschema (or to an array of names). draft-07-declared documents are precisely
the documents this normalizer exists for, and the repo already ships one
(`fuzz/corpus/fuzz_schema_draft_pin/05_draft07_dependencies`).

Measured on this tree, with the shipped walk (`first_legacy_dialect`,
`first_legacy_dialect_in_member`, `pin_dialect_in_place`, `pin_dialect_in_member`,
`normalize_schema_dialect`) copied byte-for-byte into an isolated crate pinned to
`jsonschema =0.49.2`, over two documents differing ONLY in the NAME of the entry:

| Document | `normalize_schema_dialect` |
|---|---|
| `{"type":"object","properties":{"n":{"$ref":"#/dependencies/Inner"}},"dependencies":{"Inner":{"$id":"https://example.test/inner","$schema":"http://json-schema.org/draft-07/schema#","type":"integer"}}}` | `Cow::Owned`, **rewritten** |
| the same with the entry named `default` | `Cow::Borrowed`, **nothing rewritten** |

The second row is `115-VERIFICATION.md`'s reproduction document with `$defs`
replaced by `dependencies`. It is reachable by any author who writes a draft-07
schema with a dependent subschema keyed on a property named `default`, `const`,
`enum` or `examples`.

The same holds for **any** non-keyword container an author invents, e.g.
`{"components": {"default": {…}}}` (measured `rewritten=false`), because the
data-only deny-list is applied at every object node regardless of whether that
node is a schema at all.

Three consequences, in descending certainty:

1. **The D-02 diagnostic is silently suppressed.** `compile_2020_12` only warns
   when `normalize_schema_dialect` returns `Owned`. For the `default`-named row
   it returns `Borrowed`, so the author is never told their declaration was
   ignored. This is certain and is a behavioural defect today.
2. **The module's and the contract's stated postcondition is false for a
   reachable three-line document.** `src/server/output_validation.rs:384-387`
   asserts "Rewriting every declaration is deliberately a SUPERSET of what
   `jsonschema` honours … which is what makes the postcondition above statable
   without a per-node `$id` analysis". It is not a superset: it is name-dependent
   at these positions.
3. **Latent validation bypass.** I could NOT demonstrate a v2 verdict flip
   through this position on `jsonschema` 0.49.2 — both `dependencies.Inner` and
   `dependencies.default` measured `v2 = Violates`, i.e. 0.49.2 does not appear to
   treat a `#/dependencies/…` node as an embedded-resource root today. That is
   stated plainly rather than glossed. But the module itself refuses to rely on
   that: the `properties` half of
   `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`
   (`src/server/output_validation.rs:1101-1130`) is fenced STRUCTURALLY for
   exactly this reason — "a behavioural assertion would pass against the
   defective code" — and the honouring behaviour has already shifted across
   0.46.10 → 0.49.2 within this phase's own measurements. A safety property that
   holds only because of an un-pinned library implementation detail is the
   condition under which this defect shipped twice already.

**Why nothing catches it.** Every fence added by 115-15 is parameterised by the
same incomplete list:

- `tests/property_tests.rs:1034-1036` — `arb_container()` draws
  `$defs | definitions | properties`. `dependencies` is unreachable in the
  generated space.
- `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:591` — invariant 6 skips any root
  member whose key is not in the fuzz copy of `SUBSCHEMA_MAP_KEYWORDS`.
- `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:451` — invariant 5's scan restates
  the same list, so it agrees with the shipped walk and reports nothing.
- `src/server/output_validation.rs:1080`, `1109`, `1394-1435` — the unit fences
  and `normalization_cases()` use only `$defs` and `properties`.

The one instrument the round advertises as "DERIVED from a JSON Schema 2020-12
fact … consults no `DATA_ONLY_KEYWORDS` list at all" is in fact gated by a
crate-derived list one line earlier.

**Fix:**

```rust
// src/server/output_validation.rs
const SUBSCHEMA_MAP_KEYWORDS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
    // draft-04..2019-09 spelling; its values are subschemas keyed by INSTANCE
    // PROPERTY NAME, and `jsonschema` 0.49.2 still honours the keyword under the
    // 2020-12 pin (D-115-03-C), so its values are live schema positions.
    "dependencies",
];
```

mirrored in `tests/property_tests.rs:960` and
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:272`, plus
`Just("dependencies")` added to `arb_container()` and a
`dependencies.default` row added to `normalization_cases()` and to
`v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword`.

That closes the measured case. It does **not** close the general one — an
arbitrary container (`components`, a vendor extension) still gets the deny-list
applied to its name keys. Round-1 `WR-04`'s recommendation stands and should be
booked explicitly: a deny-list over an open keyword space cannot be completed, so
the durable fix is to invert the walk — descend only into positions the JSON
Schema core/applicator vocabularies DEFINE as subschemas, and treat everything
else as opaque. Whichever route is taken, the contract text (`WR-04` below) must
be corrected to match, because it currently asserts the superset property that
CR-01 falsifies.

## Warnings

### WR-01: Three literal copies of two keyword lists, no gate that they agree — and each file's rustdoc calls the mirror "REQUIRED"

**File:** `src/server/output_validation.rs:140`, `160-166`;
`tests/property_tests.rs:941`, `960-966`;
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:252`, `272-278`

**Issue:** `DATA_ONLY_KEYWORDS` and `SUBSCHEMA_MAP_KEYWORDS` are each written out
three times. `grep -rn "SUBSCHEMA_MAP_KEYWORDS: &\[&str\]" src tests fuzz` returns
exactly three definitions; `grep -n "SUBSCHEMA\|DATA_ONLY" Makefile` returns
nothing. There is no test, no `include!`, and no source-text gate asserting the
three agree — while every one of the three rustdocs states that the mirror is
mandatory and that a divergence breaks the fence on CORRECT behaviour.

The failure modes are asymmetric and both are silent:

- Crate list gains an entry, mirrors do not → the property and fuzz fences turn
  into FALSE-POSITIVE generators (a legitimately-left-alone name-bound `$schema`
  is reported by the position-blind scan as a surviving legacy declaration).
- An entry is removed from all three in lockstep → coverage disappears with zero
  test failures (see WR-02).

This is the same "the defensive layer restates the rule and nothing checks the
restatement" mechanism that produced the 115-14 defect, reinstated at a new seam.

**Fix:** publish the two lists through the existing `fuzzing` seam and assert
equality, so drift is a compile-or-test failure rather than a silent one:

```rust
// src/server/output_validation.rs, inside `pub mod fuzz_support`
pub const DATA_ONLY_KEYWORDS: &[&str] = super::DATA_ONLY_KEYWORDS;
pub const SUBSCHEMA_MAP_KEYWORDS: &[&str] = super::SUBSCHEMA_MAP_KEYWORDS;
```

```rust
// tests/property_tests.rs (and the same shape in the fuzz target's own tests)
#[test]
fn keyword_lists_mirror_the_shipped_ones() {
    use pmcp::server::output_validation::fuzz_support as seam;
    assert_eq!(SUBSCHEMA_MAP_KEYWORDS, seam::SUBSCHEMA_MAP_KEYWORDS);
    assert_eq!(DATA_ONLY_KEYWORDS, seam::DATA_ONLY_KEYWORDS);
}
```

The seam is already `fuzzing`-gated and therefore off the public API surface, so
this adds nothing to `cargo public-api`.

### WR-02: `patternProperties` and `dependentSchemas` are in the list but no test, property draw or corpus seed exercises either

**File:** `src/server/output_validation.rs:160-166`, `1080`, `1109`,
`1394-1435`; `tests/property_tests.rs:1034-1036`;
`fuzz/corpus/fuzz_schema_draft_pin/`

**Issue:** Of the five entries added by 115-14, only `$defs` and `properties`
appear in any fence:

- `normalization_cases()` (`:1394-1435`) — `$defs`, `properties`
- `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword` — `$defs`
  (loop at `:1080`), `properties` (loop at `:1109`)
- `arb_container()` — `$defs`, `definitions`, `properties`
- corpus seeds 12/13/14 — `$defs`, `properties`

Deleting `"patternProperties"` and `"dependentSchemas"` from all three copies of
the list passes the entire suite. I verified both positions currently work
(`$ref -> #/patternProperties/default` and `#/dependentSchemas/default` both
measured `rewritten=true`), so this is an unfenced-correct-behaviour gap rather
than a live defect — but two of the five entries the round added are protected by
nothing.

**Fix:** widen `arb_container()` to all five entries (it costs one `Just` each
and the generator already parameterises the pointer), and add a
`patternProperties.default` row to `normalization_cases()`. `patternProperties`
keys are regexes, so `arb_definition_name()`'s literals are all valid patterns
and no escaping is needed.

### WR-03: `assert_no_legacy_dialect_survives`'s own rustdoc and the `fuzz_target!` call-site still assert the exact claim the module doc's 115-15 CORRECTION labels false

**File:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:94-113` (the correction),
`:498-501` (the contradicting rustdoc), `:723-726` (the contradicting comment)

**Issue:** 115-15 was booked to close round-1 `WR-02` ("Fuzz invariant 5 is
documented as 'TOTAL — no skip condition'; its collector has a skip condition").
The module-level doc was corrected at `:94-113`:

> The scan DOES have a skip condition — `collect_dialect_declarations` does not
> descend into a `DATA_ONLY_KEYWORDS` payload … The invariant is therefore total
> over SCHEMA POSITIONS, not over every input

Two copies of the retracted claim survive verbatim in the same file:

```rust
// :498-501
/// Total — no skip condition. It holds for every input that parses as JSON,
/// including the documents `is_dialect_neutral` excludes, which is exactly why
/// it is a second invariant rather than a relaxation of that predicate.

// :723-726
// Invariant 5. TOTAL — it holds for every input that parses as JSON,
```

A reader who opens the function or the `fuzz_target!` body — the two places a
maintainer actually looks — reads the false version. `{"const":{"$schema":"…draft-07…"}}`
parses as JSON and the invariant does not hold over it, by design.

**Fix:** replace both with the corrected scope, e.g. `/// Total over SCHEMA
POSITIONS under the traversal rule the shipped walk implements — NOT over every
input: a `$schema` inside a const / enum / default / examples payload is instance
DATA and must survive. See the 115-15 correction on invariant 5 in the module
docs.` and cross-reference `:94-113` from both.

### WR-04: The contract's normative `equation:` still states the unscoped total that 115-14 corrected in the POSTCONDITION three lines below

**File:** `contracts/mcp-protocol-sdk-v1.yaml:248-252` and `:299-320`;
`contracts/binding.yaml:508-513` (`normalize_schema_dialect`), `:542-547`
(`first_legacy_dialect`), `:569-575` (`pin_dialect_in_place`)

**Issue:** 115-14 rewrote the `walk:` clause (`:253-261`) and the POSTCONDITION
invariant (`:299`, now correctly scoped to "any SCHEMA POSITION"), but left the
equation head above them untouched:

```yaml
normalize_schema_dialect(s)
  = s   when NO string-valued $schema anywhere in s names a dialect
        other than DRAFT_2020_12 (root or any depth; ...)
  = clone(s) with EVERY such $schema := DRAFT_2020_12, otherwise
```

So the same YAML block now says two different things four lines apart. The head
is falsified by three documents the shipped code handles CORRECTLY:

- `{"const": {"$schema": "http://json-schema.org/draft-07/schema#"}}` → returns
  `s`, must (data guard, fenced by
  `normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone`)
- `{"properties": {"$schema": "http://json-schema.org/draft-07/schema#"}}` →
  returns `s`, must (name position bound to a non-schema; this is the shape
  `src/server/output_validation.rs:416-426` calls out by name)

and by one it handles INCORRECTLY (`{"dependencies": {"default": …}}`, CR-01).
An equation that no correct implementation can satisfy cannot gate anything —
this is round-1 `WR-01` half-closed: the postcondition was fixed, the equation
that defines the function was not.

The three `binding.yaml` notes have the same shape: each opens with the unscoped
claim ("anywhere in the document", "at any depth", "overwrites EVERY string-valued
`$schema`") and only qualifies it in an appended `115-14 POSITION CORRECTION`
paragraph. A reader who stops at the first sentence — which is what a
`pmat comply` reviewer diffing a signature does — reads the retracted scope.

**Fix:** bring the equation head into line with the POSTCONDITION it sits above:

```yaml
normalize_schema_dialect(s)
  = s   when no string-valued $schema in any SCHEMA POSITION of s (see `walk:`
        below) names a dialect other than DRAFT_2020_12
  = clone(s) with every such $schema := DRAFT_2020_12, otherwise
```

and prefix each of the three `binding.yaml` note heads with "(scope corrected by
115-14 — read the POSITION CORRECTION below before this sentence)" or, better,
rewrite the head and keep the correction as changelog.

### WR-05: Detector and rewriter are written in structurally different shapes despite being documented as "visibly mirror-image", and nothing enforces the list disjointness they silently depend on

**File:** `src/server/output_validation.rs:209-227` vs `:265-280`

**Issue:** The detector dispatch is a `match` whose first arm guards on the VALUE
kind and the key class together:

```rust
Value::Object(named_subschemas) if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) => …,
_ if DATA_ONLY_KEYWORDS.contains(&member_key) => None,
_ => first_legacy_dialect(member_value),
```

The rewriter dispatch is an `if` chain that tests the KEY class first and the
value kind second:

```rust
if SUBSCHEMA_MAP_KEYWORDS.contains(&member_key) { match member_value { … } }
else if !DATA_ONLY_KEYWORDS.contains(&member_key) { pin_dialect_in_place(member_value); }
```

I enumerated all five (key-class × value-kind) combinations and they agree
today — but only because the two lists are disjoint. Put one key in both (a
plausible future edit: `dependencies` is a subschema map in draft-07 and pure
data in some vendor dialects; `examples` is a container in some OpenAPI
tooling) and for a NON-object value the detector takes arm 2 and returns `None`
while the rewriter takes the SUBSCHEMA branch and descends — a detector/rewriter
divergence, which `:172-177` states "is a defect". The rustdoc at `:199-204`
claims both halves were split "so the two remain visibly mirror-image; a reader
comparing them should be comparing like with like", which the current shapes do
not deliver.

Note also that the two RESTATED copies
(`tests/property_tests.rs:1262-1281`, `fuzz/…:397-414`) both use the rewriter's
`if`-chain shape, so they mirror one half and not the other.

**Fix:** write both dispatches in the same three-way shape, and make the
dependency explicit rather than implicit:

```rust
#[test]
fn keyword_lists_are_disjoint() {
    assert!(
        SUBSCHEMA_MAP_KEYWORDS.iter().all(|k| !DATA_ONLY_KEYWORDS.contains(k)),
        "a key in BOTH lists makes the detector's match and the rewriter's if-chain \
         disagree for a non-object value — the divergence normalize_schema_dialect's \
         postcondition exists to forbid"
    );
}
```

### WR-06: The mirrors' stated justification for the strip half is false — only the SCAN half can false-positive

**File:** `tests/property_tests.rs:946-955`;
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:256-266`

**Issue:** Both mirror rustdocs justify the position-aware `strip` with:

> a position-blind strip here would remove it from only one side of the
> surgical-scope comparison

That is not how the comparison works. `strip_dialect_declarations` is applied to
BOTH `stripped_input` and `stripped_once`
(`tests/property_tests.rs:1360-1363`, `fuzz/…:483-486`). For the cited input
`{"properties": {"$schema": "…draft-07…"}}` the shipped walk correctly leaves the
document unchanged, so `input == once`, so the two clones are identical, so ANY
deterministic strip — position-blind included — keeps them equal. The assertion
cannot fire.

The claim IS true for the other half: a position-blind
`collect_dialect_declarations` descends into the `properties` MAP as though the
map were a schema, sees the string-valued `$schema` bound to a NAME, and reports
it to invariant 5 as a surviving legacy declaration — a genuine false positive on
correct behaviour. `src/server/output_validation.rs:420-426` states exactly this
and only this ("their surviving-declaration **scan** report a FALSE positive").
The two mirrors over-generalised it to cover the stripper as well.

Not a code defect — the strippers are correct — but in a round whose entire
subject is "the restated copies must state the rule the code actually
implements", two of the three copies carry a justification that does not survive
inspection, which is how a future maintainer talks themselves into a wrong
simplification.

**Fix:** in both mirror rustdocs, attribute the false-positive risk to the scan
only:

> a position-blind [`collect_dialect_declarations`] would report a name-bound
> `$schema` STRING as a surviving legacy declaration — a false positive against a
> correct normalizer. The stripper is applied to BOTH sides of the surgical-scope
> comparison so a blind strip cannot fire that assertion; it mirrors the shipped
> rule so the two walks stay readable as one rule, not because it could
> false-positive.

## Info

### IN-01: The README row for seed `14_defs_named_default` contradicts itself about the root `$schema`, and misidentifies which seed it derives from

**File:** `fuzz/corpus/fuzz_schema_draft_pin/README.md:87`

**Issue:** The row says seed 14 is "**`12`'s shape** with the `$defs` entry
RENAMED from `Inner` to `default`" and then, in the same sentence, "**No root
`$schema`**". Decoded, the two seeds are:

```
12: {"$schema":"http://json-schema.org/draft-07/schema#","type":"object",
     "properties":{"n":{"$ref":"#/$defs/Inner"}},"$defs":{"Inner":{...}}}
14: {"type":"object","properties":{"n":{"$ref":"#/$defs/default"}},
     "$defs":{"default":{"$id":...,"$schema":"...draft-07...","type":"integer"}}}
```

Seed 12 carries a root declaration; seed 14 does not. Seed 14 is seed **13**'s
shape with the embedded `$schema` restored and the entry renamed. Both halves of
the sentence cannot be true. Consequence: seed 14 never reaches
`compile_2020_12`'s warn path via a root declaration and never exercises the
`(Violates, Conforms)` regression row that seed 12 covers. The seed still trips
invariants 5 and 6 against a position-blind normalizer (I traced both), so the
coverage claim holds — only the provenance sentence is wrong.

**Fix:** change the row to "`13`'s shape with the embedded resource's draft-07
`$schema` restored and the `$defs` entry renamed from `Inner` to `default`", or
add the root declaration to the seed so the sentence becomes true and the seed
also covers the warn path.

### IN-02: The "cognitive 24 against a threshold of 23" justification for the two extracted helpers is not the project's documented threshold and is not reproducible from the gate invocation

**File:** `src/server/output_validation.rs:199-204`, `:259-264`;
`contracts/binding.yaml:604-610`

**Issue:** Three places justify the `*_in_member` extraction — and instruct "Do
not inline either back" — with "`pin_dialect_in_place` at cognitive 24 against a
threshold of 23". `CLAUDE.md` documents the CI cap as cognitive **≤25**
(`pmat analyze complexity --format json --max-cognitive 25`, hard cap 50); the CI
job runs `pmat quality-gate --fail-on-violation --checks complexity` with no
threshold flag, and `.pmat/project.toml` carries no cognitive threshold. A
threshold of 23 appears nowhere in the gate configuration. Verified on this tree
with pmat 3.15.0: `pmat quality-gate --fail-on-violation --checks complexity`
→ `Total violations: 0`, and `pmat analyze complexity --max-cognitive 25`
→ `violations: 0`.

The extraction is independently justified (the two halves ARE more readable
split) and the gate passes either way, so nothing is broken. But a "do not
change this back" instruction resting on a number that contradicts the project's
own documented cap will not survive its first challenge.

**Fix:** cite the reproducible command and its output, or drop the specific
numbers and keep the readability rationale.

### IN-03: `disambiguate()` narrows the generated name space for containers where no collision is possible

**File:** `tests/property_tests.rs:1130-1136`, used at `:1169` and `:1212`

**Issue:** `disambiguate()` maps a drawn name `"n"` to `"n_resource"`
unconditionally. The collision it guards against exists only when
`container == "properties"`, because that is the only case where
`embed_resource()` puts the resource and the `$ref` holder (`"n"`) in the same
map (`:1099-1111`). For `$defs` and `definitions` the name `"n"` is perfectly
safe and is now unreachable in the generated space.

**Fix:**

```rust
fn disambiguate(container: &str, name: String) -> String {
    if container == "properties" && name == "n" {
        "n_resource".to_string()
    } else {
        name
    }
}
```

---

## Verification performed for this review

- `cargo test --features "full fuzzing" --lib output_validation -- --test-threads=1`
  → 23 passed, 0 failed.
- `cargo test --features "full fuzzing" --test property_tests schema_dialect_normalization`
  → 2 passed (`property_schema_normalization_is_idempotent_and_surgical`,
  `property_normalization_does_not_depend_on_a_subschema_map_key_name`).
- `pmat quality-gate --fail-on-violation --checks complexity` (pmat 3.15.0)
  → `Total violations: 0`.
- `pmat analyze complexity --format json --max-cognitive 25` → 0 violations.
- `git ls-files fuzz/corpus/fuzz_schema_draft_pin/ | grep -c '/[0-9][0-9]_'`
  → `14`, matching the README's documented count; `fuzz/.gitignore`'s
  `!corpus/fuzz_schema_draft_pin/[0-9][0-9]_*` re-include does match the new seed.
- Seeds 05, 12, 13 and 14 decoded and checked against their README rows.
- The shipped walk (all four helpers plus `normalize_schema_dialect`) copied
  byte-for-byte into an isolated crate pinned to `jsonschema =0.49.2` and driven
  over 14 documents covering `$defs`, `definitions`, `properties`,
  `patternProperties`, `dependentSchemas`, `dependencies`, an arbitrary
  container, a nested `$defs`, and an `allOf` array — each in a `Inner` vs
  `default` pair. This is the source of every "measured" claim above.
- Detector/rewriter agreement enumerated over all five (key-class × value-kind)
  combinations reachable with the current disjoint lists.
- Fuzz and property mirrors traced by hand against the shipped rule on the six
  shapes most likely to false-positive (`properties.$schema` bound to a string,
  `$schema` with an object value, a `SUBSCHEMA_MAP` member with a non-object
  value, an array under `properties`, `const`/`enum`/`default`/`examples`
  payloads, and the rename probe over a non-schema subtree). No false positive
  found in either mirror.

_Reviewed: 2026-08-02T04:55:48Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
_Scope: gap-closure round 2 (plans 115-14 and 115-15) — NOT full-phase coverage_
