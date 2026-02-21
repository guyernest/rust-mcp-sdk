//! Constants for MCP Tasks protocol meta keys and method names.
//!
//! These constants ensure consistent use of protocol-defined strings
//! across the crate. Method name constants match the JSON-RPC `method`
//! field values defined by the MCP 2025-11-25 Tasks specification.

// === Meta Key Constants ===

/// Meta key for related-task metadata on `tasks/result` responses.
///
/// Per the MCP spec, this key links a tool result back to its
/// originating task: `{ "io.modelcontextprotocol/related-task": { "taskId": "..." } }`.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";

/// Meta key for model-immediate-response on `CreateTaskResult._meta`.
///
/// Per the MCP spec, this key provides an immediate result for the model
/// to use while the task continues running asynchronously.
pub const MODEL_IMMEDIATE_RESPONSE_META_KEY: &str =
    "io.modelcontextprotocol/model-immediate-response";

// === Method Name Constants ===

/// JSON-RPC method name for retrieving a task's current status.
pub const METHOD_TASKS_GET: &str = "tasks/get";

/// JSON-RPC method name for retrieving a task's final result (blocks until terminal).
pub const METHOD_TASKS_RESULT: &str = "tasks/result";

/// JSON-RPC method name for listing tasks (paginated).
pub const METHOD_TASKS_LIST: &str = "tasks/list";

/// JSON-RPC method name for cancelling a task.
pub const METHOD_TASKS_CANCEL: &str = "tasks/cancel";

/// JSON-RPC method name for task status change notifications.
pub const METHOD_TASKS_STATUS_NOTIFICATION: &str = "notifications/tasks/status";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_key_values() {
        assert_eq!(
            RELATED_TASK_META_KEY,
            "io.modelcontextprotocol/related-task"
        );
        assert_eq!(
            MODEL_IMMEDIATE_RESPONSE_META_KEY,
            "io.modelcontextprotocol/model-immediate-response"
        );
    }

    #[test]
    fn method_name_values() {
        assert_eq!(METHOD_TASKS_GET, "tasks/get");
        assert_eq!(METHOD_TASKS_RESULT, "tasks/result");
        assert_eq!(METHOD_TASKS_LIST, "tasks/list");
        assert_eq!(METHOD_TASKS_CANCEL, "tasks/cancel");
        assert_eq!(
            METHOD_TASKS_STATUS_NOTIFICATION,
            "notifications/tasks/status"
        );
    }
}
