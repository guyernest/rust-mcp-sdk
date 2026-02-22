//! Workflow progress types for task-backed workflow execution.
//!
//! These types track the execution state of a [`SequentialWorkflow`] that is
//! backed by a task. The [`WorkflowProgress`] struct is serialized to the
//! task's variable store under the [`WORKFLOW_PROGRESS_KEY`] key, allowing
//! clients and servers to inspect which steps have completed, failed, or
//! remain pending.
//!
//! # Variable Key Convention
//!
//! Workflow-related task variables use the `_workflow.` prefix:
//! - `_workflow.progress` -- the full [`WorkflowProgress`] struct
//! - `_workflow.result.<step_name>` -- per-step tool result (raw JSON)
//! - `_workflow.pause_reason` -- the [`PauseReason`] when execution pauses
//!
//! # Schema Versioning
//!
//! [`WorkflowProgress`] includes a `schema_version` field (starting at 1)
//! for forward compatibility. Readers MUST tolerate unknown fields; writers
//! MUST set `schema_version` to the version they produce.

use serde::{Deserialize, Serialize};

// === Variable Key Constants ===

/// Task variable key for the structured workflow progress object.
///
/// The value stored under this key is a serialized [`WorkflowProgress`].
///
/// # Examples
///
/// ```
/// use pmcp_tasks::types::workflow::WORKFLOW_PROGRESS_KEY;
///
/// assert_eq!(WORKFLOW_PROGRESS_KEY, "_workflow.progress");
/// ```
pub const WORKFLOW_PROGRESS_KEY: &str = "_workflow.progress";

/// Task variable key for the pause reason when workflow execution pauses.
///
/// The value stored under this key is a serialized [`PauseReason`] that
/// describes why the execution engine stopped before completing all steps.
/// Clients inspect this value to decide what corrective action to take
/// (e.g., calling a suggested tool, providing missing parameters).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::types::workflow::WORKFLOW_PAUSE_REASON_KEY;
///
/// assert_eq!(WORKFLOW_PAUSE_REASON_KEY, "_workflow.pause_reason");
/// ```
pub const WORKFLOW_PAUSE_REASON_KEY: &str = "_workflow.pause_reason";

/// Prefix for per-step result variable keys.
///
/// Each completed step stores its raw tool result under
/// `_workflow.result.<step_name>`. Use [`workflow_result_key`] to build
/// the full key.
pub const WORKFLOW_RESULT_PREFIX: &str = "_workflow.result.";

/// Builds the task variable key for a step's tool result.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::types::workflow::workflow_result_key;
///
/// assert_eq!(workflow_result_key("validate"), "_workflow.result.validate");
/// assert_eq!(workflow_result_key("deploy"), "_workflow.result.deploy");
/// ```
pub fn workflow_result_key(step_name: &str) -> String {
    format!("{WORKFLOW_RESULT_PREFIX}{step_name}")
}

// === Workflow Progress Types ===

/// Tracks the execution state of a task-backed sequential workflow.
///
/// This struct is serialized to the task's variable store under the
/// [`WORKFLOW_PROGRESS_KEY`] key. It provides a snapshot of which steps
/// have been executed, their outcomes, and the workflow's overall goal.
///
/// # Serialization
///
/// Fields use `camelCase` naming to match the MCP JSON conventions.
/// The `schema_version` field starts at 1 and is incremented when the
/// schema changes in a backward-incompatible way.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::types::workflow::{WorkflowProgress, WorkflowStepProgress, StepStatus};
///
/// let progress = WorkflowProgress {
///     goal: "Deploy service to us-east-1".to_string(),
///     steps: vec![
///         WorkflowStepProgress {
///             name: "validate".to_string(),
///             tool: Some("validate_config".to_string()),
///             status: StepStatus::Completed,
///         },
///         WorkflowStepProgress {
///             name: "deploy".to_string(),
///             tool: Some("deploy_service".to_string()),
///             status: StepStatus::Pending,
///         },
///     ],
///     schema_version: 1,
/// };
///
/// let json = serde_json::to_value(&progress).unwrap();
/// assert_eq!(json["goal"], "Deploy service to us-east-1");
/// assert_eq!(json["schemaVersion"], 1);
/// assert_eq!(json["steps"][0]["status"], "completed");
/// assert_eq!(json["steps"][1]["status"], "pending");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProgress {
    /// The workflow's overall goal description.
    pub goal: String,

    /// Ordered list of steps in the workflow.
    pub steps: Vec<WorkflowStepProgress>,

    /// Schema version for forward compatibility (starts at 1).
    pub schema_version: u32,
}

