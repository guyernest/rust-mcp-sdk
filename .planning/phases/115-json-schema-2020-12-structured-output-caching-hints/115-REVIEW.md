---
phase: 115-json-schema-2020-12-structured-output-caching-hints
reviewed: 2026-08-02T00:58:19Z
depth: standard
scope: gap-closure only (plans 115-12 and 115-13) — NOT full-phase coverage
diff_base: c478e75a
supersedes_note: >-
  This file replaces the 2026-08-01 full-phase review (38 files) at the same
  path. That pass is preserved in git history at c478e75a; its findings are not
  re-litigated here.
files_reviewed: 6
files_reviewed_list:
  - src/server/output_validation.rs
  - tests/property_tests.rs
  - fuzz/fuzz_targets/fuzz_schema_draft_pin.rs
  - fuzz/corpus/fuzz_schema_draft_pin/README.md
  - contracts/mcp-protocol-sdk-v1.yaml
  - contracts/binding.yaml
findings:
  critical: 1
  warning: 7
  info: 3
  total: 11
status: issues_found
---

# Phase 115: Code Review Report (GAP-CLOSURE PASS)

**Reviewed:** 2026-08-02T00:58:19Z
**Depth:** standard
**Files Reviewed:** 6
**Status:** issues_found

## Scope

**This is a GAP-CLOSURE review, not a full-phase review.** Phase 115 previously
received a 38-file full-phase review (2026-08-01, at this same path, preserved in
git history at `c478e75a`). This pass covers only the six files changed by
gap-closure plans 115-12 and 115-13 (commits `fdf236c8`, `a9af3a5d`, `60cda794`,
`c913aeb1`, `d74ef8b7`, `cab8937a`, `1621b3b0`). A later reader must not read
this document as full-phase coverage — 32 of the phase's files were not looked at
here.

## Summary

The recursive normalizer is a real improvement over the root-only body it
replaced. The `Cow::Borrowed` fast path, the string-valued-`$schema`
discrimination and the `const`/`enum`/`default`/`examples` data guard all behave
as documented on the cases the tests cover. All 17 `output_validation` unit tests
and all 5 `phase115_contract_bindings` tests pass on this tree (verified).

They pass over a hole. **The traversal rule 115-12 shipped applies the
`DATA_ONLY_KEYWORDS` skip at every object node without regard to whether that
object is a SCHEMA node or a MAP OF SUBSCHEMAS keyed by AUTHOR-CHOSEN NAMES.** A
`$defs`, `properties`, `patternProperties`, `definitions` or `dependentSchemas`
entry whose name happens to be `const`, `enum`, `default` or `examples` is
therefore never visited by either the detector or the rewriter, and a legacy
dialect declaration on an `$id`-bearing embedded schema resource in that position
survives the v2 pin. Measured against `jsonschema` 0.49.2 through the shipped
`fuzz_support` seam, it reproduces the exact `115-VERIFICATION.md` bypass: v2
accepts an instance that the *same document with the definition renamed*
correctly rejects.

