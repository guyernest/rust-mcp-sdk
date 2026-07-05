//! MCP Task protocol types (2025-11-25).
//!
//! This module contains the wire types for MCP Tasks as defined
//! in the 2025-11-25 protocol version.

use serde::{Deserialize, Serialize};

/// Related task metadata key per MCP spec.
pub const RELATED_TASK_META_KEY: &str = "io.modelcontextprotocol/related-task";

/// Task status (5-value enum).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is actively being worked on
    #[default]
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

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
}

/// The classification of a polled [`Task`], derived purely from its
/// already-deserialized [`TaskStatus`] — the single decision primitive every
/// task poller consumes so the branch logic cannot drift.
///
/// Produced by [`Task::poll_decision`]. It answers the one question a poll loop
/// asks each tick: *stop, ask the user, or sleep and poll again?* — without a
/// network round-trip, a [`CallToolResult`](crate::types::CallToolResult)
/// fetch, or any I/O. It is a pure, replay-deterministic function of the polled
/// `Task`, so it is safe to call inside a memoized durable/replay step (D-01,
/// D-03).
///
/// # Non-exhaustive
///
/// This enum is `#[non_exhaustive]` for future-proofing (D-04): adding a variant
/// later is a non-breaking change, so external `match` sites must carry a
/// wildcard `_ =>` arm. This is a distinct claim from [`TaskStatus`], which is
/// deliberately **exhaustive** today (D-15): `#[non_exhaustive]` here does NOT
/// imply that unknown or future wire statuses are handled gracefully at runtime.
/// An unknown status fails at serde deserialization during `tasks/get`, BEFORE
/// classification ever runs — `poll_decision()` only ever sees one of the five
/// known `TaskStatus` values.
///
/// Unlike every other type in this module, `TaskPollDecision` intentionally
/// derives neither `Serialize` nor `Deserialize`: it is a returned classifier
/// value consumed in-process, never a wire type.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaskPollDecision {
    /// The task has reached a terminal status ([`TaskStatus::Completed`],
    /// [`TaskStatus::Failed`], or [`TaskStatus::Cancelled`]) and will not
    /// transition further — the poll loop should stop.
    ///
    /// The classifier carries the terminal [`TaskStatus`] by value (it is
    /// `Copy`) but deliberately does NOT carry the final
    /// [`CallToolResult`](crate::types::CallToolResult): to retrieve the
    /// result the caller still issues a **separate `tasks/result`** call
    /// (D-06/D-16). This keeps classification a pure, I/O-free decision.
    Terminal {
        /// The terminal status that ended the task.
        status: TaskStatus,
    },
    /// The task is still running ([`TaskStatus::Working`]) — the poll loop
    /// should sleep and poll again.
    ///
    /// `poll_hint` is the raw server-reported `pollInterval` in **milliseconds**
    /// ([`Task::poll_interval`]), passed through verbatim including `None`
    /// (D-07). Feed it to [`resolve_poll_interval`] to obtain the concrete sleep
    /// duration honoring the caller override, this hint, the default, and the
    /// hot-loop floor.
    InProgress {
        /// The raw server-reported `pollInterval` in ms, verbatim (`None` when
        /// the server suggested no interval).
        poll_hint: Option<u64>,
    },
    /// The task is blocked on user input ([`TaskStatus::InputRequired`]).
    ///
    /// This is a unit variant carrying no payload (D-05): the caller already
    /// holds the polled `Task`, and a blocking poller cannot supply the input,
    /// so it must route to elicitation rather than continue spinning.
    InputRequired,
}

/// A task resource representing an in-progress or completed operation.
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Use the constructor to remain
/// forward-compatible:
///
/// ```rust
/// use pmcp::types::tasks::{Task, TaskStatus};
///
/// let task = Task::new("t-123", TaskStatus::Working)
///     .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z")
///     .with_ttl(60000)
///     .with_poll_interval(5000)
///     .with_status_message("Processing...");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct Task {
    /// Unique task identifier
    pub task_id: String,
    /// Current task status
    pub status: TaskStatus,
    /// Time-to-live in milliseconds. Required but nullable per MCP spec:
    /// `None` serializes as `null` (unlimited TTL), `Some(ms)` as a number.
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

