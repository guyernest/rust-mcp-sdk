# Seed corpus — `fuzz_schema_draft_pin`

Phase 115-09 (SCHM-01). These files are COMMITTED on purpose. Without them the
target is overwhelmingly likely to spend its whole budget on inputs where
neither half parses as JSON, i.e. to degenerate into a JSON-parser fuzz that
never reaches schema COMPILATION — which is the code path this target exists to
exercise. Replaying the corpus is an acceptance criterion of `115-09-PLAN.md`:

```bash
cd fuzz
cargo +nightly fuzz run fuzz_schema_draft_pin -- -runs=0 corpus/fuzz_schema_draft_pin
```

`-runs=0` replays every committed seed with NO mutation. A non-zero exit means a
seed now trips an invariant.

**`+nightly` is REQUIRED** (115-13): `cargo fuzz` passes `-Zsanitizer=address`,
which stable rustc refuses, so the plain form fails to BUILD. `make test-fuzz`
invokes the plain form and swallows the non-zero exit, reporting success having
fuzzed nothing — do not cite it as evidence (ledger `D-115-U`).

## Byte layout

The layout is deliberately simple, stable and hand-writable — that last property
is the reason a committed corpus is possible at all.

```text
byte 0                  : case selector — 0 = RAW family, non-zero = JSON family
bytes 1..5              : u32 little-endian schema_len
bytes 5..5+schema_len   : schema bytes
bytes 5+schema_len..    : instance bytes (the remainder)
input shorter than 5    : the target returns immediately; no assertion possible
schema_len > remaining  : clamped to the remainder, leaving an empty instance
```

The selector's one behavioural effect is that on the JSON family the target also
validates the schema against ITSELF (a JSON Schema document is itself a JSON
instance). Every invariant holds for both families.

## Adding a case

Write it with a short `python3` heredoc rather than by hand-counting bytes:

```python
import json, struct
schema   = json.dumps({"type": "object"}, separators=(",", ":")).encode()
instance = json.dumps({}, separators=(",", ":")).encode()
open("14_my_case", "wb").write(bytes([1]) + struct.pack("<I", len(schema)) + schema + instance)
```

Name the file after what it covers, keep the two-digit numeric prefix (the
plan's acceptance check counts `^[0-9]`), and add a row below.

## The files

| File                           | Sel | Covers                                                                                               |
| ------------------------------ | --- | ---------------------------------------------------------------------------------------------------- |
| `01_draft07_object_violating`  | 1   | draft-07-declared object schema, instance missing the required key. **Dialect-neutral**, so invariant 3 applies — this is the seed the vacuous-pin negative control fires on. |
| `02_draft07_object_conforming` | 1   | The same schema with a conforming instance: the pin must not make everything fail.                    |
| `03_undeclared_object`         | 1   | No `$schema` at all. `Draft::default() == Draft202012`, so both eras must already agree.               |
| `04_2020_12_declared`          | 1   | An explicit 2020-12 declaration — the normalizer's borrow (no-rewrite) path.                           |
| `05_draft07_dependencies`      | 1   | `dependencies`, which 2020-12 split into `dependentRequired` / `dependentSchemas`. EXCLUDED from the neutrality allowlist, so this seed exercises the invariant-3 SKIP. (Measured on `jsonschema` 0.49.2 the crate still honours it and both eras agree — see D-115-03-C — but the spec-level divergence is why it stays excluded.) |
| `06_exclusive_minimum_boolean` | 1   | draft-04/07 boolean `exclusiveMinimum`: a COMPILE ERROR under the 2020-12 pin, so the v2 verdict is `InvalidSchema` and invariant 3 skips. |
| `07_array_form_items`          | 1   | draft-07 array-form `items` (2020-12 spells it `prefixItems`): the other measured compile error.        |
| `08_external_ref_https`        | 1   | SEP-2106. An external `$ref` must be a compile error, never a fetch. Asserted structurally by `tests/v2_schema_tripwires.rs`; here the obligation is only that the target does not hang. |
| `09_deeply_nested_object`      | 1   | 20 levels of nested `properties`, with a matching 20-level instance — recursion depth in the normalizer, the neutrality predicate and the validator. |
| `10_raw_garbage`               | 0   | The RAW family: non-JSON on both sides. Only invariant 1 (totality) applies.                            |
| `11_draft07_content_encoding`  | 1   | The MEASURED era divergence: `contentEncoding` is an assertion in draft-07 and only an annotation from 2019-09, so v1 says `Violates` and v2 says `Conforms`. Not dialect-neutral, so invariant 3 skips — this seed is what keeps a future "just assert the eras always agree" edit honest. |
| `12_embedded_legacy_resource`  | 1   | Phase 115-13. The `115-VERIFICATION.md` BLOCKER document: root `$schema` draft-07, `properties.n` a local `#/$defs/Inner` pointer, and `$defs.Inner` an `$id`-bearing EMBEDDED SCHEMA RESOURCE carrying its OWN draft-07 `$schema` + `type: integer`; instance `{"n": "NOT-AN-INTEGER"}`. Not dialect-neutral (nested `$schema`), so invariant 3 SKIPS it — this seed is what **invariant 5** (post-normalization dialect purity) exercises, and it has been OBSERVED to trip that invariant with a non-zero exit against a root-only normalizer. |
| `13_embedded_resource_no_dialect` | 1 | Phase 115-13. The same document with the nested `$schema` removed and no root declaration, so it IS dialect-neutral under the widened `$defs`/`$id`/`$ref` allowlist — the first seed to exercise **invariant 3** over an embedded-resource shape. The pair `12`/`13` is the control: `13` proves enforcement works on the shape, `12` proves the dialect switch on it cannot survive the pin. |