/// Tracks the execution state of a single workflow step.
///
/// Each step corresponds to a [`WorkflowStep`] in the workflow definition.
/// The `tool` field is `None` for resource-only steps that don't invoke a tool.
///
/// # Serialization
///
/// The `tool` field is omitted from JSON when `None` (resource-only steps).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::types::workflow::{WorkflowStepProgress, StepStatus};
///
/// // Tool-backed step
/// let step = WorkflowStepProgress {
///     name: "validate".to_string(),
///     tool: Some("validate_config".to_string()),
///     status: StepStatus::Completed,
/// };
/// let json = serde_json::to_value(&step).unwrap();
/// assert_eq!(json["tool"], "validate_config");
///
/// // Resource-only step (no tool)
/// let step = WorkflowStepProgress {
///     name: "read_config".to_string(),
///     tool: None,
///     status: StepStatus::Pending,
/// };
/// let json = serde_json::to_value(&step).unwrap();
/// assert!(json.get("tool").is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStepProgress {
    /// Step name (from the workflow step definition).
    pub name: String,

    /// Tool name, or `None` for resource-only steps.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,

    /// Current status of this step.
    pub status: StepStatus,
}

/// Runtime outcome of a workflow step.
///
/// Steps start as [`Pending`](StepStatus::Pending) and transition to one of
/// the terminal states based on execution outcome. There is no pre-classification
/// of steps -- the server attempts best-effort execution at runtime.
///
/// # Default
///
/// The default value is [`Pending`](StepStatus::Pending).
///
/// # Serialization
///
/// Variants serialize as `snake_case` strings: `"pending"`, `"completed"`,
/// `"failed"`, `"skipped"`.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::types::workflow::StepStatus;
///
/// assert_eq!(StepStatus::default(), StepStatus::Pending);
///
/// let json = serde_json::to_value(StepStatus::Completed).unwrap();
/// assert_eq!(json, "completed");
///
/// let status: StepStatus = serde_json::from_value(serde_json::json!("failed")).unwrap();
/// assert_eq!(status, StepStatus::Failed);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Step has not been attempted yet.
    #[default]
    Pending,
    /// Step completed successfully.
    Completed,
    /// Step failed (error recorded in variables).
    Failed,
    /// Step was skipped (server couldn't execute, client should continue).
    Skipped,
}

// === Pause Reason Types ===