Why this shipped green is structural, and it is the same failure mode 115-12 was
written to fix: **all three fences replicate the defective traversal rule
verbatim.** The unit-test postcondition calls the crate's own
`first_legacy_dialect`; `tests/property_tests.rs:903` restates
`DATA_ONLY_KEYWORDS` and applies the same skip;
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:224` restates them a third time and
the module doc calls the result an "INDEPENDENT" walk. Independence in
*implementation* without independence in *rule* buys nothing against a rule
defect. The correct rule — "`properties` and `$defs` map AUTHOR-CHOSEN NAMES to
subschemas, so their keys are not keywords and must not be allowlist-checked" —
already exists in this very changeset at `fuzz_schema_draft_pin.rs:286`, roughly
200 lines from the code that gets it wrong.

Secondary findings: the contract now asserts two mutually contradictory
invariants; the fuzz target documents invariant 5 as "TOTAL — no skip condition"
when its collector demonstrably has one; the fragment-suffixed spelling of the
2020-12 URI is misclassified as legacy and produces a false "your declaration is
ignored" warning; seed 13 performs no cross-dialect comparison at all; and the
widened property generator still cannot emit a nested `$schema` anywhere except
one hard-coded pointer.

### Reproduction harness

Every measurement below was taken with a throwaway crate that path-depends on
`pmcp` with `default-features = false, features = ["fuzzing", "validation"]` and
drives `pmcp::server::output_validation::fuzz_support::{normalize_bytes,
validate_bytes}`. No repository file was modified during this review.

---

## Critical Issues

### CR-01: `DATA_ONLY_KEYWORDS` is applied position-blind, so a legacy dialect declaration survives the v2 pin on any subschema whose author-chosen name collides with a data-only keyword

**File:** `src/server/output_validation.rs:141-156` (`first_legacy_dialect`),
`src/server/output_validation.rs:165-185` (`pin_dialect_in_place`),
`src/server/output_validation.rs:199-213` (the stated traversal rule)

**Issue:**

Both walkers dispatch purely on the JSON *shape* of the node and apply

```rust
.filter(|(key, _)| !DATA_ONLY_KEYWORDS.contains(&key.as_str()))
```

at *every* object, including objects that are not schemas at all. `properties`,
`patternProperties`, `$defs`, `definitions` and `dependentSchemas` are maps from
**instance-property / definition names** to subschemas. Their keys are names, not
keywords. When such a name is `const`, `enum`, `default` or `examples`, the entire
subtree under it is silently skipped by the detector *and* by the rewriter.

Measured on this tree against `jsonschema` 0.49.2. Two documents differing only in
the NAME of the definition:

| Document | `normalize_schema_dialect` | `(v1, v2)` for `{"n": "NOT-AN-INTEGER"}` |
|---|---|---|
| `$defs.Inner` (control) | legacy `$schema` rewritten to 2020-12 | `(Conforms, Violates)` |
| `$defs.default` (identical, renamed) | **byte-identical to input — nothing rewritten** | `(Conforms, `**`Conforms`**`)` |

The defective document:

```json
{
  "type": "object",
  "properties": { "n": { "$ref": "#/$defs/default" } },
  "$defs": {
    "default": {
      "$id": "https://example.test/inner",
      "$schema": "http://json-schema.org/draft-07/schema#",
      "type": "integer"
    }
  }
}
```

`normalize_schema_dialect` returns `Cow::Borrowed` for it, so no `tracing::warn!`
fires either — the author gets *no* signal. `draft202012::new` then resolves an
EMPTY vocabulary set on the embedded resource and the sub-validator accepts
everything. This is exactly the vacuous-validator bypass described at
`output_validation.rs:224-269`, still reachable.

`properties.default` and `properties.examples` carrying an `$id`-bearing legacy
resource are likewise left unrewritten (verified; today's `jsonschema` happens to
still enforce `type` in that position, but the module's own doc at line 266 states
the walk is deliberately a superset of what the library honours precisely so
behaviour cannot depend on that).

This breaks three things that are supposed to be load-bearing:

1. `contracts/mcp-protocol-sdk-v1.yaml:262-268` — "EVERY such declaration is
   normalized to the 2020-12 URI before compilation" is false as shipped.
2. `contracts/mcp-protocol-sdk-v1.yaml:284-292` — the POSTCONDITION "after
   normalization no `$schema` string anywhere in the document ... is anything
   other than the Draft 2020-12 URI" is false as shipped.
3. `src/server/output_validation.rs:214-222` — "after an `Owned` return,
   `first_legacy_dialect(&owned)` is `None`" holds only because the detector
   shares the blind spot; the *document* still carries the declaration.

And it is invisible to every fence, all of which restate the same rule:

- `normalize_schema_dialect_changes_only_dollar_schema_keys`
  (`src/server/output_validation.rs:1174`) asserts the postcondition via
  `first_legacy_dialect` — the blind detector checking itself.
- `tests/property_tests.rs:903` restates `DATA_ONLY_KEYWORDS` and applies the
  same skip in `strip_dialect_declarations` / `collect_dialect_declarations`.
- `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:224` restates them a third time.
  Invariant 5's collector (line 334) skips the `default` key, so it never
  collects the surviving declaration. Invariant 3 also skips the document
  (measured: `is_dialect_neutral` returns `false`, because `is_neutral_subschema`
  *does* descend into `$defs` values by name and sees the nested `$schema`).

**Fix:** make the walk position-aware. The correct distinction already exists in
this changeset at `fuzz_schema_draft_pin.rs:286` — apply it in the normalizer:

```rust
/// Keywords whose value is a MAP from author-chosen NAMES to subschemas. The
/// keys of these maps are names, never keywords, so the DATA_ONLY_KEYWORDS
/// filter must not be applied to them.
#[cfg(feature = "validation")]
const SUBSCHEMA_MAP_KEYWORDS: &[&str] = &[
    "properties",
    "patternProperties",
    "$defs",
    "definitions",
    "dependentSchemas",
];

