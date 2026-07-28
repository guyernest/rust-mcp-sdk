//! The v2 reserved-field registry must grant `inputRequests` PER OWNER, not per
//! disposition (Phase 114, plan 10 — `114-SPEC-RECHECK.md` row 23).
//!
//! # What is being pinned, and why it needed its own suite
//!
//! Phase 113 introduced `server::core::own_reserved_result_fields` as "the
//! authoritative reserved-field registry": a closed set of result keys the
//! SERVER owns, so a tool handler cannot forge an MRTR continuation or an
//! input-request set on its own result. Its ownership test was
//!
//! ```text
//! let mrtr_owned = disposition == ResponseDisposition::InputRequired;
//! ```
//!
//! which was correct while the MRTR egress was the ONLY minter of those keys.
//! Phase 114 introduces a second legitimate minter. A v2 `tasks/get` on an
//! `input_required` task is a **complete** JSON-RPC result (the TASK is waiting,
//! not the request), and the ext-tasks schema makes `inputRequests` a REQUIRED
//! TOP-LEVEL key of that result:
//!
//! ```text
//! $defs.GetTaskResult   = Result & (WorkingTask | InputRequiredTask | …)   // flat allOf
//! $defs.InputRequiredTask.required = [taskId, status, createdAt, lastUpdatedAt, ttlMs, inputRequests]
//! ```
//!
//! Under the derived flag that result's disposition is `Complete`, so the
//! registry DELETED the required field — and deleted it **silently**, with a
//! `tracing::warn!` rather than an error. An integration test asserting only
//! "the request succeeded", or "status == `input_required`", passes against a
//! response a conformant client rejects. That is why this suite asserts on RAW
//! RESPONSE BYTES and on the warning, never on a parsed struct: every one of
//! these result types carries `skip_serializing_if`, so a deleted key and an
//! absent key are the same `None`.
//!
//! # The six properties
//!
//! Each is independently failing-if-broken, and the STRIP case of each pair
//! fires before the SURVIVE case:
//!
//! | # | test | property |
//! |---|------|----------|
//! | 1 | `handler_supplied_input_requests_is_still_stripped` | the forgery protection Phase 113 built, unchanged |
//! | 2 | `tasks_minted_input_requests_survives_egress` | the row-23 fix |
//! | 3 | `tasks_minted_request_state_is_still_stripped` | `requestState` is MRTR-ONLY; the grant is per-KEY per-OWNER |
//! | 4 | `mrtr_minted_input_requests_still_survives` | Phase 113's original behaviour, unbroken |
//! | 5 | `a_v1_result_is_untouched_by_the_registry` | the egress is v2-only |
//! | 6 | `a_non_object_v2_result_is_untouched` | the object-results-only guard |
//!
//! Tests 3-6 land in Task 3 of the plan; tests 1-2 are the Task 1 reproduction
//! pair, written against the UNFIXED tree so the fix has a measurement behind it
//! rather than a prediction.

#![cfg(not(target_arch = "wasm32"))]

use pmcp::testing::{
    v2_result_envelope, EnvelopeOutcome, ReservedFieldEgress, RESERVED_INPUT_REQUESTS,
};
use pmcp::types::mrtr::{InputRequest, InputRequests};
use serde_json::{json, Value};

/// A real `inputRequests` map, built from the PRODUCTION [`InputRequest`] type.
///
/// Hand-writing the JSON would let this fixture drift from the shape the two
/// minters actually emit, which is the failure mode `pmcp::testing` exists to
/// prevent.
fn input_requests() -> Value {
    let mut requests = InputRequests::new();
    requests.insert("roots".to_string(), InputRequest::ListRoots);
    serde_json::to_value(requests).expect("InputRequests serializes")
}

