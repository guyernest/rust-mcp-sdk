//! Fuzz target for the era-branched `outputSchema` validation path.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing:
//!
//! ```bash
//! cd fuzz && cargo +nightly fuzz run fuzz_schema_draft_pin
//! ```
//!
//! **`+nightly` is REQUIRED and is not a style choice.** `cargo fuzz` passes
//! `-Zsanitizer=address`, which stable rustc refuses with "the option `Z` is
//! only accepted on the nightly compiler" — the build fails before a single
//! iteration runs. This was MEASURED on 2026-08-01 with cargo-fuzz 0.13.1 and a
//! stable default toolchain. The repo's `make test-fuzz` invokes the PLAIN form
//! (`Makefile:241`) and pipes every non-zero exit into `|| echo "… completed"`,
//! so on a stable default toolchain that target reports success having fuzzed
//! NOTHING. Do not cite `make test-fuzz` as evidence for this target; run the
//! `+nightly` command above, or the corpus replay in
//! `fuzz/corpus/fuzz_schema_draft_pin/README.md`, and check
//! `fuzz/artifacts/fuzz_schema_draft_pin/` is empty afterwards.
//!
//! # What is being fuzzed, and why
//!
//! `src/server/output_validation.rs` checks a handler's `structuredContent`
//! against the tool's declared `outputSchema` on EVERY structured result, and
//! Phase 115 (SCHM-01) branched it by protocol era: v1 auto-detects the dialect
//! from the document's own `$schema`, v2 pins JSON Schema Draft 2020-12 by
//! REWRITING that declaration first. Both halves of the input are attacker- or
//! author-shaped JSON, and the path runs inside request handling, so a panic
//! here is a remotely reachable unwind. The target drives the whole branch
//! through the `feature = "fuzzing"`-gated `fuzz_support` seam, which uses the
//! UNCACHED compile path so fuzzing cannot grow the process-global validator
//! cache without bound.
//!
//! # Input layout
//!
//! Splitting raw bytes at an arbitrary point makes BOTH halves fail to parse as
//! JSON on nearly every iteration, which degenerates this into a JSON-parser
//! fuzz that never reaches schema compilation. A length prefix fixes that, and
//! — being writable by hand — is what makes a committed seed corpus possible:
//!
//! ```text
//! byte 0                  : case selector — 0 = RAW family, non-zero = JSON family
//! bytes 1..5              : u32 little-endian schema_len
//! bytes 5..5+schema_len   : schema bytes
//! bytes 5+schema_len..    : instance bytes (the remainder)
//! input shorter than 5    : return immediately; no assertion is possible
//! schema_len > remaining  : clamped to the remainder, leaving an empty instance
//! ```
//!
//! The selector's one behavioural effect: on the JSON family the schema is ALSO
//! validated against ITSELF (a JSON Schema document is itself a JSON instance),
//! which doubles the semantic coverage of an input whose author declared both
//! halves to be JSON, and is pointless on raw garbage. Every invariant below
//! holds for both families.
//!
//! libFuzzer still mutates the JSON text freely, so plenty of near-valid JSON is
//! explored around each seed. See `fuzz/corpus/fuzz_schema_draft_pin/README.md`.
//!
//! # Invariants
//!
//! 1. **Totality.** `validate_bytes` and `normalize_bytes` return for ANY input.
//!    A panic inside emit-time validation is a remotely reachable unwind
//!    (T-115-24).
//! 2. **Normalization idempotence and surgical scope.** Normalizing twice equals
//!    normalizing once, and the normalized document differs from the input ONLY
//!    at the root `$schema` key. A normalizer that also dropped a sibling
//!    keyword would silently weaken every v2 validator while every
//!    behavioural test still passed.
//! 3. **Dialect-neutral era AGREEMENT.** When the schema's keyword set is drawn
//!    only from keywords whose meaning is IDENTICAL in draft-07 and 2020-12, the
//!    two eras must return the SAME verdict. Skipped when either verdict is
//!    `InvalidSchema` — that comparison would be about compilation, not
//!    semantics, and the skip is only expressible because the verdict is
//!    three-state. This is the generalized fence against the vacuous-validator
//!    bypass (T-115-01): a vacuous v2 validator returns `Conforms` where v1
//!    returns `Violates`, which the equality catches.
//! 4. **Documented, NOT asserted.** An external `$ref` must stay a compile error
//!    rather than a fetch. The structural fence for that is
//!    `tests/v2_schema_tripwires.rs` (`jsonschema` is declared
//!    `default-features = false` everywhere, so no resolver is compiled in);
//!    this target's only obligation is not to hang. Seed `08` keeps the case in
//!    the corpus.
//!
//! # Why invariant 3 is an EQUALITY and not a monotonicity claim
//!
//! A global cross-dialect MONOTONICITY assertion — "the 2020-12 pin never
//! accepts what v1 rejects", or its converse — would be FALSE, and the repo
//! already contains the counterexamples in both directions, MEASURED on
//! `jsonschema` 0.49.2:
//!
//! - `contentEncoding` is an ASSERTION in draft-07 and only an ANNOTATION from
//!   2019-09 onwards, so a draft-07-declared `{"contentEncoding": "base64"}`
//!   REJECTS a non-base64 string on v1 and ACCEPTS it on v2
//!   (`src/server/output_validation.rs`,
//!   `same_schema_text_yields_independent_verdicts_per_era_in_one_process`).
//! - `$ref` siblings are ignored in draft-07 and honoured under 2020-12, which
//!   makes v2 the stricter era for that document.
//!
//! (115-03-PLAN and 115-RESEARCH both name `dependencies` as the divergence
//! case. That is WRONG as measured: `jsonschema` 0.49.2 still honours
//! `dependencies` under the 2020-12 pin and both eras agree on it — see
//! D-115-03-C. `contentEncoding` is the real case.)
//!
//! So divergent keywords are EXCLUDED from the neutrality predicate rather than
//! asserted about, and the assertion over what remains is an equality.
//!
//! ## The dialect-neutral keyword allowlist
//!
//! `type`, `properties`, `required`, `enum`, `const`, `minimum`, `maximum`,
//! `minLength`, `maxLength`, `pattern`, `additionalProperties`, `minItems`,
//! `maxItems`.
//!
//! ## Excluded, each with its reason
//!
//! - `dependencies` — split into `dependentRequired` / `dependentSchemas` in
//!   2020-12 (a spec-level divergence, even though the crate still honours the
//!   old spelling today).
//! - `items` (array form) — replaced by `prefixItems`; the array form is a
//!   COMPILE ERROR under 2020-12.
//! - `exclusiveMinimum` / `exclusiveMaximum` (boolean form) — became numeric in
//!   draft-06.
//! - `definitions` — replaced by `$defs`.
//! - `$ref` alongside siblings — sibling keywords are ignored in draft-07 and
//!   honoured under 2020-12.
//! - `contentEncoding` — an assertion in draft-07, an annotation from 2019-09.
//! - `id` (vs `$id`) — the draft-04 spelling.
//!
//! ## And the DIALECT DECLARATION itself is restricted
//!
//! Neutrality also requires the ROOT `$schema` to be absent, draft-07, or
//! 2020-12, and forbids a NESTED `$schema` anywhere. draft-04 and draft-06 are
//! excluded because they change the meaning of an ALLOWLISTED keyword: under
//! draft-04 `{"type": "integer"}` rejects `1.0`, which draft-06 onwards accepts.
//! A nested `$schema` is excluded because it is a per-resource dialect switch
//! from 2019-09 onwards and merely ignored in draft-07.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::output_validation::fuzz_support::{
    normalize_bytes, validate_bytes, SchemaVerdict,
};
use serde_json::Value;

