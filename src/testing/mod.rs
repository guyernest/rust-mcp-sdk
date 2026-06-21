//! Conformance helpers for proving the `tasks/*` wire surface round-trips
//! through the SDK's real client deserialization types.
//!
//! **Why this module exists.** The tools-as-tasks incident (Phase 101) was
//! caused by hand-written `tasks/*` JSON diverging from the typed structs the
//! client deserializes into. A regression test fed the helper an
//! *author-written fixture* and passed green while the live wire path failed.
//! The lesson, adopted as an acceptance gate: for protocol-shape requirements,
//! "resolved" is gated on feeding the **actual server dispatch output** through
//! the real client type, never a hand-built value.
//!
//! [`assert_roundtrips_through_client`] is the primitive that enforces this. It
//! is feature-gated behind `testing` (folded into the `full` feature set) so it
//! is available to the integration tests / examples and the quality gate, but
//! omitted from lean default release builds.

use serde::de::DeserializeOwned;

/// Assert that real server dispatch output deserializes into the client type
/// `T`, panicking with a diagnostic message if it does not.
///
/// Feed this the `serde_json::Value` carried by an actual
/// `ResponsePayload::Result(..)` produced by
/// [`ServerCore::handle_request`](crate::server::core::ProtocolHandler::handle_request)
/// — **never** an author-written fixture. If `real_dispatch_output` does not
/// deserialize into `T`, the call panics with a message naming `T`, the serde
/// error, and the pretty-printed offending output.
///
/// # Type Parameters
///
/// - `T`: a client-facing wire type such as
///   [`GetTaskResult`](crate::types::tasks::GetTaskResult),
///   [`CallToolResult`](crate::types::CallToolResult), or
///   [`CreateTaskResult`](crate::types::tasks::CreateTaskResult). Because serde
///   ignores unknown fields by default, the extra `_meta` carried by the
///   create envelope deserializes cleanly into `CreateTaskResult`.
///
/// # Panics
///
/// Panics if `real_dispatch_output` cannot be deserialized into `T`. This is
/// the intended behavior in a test context: a deliberately-wrong wire shape
/// (e.g. a flat `Task` where a `{ "task": ... }` wrapper is expected) makes the
/// helper fail loudly.
///
/// # Examples
///
/// ```rust
/// use pmcp::testing::assert_roundtrips_through_client;
/// use pmcp::types::tasks::{GetTaskResult, Task, TaskStatus};
///
/// // A correctly-shaped `tasks/get` payload wraps the task under `task`.
/// let task = Task::new("t-1", TaskStatus::Working)
///     .with_timestamps("2026-06-21T00:00:00Z", "2026-06-21T00:00:00Z");
/// let dispatch_output = serde_json::to_value(GetTaskResult::new(task)).unwrap();
///
/// // Deserializes cleanly into the client type — returns normally.
/// assert_roundtrips_through_client::<GetTaskResult>(dispatch_output);
/// ```
pub fn assert_roundtrips_through_client<T>(real_dispatch_output: serde_json::Value)
where
    T: DeserializeOwned,
{
    // Pre-render the diagnostic before moving the owned value into `from_value`
    // (the value is consumed by the deserialize, so it is genuinely owned — not
    // merely borrowed).
    let pretty = serde_json::to_string_pretty(&real_dispatch_output)
        .unwrap_or_else(|_| real_dispatch_output.to_string());
    if let Err(error) = serde_json::from_value::<T>(real_dispatch_output) {
        panic!(
            "dispatch output does not deserialize into `{}`: {error}\noffending output was:\n{pretty}",
            std::any::type_name::<T>(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::assert_roundtrips_through_client;
    use crate::types::tasks::{CreateTaskResult, GetTaskResult, Task, TaskStatus};

    fn sample_task() -> Task {
        Task::new("t-positive", TaskStatus::Working)
            .with_timestamps("2026-06-21T00:00:00Z", "2026-06-21T00:00:00Z")
    }

    #[test]
    fn passes_on_valid_get_task_result() {
        let value = serde_json::to_value(GetTaskResult::new(sample_task())).unwrap();
        assert_roundtrips_through_client::<GetTaskResult>(value);
    }

    #[test]
    fn passes_on_valid_create_task_result() {
        // The create envelope serializes as `{ "task": { .. } }`; serde ignores
        // any extra `_meta` the live dispatch envelope also carries.
        let mut value = serde_json::to_value(CreateTaskResult::new(sample_task())).unwrap();
        value.as_object_mut().unwrap().insert(
            "_meta".to_string(),
            serde_json::json!({ "io.modelcontextprotocol/related-task": { "taskId": "t-positive" } }),
        );
        assert_roundtrips_through_client::<CreateTaskResult>(value);
    }

    #[test]
    #[should_panic(expected = "does not deserialize into")]
    fn panics_on_exact_historical_flat_task_shape() {
        // The EXACT historical bug: a serialized `Task` with top-level `taskId`
        // / `status`, NOT wrapped in `{ "task": ... }`. Feeding this into
        // `GetTaskResult` (which requires the `task` wrapper) must panic.
        let flat_task = serde_json::to_value(Task::new("t-1", TaskStatus::Working)).unwrap();
        // Sanity: the flat shape really does have a top-level `taskId`.
        assert!(flat_task.get("taskId").is_some());
        assert!(flat_task.get("task").is_none());
        assert_roundtrips_through_client::<GetTaskResult>(flat_task);
    }
}
