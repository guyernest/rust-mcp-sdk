//! Emit-time validation of `structuredContent` against a declared `outputSchema`.
//!
//! When a tool declares an `outputSchema` and the dispatcher bridges the
//! handler's value into `structuredContent`, this module checks the value
//! against the schema and logs a WARNING (never an error result) on mismatch —
//! catching schema drift in dev/CI without adding a production failure mode.
//!
//! The module compiles unconditionally so dispatcher call sites stay plain
//! one-liners; the `validation` feature gate lives INSIDE
//! [`warn_on_schema_mismatch`], which is a no-op without the feature.
//! Compiled validators are cached per `(era, schema)` pair, so steady-state
//! cost per call is one lookup plus a short-circuit `is_valid` check — the
//! error-message pass runs only on actual mismatch.
//!
//! # Era
//!
//! Validation is era-branched (Phase 115, SCHM-01):
//!
//! - **v1** (`Era::V1`, and any request with no resolved protocol context)
//!   keeps today's behaviour EXACTLY: the dialect is auto-detected from the
//!   document's own `$schema` declaration (D-01). Nothing about v1 validation
//!   changed when the v2 pin landed. See [`compile_for_era`] — the single
//!   `jsonschema` auto-detect entry point in this module lives on that arm and
//!   nowhere else.
//! - **v2** (`Era::V2`) compiles every `outputSchema` as JSON Schema Draft
//!   2020-12, which MCP 2026-07-28 pins. On v2 the pin wins UNCONDITIONALLY: a
//!   declared legacy `$schema` is ignored — neither honoured nor rejected — and
//!   the ignoring is announced through a `tracing::warn!` (D-02). See
//!   [`normalize_schema_dialect`] for why "ignored" has to mean "rewritten"
//!   rather than "compiled as-is".
//!
//! A consequence worth stating: some draft-07 constructs cannot be expressed
//! under 2020-12 at all — `exclusiveMinimum: true` and array-form `items` are
//! the measured cases — and those schemas fail to COMPILE on v2, surfacing
//! through the existing "declared outputSchema is not a valid JSON Schema: …"
//! warning. That is the loud half of D-02, and it is deliberate: a schema the
//! server cannot evaluate should say so rather than silently pass everything.
//!
//! This module stays **warn-only on BOTH eras**. Escalating v2 to a hard error
//! result is deliberately NOT done here: it would be a new production failure
//! mode, and the diagnostic value of the warning is the whole point of the
//! module. See 115-RESEARCH § Finding 6.

// Why: same tug-of-war as `task_dispatch` — rustc's `unreachable_pub` demands
// pub(crate) on items in a crate-internal module, while clippy's
// `redundant_pub_crate` flags that as redundant inside a pub(crate) module.
// rustc wins; silence the clippy side.
#![allow(clippy::redundant_pub_crate)]

use crate::types::protocol::Era;
use serde_json::Value;

/// The JSON Schema Draft 2020-12 meta-schema URI — the dialect MCP 2026-07-28
/// pins for `outputSchema`.
#[cfg(feature = "validation")]
const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// Warn (via `tracing`) when `value` does not conform to the tool's declared
/// `outputSchema`. Never fails the call. No-op unless the `validation`
/// feature is enabled.
///
/// `era` is the ALREADY-RESOLVED protocol era of the request being served
/// (Phase 112's one ingress answer), threaded through so the v2 Draft 2020-12
/// pin applies only to v2 requests. `None` — no resolved protocol context —
/// conservatively means [`Era::V1`], matching
/// [`crate::types::protocol::protocol_era`]'s unknown-to-`V1` rule.
pub(crate) fn warn_on_schema_mismatch(tool: &str, schema: &Value, value: &Value, era: Option<Era>) {
    #[cfg(feature = "validation")]
    {
        if !tracing::enabled!(tracing::Level::WARN) {
            return;
        }
        if let Some(mismatch) = schema_mismatch(schema, value, era) {
            tracing::warn!(
                tool,
                "structuredContent does not conform to the declared outputSchema: {mismatch}"
            );
        }
    }
    #[cfg(not(feature = "validation"))]
    let _ = (tool, schema, value, era);
}

/// Check `value` against `schema`, returning a human-readable description of
/// every violation, or `None` when the value conforms.
///
/// A schema that is itself invalid (fails to compile as JSON Schema) also
/// yields `Some` — a drifted declaration is exactly what this check exists
/// to surface.
///
/// `era` selects the dialect policy: `Some(Era::V2)` pins Draft 2020-12,
/// everything else (including `None`) keeps v1's `$schema` auto-detect.
#[cfg(feature = "validation")]
pub(crate) fn schema_mismatch(schema: &Value, value: &Value, era: Option<Era>) -> Option<String> {
    match cached_validator(era, schema) {
        Ok(validator) => {
            // Fast path: the conforming case is the common one and `is_valid`
            // short-circuits; build messages only on actual mismatch.
            if validator.is_valid(value) {
                return None;
            }
            let errors: Vec<String> = validator
                .iter_errors(value)
                .map(|e| format!("{} (at {})", e, e.instance_path()))
                .collect();
            Some(errors.join("; "))
        },
        Err(e) => Some(format!(
            "declared outputSchema is not a valid JSON Schema: {e}"
        )),
    }
}

