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
//! Compiled validators are cached per schema (keyed by the schema's canonical
//! JSON text), so steady-state cost per call is one lookup plus a short-circuit
//! `is_valid` check — the error-message pass runs only on actual mismatch.

// Why: same tug-of-war as `task_dispatch` — rustc's `unreachable_pub` demands
// pub(crate) on items in a crate-internal module, while clippy's
// `redundant_pub_crate` flags that as redundant inside a pub(crate) module.
// rustc wins; silence the clippy side.
#![allow(clippy::redundant_pub_crate)]

use serde_json::Value;

/// Warn (via `tracing`) when `value` does not conform to the tool's declared
/// `outputSchema`. Never fails the call. No-op unless the `validation`
/// feature is enabled.
pub(crate) fn warn_on_schema_mismatch(tool: &str, schema: &Value, value: &Value) {
    #[cfg(feature = "validation")]
    {
        if !tracing::enabled!(tracing::Level::WARN) {
            return;
        }
        if let Some(mismatch) = schema_mismatch(schema, value) {
            tracing::warn!(
                tool,
                "structuredContent does not conform to the declared outputSchema: {mismatch}"
            );
        }
    }
    #[cfg(not(feature = "validation"))]
    let _ = (tool, schema, value);
}

/// Check `value` against `schema`, returning a human-readable description of
/// every violation, or `None` when the value conforms.
///
/// A schema that is itself invalid (fails to compile as JSON Schema) also
/// yields `Some` — a drifted declaration is exactly what this check exists
/// to surface.
#[cfg(feature = "validation")]
pub(crate) fn schema_mismatch(schema: &Value, value: &Value) -> Option<String> {
    match cached_validator(schema) {
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

/// Fetch (or compile and cache) the validator for `schema`.
///
/// Keyed by the schema's canonical JSON text: correct across servers sharing
/// the process (unlike a tool-name key), and bounded by the number of distinct
/// declared schemas. Compilation errors are cached too, as the error string.
#[cfg(feature = "validation")]
fn cached_validator(
    schema: &Value,
) -> Result<std::sync::Arc<jsonschema::Validator>, std::sync::Arc<str>> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};

    type Cache = Mutex<HashMap<String, Result<Arc<jsonschema::Validator>, Arc<str>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();

    let key = schema.to_string();
    let cache = CACHE.get_or_init(Cache::default);
    // Why: a poisoned mutex here only means another thread panicked while
    // inserting; the map itself is still usable — recover rather than
    // propagate a panic out of a warn-only diagnostics path.
    let mut map = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.entry(key)
        .or_insert_with(|| {
            jsonschema::validator_for(schema)
                .map(Arc::new)
                .map_err(|e| Arc::from(e.to_string().as_str()))
        })
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
        assert_eq!(schema_mismatch(&person_schema(), &value), None);
    }

    #[test]
    fn non_conforming_value_yields_message() {
        let value = json!({ "age": "not-a-number" });
        let mismatch = schema_mismatch(&person_schema(), &value)
            .expect("missing required field + wrong type must be reported");
        assert!(
            mismatch.contains("name"),
            "message names the missing required field: {mismatch}"
        );
    }

    #[test]
    fn invalid_schema_yields_message() {
        let bad_schema = json!({ "type": 42 });
        let mismatch = schema_mismatch(&bad_schema, &json!({}))
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
            assert_eq!(schema_mismatch(&schema, &ok), None);
            assert!(schema_mismatch(&schema, &json!({ "age": i })).is_some());
        }
    }

    #[test]
    fn warn_never_panics_on_mismatch() {
        warn_on_schema_mismatch("demo_tool", &person_schema(), &json!({ "age": 1 }));
    }
}
