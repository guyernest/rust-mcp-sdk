//! MCP Task protocol types (2025-11-25).
//!
//! This module contains the wire types for MCP Tasks as defined
//! in the 2025-11-25 protocol version.

use serde::{Deserialize, Serialize};

/// Related task metadata key per MCP spec.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";

/// Task status (5-value enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is actively being worked on
    Working,
    /// Task requires user input to continue
    InputRequired,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was cancelled
    Cancelled,
}

/// A task resource representing an in-progress or completed operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique task identifier
    pub task_id: String,
    /// Current task status
    pub status: TaskStatus,
    /// Time-to-live in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
    /// ISO 8601 creation timestamp
    pub created_at: String,
    /// ISO 8601 last-updated timestamp
    pub last_updated_at: String,
    /// Suggested polling interval in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
    /// Human-readable status message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
}

/// Parameters for task creation (augments tools/call).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreationParams {
    /// Time-to-live in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
    /// Suggested polling interval in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

/// Task metadata for related-task references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedTaskMetadata {
    /// The referenced task ID
    pub task_id: String,
}

/// Result of creating a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    /// The created task
    pub task: Task,
}

/// Task status notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusNotification {
    /// Task with updated status
    pub task: Task,
}

/// Get task request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskRequest {
    /// Task ID to retrieve
    pub task_id: String,
}

/// Get task result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskResult {
    /// The requested task
    pub task: Task,
}

/// Get task payload request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTaskPayloadRequest {
    /// Task ID whose payload to retrieve
    pub task_id: String,
}

/// List tasks request (paginated).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksRequest {
    /// Pagination cursor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

/// List tasks result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksResult {
    /// List of tasks
    pub tasks: Vec<Task>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Cancel task request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    /// Task ID to cancel
    pub task_id: String,
}

/// Cancel task result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskResult {
    /// The cancelled task
    pub task: Task,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_status_serialization() {
        assert_eq!(
            serde_json::to_value(TaskStatus::Working).unwrap(),
            "working"
        );
        assert_eq!(
            serde_json::to_value(TaskStatus::InputRequired).unwrap(),
            "input_required"
        );
        assert_eq!(
            serde_json::to_value(TaskStatus::Completed).unwrap(),
            "completed"
        );
        assert_eq!(serde_json::to_value(TaskStatus::Failed).unwrap(), "failed");
        assert_eq!(
            serde_json::to_value(TaskStatus::Cancelled).unwrap(),
            "cancelled"
        );
    }

    #[test]
    fn task_roundtrip() {
        let task = Task {
            task_id: "t-123".to_string(),
            status: TaskStatus::Working,
            ttl: Some(60000),
            created_at: "2025-11-25T00:00:00Z".to_string(),
            last_updated_at: "2025-11-25T00:01:00Z".to_string(),
            poll_interval: Some(5000),
            status_message: Some("Processing...".to_string()),
        };
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["taskId"], "t-123");
        assert_eq!(json["status"], "working");
        assert_eq!(json["ttl"], 60000);
        assert_eq!(json["createdAt"], "2025-11-25T00:00:00Z");
        assert_eq!(json["pollInterval"], 5000);

        let roundtrip: Task = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.task_id, "t-123");
        assert_eq!(roundtrip.status, TaskStatus::Working);
    }

    #[test]
    fn create_task_result_roundtrip() {
        let result = CreateTaskResult {
            task: Task {
                task_id: "t-456".to_string(),
                status: TaskStatus::Completed,
                ttl: None,
                created_at: "2025-11-25T00:00:00Z".to_string(),
                last_updated_at: "2025-11-25T00:05:00Z".to_string(),
                poll_interval: None,
                status_message: None,
            },
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["task"]["taskId"], "t-456");
        assert_eq!(json["task"]["status"], "completed");

        let roundtrip: CreateTaskResult = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.task.status, TaskStatus::Completed);
    }

    #[test]
    fn task_ts_format_interop() {
        // Test deserialization from TypeScript-format JSON
        let ts_json = json!({
            "taskId": "task-abc",
            "status": "input_required",
            "createdAt": "2025-11-25T12:00:00.000Z",
            "lastUpdatedAt": "2025-11-25T12:01:00.000Z",
            "pollInterval": 3000,
            "statusMessage": "Waiting for user input"
        });
        let task: Task = serde_json::from_value(ts_json).unwrap();
        assert_eq!(task.task_id, "task-abc");
        assert_eq!(task.status, TaskStatus::InputRequired);
        assert_eq!(task.poll_interval, Some(3000));
    }

    #[test]
    fn related_task_meta_key_value() {
        assert_eq!(
            RELATED_TASK_META_KEY,
            "io.modelcontextprotocol/related-task"
        );
    }
}
