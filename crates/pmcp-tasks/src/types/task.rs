//! Core task wire types for the MCP Tasks protocol.
//!
//! This module defines the primary task types that appear on the wire:
//! [`Task`], [`TaskStatus`], [`CreateTaskResult`], [`GetTaskResult`],
//! and [`CancelTaskResult`].
//!
//! # Serialization
//!
//! All types use `#[serde(rename_all = "camelCase")]` to match the MCP
//! specification's JSON field naming. The `ttl` field serializes as `null`
//! (not omitted) when `None`, matching the spec's `number | null` type.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

use crate::constants::RELATED_TASK_META_KEY;
use crate::error::TaskError;

/// Task lifecycle status per the MCP 2025-11-25 specification.
///
/// A task progresses through these states according to a defined state
/// machine. Terminal states (`Completed`, `Failed`, `Cancelled`) reject
/// all transitions. Self-transitions are rejected.
///
/// # State Machine
///
/// ```text
/// Working -> InputRequired, Completed, Failed, Cancelled
/// InputRequired -> Working, Completed, Failed, Cancelled
/// Completed -> (terminal, no transitions)
/// Failed -> (terminal, no transitions)
/// Cancelled -> (terminal, no transitions)
/// ```
///
/// # Examples
///
/// ```
/// use pmcp_tasks::TaskStatus;
///
/// let status = TaskStatus::Working;
/// assert!(!status.is_terminal());
/// assert!(status.can_transition_to(&TaskStatus::Completed));
/// assert!(!status.can_transition_to(&TaskStatus::Working)); // self-transition rejected
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is actively being processed.
    Working,
    /// Task requires input from the client before it can proceed.
    InputRequired,
    /// Task completed successfully (terminal).
    Completed,
    /// Task failed (terminal).
    Failed,
    /// Task was cancelled (terminal).
    Cancelled,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Working => write!(f, "working"),
            Self::InputRequired => write!(f, "input_required"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl TaskStatus {
    /// Returns `true` if this status is terminal (no further transitions allowed).
    ///
    /// Terminal states are `Completed`, `Failed`, and `Cancelled`.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::TaskStatus;
    ///
    /// assert!(!TaskStatus::Working.is_terminal());
    /// assert!(!TaskStatus::InputRequired.is_terminal());
    /// assert!(TaskStatus::Completed.is_terminal());
    /// assert!(TaskStatus::Failed.is_terminal());
    /// assert!(TaskStatus::Cancelled.is_terminal());
    /// ```
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Returns `true` if transitioning from this status to `next` is valid.
    ///
    /// The MCP spec defines these valid transitions:
    /// - `Working` -> `InputRequired`, `Completed`, `Failed`, `Cancelled`
    /// - `InputRequired` -> `Working`, `Completed`, `Failed`, `Cancelled`
    /// - Terminal states -> no transitions allowed
    ///
    /// Self-transitions (e.g., `Working` -> `Working`) are rejected per spec.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::TaskStatus;
    ///
    /// assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Completed));
    /// assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Working));
    /// assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Working));
    /// assert!(!TaskStatus::Working.can_transition_to(&TaskStatus::Working));
    /// ```
    pub fn can_transition_to(&self, next: &Self) -> bool {
        if self == next {
            return false;
        }

        match self {
            Self::Working => matches!(
                next,
                Self::InputRequired | Self::Completed | Self::Failed | Self::Cancelled
            ),
            Self::InputRequired => matches!(
                next,
                Self::Working | Self::Completed | Self::Failed | Self::Cancelled
            ),
            Self::Completed | Self::Failed | Self::Cancelled => false,
        }
    }

    /// Validates a transition from this status to `next`.
    ///
    /// Returns `Ok(())` if the transition is valid, or a [`TaskError::InvalidTransition`]
    /// with context about the rejected transition.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::TaskStatus;
    ///
    /// let result = TaskStatus::Working.validate_transition(
    ///     "task-123",
    ///     &TaskStatus::Completed,
    /// );
    /// assert!(result.is_ok());
    ///
    /// let result = TaskStatus::Completed.validate_transition(
    ///     "task-123",
    ///     &TaskStatus::Working,
    /// );
    /// assert!(result.is_err());
    /// ```
    pub fn validate_transition(&self, task_id: &str, next: &Self) -> Result<(), TaskError> {
        if self.can_transition_to(next) {
            Ok(())
        } else {
            let suggested_action = if self.is_terminal() {
                Some("task is in a terminal state and cannot be transitioned".to_string())
            } else if self == next {
                Some(format!("task is already in {self} state"))
            } else {
                None
            };

            Err(TaskError::InvalidTransition {
                task_id: task_id.to_string(),
                from: *self,
                to: *next,
                suggested_action,
            })
        }
    }
}

/// A task representing a long-running operation in the MCP protocol.
///
/// This is the wire type that serializes to match the MCP 2025-11-25 spec
/// exactly. Domain extensions (variables, owner) belong on
/// [`TaskRecord`](crate::domain) instead.
///
/// # Serialization
///
/// - Fields use `camelCase` naming.
/// - `ttl` is required but nullable (`number | null` in spec). When `None`,
///   it serializes as `null`, not omitted.
/// - `poll_interval` and `status_message` are optional and omitted when `None`.
/// - `_meta` is optional and omitted when `None`.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::{Task, TaskStatus};
/// use serde_json;
///
/// let task = Task {
///     task_id: "abc-123".to_string(),
///     status: TaskStatus::Working,
///     status_message: Some("Processing data".to_string()),
///     created_at: "2025-11-25T10:30:00Z".to_string(),
///     last_updated_at: "2025-11-25T10:30:00Z".to_string(),
///     ttl: Some(60000),
///     poll_interval: Some(5000),
///     _meta: None,
/// };
///
/// let json = serde_json::to_value(&task).unwrap();
/// assert_eq!(json["taskId"], "abc-123");
/// assert_eq!(json["status"], "working");
/// assert_eq!(json["ttl"], 60000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique identifier for this task.
    pub task_id: String,

    /// Current lifecycle status.
    pub status: TaskStatus,

    /// Optional human-readable status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,

    /// ISO 8601 timestamp when the task was created.
    pub created_at: String,

    /// ISO 8601 timestamp when the task was last updated.
    pub last_updated_at: String,

    /// Time-to-live in milliseconds. Required but nullable per spec:
    /// `None` serializes as `null` (unlimited TTL), `Some(ms)` as a number.
    pub ttl: Option<u64>,

    /// Suggested polling interval in milliseconds. Omitted when `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,

    /// Optional metadata map. Used for `_meta` on result responses
    /// (e.g., related-task metadata, variables injection).
    #[serde(rename = "_meta")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<Map<String, Value>>,
}

