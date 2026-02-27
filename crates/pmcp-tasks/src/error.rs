//! Error types for MCP Tasks operations.
//!
//! Provides [`TaskError`], a rich error enum with context fields and
//! JSON-RPC error code mapping per the MCP specification.

use std::fmt;

use crate::types::task::TaskStatus;

/// Errors that can occur during task operations.
///
/// Each variant carries contextual information (task ID, status, etc.)
/// to aid debugging. Use [`error_code`](TaskError::error_code) to map
/// to the appropriate JSON-RPC error code for wire responses.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::{TaskError, TaskStatus};
///
/// let err = TaskError::NotFound {
///     task_id: "missing-task".to_string(),
/// };
/// assert_eq!(err.error_code(), -32602);
/// assert!(err.to_string().contains("missing-task"));
/// ```
#[derive(Debug)]
pub enum TaskError {
    /// Attempted an invalid state machine transition.
    InvalidTransition {
        /// The task that was being transitioned.
        task_id: String,
        /// The current status of the task.
        from: TaskStatus,
        /// The target status that was rejected.
        to: TaskStatus,
        /// Optional suggestion for the caller.
        suggested_action: Option<String>,
    },

    /// Task with the given ID was not found.
    NotFound {
        /// The task ID that was not found.
        task_id: String,
    },

    /// Task has expired past its TTL.
    Expired {
        /// The expired task's ID.
        task_id: String,
        /// When the task expired, if known.
        expired_at: Option<String>,
    },

    /// Task is not in a terminal state (needed for `tasks/result`).
    NotReady {
        /// The task ID.
        task_id: String,
        /// The task's current (non-terminal) status.
        current_status: TaskStatus,
    },

    /// Caller does not own this task.
    OwnerMismatch {
        /// The task ID.
        task_id: String,
    },

    /// Resource limits exceeded (e.g., too many active tasks).
    ResourceExhausted {
        /// Optional suggestion for the caller.
        suggested_action: Option<String>,
    },

    /// Variable payload exceeds configured size limit.
    VariableSizeExceeded {
        /// The configured limit in bytes.
        limit_bytes: usize,
        /// The actual payload size in bytes.
        actual_bytes: usize,
    },

    /// Concurrent modification detected via CAS failure.
    ConcurrentModification {
        /// The task ID.
        task_id: String,
        /// The version that was expected.
        expected_version: u64,
        /// The actual version found in storage.
        actual_version: u64,
    },

    /// Storage backend is full or at capacity.
    StorageFull {
        /// Description of the capacity issue.
        message: String,
    },

    /// Backend storage error.
    StoreError(String),
}

impl fmt::Display for TaskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransition {
                task_id,
                from,
                to,
                ..
            } => write!(
                f,
                "invalid transition from {from} to {to} for task {task_id}"
            ),
            Self::NotFound { task_id } => write!(f, "task not found: {task_id}"),
            Self::Expired {
                task_id,
                expired_at,
            } => {
                if let Some(at) = expired_at {
                    write!(f, "task expired: {task_id} (expired at {at})")
                } else {
                    write!(f, "task expired: {task_id}")
                }
            }
            Self::NotReady {
                task_id,
                current_status,
            } => write!(
                f,
                "task not in terminal state: {task_id} (status: {current_status})"
            ),
            Self::OwnerMismatch { task_id } => {
                write!(f, "owner mismatch for task {task_id}")
            }
            Self::ResourceExhausted { .. } => write!(f, "resource exhausted"),
            Self::VariableSizeExceeded {
                limit_bytes,
                actual_bytes,
            } => write!(
                f,
                "variable size limit exceeded: {actual_bytes} bytes exceeds {limit_bytes} byte limit"
            ),
            Self::ConcurrentModification {
                task_id,
                expected_version,
                actual_version,
            } => write!(
                f,
                "concurrent modification on task {task_id}: expected version {expected_version}, found {actual_version}"
            ),
            Self::StorageFull { message } => write!(f, "storage full: {message}"),
            Self::StoreError(msg) => write!(f, "store error: {msg}"),
        }
    }
}