#[cfg(feature = "validation")]
fn first_legacy_dialect(node: &Value) -> Option<&str> {
    match node {
        Value::Object(map) => {
            if let Some(declared) = map.get("$schema").and_then(Value::as_str) {
                if declared != DRAFT_2020_12 {
                    return Some(declared);
                }
            }
            map.iter().find_map(|(key, value)| {
                if DATA_ONLY_KEYWORDS.contains(&key.as_str()) {
                    None
                } else if SUBSCHEMA_MAP_KEYWORDS.contains(&key.as_str()) {
                    // Author-chosen NAMES: descend into every value, and never
                    // keyword-filter the map's own keys.
                    value
                        .as_object()
                        .and_then(|m| m.values().find_map(first_legacy_dialect))
                } else {
                    first_legacy_dialect(value)
                }
            })
        },
        Value::Array(items) => items.iter().find_map(first_legacy_dialect),
        _ => None,
    }
}
```

with the mirror-image change in `pin_dialect_in_place`. Then, in order:

1. Add the `$defs.default` document to `normalization_cases()` with
   `expected_owned == true`, and add a
   `schema_mismatch(..., Some(Era::V2)).is_some()` assertion for it beside
   `v2_pin_still_enforces_an_embedded_legacy_resource`.
2. Apply the same rule to `strip_dialect_declarations` /
   `collect_dialect_declarations` in **both** `tests/property_tests.rs` and
   `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`.
3. Add corpus seed `14_defs_named_default` with the document above and the
   instance `{"n":"NOT-AN-INTEGER"}`; confirm it trips invariant 5 against the
   current body before the fix and passes after.
4. Correct the `walk:` clause at `contracts/mcp-protocol-sdk-v1.yaml:253-255`,
   which currently *specifies* the defective rule.

---

## Warnings

### WR-01: The contract asserts two mutually contradictory invariants about the same function

**File:** `contracts/mcp-protocol-sdk-v1.yaml:278-292`

**Issue:** The DATA-guard invariant says `normalize_schema_dialect` "never alters
a `$schema` that is instance DATA: ... nor a `$schema` inside a `const` / `enum` /
`default` / `examples` payload". The very next invariant states the POSTCONDITION
that "after normalization no `$schema` string anywhere in the document — root or
any depth — is anything other than the Draft 2020-12 URI". The document
`{"const": {"$schema": "http://json-schema.org/draft-07/schema#"}}` satisfies the
first and violates the second; the shipped code (correctly) satisfies the first
and is fenced doing so by
`normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone`. A contract
that is unsatisfiable cannot be a gate — and the fuzz/property assertions that
claim to check "the postcondition" silently check the *weaker* walk-restricted
form instead, which is the mechanism by which CR-01 slipped through.

**Fix:** scope the postcondition to something checkable, e.g. "after
normalization no `$schema` string in any SCHEMA POSITION — the root, or any
subschema reachable without descending into a `const` / `enum` / `default` /
`examples` payload — is anything other than the Draft 2020-12 URI", and define
"schema position" to include the values of `properties` / `patternProperties` /
`$defs` / `definitions` / `dependentSchemas` **regardless of the name under which
they appear** (CR-01).

### WR-02: Fuzz invariant 5 is documented as "TOTAL — no skip condition"; its collector has a skip condition, and it is not independent in the way that matters

**File:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:85-97`, `224`, `328-347`,
`379-390`