impl Task {
    /// Create a task with the given ID and status.
    ///
    /// Timestamps default to empty strings (caller should set via
    /// [`Task::with_timestamps`]). Optional fields default to `None`.
    pub fn new(task_id: impl Into<String>, status: TaskStatus) -> Self {
        Self {
            task_id: task_id.into(),
            status,
            ttl: None,
            created_at: String::new(),
            last_updated_at: String::new(),
            poll_interval: None,
            status_message: None,
        }
    }

    /// Set the time-to-live in milliseconds.
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set both creation and last-updated timestamps.
    pub fn with_timestamps(
        mut self,
        created_at: impl Into<String>,
        last_updated_at: impl Into<String>,
    ) -> Self {
        self.created_at = created_at.into();
        self.last_updated_at = last_updated_at.into();
        self
    }

    /// Set the suggested polling interval in milliseconds.
    pub fn with_poll_interval(mut self, interval: u64) -> Self {
        self.poll_interval = Some(interval);
        self
    }

    /// Set a human-readable status message.
    pub fn with_status_message(mut self, message: impl Into<String>) -> Self {
        self.status_message = Some(message.into());
        self
    }

    /// Classify this already-polled task into a [`TaskPollDecision`] without any
    /// I/O — the single decision primitive every task poller consumes.
    ///
    /// This is a pure, total function of the polled `Task`'s [`TaskStatus`]:
    /// there is no `_` wildcard arm because `TaskStatus` is exhaustive (five
    /// known variants), so the mapping cannot silently drift. Because it touches
    /// nothing but the in-hand `Task`, it is replay-deterministic and safe to
    /// call inside a memoized durable/replay step — provided the `tasks/get`
    /// network call and its serde decode sit INSIDE the memoized step, so an
    /// unknown/future status fails at deserialization before this runs (D-01,
    /// D-04, D-14, D-15).
    ///
    /// Mapping:
    /// - [`TaskStatus::Working`] → [`TaskPollDecision::InProgress`] carrying the
    ///   task's `poll_interval` verbatim as `poll_hint` (D-07).
    /// - [`TaskStatus::InputRequired`] → [`TaskPollDecision::InputRequired`].
    /// - [`TaskStatus::Completed`] / [`TaskStatus::Failed`] /
    ///   [`TaskStatus::Cancelled`] → [`TaskPollDecision::Terminal`] carrying the
    ///   terminal status. The final
    ///   [`CallToolResult`](crate::types::CallToolResult) is NOT fetched here —
    ///   the caller issues a separate `tasks/result` call (D-06/D-16).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::types::tasks::{Task, TaskStatus, TaskPollDecision};
    ///
    /// let task = Task::new("t-123", TaskStatus::Working).with_poll_interval(2000);
    /// match task.poll_decision() {
    ///     TaskPollDecision::InProgress { poll_hint } => assert_eq!(poll_hint, Some(2000)),
    ///     // `TaskPollDecision` is `#[non_exhaustive]`, so external callers
    ///     // need a wildcard arm.
    ///     _ => panic!("a Working task must classify as InProgress"),
    /// }
    /// ```
    pub fn poll_decision(&self) -> TaskPollDecision {
        match self.status {
            TaskStatus::Working => TaskPollDecision::InProgress {
                poll_hint: self.poll_interval,
            },
            TaskStatus::InputRequired => TaskPollDecision::InputRequired,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                TaskPollDecision::Terminal {
                    status: self.status,
                }
            },
        }
    }
}

/// Parameters for task creation (augments tools/call).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskCreationParams {
    /// Time-to-live in milliseconds. Required but nullable per MCP spec:
    /// `None` serializes as `null` (unlimited TTL), `Some(ms)` as a number.
    pub ttl: Option<u64>,
    /// Suggested polling interval in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
}

