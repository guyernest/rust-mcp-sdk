//! SEP-1686 task-augmented tool-result DX (Phase 104, TOUT-01/TOUT-03).
//!
//! Covers the client-side surface that lets integrators stop hand-reading
//! `result._meta[related-task]` and stop hand-copying poll fields:
//!
//! - `CallToolResult::with_related_task` / `related_task` twins keyed by
//!   `RELATED_TASK_META_KEY` (accessor round-trip, minimal-shape tolerance,
//!   no-`_meta` -> `None`).
//! - `Client::wait_for_task` / `wait_for_related_task` polling convenience that
//!   composes directly from `TaskMetadata` via `WaitForTaskOptions`.

use pmcp::types::tasks::{TaskMetadata, RELATED_TASK_META_KEY};
use pmcp::types::CallToolResult;

// ---------------------------------------------------------------------------
// Task 2: CallToolResult::with_related_task / related_task accessor twins.
// ---------------------------------------------------------------------------

#[test]
fn related_task_round_trip_recovers_task_id() {
    let meta = TaskMetadata::new("t9")
        .with_poll_interval(1500)
        .with_max_poll_duration_secs(60);
    let result = CallToolResult::new(vec![]).with_related_task(meta);

    let recovered = result
        .related_task()
        .expect("related_task must recover the attached metadata");
    assert_eq!(recovered.task_id, "t9");
    assert_eq!(recovered.poll_interval, Some(1500));
    assert_eq!(recovered.max_poll_duration_secs, Some(60));
}

#[test]
fn related_task_none_when_no_meta() {
    let result = CallToolResult::new(vec![]);
    assert!(
        result.related_task().is_none(),
        "a result with no _meta must yield None"
    );
}

#[test]
fn related_task_tolerates_minimal_shape() {
    // Server emitted only the minimal { taskId } native shape under the key.
    let mut meta_map = serde_json::Map::new();
    meta_map.insert(
        RELATED_TASK_META_KEY.to_string(),
        serde_json::json!({ "taskId": "t9" }),
    );
    let result = CallToolResult::new(vec![]).with_meta(meta_map);

    let recovered = result
        .related_task()
        .expect("minimal {taskId} shape must still yield Some");
    assert_eq!(recovered.task_id, "t9");
    assert_eq!(recovered.poll_interval, None);
    assert_eq!(recovered.max_poll_duration_secs, None);
}

#[test]
fn related_task_none_on_malformed_value() {
    // A malformed related-task value must not panic — returns None.
    let mut meta_map = serde_json::Map::new();
    meta_map.insert(
        RELATED_TASK_META_KEY.to_string(),
        serde_json::json!("not-an-object"),
    );
    let result = CallToolResult::new(vec![]).with_meta(meta_map);
    assert!(
        result.related_task().is_none(),
        "malformed _meta[related-task] must yield None, not panic"
    );
}