/// Rewrite the document's ROOT `$schema` to [`DRAFT_2020_12`], leaving every
/// other byte of the document alone.
///
/// Pure and idempotent. Returns `Cow::Borrowed` when the root `$schema` is
/// absent or already 2020-12 (the common case allocates nothing, and the
/// borrow makes "this function did not copy the document" visible in the TYPE
/// rather than only in a comment), and `Cow::Owned` of a clone with the root
/// `$schema` OVERWRITTEN otherwise. Overwritten, not deleted, so the compiled
/// document STATES the dialect it was evaluated under — which also matches
/// `outputSchema`'s own declared type in the 2026-07-28 schema,
/// `{ "$schema"?: string, [key: string]: unknown }`.
///
/// # Why the rewrite is NOT cosmetic
///
/// It is tempting to read this as tidying. It is not: it is the whole safety
/// property of the v2 pin. `jsonschema`'s `with_draft` / `draft202012::new`
/// sets the KEYWORD SET, but a document that declares a legacy meta-schema
/// still resolves its VOCABULARIES from that declaration, and under 2020-12
/// vocabulary semantics a draft-04/06/07 declaration yields an EMPTY
/// vocabulary set — producing a validator that accepts EVERY instance.
///
/// Measured across `jsonschema` 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 / 0.49.2:
/// compiling a draft-07-declared document under the 2020-12 pin WITHOUT this
/// normalization silently drops `type`, `required`, `properties`, `enum`,
/// `$ref`, `minimum` and `additionalProperties`. That is a validation BYPASS,
/// not a "may validate differently" surprise — and `draft202012::meta::is_valid`
/// returns `true` for such a document, so there is no library-side detector.
/// The `$schema` key has to be inspected here.
///
/// Only the ROOT key is touched. A nested `$schema` (inside `properties.*`,
/// say) is left untouched — measured: a nested declaration does not trigger
/// the bypass.
#[cfg(feature = "validation")]
fn normalize_schema_dialect(schema: &Value) -> std::borrow::Cow<'_, Value> {
    use std::borrow::Cow;

    match schema.get("$schema").and_then(Value::as_str) {
        // Undeclared is already 2020-12: `Draft::default() == Draft202012`, and
        // the MCP spec says the same. Nothing to rewrite.
        None | Some(DRAFT_2020_12) => Cow::Borrowed(schema),
        Some(_) => {
            let mut pinned = schema.clone();
            if let Some(object) = pinned.as_object_mut() {
                object.insert(
                    "$schema".to_string(),
                    Value::String(DRAFT_2020_12.to_string()),
                );
            }
            Cow::Owned(pinned)
        },
    }
}

/// Compile `schema` under an explicitly-pinned Draft 2020-12 (the v2 dialect),
/// normalizing the document's `$schema` declaration first.
///
/// Warns when the normalization actually rewrote something: that warning is the
/// only signal a tool author gets that their declared dialect was ignored, and
/// it is the diagnostic D-02 leaves available.
#[cfg(feature = "validation")]
fn compile_2020_12(
    schema: &Value,
) -> Result<jsonschema::Validator, jsonschema::ValidationError<'static>> {
    let normalized = normalize_schema_dialect(schema);
    if matches!(normalized, std::borrow::Cow::Owned(_)) {
        let declared = schema
            .get("$schema")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        tracing::warn!(
            declared,
            "outputSchema declares JSON Schema {declared}; MCP 2026-07-28 pins Draft 2020-12, so \
             the declaration is ignored and the schema is validated as 2020-12"
        );
    }
    jsonschema::draft202012::new(&normalized)
}

/// Compile a validator for `schema` under `era`'s dialect policy, WITHOUT
/// touching the cache.
///
/// This is deliberately split out of [`cached_validator`] rather than inlined
/// into it: the process-global cache is unbounded by design (bounded in
/// practice by the number of distinct DECLARED schemas), so a property or fuzz
/// target that compiles arbitrary generated schemas needs a path that does not
/// grow it without limit. Keep this function separate — do not inline it back.
///
/// Splitting normalization, compilation and caching across three functions is
/// also what keeps each of them under CI's cognitive-complexity cap.
#[cfg(feature = "validation")]
fn compile_for_era(era: Era, schema: &Value) -> Result<jsonschema::Validator, std::sync::Arc<str>> {
    match era {
        // D-01 freezes v1: this arm is today's behaviour VERBATIM — the dialect
        // is auto-detected from the document's own `$schema` declaration.
        Era::V1 => jsonschema::validator_for(schema),
        Era::V2 => compile_2020_12(schema),
    }
    .map_err(|e| std::sync::Arc::from(e.to_string().as_str()))
}