/// Selector byte plus the little-endian `u32` schema length.
const HEADER_LEN: usize = 5;

/// The selector value marking the RAW family: both halves are arbitrary bytes
/// with no claim that either is JSON.
const SELECTOR_RAW: u8 = 0;

/// Keywords whose meaning is IDENTICAL in draft-07 and 2020-12. See the module
/// docs for the exclusions and their reasons.
const DIALECT_NEUTRAL_KEYWORDS: &[&str] = &[
    "type",
    "properties",
    "required",
    "enum",
    "const",
    "minimum",
    "maximum",
    "minLength",
    "maxLength",
    "pattern",
    "additionalProperties",
    "minItems",
    "maxItems",
];

/// Root `$schema` declarations under which every keyword in
/// [`DIALECT_NEUTRAL_KEYWORDS`] means the same thing. draft-04 and draft-06 are
/// deliberately absent — see the module docs.
const NEUTRAL_DIALECTS: &[&str] = &[
    "http://json-schema.org/draft-07/schema#",
    "https://json-schema.org/draft/2020-12/schema",
];

/// Whether the two eras are REQUIRED to agree about `schema`.
///
/// True only when the root dialect declaration is one this predicate can reason
/// about AND every keyword at every level is dialect-neutral.
fn is_dialect_neutral(schema: &Value) -> bool {
    match schema.get("$schema") {
        None => {},
        Some(Value::String(uri)) if NEUTRAL_DIALECTS.contains(&uri.as_str()) => {},
        // draft-04/06, a non-string, an unknown URI: not a dialect whose
        // keyword semantics this predicate has established.
        Some(_) => return false,
    }
    is_neutral_subschema(schema, true)
}