**Issue:** The module doc claims invariant 5 is "TOTAL — no skip condition, no
neutrality reasoning — so it holds for every input that parses as JSON".
`collect_dialect_declarations` (line 334) skips `DATA_ONLY_KEYWORDS` exactly like
the shipped walker, so the invariant is *not* total; it is total only over the
sub-document the shipped rule happens to visit. The doc further claims the scan
is "implemented INDEPENDENTLY ... so only an independent walk catches a
detector/rewriter disagreement". That clause is true and is also the ceiling: an
independently *typed* walk that restates the same *rule* catches a
detector/rewriter disagreement and nothing else. It cannot catch a rule defect,
which is what shipped (CR-01). In a phase whose own retro attributes the previous
defect to an over-generalized measurement claim, this is that error recurring.

**Fix:** after CR-01 is fixed, correct both claims — state that invariant 5 is
total over *schema positions* under the shared traversal rule, and state
explicitly that it cannot detect a defect in that shared rule. Add a differential
fence for the rule itself, e.g. a rename-invariance property: renaming a `$defs`
key must not change the normalized document apart from that key.

### WR-03: The fragment-suffixed 2020-12 URI is misclassified as a legacy dialect, producing a false "your declaration is ignored" warning

**File:** `src/server/output_validation.rs:60`, `141-148`, `292-311`;
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:232-235`

**Issue:** `DRAFT_2020_12` is compared by exact string equality, so
`"$schema": "https://json-schema.org/draft/2020-12/schema#"` — a legal and common
spelling, and the same `#`-suffixed style this repo uses for the draft-07 URI
throughout — is treated as legacy. Measured:

```
input      : {"$schema":"https://json-schema.org/draft/2020-12/schema#","type":"object"}
normalized : {"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}
```

Consequences: (a) an unnecessary full-document clone for every distinct schema of
that shape; (b) `compile_2020_12` emits `tracing::warn!` telling a tool author
that their **correct 2020-12 declaration** "is ignored and the schema is validated
as 2020-12" — the single diagnostic D-02 leaves available, fired on a false
positive, which trains operators to ignore it; (c) `NEUTRAL_DIALECTS` in the fuzz
target lists only the non-`#` spelling, so invariant 3 never reaches such
documents either.

**Fix:** compare after stripping an empty trailing fragment, and use it in both
halves of the walk:

```rust
#[cfg(feature = "validation")]
fn is_pinned_dialect(declared: &str) -> bool {
    declared.trim_end_matches('#') == DRAFT_2020_12
}
```

and add `"https://json-schema.org/draft/2020-12/schema#"` to `NEUTRAL_DIALECTS`
in the fuzz target.

### WR-04: `DATA_ONLY_KEYWORDS` omits OpenAPI's singular `example` and every vendor/unknown annotation keyword, so instance data in those payloads is rewritten and warned about

**File:** `src/server/output_validation.rs:118-128`