/// Fetch (or compile and cache) the validator for `(era, schema)`.
///
/// Keyed by `(era, canonical schema text)`. The schema-text half is correct
/// across servers sharing the process (unlike a tool-name key) and bounded by
/// the number of distinct declared schemas; the era half is REQUIRED because
/// D-01 makes the same text compile to two DIFFERENT validators — keying on
/// text alone would be first-writer-wins for the process lifetime, silently
/// serving one era's validator to the other. Compilation errors are cached
/// too, as the error string.
///
/// `None` resolves to [`Era::V1`], the same conservative fallback
/// [`crate::types::protocol::protocol_era`] applies to unknown versions: a
/// request with no resolved protocol context must never reach v2 behaviour.
#[cfg(feature = "validation")]
fn cached_validator(
    era: Option<Era>,
    schema: &Value,
) -> Result<std::sync::Arc<jsonschema::Validator>, std::sync::Arc<str>> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    type Cache = Mutex<HashMap<(Era, String), Result<Arc<jsonschema::Validator>, Arc<str>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let resolved_era = era.unwrap_or(Era::V1);
    let key = (resolved_era, schema.to_string());
    let cache = CACHE.get_or_init(Cache::default);
    // Why: a poisoned mutex here only means another thread panicked while
    // inserting; the map itself is still usable — recover rather than
    // propagate a panic out of a warn-only diagnostics path.
    let mut map = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(key)
        .or_insert_with(|| compile_for_era(resolved_era, schema).map(Arc::new))
        .clone()
}

/// Minimal seam for `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs`.
///
/// # ⚠️ Not stable API
///
/// This module exists only behind `feature = "fuzzing"`, which is in neither
/// `default` nor `full` (`Cargo.toml:220`), so `cargo public-api` never sees it
/// on the shipped surface. Do not depend on it. The second gate,
/// `feature = "validation"`, is what compiles the validator this seam drives —
/// without it there is nothing here to reach.
///
/// The shape is verbatim the one `crate::server::request_state::fuzz_support`
/// established, so the crate has ONE convention for a fuzz seam rather than
/// three.
#[cfg(all(feature = "fuzzing", feature = "validation"))]
pub mod fuzz_support {
    use super::{compile_for_era, normalize_schema_dialect};
    use crate::types::protocol::Era;
    use serde_json::Value;

    /// The three distinguishable outcomes of checking ONE instance against ONE
    /// schema under ONE era.
    ///
    /// Three states, not two, and the third is load-bearing. A `(bool, bool)`
    /// seam built from `schema_mismatch(..).is_none()` collapses *the schema
    /// did not compile* and *the schema compiled and the instance violates it*
    /// into the same `false`, so a caller cannot skip the compile-failure case
    /// — and comparing eras across a compile failure compares COMPILATION, not
    /// semantics. [`InvalidSchema`](Self::InvalidSchema) is what makes that
    /// skip expressible.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SchemaVerdict {
        /// The schema compiled under this era's dialect and the instance
        /// conforms to it.
        Conforms,
        /// The schema compiled under this era's dialect and the instance does
        /// NOT conform to it.
        Violates,
        /// The schema itself failed to compile under this era's dialect, so no
        /// instance-level verdict exists. Under the v2 pin this is also how the
        /// SEP-2106 external-`$ref` refusal surfaces.
        InvalidSchema,
    }

    /// Drive emit-time validation with arbitrary schema and instance bytes,
    /// returning the `(v1, v2)` verdict pair.
    ///
    /// Returns `None` when EITHER slice fails to parse as JSON — the caller
    /// then has no schema/instance pair to hold any semantic invariant over,
    /// only the totality one.
    ///
    /// # Why this uses the UNCACHED compile path
    ///
    /// Every verdict is built through [`compile_for_era`], never through
    /// `cached_validator`. The cache is a process-global
    /// `OnceLock<Mutex<HashMap<(Era, String), _>>>` that is unbounded by design
    /// — bounded in practice only by the number of distinct DECLARED schemas.
    /// A fuzzer generates a fresh schema text on nearly every iteration, so
    /// routing this seam through the cache would grow that map without limit
    /// for the whole process lifetime, turning a correctness fuzzer into a
    /// memory-exhaustion one. Do NOT "optimize" this onto `cached_validator`;
    /// `compile_for_era` was split out of it by 115-03 precisely so this path
    /// exists.
    #[must_use]
    pub fn validate_bytes(
        schema_bytes: &[u8],
        instance_bytes: &[u8],
    ) -> Option<(SchemaVerdict, SchemaVerdict)> {
        let schema: Value = serde_json::from_slice(schema_bytes).ok()?;
        let instance: Value = serde_json::from_slice(instance_bytes).ok()?;
        Some((
            verdict(Era::V1, &schema, &instance),
            verdict(Era::V2, &schema, &instance),
        ))
    }

    /// One era's verdict, with the compile failure kept distinct from the
    /// instance-level failure.
    fn verdict(era: Era, schema: &Value, instance: &Value) -> SchemaVerdict {
        match compile_for_era(era, schema) {
            Ok(validator) => {
                if validator.is_valid(instance) {
                    SchemaVerdict::Conforms
                } else {
                    SchemaVerdict::Violates
                }
            },
            Err(_) => SchemaVerdict::InvalidSchema,
        }
    }

    /// Parse `schema_bytes` and return `(input, normalized_once,
    /// normalized_twice)`.
    ///
    /// Handing back all three documents lets a caller hold idempotence
    /// (`once == twice`) and surgical scope (`once` and `input` differ only at
    /// the root `$schema` key) DIRECTLY, rather than inferring them from
    /// downstream validation behaviour — which cannot distinguish "the
    /// normalizer dropped a sibling keyword" from "the instance happened to
    /// conform anyway".
    ///
    /// Returns `None` when the bytes do not parse as JSON.
    #[must_use]
    pub fn normalize_bytes(schema_bytes: &[u8]) -> Option<(Value, Value, Value)> {
        let input: Value = serde_json::from_slice(schema_bytes).ok()?;
        let once = normalize_schema_dialect(&input).into_owned();
        let twice = normalize_schema_dialect(&once).into_owned();
        Some((input, once, twice))
    }
}

