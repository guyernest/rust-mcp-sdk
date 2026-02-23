//! Task record -- the store's internal representation of a task.
//!
//! [`TaskRecord`] wraps the wire-format [`Task`] with additional fields
//! needed for storage, ownership, variables, and TTL expiration.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::types::task::{Task, TaskStatus};

/// Internal storage representation of a task.
///
/// A `TaskRecord` holds the wire-format [`Task`] along with domain-specific
/// fields that are not part of the MCP wire protocol: `owner_id` for access
/// control, `variables` for shared state, `result` for the operation outcome,
/// `request_method` for auditing, and `expires_at` for efficient TTL checks.
///
/// All fields are public so that store implementors have full access.
///
/// # Construction
///
/// Use [`TaskRecord::new`] to create a new record with a generated UUID task
/// ID and computed expiry:
///
/// ```
/// use pmcp_tasks::domain::TaskRecord;
///
/// let record = TaskRecord::new(
///     "session-abc".to_string(),
///     "tools/call".to_string(),
///     Some(60_000),
/// );
/// assert!(!record.task.task_id.is_empty());
/// assert_eq!(record.owner_id, "session-abc");
/// assert!(record.expires_at.is_some());
/// ```
#[derive(Debug, Clone)]
pub struct TaskRecord {
    /// The wire-format task (serialized as-is for MCP responses).
    pub task: Task,

    /// Identifier of the session or client that owns this task.
    pub owner_id: String,

    /// Shared variable store for this task. Both client and server can
    /// read and write variables. Variables are injected into the wire
    /// task's `_meta` field at the serialization boundary.
    pub variables: HashMap<String, Value>,

    /// The operation result, set when the task reaches a terminal state.
    pub result: Option<Value>,

    /// The MCP method that created this task (e.g., `"tools/call"`).
    pub request_method: String,

    /// Computed absolute expiry time based on TTL. `None` means the task
    /// does not expire (unlimited TTL).
    pub expires_at: Option<DateTime<Utc>>,
}

