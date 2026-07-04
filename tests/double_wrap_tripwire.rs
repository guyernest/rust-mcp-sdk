//! TOUT-02 double-wrap tripwire acceptance gate (Phase 104, Plan 03).
//!
//! Proves the high-precision structural detector that WARNs (all builds) and
//! `debug_assert!`-fails (debug builds) when dispatch is about to text-wrap a
//! `Value` that STRUCTURALLY resembles an already-built `CallToolResult` — the
//! exact silent bug that caused the agent-lake 2-week outage. "One local run
//! would have caught it."
//!
//! The `looks_like_call_tool_result` marker fn and the `double_wrap_tripwire`
//! decision fn are crate-private (`task_dispatch` is a `pub(crate) mod`); this
//! integration binary reaches them through the hidden `pmcp::__test_support`
//! seam (mirroring how `tests/server_request_dispatcher_integration.rs` reaches
//! the otherwise-`pub(crate)` dispatcher). Testing at the helper level keeps the
//! debug-panic behavior observable WITHOUT spinning up a full dispatch that a
//! `debug_assert!` would abort mid-call (Codex MEDIUM: such integration tests
//! are brittle).
//!
//! Task 1 (this file): the six `looks_like_call_tool_result` behavior cases plus
//! a `proptest` precision fuzz. Task 2 appends the `double_wrap_tripwire`
//! decision/panic tests and the end-to-end suppression parity.

#![cfg(not(target_arch = "wasm32"))]

use pmcp::__test_support::{looks_like_call_tool_result, DoubleWrapMarker};
use serde_json::json;

// ---------------------------------------------------------------------------
// Task 1 — looks_like_call_tool_result: the six behavior cases.
// ---------------------------------------------------------------------------

/// `_meta` carrying the related-task key → `RelatedTaskMeta` (checked first).
#[test]
fn looks_like_fires_on_related_task_meta() {
    let v = json!({
        "_meta": { "io.modelcontextprotocol/related-task": { "taskId": "t1" } }
    });
    assert_eq!(
        looks_like_call_tool_result(&v),
        Some(DoubleWrapMarker::RelatedTaskMeta)
    );
}

/// A NON-EMPTY `content` array whose every element is a `Content` → `ContentArray`.
#[test]
fn looks_like_fires_on_content_array() {
    let v = json!({ "content": [ { "type": "text", "text": "hi" } ] });
    assert_eq!(
        looks_like_call_tool_result(&v),
        Some(DoubleWrapMarker::ContentArray)
    );
}

/// An EMPTY `content: []` must NOT fire (a benign payload can carry one).
#[test]
fn looks_like_ignores_empty_content_array() {
    let v = json!({ "content": [] });
    assert_eq!(looks_like_call_tool_result(&v), None);
}

/// A `content` array holding a non-`Content` element must NOT fire (the
/// internally tagged enum rejects an element without a valid `"type"`).
#[test]
fn looks_like_ignores_non_content_element() {
    let v = json!({ "content": [ "not-a-content-item" ] });
    assert_eq!(looks_like_call_tool_result(&v), None);
}

/// A plain benign object with neither marker → `None`.
#[test]
fn looks_like_ignores_benign_object() {
    let v = json!({ "foo": 1 });
    assert_eq!(looks_like_call_tool_result(&v), None);
}

/// A non-object JSON value (e.g. a bare number) → `None` (no panic).
#[test]
fn looks_like_ignores_non_object() {
    assert_eq!(looks_like_call_tool_result(&json!(42)), None);
    assert_eq!(looks_like_call_tool_result(&json!("string")), None);
    assert_eq!(looks_like_call_tool_result(&json!([1, 2, 3])), None);
}

// ---------------------------------------------------------------------------
// Task 1 — proptest precision: near-zero false positives.
// ---------------------------------------------------------------------------

mod precision {
    use super::*;
    use proptest::prelude::*;

    /// Strategy for arbitrary JSON scalars/containers that are NOT built-result
    /// markers: scalars, and small objects/arrays whose keys avoid `_meta` and
    /// whose `content`, if present, holds only non-`Content` scalars.
    fn benign_json() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::from),
            any::<i64>().prop_map(serde_json::Value::from),
            // Keys/strings deliberately never equal the marker key.
            "[a-z]{1,6}".prop_map(serde_json::Value::from),
        ];
        leaf.prop_recursive(3, 16, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
                prop::collection::hash_map("[a-z]{1,6}", inner, 0..4)
                    .prop_map(|m| serde_json::Value::from(
                        m.into_iter().collect::<serde_json::Map<_, _>>()
                    )),
            ]
        })
    }

    proptest! {
        /// PROPERTY: an object lacking BOTH markers (no `_meta[related-task]`,
        /// and no non-empty all-`Content` `content` array) NEVER returns `Some`.
        /// The `benign_json` strategy never emits the marker key nor a valid
        /// `Content` element, so the detector must always yield `None`.
        #[test]
        fn benign_json_never_trips(v in benign_json()) {
            prop_assert_eq!(looks_like_call_tool_result(&v), None);
        }
    }
}