/// The seam cannot rot silently: if `fuzz_support` is removed, its verdict
/// discriminants shift, or its skip condition collapses back into a two-state
/// boolean, these fail under `--features "full fuzzing"`.
///
/// Named `fuzz_support_tests` (not `tests`) so `cargo nextest run -E
/// 'test(/output_validation::fuzz_support/)'` selects exactly these five and
/// nothing else — `test(/fuzz_support/)` alone also matches
/// `server::request_state::tests::fuzz_support_seam_rejects_garbage`, which
/// predates this module.
#[cfg(all(test, feature = "fuzzing", feature = "validation"))]
mod fuzz_support_tests {
    use super::fuzz_support::{normalize_bytes, validate_bytes, SchemaVerdict};

    /// Unparseable bytes on EITHER side yield `None` — the target then has only
    /// the totality invariant to hold, which is the whole point of the `Option`.
    #[test]
    fn fuzz_support_returns_none_for_unparseable_input() {
        assert_eq!(
            validate_bytes(b"{not json", b"{}"),
            None,
            "an unparseable SCHEMA must produce no verdict pair"
        );
        assert_eq!(
            validate_bytes(b"{}", b"{not json"),
            None,
            "an unparseable INSTANCE must produce no verdict pair"
        );
        assert_eq!(
            normalize_bytes(b"\xff\xfe\xfd"),
            None,
            "normalize_bytes must refuse non-JSON rather than panicking"
        );
    }

