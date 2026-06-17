//! The DETERMINISTIC bundle-JSON serialization choke point (Codex MEDIUM —
//! the precondition for golden diffing).
//!
//! Every bundle member is written through [`to_bundle_json`] so two emits of the
//! same content produce BYTE-IDENTICAL output. Without this, golden diffing
//! (structural OR byte-identical) is non-reproducible: a real diff could be
//! masked or a spurious diff raised (threat T-93-05-NONDET).
//!
//! # The pinned policy (matched to the Phase 92 golden)
//!
//! The committed Phase 92 golden bundle
//! (`pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/`) was inspected to fix
//! the policy. Every golden member is:
//!
//! 1. **Pretty, 2-space indented** — `serde_json::to_string_pretty` (which uses a
//!    2-space indent). The loader re-parses these bytes, so the format is purely a
//!    human-readability + diff-stability choice; the golden uses pretty.
//! 2. **No trailing newline** — every golden file's last byte is `}` (0x7D), NOT
//!    a `\n`. `to_string_pretty` already omits the trailing newline, so the policy
//!    is "emit the pretty string verbatim, append nothing."
//! 3. **Stable key order** — Rust structs serialize their fields in declaration
//!    order (stable across runs); the ONE nondeterminism risk is a `HashMap`,
//!    whose iteration order is per-process random. [`to_bundle_json_sorted_map`]
//!    serializes a `HashMap` through a `BTreeMap` projection so its keys emit in
//!    sorted order (the `executable.ir.json` member relies on this).
//!
//! Because the loader recomputes the `BUNDLE.lock` hashes over these EXACT bytes,
//! a single byte of format drift would flip the combined hash — so this module is
//! the single place the format is decided, and every member routes through it.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use super::EmitError;

/// Serialize `value` to the bundle's pinned deterministic JSON: pretty (2-space)
/// with NO trailing newline, matching the Phase 92 golden format.
///
/// This is the SINGLE choke point for bundle-member JSON. Structs serialize their
/// fields in declaration order, so the output is stable across runs for any value
/// that does not embed a `HashMap` (for those, use
/// [`to_bundle_json_sorted_map`]). The returned `String` is exactly the bytes
/// written to disk and hashed into `BUNDLE.lock`.
///
/// # Errors
/// Returns [`EmitError::Serde`] (tagged with `what`) if serialization fails.
pub fn to_bundle_json<T: Serialize>(value: &T, what: &str) -> Result<String, EmitError> {
    serde_json::to_string_pretty(value).map_err(|e| EmitError::Serde {
        what: what.to_string(),
        detail: e.to_string(),
    })
}

/// Serialize a `HashMap` to the bundle's pinned deterministic JSON with its keys
/// in SORTED order.
///
/// A bare `HashMap` serializes its entries in per-process random iteration order,
/// which breaks byte-determinism (the idempotent re-emit gate hashes these exact
/// bytes). This projects the map through a borrowing [`BTreeMap`] so the keys emit
/// in sorted order; the underlying map's value type is unchanged (the deserialized
/// runtime shape is still a `HashMap`). The `executable.ir.json` IR map routes
/// through here.
///
/// # Errors
/// Returns [`EmitError::Serde`] (tagged with `what`) if serialization fails.
pub fn to_bundle_json_sorted_map<K, V>(map: &HashMap<K, V>, what: &str) -> Result<String, EmitError>
where
    K: Serialize + Ord,
    V: Serialize,
{
    let sorted: BTreeMap<&K, &V> = map.iter().collect();
    to_bundle_json(&sorted, what)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Sample {
        b: u32,
        a: String,
    }

    #[test]
    fn pretty_two_space_no_trailing_newline() {
        let s = to_bundle_json(
            &Sample {
                b: 1,
                a: "x".to_string(),
            },
            "sample",
        )
        .expect("serialize");
        // Pretty → contains newlines + 2-space indent; struct field order is
        // declaration order (b before a), which is stable across runs.
        assert!(s.contains('\n'), "pretty JSON contains newlines");
        assert!(
            s.contains("  \"b\": 1"),
            "2-space indent on the first field"
        );
        // NO trailing newline (matches the Phase 92 golden: last byte is `}`).
        assert!(
            s.ends_with('}'),
            "no trailing newline — last char is the closing brace: {s:?}"
        );
        assert!(!s.ends_with('\n'), "no trailing newline appended");
    }

    #[test]
    fn serialize_is_deterministic_across_runs() {
        let v = Sample {
            b: 7,
            a: "deterministic".to_string(),
        };
        let one = to_bundle_json(&v, "sample").expect("serialize 1");
        let two = to_bundle_json(&v, "sample").expect("serialize 2");
        assert_eq!(one, two, "two emits of the same content are byte-identical");
    }

    #[test]
    fn hashmap_keys_emit_in_sorted_order() {
        // Build the SAME logical map in two different insertion orders; the
        // sorted-map serializer must produce byte-identical output (deterministic
        // despite HashMap iteration-order randomness).
        let mut a: HashMap<String, u32> = HashMap::new();
        a.insert("zebra".to_string(), 1);
        a.insert("alpha".to_string(), 2);
        a.insert("mike".to_string(), 3);

        let mut b: HashMap<String, u32> = HashMap::new();
        b.insert("mike".to_string(), 3);
        b.insert("alpha".to_string(), 2);
        b.insert("zebra".to_string(), 1);

        let sa = to_bundle_json_sorted_map(&a, "map").expect("serialize a");
        let sb = to_bundle_json_sorted_map(&b, "map").expect("serialize b");
        assert_eq!(sa, sb, "sorted-map emit is insertion-order-independent");
        // And the keys are actually sorted: alpha < mike < zebra.
        let ia = sa.find("alpha").expect("alpha present");
        let im = sa.find("mike").expect("mike present");
        let iz = sa.find("zebra").expect("zebra present");
        assert!(ia < im && im < iz, "keys emit in sorted order");
    }
}