**Issue:** The list covers `const`, `enum`, `default`, `examples` but not
`example` (OpenAPI 3.0's singular spelling) nor any `x-`/vendor extension. Both
carry arbitrary author data. Measured:

```
input      : {"type":"object","example":{"$schema":"http://json-schema.org/draft-07/schema#"}}
normalized : {"type":"object","example":{"$schema":"https://json-schema.org/draft/2020-12/schema"}}

input      : {"type":"object","x-vendor":{"$schema":"urn:vendor:not-a-dialect"}}
normalized : {"type":"object","x-vendor":{"$schema":"https://json-schema.org/draft/2020-12/schema"}}
```

Neither rewrite changes a verdict today (unknown keywords are ignored under
2020-12), but both force a clone and both fire the D-02 warning claiming a dialect
declaration was ignored when there was none. This repo ships
`crates/pmcp-openapi-server`, whose premise is compiling third-party OpenAPI specs
into `outputSchema` documents, so `example` payloads are a first-party expected
input, not a hypothetical.

**Fix:** add `"example"` to `DATA_ONLY_KEYWORDS` and mirror it in the two restated
copies (`tests/property_tests.rs:903`,
`fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:224`). For the general
unknown-keyword case, prefer the inverse of CR-01's fix: descend only into
positions the JSON Schema vocabulary *defines* as subschemas, rather than into
everything not on a deny-list. A deny-list over an open keyword space cannot be
completed.

### WR-05: Seed `13_embedded_resource_no_dialect` performs no cross-dialect comparison, so the widened allowlist gains no seed that actually exercises draft-07 vs 2020-12 over the embedded-resource shape

**File:** `fuzz/corpus/fuzz_schema_draft_pin/README.md:70`;
`fuzz/corpus/fuzz_schema_draft_pin/13_embedded_resource_no_dialect`

**Issue:** Decoded, seed 13's schema declares **no root `$schema`**:

```json
{"type":"object","properties":{"n":{"$ref":"#/$defs/Inner"}},
 "$defs":{"Inner":{"$id":"https://example.test/inner","type":"integer"}}}
```

`Draft::default() == Draft202012`, so v1's auto-detect and the v2 pin compile the
*same* dialect and invariant 3's equality is satisfied trivially. The README calls
this seed the control that "proves enforcement works on the shape" and "the first
seed to exercise invariant 3 over an embedded-resource shape" — invariant 3 is
evaluated, but it compares 2020-12 against 2020-12. Measured `(v1, v2) =
(Violates, Violates)`. The same document with a root
`"$schema": "http://json-schema.org/draft-07/schema#"` added is *also* neutral
under the widened allowlist and *also* measures `(Violates, Violates)` — that
variant is the one that would genuinely compare the two dialects, and it is not in
the corpus.

**Fix:** add seed `15_embedded_resource_draft07_root` (seed 13's document plus a
root draft-07 declaration) and correct the README row for 13 so it claims only
what it does: it fixes the undeclared-root baseline for the shape.

### WR-06: `arb_schema_document()` still structurally excludes every nested-`$schema` location except one hard-coded pointer

**File:** `tests/property_tests.rs:943-1002`, `1147-1163`

**Issue:** The generator's doc says the old version "stripped every non-root
`$schema` before generating, so the generated space structurally excluded the
`$id`-bearing EMBEDDED SCHEMA RESOURCE", and that "the generator now EMITS that
shape and the property asserts over it". It emits exactly *one* shape at exactly
*one* pointer — `/$defs/Inner/$schema`, with the literal key `Inner`. The body
comes from `arb_json()`, whose object-key strategy is
`"[a-zA-Z_][a-zA-Z0-9_]{0,8}"` (`tests/property_tests.rs:683`), which cannot
produce any `$`-prefixed key and therefore can never generate a `$schema`, a
`$defs` or a `$ref` of its own; the root `$schema` is then explicitly removed and
re-injected. Net: the generated space contains a nested `$schema` at
`/$defs/Inner/$schema` and nowhere else, and the final assertion (line 1147)
hard-codes that pointer. This is a fixed-shape test wearing a generator, and it is
why the property could not contain CR-01 either.

**Fix:** draw the definition NAME and the container keyword from strategies rather
than hard-coding them:

```rust
fn arb_definition_name() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("Inner".to_string()),
        // Names that collide with DATA_ONLY_KEYWORDS — the CR-01 shape.
        Just("default".to_string()),
        Just("const".to_string()),
        Just("examples".to_string()),
        "[a-zA-Z_][a-zA-Z0-9_]{0,6}",
    ]
}

fn arb_container() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("$defs"), Just("definitions"), Just("properties")]
}
```

and replace the hard-coded `input.pointer("/$defs/Inner/$schema")` assertion with
one built from the drawn container and name, so the property covers the space
rather than one point in it.

### WR-07: The README's documented acceptance check counts 3382 files, not 13

**File:** `fuzz/corpus/fuzz_schema_draft_pin/README.md:52-53`

**Issue:** "keep the two-digit numeric prefix (the plan's acceptance check counts
`^[0-9]`)". `fuzz/.gitignore:20` ignores `corpus/fuzz_schema_draft_pin/*`, so any
tree in which the fuzzer has actually run fills with libFuzzer's hex-named
artifacts — 5335 files here, of which `ls | grep -c '^[0-9]'` returns **3382**
against 13 tracked seeds. An acceptance criterion that returns 3382 when it means
13 verifies nothing, which is the same defect class this README correctly calls
out one paragraph earlier for `make test-fuzz`.

