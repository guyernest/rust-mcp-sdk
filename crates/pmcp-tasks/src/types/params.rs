//! Request parameter types for MCP Tasks protocol methods.
//!
//! These types correspond to the `params` field in JSON-RPC requests
//! for `tasks/get`, `tasks/result`, `tasks/list`, and `tasks/cancel`.
//! The [`TaskParams`] struct is used in the `task` field of `tools/call`
//! requests when task augmentation is enabled.

use serde::{Deserialize, Serialize};

/// Task parameters for `tools/call` request augmentation.
///
/// When a client sends `tools/call` with task support, this struct
/// appears in the `task` field. If `task_id` is `None`, the server
/// creates a new task. If present, it references an existing task.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::TaskParams;
/// use serde_json;
///
/// // New task creation (no task_id)
/// let params = TaskParams {
///     task_id: None,
///     ttl: Some(60000),
///     poll_interval: Some(5000),
/// };
/// let json = serde_json::to_value(&params).unwrap();
/// assert!(json.get("taskId").is_none());
/// assert_eq!(json["ttl"], 60000);
///
/// // Existing task reference
/// let params = TaskParams {
///     task_id: Some("task-123".to_string()),
///     ttl: None,
///     poll_interval: None,
/// };
/// let json = serde_json::to_value(&params).unwrap();
/// assert_eq!(json["taskId"], "task-123");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskParams {
    /// Task ID to reference. `None` for new task creation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,

    /// Time-to-live in milliseconds for the task.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,

    /// Suggested polling interval in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

/// Parameters for `tasks/get` requests.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::TaskGetParams;
/// use serde_json;
///
/// let params = TaskGetParams {
///     task_id: "abc-123".to_string(),
/// };
/// let json = serde_json::to_value(&params).unwrap();
/// assert_eq!(json["taskId"], "abc-123");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGetParams {
    /// The task ID to retrieve.
    pub task_id: String,
}

/// Parameters for `tasks/result` requests.
///
/// The server MUST block until the task reaches a terminal state
/// before responding. This is a transport/handler concern, not
/// encoded in the type itself.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::TaskResultParams;
/// use serde_json;
///
/// let params = TaskResultParams {
///     task_id: "abc-123".to_string(),
/// };
/// let json = serde_json::to_value(&params).unwrap();
/// assert_eq!(json["taskId"], "abc-123");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResultParams {
    /// The task ID whose result to retrieve.
    pub task_id: String,
}

/// Parameters for `tasks/list` requests (paginated).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::TaskListParams;
/// use serde_json;
///
/// // First page (no cursor)
/// let params = TaskListParams { cursor: None };
/// let json = serde_json::to_value(&params).unwrap();
/// assert!(json.get("cursor").is_none());
///
/// // Subsequent page
/// let params = TaskListParams {
///     cursor: Some("page-2-token".to_string()),
/// };
/// let json = serde_json::to_value(&params).unwrap();
/// assert_eq!(json["cursor"], "page-2-token");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListParams {
    /// Pagination cursor for the next page. `None` for the first page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// Parameters for `tasks/cancel` requests.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::TaskCancelParams;
/// use serde_json;
///
/// let params = TaskCancelParams {
///     task_id: "cancel-me".to_string(),
/// };
/// let json = serde_json::to_value(&params).unwrap();
/// assert_eq!(json["taskId"], "cancel-me");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCancelParams {
    /// The task ID to cancel.
    pub task_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_params_new_task() {
        let params = TaskParams {
            task_id: None,
            ttl: Some(60000),
            poll_interval: Some(5000),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("taskId").is_none());
        assert_eq!(json["ttl"], 60000);
        assert_eq!(json["pollInterval"], 5000);
    }

    #[test]
    fn task_params_existing_task() {
        let params = TaskParams {
            task_id: Some("existing-task".to_string()),
            ttl: None,
            poll_interval: None,
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["taskId"], "existing-task");
        assert!(json.get("ttl").is_none());
        assert!(json.get("pollInterval").is_none());
    }

    #[test]
    fn task_get_params_serialization() {
        let params = TaskGetParams {
            task_id: "get-me".to_string(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["taskId"], "get-me");
    }

    #[test]
    fn task_result_params_serialization() {
        let params = TaskResultParams {
            task_id: "result-me".to_string(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["taskId"], "result-me");
    }

    #[test]
    fn task_list_params_no_cursor() {
        let params = TaskListParams { cursor: None };
        let json = serde_json::to_value(&params).unwrap();
        assert!(json.get("cursor").is_none());
    }

    #[test]
    fn task_list_params_with_cursor() {
        let params = TaskListParams {
            cursor: Some("next-page".to_string()),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["cursor"], "next-page");
    }

    #[test]
    fn task_cancel_params_serialization() {
        let params = TaskCancelParams {
            task_id: "cancel-me".to_string(),
        };
        let json = serde_json::to_value(&params).unwrap();
        assert_eq!(json["taskId"], "cancel-me");
    }

    #[test]
    fn task_params_round_trip() {
        let original = TaskParams {
            task_id: Some("rt-test".to_string()),
            ttl: Some(30000),
            poll_interval: Some(2000),
        };
        let json_str = serde_json::to_string(&original).unwrap();
        let back: TaskParams = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.task_id.as_deref(), Some("rt-test"));
        assert_eq!(back.ttl, Some(30000));
        assert_eq!(back.poll_interval, Some(2000));
    }
}