impl TaskRecord {
    /// Creates a new task record in the `Working` state.
    ///
    /// Generates a `UUIDv4` task ID, sets timestamps to the current UTC time,
    /// and computes `expires_at` from `ttl` (milliseconds from now). If `ttl`
    /// is `None`, the task does not expire.
    ///
    /// # Arguments
    ///
    /// * `owner_id` - The session or client identifier that owns this task.
    /// * `request_method` - The MCP method name that created this task.
    /// * `ttl` - Time-to-live in milliseconds, or `None` for unlimited.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::TaskRecord;
    ///
    /// let record = TaskRecord::new(
    ///     "session-1".to_string(),
    ///     "tools/call".to_string(),
    ///     Some(30_000),
    /// );
    ///
    /// assert_eq!(record.task.status, pmcp_tasks::TaskStatus::Working);
    /// assert_eq!(record.owner_id, "session-1");
    /// assert_eq!(record.request_method, "tools/call");
    /// assert!(record.variables.is_empty());
    /// assert!(record.result.is_none());
    /// assert!(record.expires_at.is_some());
    /// ```
    pub fn new(owner_id: String, request_method: String, ttl: Option<u64>) -> Self {
        let now = Utc::now();
        let now_str = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        // Use checked arithmetic to avoid panics on extremely large TTL values.
        // If the TTL overflows i64 milliseconds or the resulting DateTime overflows,
        // we treat it as "never expires" (None).
        let expires_at = ttl.and_then(|ms| {
            let ms_i64 = i64::try_from(ms).ok()?;
            let duration = Duration::try_milliseconds(ms_i64)?;
            now.checked_add_signed(duration)
        });

        let task = Task {
            task_id: Uuid::new_v4().to_string(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: now_str.clone(),
            last_updated_at: now_str,
            ttl,
            poll_interval: None,
            _meta: None,
        };

        Self {
            task,
            owner_id,
            variables: HashMap::new(),
            result: None,
            request_method,
            expires_at,
        }
    }

    /// Returns `true` if the task has expired based on its TTL.
    ///
    /// A task with no `expires_at` (unlimited TTL) never expires.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::TaskRecord;
    ///
    /// // Task with no TTL never expires
    /// let record = TaskRecord::new(
    ///     "owner".to_string(),
    ///     "tools/call".to_string(),
    ///     None,
    /// );
    /// assert!(!record.is_expired());
    /// ```
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expiry) => Utc::now() > expiry,
            None => false,
        }
    }

    /// Returns a clone of the wire-format task without variable injection.
    ///
    /// Use this when you need the raw task without PMCP extensions.
    pub fn to_wire_task(&self) -> Task {
        self.task.clone()
    }

    /// Returns a clone of the wire-format task with variables injected
    /// into the `_meta` field.
    ///
    /// Variables are placed at the top level of `_meta` (not nested under
    /// a PMCP-specific key), per the locked design decision. If the task
    /// already has `_meta` entries, variables are merged in (variables take
    /// precedence on key conflict).
    ///
    /// If the variables map is empty, the existing `_meta` is returned
    /// unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::TaskRecord;
    /// use serde_json::json;
    ///
    /// let mut record = TaskRecord::new(
    ///     "owner".to_string(),
    ///     "tools/call".to_string(),
    ///     None,
    /// );
    /// record.variables.insert("progress".to_string(), json!(42));
    ///
    /// let wire = record.to_wire_task_with_variables();
    /// let meta = wire._meta.expect("_meta should be present");
    /// assert_eq!(meta["progress"], json!(42));
    /// ```
    pub fn to_wire_task_with_variables(&self) -> Task {
        let mut task = self.task.clone();

        if self.variables.is_empty() {
            return task;
        }

        let meta = task._meta.get_or_insert_with(serde_json::Map::new);
        for (key, value) in &self.variables {
            meta.insert(key.clone(), value.clone());
        }

        task
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn new_record_has_uuid_task_id() {
        let record = TaskRecord::new(
            "owner-1".to_string(),
            "tools/call".to_string(),
            Some(60_000),
        );
        // UUID v4 format: 8-4-4-4-12 hex chars
        assert_eq!(record.task.task_id.len(), 36);
        assert!(record.task.task_id.contains('-'));
    }

    #[test]
    fn new_record_is_working_status() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert_eq!(record.task.status, TaskStatus::Working);
    }

    #[test]
    fn new_record_timestamps_are_set() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(!record.task.created_at.is_empty());
        assert!(!record.task.last_updated_at.is_empty());
        assert_eq!(record.task.created_at, record.task.last_updated_at);
    }

    #[test]
    fn new_record_with_ttl_has_expiry() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(60_000));
        assert!(record.expires_at.is_some());
        assert_eq!(record.task.ttl, Some(60_000));
    }

    #[test]
    fn new_record_without_ttl_has_no_expiry() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(record.expires_at.is_none());
        assert!(record.task.ttl.is_none());
    }

    #[test]
    fn new_record_has_empty_variables_and_no_result() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(record.variables.is_empty());
        assert!(record.result.is_none());
    }

    #[test]
    fn is_expired_returns_false_for_no_ttl() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(!record.is_expired());
    }

    #[test]
    fn is_expired_returns_false_for_future_expiry() {
        let record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            Some(60_000), // 60 seconds from now
        );
        assert!(!record.is_expired());
    }

    #[test]
    fn is_expired_returns_true_for_past_expiry() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(1));
        // Force the expiry into the past
        record.expires_at = Some(Utc::now() - Duration::seconds(10));
        assert!(record.is_expired());
    }

    #[test]
    fn to_wire_task_returns_clone() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let wire = record.to_wire_task();
        assert_eq!(wire.task_id, record.task.task_id);
        assert_eq!(wire.status, record.task.status);
    }

    #[test]
    fn to_wire_task_with_variables_empty_vars() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let wire = record.to_wire_task_with_variables();
        // No variables, so _meta should be unchanged (None)
        assert!(wire._meta.is_none());
    }

    #[test]
    fn to_wire_task_with_variables_injects_at_top_level() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        record
            .variables
            .insert("server.progress".to_string(), json!(75));
        record
            .variables
            .insert("client.note".to_string(), json!("test note"));

        let wire = record.to_wire_task_with_variables();
        let meta = wire._meta.expect("_meta should be present");
        assert_eq!(meta["server.progress"], json!(75));
        assert_eq!(meta["client.note"], json!("test note"));
    }

    #[test]
    fn to_wire_task_with_variables_merges_existing_meta() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        // Set existing _meta
        let mut existing = serde_json::Map::new();
        existing.insert("existing_key".to_string(), json!("existing_value"));
        record.task._meta = Some(existing);

        // Add variable
        record
            .variables
            .insert("server.count".to_string(), json!(10));

        let wire = record.to_wire_task_with_variables();
        let meta = wire._meta.expect("_meta should be present");
        assert_eq!(meta["existing_key"], json!("existing_value"));
        assert_eq!(meta["server.count"], json!(10));
    }

    #[test]
    fn to_wire_task_with_variables_overwrites_on_conflict() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        // Set existing _meta with key that will conflict
        let mut existing = serde_json::Map::new();
        existing.insert("conflict_key".to_string(), json!("old_value"));
        record.task._meta = Some(existing);

        // Add variable with same key
        record
            .variables
            .insert("conflict_key".to_string(), json!("new_value"));

        let wire = record.to_wire_task_with_variables();
        let meta = wire._meta.expect("_meta should be present");
        // Variable takes precedence
        assert_eq!(meta["conflict_key"], json!("new_value"));
    }

    #[test]
    fn owner_id_and_request_method_preserved() {
        let record = TaskRecord::new("session-xyz".to_string(), "tools/call".to_string(), None);
        assert_eq!(record.owner_id, "session-xyz");
        assert_eq!(record.request_method, "tools/call");
    }
}