**Fix:** specify the check against tracked files —
`git ls-files fuzz/corpus/fuzz_schema_draft_pin/ | grep -c '/[0-9][0-9]_'` — and
update the README sentence to match.

---

## Info

### IN-01: `Vec<&&str>` double reference in both restated collectors

**File:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs:398-401`;
`tests/property_tests.rs:1128-1131`

**Issue:** `let legacy: Vec<&&str> = surviving.iter().filter(|d| **d != DRAFT_2020_12).collect();`
collects references-to-references purely to satisfy `{:?}`. The `fuzz/` directory
is excluded from the workspace (`Cargo.toml:665`) so clippy never sees the first
copy; the second sits behind `feature = "fuzzing"`, which is in neither `default`
nor `full`, so `make quality-gate` does not lint it either. Both copies are
outside every lint gate — worth knowing when reading them.

**Fix:** `let legacy: Vec<&str> = surviving.into_iter().filter(|d| *d != DRAFT_2020_12).collect();`

### IN-02: "searched root-first" understates the nondeterminism of the reported `declared` value

**File:** `src/server/output_validation.rs:130-156`, `300-308`

**Issue:** `first_legacy_dialect` checks the current node's `$schema` before
descending (root-first, as documented), but among siblings it follows
`serde_json::Map` iteration order. For a document carrying two different legacy
declarations at the same depth, which one lands in the `declared` field of the
D-02 warning depends on map ordering, and flips between the default `BTreeMap`
backing and a `preserve_order` (`IndexMap`) backing. The warning text implies it
names *the* declaration that triggered the rewrite; it names *a* declaration.

**Fix:** document that `declared` is one of possibly several, or have
`compile_2020_12` report the count alongside the first value.

### IN-03: `pin_dialect_in_place` allocates two `String`s per visited `$schema`, including no-op rewrites

**File:** `src/server/output_validation.rs:170-174`

**Issue:** `map.insert("$schema".to_string(), Value::String(DRAFT_2020_12.to_string()))`
runs for every string-valued `$schema`, including ones already equal to
`DRAFT_2020_12`, and allocates a fresh key `String` that `insert` discards for an
existing key.

**Fix:**

```rust
if let Some(slot) = map.get_mut("$schema") {
    if slot.is_string() && slot.as_str() != Some(DRAFT_2020_12) {
        *slot = Value::String(DRAFT_2020_12.to_string());
    }
}
```

---

## Verification performed during this review

| Check | Result |
|---|---|
| `cargo test --features validation --lib server::output_validation` | 17 passed — the suite is green *with* CR-01 present |
| `cargo test --test phase115_contract_bindings` | 5 passed — the two new bindings (`first_legacy_dialect`, `pin_dialect_in_place`) resolve to real source |
| `git ls-files fuzz/corpus/fuzz_schema_draft_pin/` | 14 tracked (13 seeds + README); the 5321 other files are correctly gitignored |
| Seed decode of all 13 tracked seeds | Layout matches the README byte layout; seed 13's schema confirmed to have no root `$schema` (WR-05) |
| `normalize_bytes` / `validate_bytes` probes (11 documents) | CR-01, WR-03, WR-04, WR-05 measurements above |

---

_Reviewed: 2026-08-02T00:58:19Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
_Scope: gap-closure (115-12, 115-13) — 6 of the phase's files_
