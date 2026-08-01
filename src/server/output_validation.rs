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
}