/// Result of creating a new task via `tools/call` with task augmentation.
///
/// Unlike [`GetTaskResult`] and [`CancelTaskResult`], the task is wrapped
/// in a `task` field per the MCP spec (`CreateTaskResult = Result & { task: Task }`).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::{CreateTaskResult, Task, TaskStatus};
/// use serde_json;
///
/// let result = CreateTaskResult {
///     task: Task {
///         task_id: "new-task-1".to_string(),
///         status: TaskStatus::Working,
///         status_message: None,
///         created_at: "2025-11-25T10:30:00Z".to_string(),
///         last_updated_at: "2025-11-25T10:30:00Z".to_string(),
///         ttl: Some(30000),
///         poll_interval: None,
///         _meta: None,
///     },
///     _meta: None,
/// };
///
/// let json = serde_json::to_value(&result).unwrap();
/// assert_eq!(json["task"]["taskId"], "new-task-1");
/// assert_eq!(json["task"]["status"], "working");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    /// The created task.
    pub task: Task,

    /// Optional result-level metadata.
    #[serde(rename = "_meta")]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[allow(clippy::pub_underscore_fields)]
    pub _meta: Option<Map<String, Value>>,
}

/// Result of `tasks/get` -- flat task fields at the result level.
///
/// Per the MCP spec, `GetTaskResult = Result & Task` (no wrapper).
/// The task fields ARE the result.
pub type GetTaskResult = Task;

/// Result of `tasks/cancel` -- flat task fields at the result level.
///
/// Per the MCP spec, `CancelTaskResult = Result & Task` (no wrapper).
/// The task fields ARE the result.
pub type CancelTaskResult = Task;