impl TaskCreationParams {
    /// Create empty task creation parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the time-to-live in milliseconds.
    pub fn with_ttl(mut self, ttl: u64) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Set the suggested polling interval in milliseconds.
    pub fn with_poll_interval(mut self, interval: u64) -> Self {
        self.poll_interval = Some(interval);
        self
    }
}

/// Task metadata for related-task references.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedTaskMetadata {
    /// The referenced task ID
    pub task_id: String,
}

/// Typed metadata carried on a `CallToolResult` under
/// [`RELATED_TASK_META_KEY`], linking a synchronous tool result to the async
/// task that produced (or is producing) it (SEP-1686).
///
/// This is the richer twin of [`RelatedTaskMetadata`]: it carries the optional
/// polling hints a client needs to drive
/// [`Client::wait_for_task`](crate::Client::wait_for_task) without hand-copying
/// fields. Server code builds it via
/// [`CallToolResult::with_related_task`](crate::types::CallToolResult::with_related_task);
/// client code reads it via
/// [`CallToolResult::related_task`](crate::types::CallToolResult::related_task).
///
/// The two `Option` polling fields default to `None`, so the minimal native
/// emit shape `{ "taskId": "t1" }` deserializes cleanly (extra fields absent).
///
/// # Backward Compatibility
///
/// This struct is `#[non_exhaustive]`. Construct it via [`TaskMetadata::new`]
/// and the builder methods to stay forward-compatible:
///
/// ```rust
/// use pmcp::types::tasks::TaskMetadata;
///
/// let meta = TaskMetadata::new("t-123")
///     .with_poll_interval(5000)
///     .with_max_poll_duration_secs(300);
/// let json = serde_json::to_value(&meta).unwrap();
/// assert_eq!(json["taskId"], "t-123");
/// assert_eq!(json["pollInterval"], 5000);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct TaskMetadata {
    /// The referenced task ID.
    pub task_id: String,
    /// Suggested polling interval, in **milliseconds**.
    ///
    /// This is the same unit as [`Task::poll_interval`]. A client poller uses
    /// it as the delay between `tasks/get` calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll_interval: Option<u64>,
    /// Maximum total time to poll before giving up, in **seconds**.
    ///
    /// Note the unit differs from [`TaskMetadata::poll_interval`] (which is
    /// milliseconds): this is a coarse overall budget expressed in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_poll_duration_secs: Option<u64>,
}

impl TaskMetadata {
    /// Create related-task metadata referencing `task_id`.
    ///
    /// Both polling hints default to `None`; set them via
    /// [`TaskMetadata::with_poll_interval`] and
    /// [`TaskMetadata::with_max_poll_duration_secs`].
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            poll_interval: None,
            max_poll_duration_secs: None,
        }
    }

    /// Set the suggested polling interval, in **milliseconds**.
    pub fn with_poll_interval(mut self, interval_ms: u64) -> Self {
        self.poll_interval = Some(interval_ms);
        self
    }

    /// Set the maximum total polling duration, in **seconds**.
    pub fn with_max_poll_duration_secs(mut self, secs: u64) -> Self {
        self.max_poll_duration_secs = Some(secs);
        self
    }
}

/// Result of creating a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskResult {
    /// The created task
    pub task: Task,
}