/// The recursive half of [`is_dialect_neutral`].
///
/// `is_root` exists because a `$schema` at the root is the document's dialect
/// declaration (already checked by the caller), while a NESTED one is a
/// per-resource dialect switch from 2019-09 onwards and simply ignored in
/// draft-07 — a divergence, so it makes the document non-neutral.
fn is_neutral_subschema(schema: &Value, is_root: bool) -> bool {
    let Some(object) = schema.as_object() else {
        // A boolean schema (`true` / `false`) means the same in every draft.
        // Any other scalar is not a schema at all and fails to compile on both
        // eras, which invariant 3 skips anyway.
        return schema.is_boolean();
    };
    for (key, value) in object {
        if key == "$schema" {
            if !is_root {
                return false;
            }
            continue;
        }
        if !DIALECT_NEUTRAL_KEYWORDS.contains(&key.as_str()) {
            return false;
        }
        let nested_is_neutral = match key.as_str() {
            // `properties` maps AUTHOR-CHOSEN NAMES to subschemas, so its keys
            // are not keywords and must not be allowlist-checked; only its
            // values are schemas.
            "properties" => value
                .as_object()
                .is_some_and(|map| map.values().all(|v| is_neutral_subschema(v, false))),
            // A schema (or a boolean) in its own right.
            "additionalProperties" => is_neutral_subschema(value, false),
            // `type`, `required`, `enum`, `const` and the numeric / string /
            // array bounds carry DATA, not subschemas — nothing to recurse into.
            _ => true,
        };
        if !nested_is_neutral {
            return false;
        }
    }
    true
}

/// Invariant 2, over one parsed schema.
fn assert_normalization_is_idempotent_and_surgical(schema_bytes: &[u8]) {
    let Some((input, once, twice)) = normalize_bytes(schema_bytes) else {
        return;
    };

    assert_eq!(
        once, twice,
        "normalize_schema_dialect is NOT idempotent: a second pass changed the document. \
         The v2 pin normalizes before compiling and the cache is keyed by schema TEXT, so a \
         non-idempotent rewrite means the same declaration can compile to two different \
         validators. Input was: {input}"
    );

    let mut stripped_input = input.clone();
    let mut stripped_once = once.clone();
    for document in [&mut stripped_input, &mut stripped_once] {
        if let Some(object) = document.as_object_mut() {
            object.remove("$schema");
        }
    }
    assert_eq!(
        stripped_input, stripped_once,
        "normalization touched a key other than the ROOT $schema. Dropping or rewriting a \
         sibling keyword silently WEAKENS every v2 validator while behavioural tests keep \
         passing. Input was: {input}, normalized to: {once}"
    );
}

