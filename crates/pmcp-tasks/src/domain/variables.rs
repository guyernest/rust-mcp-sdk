//! Task-with-variables domain type for variable injection.
//!
//! [`TaskWithVariables`] is a convenience type that pairs a wire-format
//! [`Task`] with a variable map and provides a method to produce a wire
//! task with variables injected into `_meta`.
//!
//! This type is **not** serialized directly. It exists to encapsulate the
//! variable-injection logic at the domain boundary. The wire task (with
//! `_meta` populated) is what gets serialized in MCP responses.

use std::collections::HashMap;

use serde_json::Value;

use super::record::TaskRecord;
use crate::types::task::Task;

/// A task paired with its shared variable map.
///
/// Variables are injected at the top level of the wire task's `_meta` field
/// (not nested under a PMCP-specific key) per the locked design decision.
///
/// # Usage
///
/// ```
/// use pmcp_tasks::domain::{TaskRecord, TaskWithVariables};
/// use serde_json::json;
///
/// let mut record = TaskRecord::new(
///     "owner".to_string(),
///     "tools/call".to_string(),
///     None,
/// );
/// record.variables.insert("step".to_string(), json!(3));
///
/// let twv = TaskWithVariables::from_record(&record);
/// let wire = twv.to_wire_task();
/// let meta = wire._meta.expect("_meta should be present");
/// assert_eq!(meta["step"], json!(3));
/// ```
#[derive(Debug, Clone)]
pub struct TaskWithVariables {
    /// The wire-format task.
    pub task: Task,

    /// Shared variable store. Variables are injected into `_meta` when
    /// converting to a wire task via [`to_wire_task`](Self::to_wire_task).
    pub variables: HashMap<String, Value>,
}

impl TaskWithVariables {
    /// Constructs a `TaskWithVariables` from a [`TaskRecord`], copying the
    /// wire task and variables map.
    ///
    /// # Arguments
    ///
    /// * `record` - The task record to extract from.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::{TaskRecord, TaskWithVariables};
    ///
    /// let record = TaskRecord::new(
    ///     "owner".to_string(),
    ///     "tools/call".to_string(),
    ///     None,
    /// );
    /// let twv = TaskWithVariables::from_record(&record);
    /// assert_eq!(twv.task.task_id, record.task.task_id);
    /// assert!(twv.variables.is_empty());
    /// ```
    pub fn from_record(record: &TaskRecord) -> Self {
        Self {
            task: record.task.clone(),
            variables: record.variables.clone(),
        }
    }

    /// Produces a wire-format [`Task`] with variables injected into `_meta`.
    ///
    /// For each `(key, value)` in [`variables`](Self::variables), the entry
    /// is inserted at the top level of the task's `_meta` map. If `_meta`
    /// is `None`, a new map is created. If `variables` is empty, the
    /// existing `_meta` is returned unchanged.
    ///
    /// Variables take precedence over existing `_meta` keys on conflict.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::domain::{TaskRecord, TaskWithVariables};
    /// use serde_json::json;
    ///
    /// let mut record = TaskRecord::new(
    ///     "owner".to_string(),
    ///     "tools/call".to_string(),
    ///     None,
    /// );
    /// record.variables.insert("progress".to_string(), json!(50));
    ///
    /// let twv = TaskWithVariables::from_record(&record);
    /// let wire = twv.to_wire_task();
    /// let meta = wire._meta.expect("_meta should be set");
    /// assert_eq!(meta["progress"], json!(50));
    /// ```
    pub fn to_wire_task(&self) -> Task {
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
    fn from_record_copies_task_and_variables() {
        let mut record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            Some(60_000),
        );
        record
            .variables
            .insert("key1".to_string(), json!("value1"));

        let twv = TaskWithVariables::from_record(&record);
        assert_eq!(twv.task.task_id, record.task.task_id);
        assert_eq!(twv.task.status, record.task.status);
        assert_eq!(twv.variables.len(), 1);
        assert_eq!(twv.variables["key1"], json!("value1"));
    }

    #[test]
    fn to_wire_task_empty_variables() {
        let record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            None,
        );
        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();
        assert!(wire._meta.is_none());
    }

    #[test]
    fn to_wire_task_injects_variables_at_top_level() {
        let mut record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            None,
        );
        record
            .variables
            .insert("server.stage".to_string(), json!("processing"));
        record.variables.insert("client.id".to_string(), json!(42));

        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();
        let meta = wire._meta.expect("_meta should be present");

        assert_eq!(meta["server.stage"], json!("processing"));
        assert_eq!(meta["client.id"], json!(42));
    }

    #[test]
    fn to_wire_task_merges_with_existing_meta() {
        let mut record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            None,
        );
        // Pre-populate _meta
        let mut existing_meta = serde_json::Map::new();
        existing_meta.insert("existing".to_string(), json!(true));
        record.task._meta = Some(existing_meta);

        record
            .variables
            .insert("injected".to_string(), json!("yes"));

        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();
        let meta = wire._meta.expect("_meta should be present");

        assert_eq!(meta["existing"], json!(true));
        assert_eq!(meta["injected"], json!("yes"));
    }

    #[test]
    fn to_wire_task_variables_overwrite_meta_on_conflict() {
        let mut record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            None,
        );
        let mut existing_meta = serde_json::Map::new();
        existing_meta.insert("shared_key".to_string(), json!("original"));
        record.task._meta = Some(existing_meta);

        record
            .variables
            .insert("shared_key".to_string(), json!("overwritten"));

        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();
        let meta = wire._meta.expect("_meta should be present");

        assert_eq!(meta["shared_key"], json!("overwritten"));
    }

    #[test]
    fn debug_and_clone() {
        let record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            None,
        );
        let twv = TaskWithVariables::from_record(&record);
        let cloned = twv.clone();
        assert_eq!(cloned.task.task_id, twv.task.task_id);

        // Debug trait works
        let debug_str = format!("{:?}", twv);
        assert!(debug_str.contains("TaskWithVariables"));
    }
}