impl CreateTaskResult {
    /// Create a task creation result.
    pub fn new(task: Task) -> Self {
        Self { task }
    }
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
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct GetTaskResult {
    /// The requested task
    pub task: Task,
}

impl GetTaskResult {
    /// Create a get task result.
    pub fn new(task: Task) -> Self {
        Self { task }
    }
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ListTasksResult {
    /// List of tasks
    pub tasks: Vec<Task>,
    /// Pagination cursor for next page
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl ListTasksResult {
    /// Create a list tasks result.
    pub fn new(tasks: Vec<Task>) -> Self {
        Self {
            tasks,
            next_cursor: None,
        }
    }

    /// Set the pagination cursor for the next page.
    pub fn with_next_cursor(mut self, cursor: impl Into<String>) -> Self {
        self.next_cursor = Some(cursor.into());
        self
    }
}

/// Cancel task request.
///
/// When `result` is `Some`, the task transitions to `Completed` status
/// (workflow completion). When `result` is `None`, the task transitions to
/// `Cancelled` status (standard cancel).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskRequest {
    /// Task ID to cancel
    pub task_id: String,
    /// Optional result value for workflow completion.
    ///
    /// When present, completes the task (transitions to `Completed` status)
    /// instead of cancelling it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
}

/// Cancel task result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct CancelTaskResult {
    /// The cancelled task
    pub task: Task,
}

impl CancelTaskResult {
    /// Create a cancel task result.
    pub fn new(task: Task) -> Self {
        Self { task }
    }
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
        let task = Task::new("t-123", TaskStatus::Working)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z")
            .with_ttl(60000)
            .with_poll_interval(5000)
            .with_status_message("Processing...");
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
        let result = CreateTaskResult::new(
            Task::new("t-456", TaskStatus::Completed)
                .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:05:00Z"),
        );
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
    fn task_metadata_serde_round_trip() {
        let meta = TaskMetadata {
            task_id: "t1".to_string(),
            poll_interval: Some(5000),
            max_poll_duration_secs: None,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["taskId"], "t1");
        assert_eq!(json["pollInterval"], 5000);
        // maxPollDurationSecs omitted (skip_serializing_if = None)
        assert!(
            json.get("maxPollDurationSecs").is_none(),
            "maxPollDurationSecs should be omitted when None"
        );

        let roundtrip: TaskMetadata = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip.task_id, "t1");
        assert_eq!(roundtrip.poll_interval, Some(5000));
        assert_eq!(roundtrip.max_poll_duration_secs, None);
    }

    #[test]
    fn task_metadata_minimal_shape_deserializes() {
        // The minimal native emit shape carries only taskId.
        let meta: TaskMetadata = serde_json::from_value(json!({ "taskId": "t1" })).unwrap();
        assert_eq!(meta.task_id, "t1");
        assert_eq!(meta.poll_interval, None);
        assert_eq!(meta.max_poll_duration_secs, None);
    }

    #[test]
    fn related_task_meta_key_value() {
        assert_eq!(
            RELATED_TASK_META_KEY,
            "io.modelcontextprotocol/related-task"
        );
    }

    #[test]
    fn task_status_is_terminal() {
        assert!(!TaskStatus::Working.is_terminal());
        assert!(!TaskStatus::InputRequired.is_terminal());
        assert!(TaskStatus::Completed.is_terminal());
        assert!(TaskStatus::Failed.is_terminal());
        assert!(TaskStatus::Cancelled.is_terminal());
    }