    /// The ordinary two-state case, on both eras: an object schema and a scalar
    /// instance.
    #[test]
    fn fuzz_support_reports_violates_for_a_scalar_against_an_object_schema() {
        assert_eq!(
            validate_bytes(br#"{"type":"object"}"#, b"42"),
            Some((SchemaVerdict::Violates, SchemaVerdict::Violates)),
            "an object schema must report a scalar instance on both eras"
        );
    }

    /// SEP-2106's refusal expressed as the THIRD state. A `(bool, bool)` seam
    /// could not tell this apart from a violating instance, which is why the
    /// verdict is three-state.
    #[test]
    fn fuzz_support_reports_invalid_schema_for_an_external_ref() {
        assert_eq!(
            validate_bytes(br#"{"$ref":"https://example.com/x.json"}"#, b"{}"),
            Some((SchemaVerdict::InvalidSchema, SchemaVerdict::InvalidSchema)),
            "an external $ref must be a COMPILE failure — never a fetch, and never \
             indistinguishable from a violating instance"
        );
    }

    /// The CONCRETE case that makes a cross-dialect MONOTONICITY claim false.
    ///
    /// `contentEncoding` is an ASSERTION in draft-07 and only an ANNOTATION
    /// from 2019-09 onwards, so a non-base64 string VIOLATES under v1's
    /// auto-detect and CONFORMS under the v2 2020-12 pin — `v2 == Conforms &&
    /// v1 == Violates`. Asserting it here documents, at the seam itself, why
    /// `fuzz_schema_draft_pin` must not claim "v2 rejects everything v1
    /// rejects".
    ///
    /// **This is `contentEncoding`, not `dependencies`.** 115-03 measured on
    /// `jsonschema` 0.49.2 that the crate still honours `dependencies` under
    /// the 2020-12 pin, so both eras return the SAME verdict for it and it is
    /// not a divergence case at all (D-115-03-C). The converse direction is
    /// reachable too — `$ref` siblings are ignored in draft-07 and honoured
    /// under 2020-12 — so the era relation is non-monotonic in BOTH directions.
    #[test]
    fn fuzz_support_reports_the_divergent_content_encoding_case_asymmetrically() {
        let schema = br#"{
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "string",
            "contentEncoding": "base64",
            "description": "fuzz-seam-divergence"
        }"#;
        assert_eq!(
            validate_bytes(schema, br#""!!!not-base64!!!""#),
            Some((SchemaVerdict::Violates, SchemaVerdict::Conforms)),
            "the eras must genuinely disagree here; if they agree, either the v1 arm stopped \
             auto-detecting draft-07 or the v2 pin stopped applying, and the fuzz target's \
             dialect-NEUTRAL restriction has lost its reason to exist"
        );
    }

    /// Normalizing twice equals normalizing once, and the rewrite is surgical —
    /// the fixed-example half of what the fuzz target holds over arbitrary
    /// generated schemas.
    #[test]
    fn fuzz_support_normalize_bytes_is_idempotent() {
        let (input, once, twice) = normalize_bytes(
            br#"{"$schema":"http://json-schema.org/draft-07/schema#","type":"object"}"#,
        )
        .expect("the literal above is valid JSON");

        assert_eq!(once, twice, "normalization must be idempotent");
        assert_eq!(
            once.get("$schema").and_then(serde_json::Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "the root $schema must be OVERWRITTEN with the 2020-12 URI, not deleted"
        );

        let mut before = input;
        let mut after = once;
        for document in [&mut before, &mut after] {
            if let Some(object) = document.as_object_mut() {
                object.remove("$schema");
            }
        }
        assert_eq!(
            before, after,
            "normalization touched a key other than the root $schema"
        );
    }
}

#[cfg(all(test, feature = "validation"))]
mod tests {
    use super::*;
    use serde_json::json;

    fn person_schema() -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name"]
        })
    }

    #[test]
    fn conforming_value_yields_none() {
        let value = json!({ "name": "Ada", "age": 36 });
        assert_eq!(schema_mismatch(&person_schema(), &value, None), None);
    }

    #[test]
    fn non_conforming_value_yields_message() {
        let value = json!({ "age": "not-a-number" });
        let mismatch = schema_mismatch(&person_schema(), &value, None)
            .expect("missing required field + wrong type must be reported");
        assert!(
            mismatch.contains("name"),
            "message names the missing required field: {mismatch}"
        );
    }

    #[test]
    fn invalid_schema_yields_message() {
        let bad_schema = json!({ "type": 42 });
        let mismatch = schema_mismatch(&bad_schema, &json!({}), None)
            .expect("an uncompilable schema must be reported, not ignored");
        assert!(
            mismatch.contains("outputSchema"),
            "message says the schema itself is at fault: {mismatch}"
        );
    }

    #[test]
    fn repeated_checks_reuse_the_cached_validator() {
        // Same schema, many values: exercises the cache path both ways.
        let schema = person_schema();
        for i in 0..8 {
            let ok = json!({ "name": format!("p{i}") });
            assert_eq!(schema_mismatch(&schema, &ok, None), None);
            assert!(schema_mismatch(&schema, &json!({ "age": i }), None).is_some());
        }
    }

    #[test]
    fn warn_never_panics_on_mismatch() {
        warn_on_schema_mismatch("demo_tool", &person_schema(), &json!({ "age": 1 }), None);
    }

    // =======================================================================
    // Phase 115 / SCHM-01 — the era branch and the bypass it exists to avoid
    // =======================================================================

    /// The draft-07 meta-schema URI, spelled out once so every fence below
    /// uses the exact string the measured bypass was observed with.
    const DRAFT_07: &str = "http://json-schema.org/draft-07/schema#";

    /// `{n: integer}`, required, declaring draft-07 — the exact document the
    /// bypass was measured on.
    fn draft_07_declared_schema() -> Value {
        json!({
            "$schema": DRAFT_07,
            "type": "object",
            "properties": { "n": { "type": "integer" } },
            "required": ["n"]
        })
    }