/// Describes why the partial execution engine paused before completing all
/// workflow steps.
///
/// When the execution engine encounters a condition that prevents further
/// progress, it stores a `PauseReason` in the task's variable store under
/// [`WORKFLOW_PAUSE_REASON_KEY`]. The client inspects this value to decide
/// what corrective action to take before resuming execution.
///
/// # Serialization
///
/// Variants are internally tagged with `"type"` and all field names use
/// `camelCase`:
///
/// ```
/// use pmcp_tasks::types::workflow::PauseReason;
///
/// let reason = PauseReason::ToolError {
///     failed_step: "deploy".to_string(),
///     error: "connection timeout".to_string(),
///     retryable: true,
///     suggested_tool: "deploy_service".to_string(),
/// };
///
/// let json = serde_json::to_value(&reason).unwrap();
/// assert_eq!(json["type"], "toolError");
/// assert_eq!(json["failedStep"], "deploy");
/// assert_eq!(json["retryable"], true);
/// ```
///
/// # Variant Selection Guide
///
/// | Condition | Variant |
/// |-----------|---------|
/// | Step parameter cannot be resolved from context | [`UnresolvableParams`](PauseReason::UnresolvableParams) |
/// | Resolved parameters miss required schema fields | [`SchemaMismatch`](PauseReason::SchemaMismatch) |
/// | Tool execution returned an error | [`ToolError`](PauseReason::ToolError) |
/// | Step depends on a failed or skipped step's output | [`UnresolvedDependency`](PauseReason::UnresolvedDependency) |
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PauseReason {
    /// A step's parameters could not be resolved from the available execution
    /// context (prompt arguments, step bindings, constants).
    ///
    /// The client should call the `suggested_tool` to produce the missing
    /// value, then resume execution.
    ///
    /// # JSON Shape
    ///
    /// ```json
    /// {
    ///   "type": "unresolvableParams",
    ///   "blockedStep": "add_task",
    ///   "missingParam": "project_id",
    ///   "suggestedTool": "list_projects"
    /// }
    /// ```
    #[serde(rename_all = "camelCase")]
    UnresolvableParams {
        /// Name of the step that could not proceed.
        blocked_step: String,
        /// The parameter name that could not be resolved.
        missing_param: String,
        /// Tool the client should call to produce the missing value.
        suggested_tool: String,
    },

    /// A step's resolved parameters do not satisfy the target tool's input
    /// schema (one or more required fields are missing).
    ///
    /// The client should call the `suggested_tool` with the resolved
    /// parameters to obtain the missing fields, then resume execution.
    ///
    /// # JSON Shape
    ///
    /// ```json
    /// {
    ///   "type": "schemaMismatch",
    ///   "blockedStep": "add_task",
    ///   "missingFields": ["project_path", "task_format"],
    ///   "suggestedTool": "get_project_config"
    /// }
    /// ```
    #[serde(rename_all = "camelCase")]
    SchemaMismatch {
        /// Name of the step that could not proceed.
        blocked_step: String,
        /// Required schema fields that are missing from resolved parameters.
        missing_fields: Vec<String>,
        /// Tool the client should call to produce the missing fields.
        suggested_tool: String,
    },

    /// A tool execution returned an error.
    ///
    /// If `retryable` is `true`, the client may retry the same tool call
    /// (the failure was likely transient). Otherwise, the client should call
    /// the `suggested_tool` for an alternative approach.
    ///
    /// # JSON Shape
    ///
    /// ```json
    /// {
    ///   "type": "toolError",
    ///   "failedStep": "deploy",
    ///   "error": "connection timeout",
    ///   "retryable": true,
    ///   "suggestedTool": "deploy_service"
    /// }
    /// ```
    #[serde(rename_all = "camelCase")]
    ToolError {
        /// Name of the step that failed.
        failed_step: String,
        /// Human-readable error message from the tool.
        error: String,
        /// Whether the error is transient and the step can be retried.
        retryable: bool,
        /// Tool the client should call (retry or alternative).
        suggested_tool: String,
    },

    /// A step depends on output from a step that was failed or skipped,
    /// making its own execution impossible.
    ///
    /// The client should call the `suggested_tool` (typically the producing
    /// step's tool) to generate the missing output, then resume execution.
    ///
    /// # JSON Shape
    ///
    /// ```json
    /// {
    ///   "type": "unresolvedDependency",
    ///   "blockedStep": "add_task",
    ///   "missingOutput": "project_info",
    ///   "producingStep": "verify_project",
    ///   "suggestedTool": "verify_project"
    /// }
    /// ```
    #[serde(rename_all = "camelCase")]
    UnresolvedDependency {
        /// Name of the step that could not proceed.
        blocked_step: String,
        /// The binding name of the missing output.
        missing_output: String,
        /// Name of the step that was supposed to produce the output.
        producing_step: String,
        /// Tool the client should call to produce the missing output.
        suggested_tool: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_status_default_is_pending() {
        assert_eq!(StepStatus::default(), StepStatus::Pending);
    }

    #[test]
    fn step_status_serde_round_trip() {
        for status in [
            StepStatus::Pending,
            StepStatus::Completed,
            StepStatus::Failed,
            StepStatus::Skipped,
        ] {
            let json = serde_json::to_value(status).unwrap();
            let back: StepStatus = serde_json::from_value(json).unwrap();
            assert_eq!(status, back);
        }
    }

    #[test]
    fn step_status_serializes_snake_case() {
        assert_eq!(serde_json::to_value(StepStatus::Pending).unwrap(), "pending");
        assert_eq!(
            serde_json::to_value(StepStatus::Completed).unwrap(),
            "completed"
        );
        assert_eq!(serde_json::to_value(StepStatus::Failed).unwrap(), "failed");
        assert_eq!(
            serde_json::to_value(StepStatus::Skipped).unwrap(),
            "skipped"
        );
    }

    #[test]
    fn workflow_progress_serde_round_trip() {
        let progress = WorkflowProgress {
            goal: "Deploy service to us-east-1".to_string(),
            steps: vec![
                WorkflowStepProgress {
                    name: "validate".to_string(),
                    tool: Some("validate_config".to_string()),
                    status: StepStatus::Completed,
                },
                WorkflowStepProgress {
                    name: "deploy".to_string(),
                    tool: Some("deploy_service".to_string()),
                    status: StepStatus::Pending,
                },
                WorkflowStepProgress {
                    name: "notify".to_string(),
                    tool: None,
                    status: StepStatus::Skipped,
                },
            ],
            schema_version: 1,
        };

        let json = serde_json::to_value(&progress).unwrap();
        let round_trip: WorkflowProgress = serde_json::from_value(json).unwrap();
        assert_eq!(progress, round_trip);
    }

    #[test]
    fn workflow_progress_json_shape() {
        let progress = WorkflowProgress {
            goal: "Test deployment".to_string(),
            steps: vec![
                WorkflowStepProgress {
                    name: "check".to_string(),
                    tool: Some("checker".to_string()),
                    status: StepStatus::Completed,
                },
                WorkflowStepProgress {
                    name: "log".to_string(),
                    tool: None,
                    status: StepStatus::Failed,
                },
            ],
            schema_version: 1,
        };

        let json = serde_json::to_value(&progress).unwrap();

        // camelCase field names
        assert!(json.get("goal").is_some());
        assert!(json.get("steps").is_some());
        assert!(json.get("schemaVersion").is_some());

        // snake_case enum values
        assert_eq!(json["steps"][0]["status"], "completed");
        assert_eq!(json["steps"][1]["status"], "failed");

        // tool omitted when None
        assert!(json["steps"][0].get("tool").is_some());
        assert!(json["steps"][1].get("tool").is_none());
    }

    #[test]
    fn workflow_result_key_produces_correct_keys() {
        assert_eq!(
            workflow_result_key("validate"),
            "_workflow.result.validate"
        );
        assert_eq!(workflow_result_key("deploy"), "_workflow.result.deploy");
        assert_eq!(
            workflow_result_key("check-config"),
            "_workflow.result.check-config"
        );
    }

    #[test]
    fn workflow_progress_key_constant() {
        assert_eq!(WORKFLOW_PROGRESS_KEY, "_workflow.progress");
    }

    #[test]
    fn workflow_result_prefix_constant() {
        assert_eq!(WORKFLOW_RESULT_PREFIX, "_workflow.result.");
    }

    #[test]
    fn workflow_progress_empty_steps() {
        let progress = WorkflowProgress {
            goal: "Empty workflow".to_string(),
            steps: vec![],
            schema_version: 1,
        };

        let json = serde_json::to_value(&progress).unwrap();
        let round_trip: WorkflowProgress = serde_json::from_value(json).unwrap();
        assert_eq!(progress, round_trip);
        assert!(round_trip.steps.is_empty());
    }

    #[test]
    fn workflow_step_progress_resource_only() {
        let step = WorkflowStepProgress {
            name: "read_config".to_string(),
            tool: None,
            status: StepStatus::Pending,
        };

        let json = serde_json::to_value(&step).unwrap();
        assert!(json.get("tool").is_none(), "tool should be omitted when None");

        let round_trip: WorkflowStepProgress = serde_json::from_value(json).unwrap();
        assert_eq!(step, round_trip);
    }

    #[test]
    fn deserialization_tolerates_missing_optional_tool() {
        // JSON without "tool" field should deserialize with tool: None
        let json = serde_json::json!({
            "name": "resource_step",
            "status": "pending"
        });

        let step: WorkflowStepProgress = serde_json::from_value(json).unwrap();
        assert_eq!(step.name, "resource_step");
        assert!(step.tool.is_none());
        assert_eq!(step.status, StepStatus::Pending);
    }

    // --- PauseReason tests ---

    #[test]
    fn pause_reason_unresolvable_params_serde_round_trip() {
        let reason = PauseReason::UnresolvableParams {
            blocked_step: "add_task".to_string(),
            missing_param: "project_id".to_string(),
            suggested_tool: "list_projects".to_string(),
        };

        let json = serde_json::to_value(&reason).unwrap();
        let round_trip: PauseReason = serde_json::from_value(json).unwrap();
        assert_eq!(reason, round_trip);
    }

    #[test]
    fn pause_reason_schema_mismatch_serde_round_trip() {
        let reason = PauseReason::SchemaMismatch {
            blocked_step: "add_task".to_string(),
            missing_fields: vec!["project_path".to_string(), "task_format".to_string()],
            suggested_tool: "get_project_config".to_string(),
        };

        let json = serde_json::to_value(&reason).unwrap();
        let round_trip: PauseReason = serde_json::from_value(json).unwrap();
        assert_eq!(reason, round_trip);
    }

    #[test]
    fn pause_reason_tool_error_serde_round_trip() {
        let reason = PauseReason::ToolError {
            failed_step: "deploy".to_string(),
            error: "connection timeout".to_string(),
            retryable: true,
            suggested_tool: "deploy_service".to_string(),
        };

        let json = serde_json::to_value(&reason).unwrap();
        let round_trip: PauseReason = serde_json::from_value(json).unwrap();
        assert_eq!(reason, round_trip);
    }

    #[test]
    fn pause_reason_unresolved_dependency_serde_round_trip() {
        let reason = PauseReason::UnresolvedDependency {
            blocked_step: "add_task".to_string(),
            missing_output: "project_info".to_string(),
            producing_step: "verify_project".to_string(),
            suggested_tool: "verify_project".to_string(),
        };

        let json = serde_json::to_value(&reason).unwrap();
        let round_trip: PauseReason = serde_json::from_value(json).unwrap();
        assert_eq!(reason, round_trip);
    }

    #[test]
    fn pause_reason_json_shape_uses_camel_case() {
        // UnresolvableParams
        let reason = PauseReason::UnresolvableParams {
            blocked_step: "step_a".to_string(),
            missing_param: "param_x".to_string(),
            suggested_tool: "tool_y".to_string(),
        };
        let json = serde_json::to_value(&reason).unwrap();
        assert_eq!(json["type"], "unresolvableParams");
        assert_eq!(json["blockedStep"], "step_a");
        assert_eq!(json["missingParam"], "param_x");
        assert_eq!(json["suggestedTool"], "tool_y");

        // SchemaMismatch
        let reason = PauseReason::SchemaMismatch {
            blocked_step: "step_b".to_string(),
            missing_fields: vec!["field_1".to_string()],
            suggested_tool: "tool_z".to_string(),
        };
        let json = serde_json::to_value(&reason).unwrap();
        assert_eq!(json["type"], "schemaMismatch");
        assert_eq!(json["blockedStep"], "step_b");
        assert_eq!(json["missingFields"], serde_json::json!(["field_1"]));
        assert_eq!(json["suggestedTool"], "tool_z");

        // ToolError
        let reason = PauseReason::ToolError {
            failed_step: "deploy".to_string(),
            error: "timeout".to_string(),
            retryable: true,
            suggested_tool: "deploy_service".to_string(),
        };
        let json = serde_json::to_value(&reason).unwrap();
        assert_eq!(json["type"], "toolError");
        assert_eq!(json["failedStep"], "deploy");
        assert_eq!(json["error"], "timeout");
        assert_eq!(json["retryable"], true);
        assert_eq!(json["suggestedTool"], "deploy_service");

        // UnresolvedDependency
        let reason = PauseReason::UnresolvedDependency {
            blocked_step: "step_c".to_string(),
            missing_output: "data".to_string(),
            producing_step: "step_a".to_string(),
            suggested_tool: "fetch_data".to_string(),
        };
        let json = serde_json::to_value(&reason).unwrap();
        assert_eq!(json["type"], "unresolvedDependency");
        assert_eq!(json["blockedStep"], "step_c");
        assert_eq!(json["missingOutput"], "data");
        assert_eq!(json["producingStep"], "step_a");
        assert_eq!(json["suggestedTool"], "fetch_data");
    }

    #[test]
    fn pause_reason_key_constant() {
        assert_eq!(WORKFLOW_PAUSE_REASON_KEY, "_workflow.pause_reason");
    }
}

#[cfg(test)]
mod proptest_workflow {
    use super::*;
    use proptest::prelude::*;

    fn arb_step_status() -> impl Strategy<Value = StepStatus> {
        prop_oneof![
            Just(StepStatus::Pending),
            Just(StepStatus::Completed),
            Just(StepStatus::Failed),
            Just(StepStatus::Skipped),
        ]
    }

    fn arb_step_progress() -> impl Strategy<Value = WorkflowStepProgress> {
        (
            "[a-zA-Z0-9_-]{1,50}",
            prop::option::of("[a-zA-Z0-9_-]{1,50}"),
            arb_step_status(),
        )
            .prop_map(|(name, tool, status)| WorkflowStepProgress { name, tool, status })
    }

    fn arb_workflow_progress() -> impl Strategy<Value = WorkflowProgress> {
        (
            ".{0,200}",
            prop::collection::vec(arb_step_progress(), 0..20),
            1u32..=10,
        )
            .prop_map(|(goal, steps, schema_version)| WorkflowProgress {
                goal,
                steps,
                schema_version,
            })
    }

    fn arb_pause_reason() -> impl Strategy<Value = PauseReason> {
        prop_oneof![
            (
                "[a-zA-Z0-9_]{1,30}",
                "[a-zA-Z0-9_]{1,30}",
                "[a-zA-Z0-9_]{1,30}",
            )
                .prop_map(|(blocked_step, missing_param, suggested_tool)| {
                    PauseReason::UnresolvableParams {
                        blocked_step,
                        missing_param,
                        suggested_tool,
                    }
                }),
            (
                "[a-zA-Z0-9_]{1,30}",
                prop::collection::vec("[a-zA-Z0-9_]{1,30}", 1..5),
                "[a-zA-Z0-9_]{1,30}",
            )
                .prop_map(|(blocked_step, missing_fields, suggested_tool)| {
                    PauseReason::SchemaMismatch {
                        blocked_step,
                        missing_fields,
                        suggested_tool,
                    }
                }),
            (
                "[a-zA-Z0-9_]{1,30}",
                ".{0,100}",
                proptest::bool::ANY,
                "[a-zA-Z0-9_]{1,30}",
            )
                .prop_map(|(failed_step, error, retryable, suggested_tool)| {
                    PauseReason::ToolError {
                        failed_step,
                        error,
                        retryable,
                        suggested_tool,
                    }
                }),
            (
                "[a-zA-Z0-9_]{1,30}",
                "[a-zA-Z0-9_]{1,30}",
                "[a-zA-Z0-9_]{1,30}",
                "[a-zA-Z0-9_]{1,30}",
            )
                .prop_map(
                    |(blocked_step, missing_output, producing_step, suggested_tool)| {
                        PauseReason::UnresolvedDependency {
                            blocked_step,
                            missing_output,
                            producing_step,
                            suggested_tool,
                        }
                    }
                ),
        ]
    }

    proptest! {
        #[test]
        fn serde_round_trip(progress in arb_workflow_progress()) {
            let value = serde_json::to_value(&progress).unwrap();
            let round_trip: WorkflowProgress = serde_json::from_value(value).unwrap();
            prop_assert_eq!(progress, round_trip);
        }

        #[test]
        fn pause_reason_serde_round_trip(reason in arb_pause_reason()) {
            let value = serde_json::to_value(&reason).unwrap();
            let round_trip: PauseReason = serde_json::from_value(value).unwrap();
            prop_assert_eq!(reason, round_trip);
        }
    }
}