    #[test]
    fn task_status_can_transition_to() {
        // Working can transition to all except itself
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::InputRequired));
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Completed));
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Failed));
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Cancelled));

        // InputRequired can transition to all except itself
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Working));
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Completed));
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Failed));
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Cancelled));
    }

    #[test]
    fn task_status_self_transition_rejected() {
        assert!(!TaskStatus::Working.can_transition_to(&TaskStatus::Working));
        assert!(!TaskStatus::InputRequired.can_transition_to(&TaskStatus::InputRequired));
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Completed));
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::Failed));
        assert!(!TaskStatus::Cancelled.can_transition_to(&TaskStatus::Cancelled));
    }

    #[test]
    fn task_status_terminal_rejects_all() {
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
                    "{terminal:?} should not transition to {target:?}"
                );
            }
        }
    }

    #[test]
    fn task_ttl_null_serialization() {
        let task = Task::new("test-null-ttl", TaskStatus::Working)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z");
        let json = serde_json::to_value(&task).unwrap();
        // ttl MUST be present as null, not omitted (MCP spec: number | null)
        assert!(json.get("ttl").is_some(), "ttl must be present");
        assert!(json["ttl"].is_null(), "ttl must be null when None");
        // pollInterval SHOULD be omitted when None
        assert!(
            json.get("pollInterval").is_none(),
            "pollInterval should be omitted when None"
        );
    }

    #[test]
    fn task_ttl_present_serialization() {
        let task = Task::new("test-present-ttl", TaskStatus::Working)
            .with_timestamps("2025-11-25T00:00:00Z", "2025-11-25T00:01:00Z")
            .with_ttl(60000);
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["ttl"], 60000);
    }

    #[test]
    fn task_status_display() {
        assert_eq!(TaskStatus::Working.to_string(), "working");
        assert_eq!(TaskStatus::InputRequired.to_string(), "input_required");
        assert_eq!(TaskStatus::Completed.to_string(), "completed");
        assert_eq!(TaskStatus::Failed.to_string(), "failed");
        assert_eq!(TaskStatus::Cancelled.to_string(), "cancelled");
    }

    /// All five `TaskStatus` values, for exhaustive table tests.
    const ALL_STATUSES: [TaskStatus; 5] = [
        TaskStatus::Working,
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ];

    /// The expected `poll_decision()` classification for a status, given the
    /// task's `poll_interval` (only consulted for the `Working` case).
    fn expected_decision(status: TaskStatus, poll_interval: Option<u64>) -> TaskPollDecision {
        match status {
            TaskStatus::Working => TaskPollDecision::InProgress {
                poll_hint: poll_interval,
            },
            TaskStatus::InputRequired => TaskPollDecision::InputRequired,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled => {
                TaskPollDecision::Terminal { status }
            },
        }
    }

    #[test]
    fn poll_decision_maps_every_status() {
        // Working -> InProgress carrying the poll_interval verbatim.
        let working = Task::new("t-w", TaskStatus::Working).with_poll_interval(2500);
        assert_eq!(
            working.poll_decision(),
            TaskPollDecision::InProgress {
                poll_hint: Some(2500)
            }
        );

        // Working with no poll_interval -> InProgress { poll_hint: None }.
        let working_none = Task::new("t-wn", TaskStatus::Working);
        assert_eq!(
            working_none.poll_decision(),
            TaskPollDecision::InProgress { poll_hint: None }
        );

        // InputRequired -> unit variant.
        let waiting = Task::new("t-i", TaskStatus::InputRequired);
        assert_eq!(waiting.poll_decision(), TaskPollDecision::InputRequired);

        // Each terminal status -> Terminal carrying that status.
        for status in [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ] {
            let task = Task::new("t-term", status);
            assert_eq!(
                task.poll_decision(),
                TaskPollDecision::Terminal { status },
                "{status:?} must classify as Terminal"
            );
        }
    }

    #[test]
    fn poll_decision_covers_all_statuses_exhaustively() {
        // Table drive across EVERY TaskStatus value (guards D-15 exhaustiveness).
        for status in ALL_STATUSES {
            let task = Task::new("t-x", status).with_poll_interval(1234);
            assert_eq!(
                task.poll_decision(),
                expected_decision(status, Some(1234)),
                "poll_decision() drifted for {status:?}"
            );
        }
    }

    proptest::proptest! {
        /// For every TaskStatus and any poll_interval, poll_decision() returns
        /// exactly the mapped variant — the classifier never drifts.
        #[test]
        fn poll_decision_matches_expected_map(
            status_idx in 0usize..ALL_STATUSES.len(),
            poll_interval in proptest::option::of(proptest::prelude::any::<u64>()),
        ) {
            let status = ALL_STATUSES[status_idx];
            let mut task = Task::new("t-prop", status);
            task.poll_interval = poll_interval;
            proptest::prop_assert_eq!(
                task.poll_decision(),
                expected_decision(status, poll_interval)
            );
        }
    }
}
