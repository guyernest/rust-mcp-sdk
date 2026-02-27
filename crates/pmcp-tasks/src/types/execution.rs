//! Task execution metadata types for tool configuration.
//!
//! These types allow tools to declare their task support level via
//! the `execution` field on `ToolInfo`. The server uses this to
//! determine whether a tool call can be augmented with task lifecycle.

use serde::{Deserialize, Serialize};

/// Declares a tool's level of task support.
///
/// This value appears in the `execution.taskSupport` field of
/// `ToolInfo` to indicate whether the tool can participate in
/// task lifecycle management.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::TaskSupport;
/// use serde_json;
///
/// // Default is Forbidden
/// let support = TaskSupport::default();
/// assert_eq!(serde_json::to_value(&support).unwrap(), "forbidden");
///
/// // Required means clients MUST use task augmentation
/// let support = TaskSupport::Required;
/// assert_eq!(serde_json::to_value(&support).unwrap(), "required");
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskSupport {
    /// Tool does not support tasks. Calls with task augmentation will fail.
    #[default]
    Forbidden,
    /// Tool optionally supports tasks. Clients may or may not use task augmentation.
    Optional,
    /// Tool requires task augmentation. Clients MUST include task params.
    Required,
}

/// Execution metadata for a tool, specifying task support.
///
/// This struct is placed in the `execution` field of
/// `ToolInfo` to declare whether the tool supports
/// task lifecycle management.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::{ToolExecution, TaskSupport};
/// use serde_json;
///
/// let execution = ToolExecution {
///     task_support: TaskSupport::Optional,
/// };
/// let json = serde_json::to_value(&execution).unwrap();
/// assert_eq!(json["taskSupport"], "optional");
///
/// // Default task_support is Forbidden
/// let execution = ToolExecution::default();
/// let json = serde_json::to_value(&execution).unwrap();
/// assert_eq!(json["taskSupport"], "forbidden");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecution {
    /// The tool's level of task support.
    #[serde(default)]
    pub task_support: TaskSupport,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_support_serialization() {
        assert_eq!(
            serde_json::to_value(TaskSupport::Forbidden).unwrap(),
            "forbidden"
        );
        assert_eq!(
            serde_json::to_value(TaskSupport::Optional).unwrap(),
            "optional"
        );
        assert_eq!(
            serde_json::to_value(TaskSupport::Required).unwrap(),
            "required"
        );
    }

    #[test]
    fn task_support_default_is_forbidden() {
        assert_eq!(TaskSupport::default(), TaskSupport::Forbidden);
    }

    #[test]
    fn tool_execution_serialization() {
        let execution = ToolExecution {
            task_support: TaskSupport::Required,
        };
        let json = serde_json::to_value(&execution).unwrap();
        assert_eq!(json["taskSupport"], "required");
    }

    #[test]
    fn tool_execution_default() {
        let execution = ToolExecution::default();
        let json = serde_json::to_value(&execution).unwrap();
        assert_eq!(json["taskSupport"], "forbidden");
    }

    #[test]
    fn task_support_round_trip() {
        for support in [
            TaskSupport::Forbidden,
            TaskSupport::Optional,
            TaskSupport::Required,
        ] {
            let json = serde_json::to_value(support).unwrap();
            let back: TaskSupport = serde_json::from_value(json).unwrap();
            assert_eq!(support, back);
        }
    }

    #[test]
    fn tool_execution_deserialization_with_default() {
        // When taskSupport is missing, it should default to Forbidden
        let json_str = r#"{}"#;
        let execution: ToolExecution = serde_json::from_str(json_str).unwrap();
        assert_eq!(execution.task_support, TaskSupport::Forbidden);
    }
}