/// Produces the `_meta` map entry for related-task metadata.
///
/// The MCP spec requires `_meta` on `tasks/result` responses to include
/// the `io.modelcontextprotocol/related-task` key linking the result
/// to the originating task.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::related_task_meta;
///
/// let meta = related_task_meta("task-xyz");
/// let json = serde_json::to_value(&meta).unwrap();
/// assert_eq!(
///     json["io.modelcontextprotocol/related-task"]["taskId"],
///     "task-xyz"
/// );
/// ```
pub fn related_task_meta(task_id: &str) -> Map<String, Value> {
    let mut inner = Map::new();
    inner.insert("taskId".to_string(), Value::String(task_id.to_string()));

    let mut meta = Map::new();
    meta.insert(RELATED_TASK_META_KEY.to_string(), Value::Object(inner));

    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_display_matches_serde() {
        assert_eq!(TaskStatus::Working.to_string(), "working");
        assert_eq!(TaskStatus::InputRequired.to_string(), "input_required");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Failed.to_string(), "failed");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
    }

    #[test]
    fn task_status_serde_round_trip() {
        for status in [
            TaskStatus::Working,
            TaskStatus::InputRequired,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            let json = serde_json::to_value(status).unwrap();
            let back: TaskStatus = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(status, back, "round-trip failed for {status}");
        }
    }

    #[test]
    fn task_status_serializes_snake_case() {
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
    fn terminal_states() {
        assert!(!TaskStatus::Working.is_terminal());
        assert!(!TaskStatus::InputRequired.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
    }

    #[test]
    fn valid_transitions_from_working() {
        let working = TaskStatus::Working;
        assert!(working.can_transition_to(&TaskStatus::InputRequired));
        assert!(working.can_transition_to(&TaskStatus::Completed));
        assert!(working.can_transition_to(&TaskStatus::Failed));
        assert!(working.can_transition_to(&TaskStatus::Cancelled));
        assert!(!working.can_transition_to(&TaskStatus::Working));
    }

    #[test]
    fn valid_transitions_from_input_required() {
        let input = TaskStatus::InputRequired;
        assert!(input.can_transition_to(&TaskStatus::Working));
        assert!(input.can_transition_to(&TaskStatus::Completed));
        assert!(input.can_transition_to(&TaskStatus::Failed));
        assert!(input.can_transition_to(&TaskStatus::Cancelled));
        assert!(!input.can_transition_to(&TaskStatus::InputRequired));
    }

    #[test]
    fn terminal_states_reject_all_transitions() {
        for terminal in [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            for target in [
                TaskStatus::Working,
                TaskStatus::InputRequired,
                TaskStatus::Completed,
                TaskStatus::Failed,
                TaskStatus::Cancelled,
            ] {
                assert!(
                    !terminal.can_transition_to(&target),
                    "{terminal} should not transition to {target}"
                );
            }
        }
    }

    #[test]
    fn validate_transition_ok() {
        let result = TaskStatus::Working.validate_transition("task-1", &TaskStatus::Completed);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_transition_err_terminal() {
        let result = TaskStatus::Completed.validate_transition("task-1", &TaskStatus::Working);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("task-1"),
            "error should include task_id"
        );
    }

    #[test]
    fn validate_transition_err_self() {
        let result = TaskStatus::Working.validate_transition("task-2", &TaskStatus::Working);
        assert!(result.is_err());
    }

    #[test]
    fn task_serialization_with_ttl() {
        let task = Task {
            task_id: "786512e2-9e0d-44bd-8f29-789f320fe840".to_string(),
            status: TaskStatus::Working,
            status_message: Some("The operation is now in progress.".to_string()),
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:40:00Z".to_string(),
            ttl: Some(60000),
            poll_interval: Some(5000),
            _meta: None,
        };

        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["taskId"], "786512e2-9e0d-44bd-8f29-789f320fe840");
        assert_eq!(json["status"], "working");
        assert_eq!(json["statusMessage"], "The operation is now in progress.");
        assert_eq!(json["createdAt"], "2025-11-25T10:30:00Z");
        assert_eq!(json["lastUpdatedAt"], "2025-11-25T10:40:00Z");
        assert_eq!(json["ttl"], 60000);
        assert_eq!(json["pollInterval"], 5000);
        assert!(json.get("_meta").is_none());
    }

    #[test]
    fn task_ttl_null_serialization() {
        let task = Task {
            task_id: "test-id".to_string(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:30:00Z".to_string(),
            ttl: None,
            poll_interval: None,
            _meta: None,
        };

        let json = serde_json::to_value(&task).unwrap();
        // ttl MUST be present as null, not omitted
        assert!(json.get("ttl").is_some(), "ttl must be present");
        assert!(json["ttl"].is_null(), "ttl must be null when None");
        // pollInterval SHOULD be omitted when None
        assert!(
            json.get("pollInterval").is_none(),
            "pollInterval should be omitted when None"
        );
        // statusMessage SHOULD be omitted when None
        assert!(
            json.get("statusMessage").is_none(),
            "statusMessage should be omitted when None"
        );
    }

    #[test]
    fn create_task_result_wraps_task() {
        let result = CreateTaskResult {
            task: Task {
                task_id: "task-abc".to_string(),
                status: TaskStatus::Working,
                status_message: None,
                created_at: "2025-11-25T10:30:00Z".to_string(),
                last_updated_at: "2025-11-25T10:30:00Z".to_string(),
                ttl: Some(60000),
                poll_interval: None,
                _meta: None,
            },
            _meta: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        // CreateTaskResult wraps task in a "task" field
        assert!(json.get("task").is_some(), "must have task field");
        assert_eq!(json["task"]["taskId"], "task-abc");
        assert_eq!(json["task"]["status"], "working");
    }

    #[test]
    fn get_task_result_is_flat() {
        // GetTaskResult is a type alias for Task, so it serializes flat
        let result: GetTaskResult = Task {
            task_id: "task-def".to_string(),
            status: TaskStatus::Completed,
            status_message: Some("Done".to_string()),
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:35:00Z".to_string(),
            ttl: None,
            poll_interval: None,
            _meta: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        // GetTaskResult is flat -- no "task" wrapper
        assert!(json.get("task").is_none(), "must NOT have task wrapper");
        assert_eq!(json["taskId"], "task-def");
        assert_eq!(json["status"], "completed");
    }

    #[test]
    fn related_task_meta_produces_correct_structure() {
        let meta = related_task_meta("task-xyz");
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(
            json["io.modelcontextprotocol/related-task"]["taskId"],
            "task-xyz"
        );
    }

    #[test]
    fn task_round_trip_deserialization() {
        let json_str = r#"{
            "taskId": "round-trip-1",
            "status": "input_required",
            "statusMessage": "Need more info",
            "createdAt": "2025-11-25T10:30:00Z",
            "lastUpdatedAt": "2025-11-25T10:35:00Z",
            "ttl": null,
            "pollInterval": 3000
        }"#;

        let task: Task = serde_json::from_str(json_str).unwrap();
        assert_eq!(task.task_id, "round-trip-1");
        assert_eq!(task.status, TaskStatus::InputRequired);
        assert_eq!(task.status_message.as_deref(), Some("Need more info"));
        assert!(task.ttl.is_none());
        assert_eq!(task.poll_interval, Some(3000));

        // Round-trip back to JSON
        let re_json = serde_json::to_value(&task).unwrap();
        assert_eq!(re_json["taskId"], "round-trip-1");
        assert_eq!(re_json["status"], "input_required");
        assert!(re_json["ttl"].is_null());
    }

    #[test]
    fn create_task_result_spec_example() {
        // Matches the spec example from the MCP Tasks documentation
        let result = CreateTaskResult {
            task: Task {
                task_id: "786512e2-9e0d-44bd-8f29-789f320fe840".to_string(),
                status: TaskStatus::Working,
                status_message: Some("The operation is now in progress.".to_string()),
                created_at: "2025-11-25T10:30:00Z".to_string(),
                last_updated_at: "2025-11-25T10:40:00Z".to_string(),
                ttl: Some(60000),
                poll_interval: Some(5000),
                _meta: None,
            },
            _meta: None,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(
            json["task"]["taskId"],
            "786512e2-9e0d-44bd-8f29-789f320fe840"
        );
        assert_eq!(json["task"]["status"], "working");
        assert_eq!(json["task"]["ttl"], 60000);
        assert_eq!(json["task"]["pollInterval"], 5000);
    }
}