/// Invariant 3, over one schema/instance pair.
fn assert_dialect_neutral_eras_agree(schema_bytes: &[u8], instance_bytes: &[u8]) {
    let Some((v1, v2)) = validate_bytes(schema_bytes, instance_bytes) else {
        return;
    };
    // The skip that only a THREE-state verdict can express: comparing eras
    // across a compile failure compares COMPILATION, not semantics.
    if v1 == SchemaVerdict::InvalidSchema || v2 == SchemaVerdict::InvalidSchema {
        return;
    }
    let Ok(schema) = serde_json::from_slice::<Value>(schema_bytes) else {
        return;
    };
    if !is_dialect_neutral(&schema) {
        return;
    }

    // NOTE: this is an EQUALITY, deliberately. A cross-dialect MONOTONICITY
    // assertion — `!(v2 == Conforms && v1 == Violates)` — would be FALSE by this
    // phase's own design and would fire on the repo's OWN tests: a
    // draft-07-declared `{"contentEncoding": "base64"}` violates on v1 and
    // conforms on v2, because `contentEncoding` is an assertion in draft-07 and
    // only an annotation from 2019-09 (`output_validation.rs`,
    // `same_schema_text_yields_independent_verdicts_per_era_in_one_process`).
    // The converse direction is reachable too, via `$ref` siblings. Divergent
    // keywords are therefore EXCLUDED by `is_dialect_neutral` rather than
    // asserted about — and equality over what remains still catches the bypass
    // this target exists for, because a vacuous v2 validator returns `Conforms`
    // where v1 returns `Violates` on a neutral schema.
    assert_eq!(
        v1, v2,
        "DIALECT-NEUTRAL ERA DISAGREEMENT. This schema uses only keywords whose meaning is \
         identical in draft-07 and 2020-12, so both eras must reach the same verdict. The \
         usual cause is the vacuous-validator bypass: a legacy `$schema` declaration compiled \
         under the 2020-12 pin WITHOUT the normalization step yields an empty vocabulary set, \
         producing a validator that accepts everything — v2 says `Conforms` where v1 says \
         `Violates`. Restore the normalize-then-pin step in `compile_2020_12`. \
         schema: {}, instance: {}",
        String::from_utf8_lossy(schema_bytes),
        String::from_utf8_lossy(instance_bytes)
    );
}

fuzz_target!(|data: &[u8]| {
    if data.len() < HEADER_LEN {
        return;
    }
    let selector = data[0];
    let declared_len = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;
    let body = &data[HEADER_LEN..];
    // Clamp rather than reject: a truncated input is still a legitimate case,
    // and rejecting it would throw away every mutation that shortened the body.
    let schema_len = declared_len.min(body.len());
    let (schema_bytes, instance_bytes) = body.split_at(schema_len);

    // Invariant 1: totality. Both seam entry points must RETURN for any bytes;
    // a panic inside either fails the target.
    let _ = validate_bytes(schema_bytes, instance_bytes);
    let _ = normalize_bytes(schema_bytes);

    // Invariant 2.
    assert_normalization_is_idempotent_and_surgical(schema_bytes);

    // Invariant 3.
    assert_dialect_neutral_eras_agree(schema_bytes, instance_bytes);

    // The JSON family declares both halves to be JSON, so the schema document is
    // itself a usable instance — free extra semantic coverage from one input.
    if selector != SELECTOR_RAW {
        let _ = validate_bytes(schema_bytes, schema_bytes);
        assert_dialect_neutral_eras_agree(schema_bytes, schema_bytes);
    }
});
