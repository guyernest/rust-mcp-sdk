//! Task status notification type for the MCP Tasks protocol.
//!
//! The server sends `notifications/tasks/status` to inform the client
//! of task status changes. This notification uses the same field structure
//! as [`Task`](crate::types::task::Task) but is a separate type because
//! it appears in notification params, not as a result.

use serde::{Deserialize, Serialize};

use crate::types::task::TaskStatus;

/// Parameters for `notifications/tasks/status` notifications.
///
/// Sent by the server when a task's status changes. The fields mirror
/// the [`Task`](crate::types::task::Task) wire type.
///
/// # Serialization
///
/// - `ttl` is required but nullable: serializes as `null` when `None`.
/// - `poll_interval` and `status_message` are optional, omitted when `None`.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::{TaskStatusNotification, TaskStatus};
/// use serde_json;
///
/// let notification = TaskStatusNotification {
///     task_id: "task-42".to_string(),
///     status: TaskStatus::Completed,
///     status_message: Some("All done".to_string()),
///     created_at: "2025-11-25T10:30:00Z".to_string(),
///     last_updated_at: "2025-11-25T10:35:00Z".to_string(),
///     ttl: None,
///     poll_interval: None,
/// };
///
/// let json = serde_json::to_value(&notification).unwrap();
/// assert_eq!(json["taskId"], "task-42");
/// assert_eq!(json["status"], "completed");
/// assert!(json["ttl"].is_null());
/// assert!(json.get("pollInterval").is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusNotification {
    /// The task whose status changed.
    pub task_id: String,

    /// The new status of the task.
    pub status: TaskStatus,

    /// Optional human-readable status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// ISO 8601 timestamp when the task was created.
    pub created_at: String,

    /// ISO 8601 timestamp when the task was last updated.
    pub last_updated_at: String,

    /// Time-to-live in milliseconds. Required but nullable per spec:
    /// `None` serializes as `null` (unlimited TTL).
    pub ttl: Option<u64>,

    /// Suggested polling interval in milliseconds. Omitted when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::task::TaskStatus;

    #[test]
    fn notification_serialization() {
        let notification = TaskStatusNotification {
            task_id: "task-42".to_string(),
            status: TaskStatus::Working,
            status_message: Some("Processing...".to_string()),
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:32:00Z".to_string(),
            ttl: Some(60000),
            poll_interval: Some(5000),
        };

        let json = serde_json::to_value(&notification).unwrap();
        assert_eq!(json["taskId"], "task-42");
        assert_eq!(json["status"], "working");
        assert_eq!(json["statusMessage"], "Processing...");
        assert_eq!(json["createdAt"], "2025-11-25T10:30:00Z");
        assert_eq!(json["lastUpdatedAt"], "2025-11-25T10:32:00Z");
        assert_eq!(json["ttl"], 60000);
        assert_eq!(json["pollInterval"], 5000);
    }

    #[test]
    fn notification_ttl_null_serialization() {
        let notification = TaskStatusNotification {
            task_id: "task-99".to_string(),
            status: TaskStatus::Completed,
            status_message: None,
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:35:00Z".to_string(),
            ttl: None,
            poll_interval: None,
        };

        let json = serde_json::to_value(&notification).unwrap();
        // ttl must be present as null
        assert!(json.get("ttl").is_some());
        assert!(json["ttl"].is_null());
        // Optional fields omitted
        assert!(json.get("statusMessage").is_none());
        assert!(json.get("pollInterval").is_none());
    }

    #[test]
    fn notification_round_trip() {
        let original = TaskStatusNotification {
            task_id: "rt-notify".to_string(),
            status: TaskStatus::InputRequired,
            status_message: Some("Need user input".to_string()),
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:33:00Z".to_string(),
            ttl: Some(120000),
            poll_interval: None,
        };

        let json_str = serde_json::to_string(&original).unwrap();
        let back: TaskStatusNotification = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.task_id, "rt-notify");
        assert_eq!(back.status, TaskStatus::InputRequired);
        assert_eq!(back.ttl, Some(120000));
    }
}