    /// THE fence for SCHM-01. A schema that declares draft-07 must still be
    /// ENFORCED under the v2 Draft 2020-12 pin.
    ///
    /// Measured across `jsonschema` 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 /
    /// 0.49.2: handing this document to `draft202012::new` AS-IS produces a
    /// validator that accepts every instance. If this test ever goes green
    /// only because the assertions were relaxed, output validation has become
    /// a no-op for every legacy-declared schema in the wild.
    #[test]
    fn v2_pin_still_enforces_a_draft_07_declared_schema() {
        let schema = draft_07_declared_schema();

        assert!(
            schema_mismatch(&schema, &json!({ "wrong": true }), Some(Era::V2)).is_some(),
            "BYPASS: the v2 Draft 2020-12 pin accepted an instance missing the REQUIRED `n`. A \
             `None` here means the pin compiled the draft-07 declaration into a VACUOUS validator \
             (empty vocabulary set) and emit-time output validation has silently become a no-op \
             for every schema that declares a legacy $schema. Restore the normalize-then-pin step \
             in `compile_2020_12`."
        );
        assert!(
            schema_mismatch(&schema, &json!({ "n": "not-an-int" }), Some(Era::V2)).is_some(),
            "BYPASS: the v2 Draft 2020-12 pin accepted a STRING where the schema declares \
             `integer`. See the message above — `type` is one of the seven keywords measured to \
             be silently dropped when the $schema declaration is not normalized first."
        );
        assert_eq!(
            schema_mismatch(&schema, &json!({ "n": 7 }), Some(Era::V2)),
            None,
            "a conforming instance must still pass under the pin — the fix restores enforcement, \
             it does not make everything fail"
        );
    }

    /// D-01's freeze, asserted rather than assumed: the same draft-07 document
    /// behaves on v1 exactly as it did before the v2 pin existed, and an
    /// absent protocol context (`None`) resolves to v1, never to v2.
    #[test]
    fn v1_validation_is_unchanged_by_the_v2_pin() {
        let schema = draft_07_declared_schema();

        for era in [Some(Era::V1), None] {
            assert!(
                schema_mismatch(&schema, &json!({ "wrong": true }), era).is_some(),
                "v1 auto-detect must keep enforcing draft-07 `required` (era: {era:?})"
            );
            assert!(
                schema_mismatch(&schema, &json!({ "n": "not-an-int" }), era).is_some(),
                "v1 auto-detect must keep enforcing draft-07 `type` (era: {era:?})"
            );
            assert_eq!(
                schema_mismatch(&schema, &json!({ "n": 7 }), era),
                None,
                "v1 must keep accepting a conforming instance (era: {era:?})"
            );
        }
    }