impl std::error::Error for TaskError {}

impl TaskError {
    /// Maps this error to a JSON-RPC error code per the MCP specification.
    ///
    /// - `-32602` (Invalid params): `InvalidTransition`, `NotFound`, `Expired`,
    ///   `NotReady`, `OwnerMismatch`, `VariableSizeExceeded`
    /// - `-32603` (Internal error): `ResourceExhausted`, `ConcurrentModification`,
    ///   `StorageFull`, `StoreError`
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::{TaskError, TaskStatus};
    ///
    /// let err = TaskError::InvalidTransition {
    ///     task_id: "t1".to_string(),
    ///     from: TaskStatus::Completed,
    ///     to: TaskStatus::Working,
    ///     suggested_action: None,
    /// };
    /// assert_eq!(err.error_code(), -32602);
    ///
    /// let err = TaskError::StoreError("db timeout".to_string());
    /// assert_eq!(err.error_code(), -32603);
    /// ```
    pub fn error_code(&self) -> i32 {
        match self {
            Self::InvalidTransition { .. }
            | Self::NotFound { .. }
            | Self::Expired { .. }
            | Self::NotReady { .. }
            | Self::OwnerMismatch { .. }
            | Self::VariableSizeExceeded { .. } => -32602,
            Self::ResourceExhausted { .. }
            | Self::ConcurrentModification { .. }
            | Self::StorageFull { .. }
            | Self::StoreError(_) => -32603,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = TaskError::NotFound {
            task_id: "abc".to_string(),
        };
        assert_eq!(err.to_string(), "task not found: abc");

        let err = TaskError::Expired {
            task_id: "def".to_string(),
            expired_at: Some("2025-11-25T12:00:00Z".to_string()),
        };
        assert!(err.to_string().contains("def"));
        assert!(err.to_string().contains("2025-11-25T12:00:00Z"));

        let err = TaskError::Expired {
            task_id: "ghi".to_string(),
            expired_at: None,
        };
        assert_eq!(err.to_string(), "task expired: ghi");
    }

    #[test]
    fn error_codes() {
        assert_eq!(
            TaskError::InvalidTransition {
                task_id: "t".to_string(),
                from: TaskStatus::Working,
                to: TaskStatus::Working,
                suggested_action: None,
            }
            .error_code(),
            -32602
        );
        assert_eq!(
            TaskError::NotFound {
                task_id: "t".to_string()
            }
            .error_code(),
            -32602
        );
        assert_eq!(
            TaskError::ResourceExhausted {
                suggested_action: None
            }
            .error_code(),
            -32603
        );
        assert_eq!(
            TaskError::StoreError("fail".to_string()).error_code(),
            -32603
        );
    }

    #[test]
    fn concurrent_modification_display() {
        let err = TaskError::ConcurrentModification {
            task_id: "task-42".to_string(),
            expected_version: 3,
            actual_version: 5,
        };
        let msg = err.to_string();
        assert!(msg.contains("task-42"));
        assert!(msg.contains("expected version 3"));
        assert!(msg.contains("found 5"));
    }

    #[test]
    fn concurrent_modification_error_code() {
        let err = TaskError::ConcurrentModification {
            task_id: "t".to_string(),
            expected_version: 1,
            actual_version: 2,
        };
        assert_eq!(err.error_code(), -32603);
    }

    #[test]
    fn storage_full_display() {
        let err = TaskError::StorageFull {
            message: "max 10000 records reached".to_string(),
        };
        assert_eq!(err.to_string(), "storage full: max 10000 records reached");
    }

    #[test]
    fn storage_full_error_code() {
        let err = TaskError::StorageFull {
            message: "full".to_string(),
        };
        assert_eq!(err.error_code(), -32603);
    }
}