/// The flat v2 `tasks/get` result for an `input_required` task.
///
/// `GetTaskResult` is `Result & InputRequiredTask` in the vendored ext-tasks
/// schema — an `allOf`, not a `{ "task": … }` wrapper — so every task field,
/// `inputRequests` included, is a TOP-LEVEL key of the result object.
fn input_required_task_result() -> Value {
    json!({
        "taskId": "task-row-23",
        "status": "input_required",
        "createdAt": "2026-07-28T00:00:00Z",
        "lastUpdatedAt": "2026-07-28T00:00:01Z",
        "ttlMs": 60_000,
        "inputRequests": input_requests(),
    })
}

/// An ordinary `tools/call` completion onto which a HANDLER wrote the reserved
/// key. No egress minted it.
fn handler_forged_tool_result() -> Value {
    json!({
        "content": [{ "type": "text", "text": "done" }],
        "isError": false,
        "inputRequests": input_requests(),
    })
}

/// Assert the reserved key is PRESENT in the emitted bytes.
fn assert_key_present(outcome: &EnvelopeOutcome, key: &str) {
    assert!(
        outcome.bytes.contains(&format!("\"{key}\"")),
        "expected `{key}` to survive egress, but the emitted bytes were:\n{}\nwarnings: {:#?}",
        outcome.bytes,
        outcome.warnings,
    );
}

/// Assert the reserved key is ABSENT from the emitted bytes AND that the
/// registry SAID it removed it.
///
/// Both halves matter: absence alone cannot distinguish "the registry stripped
/// it" from "the fixture never carried it", which is exactly the ambiguity that
/// let the row-23 defect ship.
fn assert_key_stripped_with_warning(outcome: &EnvelopeOutcome, key: &str) {
    assert!(
        !outcome.bytes.contains(&format!("\"{key}\"")),
        "expected `{key}` to be stripped, but the emitted bytes were:\n{}",
        outcome.bytes,
    );
    assert!(
        outcome.warned_about(key),
        "expected the registry to warn about `{key}`; it logged:\n{:#?}",
        outcome.warnings,
    );
}

/// STRIP case, fired first: a handler that writes `inputRequests` onto its own
/// `tools/call` result is forging an MRTR/tasks continuation field, and the
/// registry must still delete it.
///
/// This is the property the fix must NOT trade away. "Always allow
/// `inputRequests`" would make test 2 pass and hand every handler the ability to
/// mint an input-request set — the grant is per-OWNER, never per-key-globally
/// (T-114-45).
#[test]
fn handler_supplied_input_requests_is_still_stripped() {
    let outcome = v2_result_envelope(handler_forged_tool_result(), ReservedFieldEgress::NoEgress);

    assert_key_stripped_with_warning(&outcome, RESERVED_INPUT_REQUESTS);
    assert!(
        outcome.bytes.contains("\"resultType\":\"complete\""),
        "an unowned result is still a complete one: {}",
        outcome.bytes,
    );
}

/// SURVIVE case: the v2 tasks dispatch is a SECOND legitimate minter of
/// `inputRequests`, and the key the schema marks REQUIRED must reach the wire.
///
/// Against the pre-fix tree this test FAILS: the registry derived ownership from
/// the disposition, this result's disposition is `complete`, so the required key
/// was silently deleted (T-114-46, `114-SPEC-RECHECK.md` row 23).
#[test]
fn tasks_minted_input_requests_survives_egress() {
    let outcome = v2_result_envelope(
        input_required_task_result(),
        ReservedFieldEgress::TasksDispatch,
    );

    assert_key_present(&outcome, RESERVED_INPUT_REQUESTS);
    assert!(
        outcome.bytes.contains("\"resultType\":\"complete\""),
        "a tasks/get on an input_required task is a COMPLETE result: {}",
        outcome.bytes,
    );
    assert!(
        !outcome.warned_about(RESERVED_INPUT_REQUESTS),
        "the registry must not even consider removing a key this egress minted; it logged:\n{:#?}",
        outcome.warnings,
    );
}