    /// A schema whose verdict genuinely DIFFERS between the two eras.
    ///
    /// `contentEncoding` is an ASSERTION in draft-07 but only an ANNOTATION
    /// from 2019-09 onwards, so a non-base64 string is rejected under v1's
    /// auto-detect and accepted under the v2 2020-12 pin. Measured on
    /// `jsonschema` 0.49.2.
    ///
    /// The `description` carries `prefix` purely to make each caller's
    /// document a DISTINCT cache key, so a test can start from a cold cache.
    /// `description` is an annotation in every draft and changes no verdict.
    fn era_divergent_schema(prefix: &str) -> Value {
        serde_json::from_str(&format!(
            r#"{{
                "$schema": "{DRAFT_07}",
                "type": "string",
                "contentEncoding": "base64",
                "description": "{prefix}"
            }}"#
        ))
        .expect("the template above is valid JSON")
    }

    /// An instance the [`era_divergent_schema`] eras disagree about.
    fn era_divergent_instance() -> Value {
        json!("!!!not-base64!!!")
    }

    /// The cache fence: one process, one schema text, two eras — the second
    /// era must NOT be served the first era's validator.
    ///
    /// Before the key was widened to `(Era, schema text)` this was
    /// first-writer-wins for the whole process lifetime. CI runs with
    /// `--test-threads=1`, so this must not depend on parallel execution; the
    /// order is expressed by the call sequence inside the test, and the twin
    /// test below runs the opposite order over a DISTINCT document so it too
    /// starts from a cold cache.
    ///
    /// Note carefully what this schema demonstrates: here `Era::V2` is MORE
    /// PERMISSIVE than `Era::V1`, deliberately, because `contentEncoding` is
    /// an assertion in draft-07 and only an annotation under 2020-12. The
    /// converse direction is ALSO reachable — `$ref` siblings are ignored in
    /// draft-07 but apply under 2020-12, making v2 stricter there. Both
    /// directions were measured on `jsonschema` 0.49.2, so any cross-era
    /// monotonicity claim ("v2 rejects everything v1 rejects", or its
    /// converse) is FALSE in BOTH directions. None is made anywhere in this
    /// phase — 115-09's fuzz target must not assert one either.
    #[test]
    fn same_schema_text_yields_independent_verdicts_per_era_in_one_process() {
        let schema = era_divergent_schema("v1first");
        let instance = era_divergent_instance();

        let v1 = schema_mismatch(&schema, &instance, Some(Era::V1));
        let v2 = schema_mismatch(&schema, &instance, Some(Era::V2));

        assert!(
            v1.is_some(),
            "v1 auto-detects draft-07, where `contentEncoding` is an ASSERTION: {v1:?}"
        );
        assert_eq!(
            v2, None,
            "v2 pins 2020-12, where `contentEncoding` is only an ANNOTATION and asserts \
             nothing. A `Some` here means the V1 entry was served for the V2 lookup — the cache \
             key lost its era half."
        );
    }

    /// The twin of the test above with the call order REVERSED, over a
    /// distinct document so the process-global cache is cold on entry. Both
    /// orders must produce the same per-era answers; if either order can flip
    /// a verdict, the cache is era-blind.
    #[test]
    fn same_schema_text_yields_independent_verdicts_in_the_opposite_order() {
        let schema = era_divergent_schema("v2first");
        let instance = era_divergent_instance();

        let v2 = schema_mismatch(&schema, &instance, Some(Era::V2));
        let v1 = schema_mismatch(&schema, &instance, Some(Era::V1));

        assert_eq!(
            v2, None,
            "v2-first must give the same v2 answer as v2-second: {v2:?}"
        );
        assert!(
            v1.is_some(),
            "v1-second must give the same v1 answer as v1-first. A `None` here means the V2 \
             entry was served for the V1 lookup — first-writer-wins across eras."
        );
    }

    /// The LOUD half of D-02: draft-07 constructs that cannot be expressed
    /// under 2020-12 fail to COMPILE, and the failure is reported through the
    /// existing schema-invalid message rather than silently passing.
    #[test]
    fn structurally_incompatible_draft_07_constructs_report_a_schema_error_not_silence() {
        // draft-04/07 boolean `exclusiveMinimum`; 2020-12 requires a number.
        let boolean_exclusive_minimum = json!({
            "$schema": DRAFT_07,
            "exclusiveMinimum": true
        });
        // draft-07 array-form `items`; 2020-12 spells this `prefixItems` and
        // requires `items` to be a single schema.
        let tuple_items = json!({
            "$schema": DRAFT_07,
            "items": [{ "type": "string" }, { "type": "number" }]
        });

        for schema in [&boolean_exclusive_minimum, &tuple_items] {
            let mismatch = schema_mismatch(schema, &json!({}), Some(Era::V2)).expect(
                "a draft-07 construct that 2020-12 cannot express must be REPORTED under the v2 \
                 pin, not silently accepted",
            );
            assert!(
                mismatch.contains("outputSchema"),
                "the message must say the schema itself is at fault: {mismatch}"
            );
        }
    }

    /// SEP-2106, behavioural half: an external `$ref` must fail to compile,
    /// with no network or filesystem I/O, on BOTH eras.
    ///
    /// This is the *behavioural* half only. The *structural* fence — that
    /// `jsonschema`'s `resolve-http` / `resolve-file` features never enter the
    /// build graph through cargo feature unification — is 115-08's manifest
    /// tripwire, because a behavioural test like this one would still pass
    /// (merely louder, and after a live fetch) if `resolve-http` were enabled.
    #[test]
    fn external_ref_fails_to_compile_with_no_network_io() {
        let remote = json!({ "$ref": "https://example.com/remote.json" });
        let local_file = json!({ "$ref": "file:///etc/passwd" });
        let relative_under_http_id = json!({
            "$id": "https://example.com/root.json",
            "$ref": "sibling.json"
        });

        let started = std::time::Instant::now();
        for era in [Some(Era::V1), Some(Era::V2)] {
            for schema in [&remote, &local_file, &relative_under_http_id] {
                assert!(
                    schema_mismatch(schema, &json!({}), era).is_some(),
                    "an external $ref must be a hard compile error, never a fetch (era: \
                     {era:?}, schema: {schema})"
                );
            }
        }
        let elapsed = started.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "six external-$ref refusals took {elapsed:?}. Refusal is measured at ~60 µs each; a \
             wall-clock cost anywhere near a second means something is resolving the URI over \
             the network or the filesystem."
        );
    }

    /// D-04, which 115-RESEARCH § Finding 7 measured is ALREADY true today: an
    /// object-typed schema does not conform to a scalar, an array or `null`,
    /// on either era.
    ///
    /// "Reject" here means **a warning is logged** — not that the call fails.
    /// [`warn_on_schema_mismatch`] returns normally for every one of these
    /// inputs and produces no error result. Escalating v2 to a hard error is
    /// deliberately OUT OF SCOPE for this phase: it is a new production
    /// failure mode, and 115-10 books it as a deferred item.
    #[test]
    fn an_object_schema_rejects_a_scalar_on_both_eras_warn_only() {
        let schema = person_schema();
        let non_objects = [json!(42), json!(null), json!([1, 2]), json!("s")];

        for era in [Some(Era::V1), Some(Era::V2)] {
            for value in &non_objects {
                assert!(
                    schema_mismatch(&schema, value, era).is_some(),
                    "an object schema must report a non-object value (era: {era:?}, value: \
                     {value})"
                );
                // Warn-only: this returns, it does not fail the call.
                warn_on_schema_mismatch("demo_tool", &schema, value, era);
            }
        }
    }

    /// The blast-radius fence for D-01. `Draft::default() == Draft202012`, so
    /// an `outputSchema` that declares no `$schema` at all already compiles as
    /// 2020-12 under today's auto-detect. The v2 pin therefore changes
    /// behaviour ONLY for schemas that declare something — this test is what
    /// keeps that claim honest.
    #[test]
    fn an_undeclared_schema_behaves_identically_on_both_eras() {
        let schema = person_schema();
        let values = [
            json!({ "name": "Ada", "age": 36 }),
            json!({ "age": "not-a-number" }),
            json!({}),
            json!(42),
        ];

        for value in &values {
            assert_eq!(
                schema_mismatch(&schema, value, Some(Era::V1)),
                schema_mismatch(&schema, value, Some(Era::V2)),
                "an undeclared schema must give the identical verdict on both eras (value: \
                 {value})"
            );
        }
    }

    /// The four `normalize_schema_dialect` cases, as a set, so both the
    /// structural test and the idempotence test cover the same ground.
    ///
    /// Each entry is `(schema, expected_owned)` — `true` means the normalizer
    /// must have rewritten the document.
    fn normalization_cases() -> Vec<(Value, bool)> {
        vec![
            // (a) no `$schema` at all
            (person_schema(), false),
            // (b) already 2020-12
            (json!({ "$schema": DRAFT_2020_12, "type": "object" }), false),
            // (c) a draft-07 root declaration, with siblings that must survive
            (draft_07_declared_schema(), true),
            // (d) a NESTED `$schema` and no root one
            (
                json!({
                    "type": "object",
                    "properties": { "a": { "$schema": DRAFT_07, "type": "string" } }
                }),
                false,
            ),
        ]
    }

    /// The PURE-function fence: `normalize_schema_dialect` alters the ROOT
    /// `$schema` and NOTHING else.
    ///
    /// Behavioural equivalence through [`schema_mismatch`] cannot prove this —
    /// a normalizer that also dropped a sibling key would still make
    /// `v2_pin_still_enforces_a_draft_07_declared_schema` pass on the cases it
    /// happens to check. This asserts the rewrite is surgical: the borrow/own
    /// decision, the rewritten value, deep equality of every other key, and
    /// that a nested `$schema` is left alone.
    #[test]
    fn normalize_schema_dialect_changes_only_the_root_dollar_schema() {
        use std::borrow::Cow;

        for (schema, expected_owned) in normalization_cases() {
            let normalized = normalize_schema_dialect(&schema);

            assert_eq!(
                matches!(normalized, Cow::Owned(_)),
                expected_owned,
                "borrow/own decision is wrong for {schema} — the no-op cases must allocate \
                 nothing"
            );

            if expected_owned {
                assert_eq!(
                    normalized.get("$schema").and_then(Value::as_str),
                    Some(DRAFT_2020_12),
                    "the root $schema must be OVERWRITTEN with the 2020-12 URI, not deleted"
                );
                // Everything except `$schema` must be deep-equal to the input.
                let mut before = schema.clone();
                let mut after = normalized.into_owned();
                for document in [&mut before, &mut after] {
                    if let Some(object) = document.as_object_mut() {
                        object.remove("$schema");
                    }
                }
                assert_eq!(
                    before, after,
                    "normalization touched a key other than the root $schema"
                );
            } else {
                assert_eq!(
                    *normalized, schema,
                    "a document needing no rewrite must come back byte-identical: {schema}"
                );
            }
        }

        // (d) in detail: the NESTED declaration is left exactly as written.
        let nested = json!({
            "type": "object",
            "properties": { "a": { "$schema": DRAFT_07, "type": "string" } }
        });
        let normalized = normalize_schema_dialect(&nested);
        assert_eq!(
            normalized
                .pointer("/properties/a/$schema")
                .and_then(Value::as_str),
            Some(DRAFT_07),
            "a nested $schema is not the dialect declaration and must never be rewritten"
        );
    }

    /// Normalizing twice equals normalizing once, for all four cases — the
    /// fixed-example half of the idempotence property 115-09 holds over
    /// arbitrary generated input.
    #[test]
    fn normalize_schema_dialect_is_idempotent() {
        for (schema, _) in normalization_cases() {
            let once = normalize_schema_dialect(&schema).into_owned();
            let twice = normalize_schema_dialect(&once).into_owned();
            assert_eq!(
                once, twice,
                "normalization must be idempotent, but a second pass changed {schema}"
            );
        }
    }
}
