//! In-memory task store backed by [`DashMap`].
//!
//! [`InMemoryTaskStore`] provides a thread-safe, concurrent implementation
//! of the [`TaskStore`] trait using `DashMap<String, TaskRecord>`. It is
//! designed for development, testing, and single-process production servers
//! that do not need durable persistence.
//!
//! # Security
//!
//! Owner isolation is enforced structurally: every operation that takes an
//! `owner_id` verifies that the record's owner matches. On mismatch, the
//! store returns [`TaskError::NotFound`] -- never revealing that a task
//! exists but belongs to someone else.
//!
//! # Concurrency
//!
//! DashMap provides fine-grained locking at the shard level. Mutation
//! operations use [`DashMap::get_mut`] which holds the entry lock for the
//! duration of the update, ensuring atomic state transitions.
//!
//! # Examples
//!
//! ```
//! use pmcp_tasks::store::memory::InMemoryTaskStore;
//! use pmcp_tasks::store::{StoreConfig, TaskStore};
//! use pmcp_tasks::security::TaskSecurityConfig;
//!
//! let store = InMemoryTaskStore::new()
//!     .with_config(StoreConfig::default())
//!     .with_security(TaskSecurityConfig::default().with_allow_anonymous(true))
//!     .with_poll_interval(3000);
//! ```

use std::collections::HashMap;

use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value;

use crate::domain::TaskRecord;
use crate::error::TaskError;
use crate::security::{TaskSecurityConfig, DEFAULT_LOCAL_OWNER};
use crate::types::task::TaskStatus;

use super::{ListTasksOptions, StoreConfig, TaskPage, TaskStore};

/// Thread-safe in-memory task store using [`DashMap`].
///
/// Implements all 11 [`TaskStore`] methods with:
/// - Structural owner isolation (mismatch returns `NotFound`)
/// - Configurable security limits via [`TaskSecurityConfig`]
/// - Atomic state transitions within DashMap entry locks
/// - Cursor-based pagination for task listing
/// - TTL enforcement with hard reject (no silent clamping)
///
/// # Construction
///
/// Use the builder pattern to configure the store:
///
/// ```
/// use pmcp_tasks::store::memory::InMemoryTaskStore;
/// use pmcp_tasks::store::StoreConfig;
/// use pmcp_tasks::security::TaskSecurityConfig;
///
/// let store = InMemoryTaskStore::new()
///     .with_config(StoreConfig {
///         max_ttl_ms: Some(7_200_000), // 2 hours
///         ..StoreConfig::default()
///     })
///     .with_security(TaskSecurityConfig::default()
///         .with_max_tasks_per_owner(50));
/// ```
#[derive(Debug)]
pub struct InMemoryTaskStore {
    /// Task storage keyed by task ID.
    tasks: DashMap<String, TaskRecord>,
    /// Storage-level configuration (variable size, TTL limits).
    config: StoreConfig,
    /// Owner-specific security configuration.
    security: TaskSecurityConfig,
    /// Default poll interval in milliseconds suggested to clients.
    default_poll_interval: u64,
}

impl InMemoryTaskStore {
    /// Creates a new in-memory task store with default configuration.
    ///
    /// Defaults:
    /// - `StoreConfig::default()` (1MB variables, 1h default TTL, 24h max TTL)
    /// - `TaskSecurityConfig::default()` (100 tasks/owner, no anonymous access)
    /// - Poll interval: 5000ms
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    ///
    /// let store = InMemoryTaskStore::new();
    /// ```
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            config: StoreConfig::default(),
            security: TaskSecurityConfig::default(),
            default_poll_interval: 5000,
        }
    }

    /// Sets the storage configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// use pmcp_tasks::store::StoreConfig;
    ///
    /// let store = InMemoryTaskStore::new()
    ///     .with_config(StoreConfig {
    ///         max_variable_size_bytes: 512_000,
    ///         ..StoreConfig::default()
    ///     });
    /// ```
    pub fn with_config(mut self, config: StoreConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the security configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// use pmcp_tasks::security::TaskSecurityConfig;
    ///
    /// let store = InMemoryTaskStore::new()
    ///     .with_security(TaskSecurityConfig::default()
    ///         .with_max_tasks_per_owner(25));
    /// ```
    pub fn with_security(mut self, security: TaskSecurityConfig) -> Self {
        self.security = security;
        self
    }

    /// Sets the default poll interval suggested to clients.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    ///
    /// let store = InMemoryTaskStore::new()
    ///     .with_poll_interval(3000);
    /// ```
    pub fn with_poll_interval(mut self, ms: u64) -> Self {
        self.default_poll_interval = ms;
        self
    }

    /// Checks if the given owner ID represents anonymous/local access.
    fn is_anonymous_owner(owner_id: &str) -> bool {
        owner_id.is_empty() || owner_id == DEFAULT_LOCAL_OWNER
    }

    /// Validates that the owner has permission to create tasks.
    ///
    /// Returns an error if anonymous access is disabled and the owner
    /// is anonymous/local.
    fn check_anonymous_access(&self, owner_id: &str) -> Result<(), TaskError> {
        if !self.security.allow_anonymous && Self::is_anonymous_owner(owner_id) {
            return Err(TaskError::StoreError(
                "anonymous access is not allowed; configure OAuth or enable allow_anonymous"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// Counts the number of tasks owned by the given owner.
    fn count_owner_tasks(&self, owner_id: &str) -> usize {
        self.tasks
            .iter()
            .filter(|entry| entry.value().owner_id == owner_id)
            .count()
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskStore for InMemoryTaskStore {
    /// Creates a new task in the `Working` state.
    ///
    /// Enforces:
    /// - Anonymous access check (if `allow_anonymous` is false)
    /// - Max tasks per owner limit (hard reject, no auto-eviction)
    /// - TTL maximum (hard reject, no silent clamping)
    /// - Default TTL application when none provided
    async fn create(
        &self,
        owner_id: &str,
        request_method: &str,
        ttl: Option<u64>,
    ) -> Result<TaskRecord, TaskError> {
        // Check anonymous access
        self.check_anonymous_access(owner_id)?;

        // Check max tasks per owner
        let owner_task_count = self.count_owner_tasks(owner_id);
        if owner_task_count >= self.security.max_tasks_per_owner {
            return Err(TaskError::ResourceExhausted {
                suggested_action: Some(
                    "Cancel or wait for existing tasks to expire".to_string(),
                ),
            });
        }

        // Validate TTL against maximum (hard reject, no clamping)
        if let (Some(requested_ttl), Some(max_ttl)) = (ttl, self.config.max_ttl_ms) {
            if requested_ttl > max_ttl {
                return Err(TaskError::StoreError(format!(
                    "TTL {requested_ttl}ms exceeds maximum allowed {max_ttl}ms"
                )));
            }
        }

        // Apply default TTL if none provided
        let effective_ttl = ttl.or(self.config.default_ttl_ms);

        // Create the task record
        let mut record = TaskRecord::new(
            owner_id.to_string(),
            request_method.to_string(),
            effective_ttl,
        );
        record.task.poll_interval = Some(self.default_poll_interval);

        // Insert into store
        let task_id = record.task.task_id.clone();
        self.tasks.insert(task_id, record.clone());

        Ok(record)
    }

    /// Retrieves a task by ID, scoped to the given owner.
    ///
    /// Returns the task even if expired (callers check `is_expired()`).
    /// Owner mismatch returns `NotFound` for security.
    async fn get(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskError> {
        let entry = self
            .tasks
            .get(task_id)
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.to_string(),
            })?;

        let record = entry.value();

        // Structural owner isolation: mismatch looks like NotFound
        if record.owner_id != owner_id {
            tracing::warn!(
                task_id = task_id,
                expected_owner = owner_id,
                actual_owner = record.owner_id,
                "owner mismatch on task get (returning NotFound)"
            );
            return Err(TaskError::NotFound {
                task_id: task_id.to_string(),
            });
        }

        // Return record even if expired per locked decision.
        // Expired tasks are readable; only mutation operations reject them.
        Ok(record.clone())
    }

    /// Transitions a task to a new status with atomic validation.
    ///
    /// Validates owner, expiry, and state machine transition within the
    /// DashMap entry lock for atomicity.
    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        new_status: TaskStatus,
        status_message: Option<String>,
    ) -> Result<TaskRecord, TaskError> {
        let mut entry = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.to_string(),
            })?;

        let record = entry.value_mut();

        // Owner isolation
        if record.owner_id != owner_id {
            tracing::warn!(
                task_id = task_id,
                expected_owner = owner_id,
                actual_owner = record.owner_id,
                "owner mismatch on task update_status (returning NotFound)"
            );
            return Err(TaskError::NotFound {
                task_id: task_id.to_string(),
            });
        }

        // Reject mutations on expired tasks
        if record.is_expired() {
            return Err(TaskError::Expired {
                task_id: task_id.to_string(),
                expired_at: record.expires_at.map(|e| e.to_rfc3339()),
            });
        }

        // Validate state machine transition
        record
            .task
            .status
            .validate_transition(task_id, &new_status)?;

        // Apply transition atomically (within DashMap entry lock)
        record.task.status = new_status;
        record.task.status_message = status_message;
        record.task.last_updated_at =
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        Ok(record.clone())
    }

    /// Merges variables with null-deletion semantics and size validation.
    ///
    /// Uses a clone-check-commit pattern: merges on a clone first, checks
    /// the serialized size, and only commits if within limits. This avoids
    /// needing rollback on size violations.
    async fn set_variables(
        &self,
        task_id: &str,
        owner_id: &str,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError> {
        let mut entry = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.to_string(),
            })?;

        let record = entry.value_mut();

        // Owner isolation
        if record.owner_id != owner_id {
            tracing::warn!(
                task_id = task_id,
                expected_owner = owner_id,
                actual_owner = record.owner_id,
                "owner mismatch on task set_variables (returning NotFound)"
            );
            return Err(TaskError::NotFound {
                task_id: task_id.to_string(),
            });
        }

        // Reject mutations on expired tasks
        if record.is_expired() {
            return Err(TaskError::Expired {
                task_id: task_id.to_string(),
                expired_at: record.expires_at.map(|e| e.to_rfc3339()),
            });
        }

        // Clone-check-commit: merge on a clone first, validate size, then commit
        let mut merged = record.variables.clone();
        for (key, value) in &variables {
            if value.is_null() {
                merged.remove(key);
            } else {
                merged.insert(key.clone(), value.clone());
            }
        }

        // Check merged size against limit
        let serialized = serde_json::to_vec(&merged).map_err(|e| {
            TaskError::StoreError(format!("failed to serialize variables: {e}"))
        })?;

        if serialized.len() > self.config.max_variable_size_bytes {
            return Err(TaskError::VariableSizeExceeded {
                limit_bytes: self.config.max_variable_size_bytes,
                actual_bytes: serialized.len(),
            });
        }

        // Commit the merged variables
        record.variables = merged;
        record.task.last_updated_at =
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        Ok(record.clone())
    }

    /// Stores the operation result for a task.
    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: Value,
    ) -> Result<(), TaskError> {
        let mut entry = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.to_string(),
            })?;

        let record = entry.value_mut();

        // Owner isolation
        if record.owner_id != owner_id {
            tracing::warn!(
                task_id = task_id,
                expected_owner = owner_id,
                actual_owner = record.owner_id,
                "owner mismatch on task set_result (returning NotFound)"
            );
            return Err(TaskError::NotFound {
                task_id: task_id.to_string(),
            });
        }

        // Reject mutations on expired tasks
        if record.is_expired() {
            return Err(TaskError::Expired {
                task_id: task_id.to_string(),
                expired_at: record.expires_at.map(|e| e.to_rfc3339()),
            });
        }

        record.result = Some(result);
        record.task.last_updated_at =
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        Ok(())
    }

    /// Retrieves the stored result for a completed task.
    ///
    /// Returns `NotReady` if the task has not reached a terminal state.
    async fn get_result(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<Value, TaskError> {
        let entry = self
            .tasks
            .get(task_id)
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.to_string(),
            })?;

        let record = entry.value();

        // Owner isolation
        if record.owner_id != owner_id {
            tracing::warn!(
                task_id = task_id,
                expected_owner = owner_id,
                actual_owner = record.owner_id,
                "owner mismatch on task get_result (returning NotFound)"
            );
            return Err(TaskError::NotFound {
                task_id: task_id.to_string(),
            });
        }

        // Must be in a terminal state to retrieve result
        if !record.task.status.is_terminal() {
            return Err(TaskError::NotReady {
                task_id: task_id.to_string(),
                current_status: record.task.status,
            });
        }

        record.result.clone().ok_or(TaskError::NotReady {
            task_id: task_id.to_string(),
            current_status: record.task.status,
        })
    }

    /// Atomically transitions to a terminal status AND stores the result.
    ///
    /// Both the status transition and result storage happen within a single
    /// DashMap entry lock, guaranteeing atomicity.
    async fn complete_with_result(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        status_message: Option<String>,
        result: Value,
    ) -> Result<TaskRecord, TaskError> {
        let mut entry = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| TaskError::NotFound {
                task_id: task_id.to_string(),
            })?;

        let record = entry.value_mut();

        // Owner isolation
        if record.owner_id != owner_id {
            tracing::warn!(
                task_id = task_id,
                expected_owner = owner_id,
                actual_owner = record.owner_id,
                "owner mismatch on task complete_with_result (returning NotFound)"
            );
            return Err(TaskError::NotFound {
                task_id: task_id.to_string(),
            });
        }

        // Reject mutations on expired tasks
        if record.is_expired() {
            return Err(TaskError::Expired {
                task_id: task_id.to_string(),
                expired_at: record.expires_at.map(|e| e.to_rfc3339()),
            });
        }

        // Validate state machine transition
        record
            .task
            .status
            .validate_transition(task_id, &status)?;

        // Atomically apply status + result (within DashMap entry lock)
        record.task.status = status;
        record.task.status_message = status_message;
        record.result = Some(result);
        record.task.last_updated_at =
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        Ok(record.clone())
    }

    /// Lists tasks for an owner with cursor-based pagination.
    ///
    /// Results are sorted by creation time (newest first). The cursor is
    /// the task ID of the last item in the previous page.
    async fn list(
        &self,
        options: ListTasksOptions,
    ) -> Result<TaskPage, TaskError> {
        // Collect and filter by owner
        let mut tasks: Vec<TaskRecord> = self
            .tasks
            .iter()
            .filter(|entry| entry.value().owner_id == options.owner_id)
            .map(|entry| entry.value().clone())
            .collect();

        // Sort by creation time, newest first
        tasks.sort_by(|a, b| b.task.created_at.cmp(&a.task.created_at));

        // Cursor-based pagination: cursor = task_id of last item in previous page
        let start_idx = if let Some(ref cursor) = options.cursor {
            tasks
                .iter()
                .position(|t| t.task.task_id == *cursor)
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        };

        let limit = options.limit.unwrap_or(50);
        let page_tasks: Vec<TaskRecord> = tasks
            .get(start_idx..)
            .unwrap_or(&[])
            .iter()
            .take(limit)
            .cloned()
            .collect();

        let next_cursor = if start_idx + limit < tasks.len() {
            page_tasks.last().map(|t| t.task.task_id.clone())
        } else {
            None
        };

        Ok(TaskPage {
            tasks: page_tasks,
            next_cursor,
        })
    }

    /// Cancels a non-terminal task.
    ///
    /// Delegates to [`update_status`](TaskStore::update_status) with
    /// `TaskStatus::Cancelled`.
    async fn cancel(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskError> {
        self.update_status(task_id, owner_id, TaskStatus::Cancelled, None)
            .await
    }

    /// Removes expired tasks from storage.
    ///
    /// Uses [`DashMap::retain`] for atomic per-entry removal. Returns the
    /// count of tasks removed.
    async fn cleanup_expired(&self) -> Result<usize, TaskError> {
        let before = self.tasks.len();
        self.tasks.retain(|_, record| !record.is_expired());
        let after = self.tasks.len();
        Ok(before - after)
    }

    /// Returns a reference to the store's configuration.
    fn config(&self) -> &StoreConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: creates a store with anonymous access enabled.
    fn test_store() -> InMemoryTaskStore {
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true))
    }

    /// Helper: creates a store with a specific max tasks limit.
    fn store_with_max_tasks(max: usize) -> InMemoryTaskStore {
        InMemoryTaskStore::new().with_security(
            TaskSecurityConfig::default()
                .with_max_tasks_per_owner(max)
                .with_allow_anonymous(true),
        )
    }

    // --- Constructor and builder tests ---

    #[test]
    fn new_creates_empty_store() {
        let store = InMemoryTaskStore::new();
        assert_eq!(store.tasks.len(), 0);
        assert_eq!(store.default_poll_interval, 5000);
    }

    #[test]
    fn default_delegates_to_new() {
        let store = InMemoryTaskStore::default();
        assert_eq!(store.tasks.len(), 0);
        assert_eq!(store.default_poll_interval, 5000);
    }

    #[test]
    fn with_config_sets_config() {
        let store = InMemoryTaskStore::new().with_config(StoreConfig {
            max_variable_size_bytes: 512_000,
            ..StoreConfig::default()
        });
        assert_eq!(store.config.max_variable_size_bytes, 512_000);
    }

    #[test]
    fn with_security_sets_security() {
        let store = InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_max_tasks_per_owner(42));
        assert_eq!(store.security.max_tasks_per_owner, 42);
    }

    #[test]
    fn with_poll_interval_sets_interval() {
        let store = InMemoryTaskStore::new().with_poll_interval(3000);
        assert_eq!(store.default_poll_interval, 3000);
    }

    // --- Create tests ---

    #[tokio::test]
    async fn create_returns_working_task() {
        let store = test_store();
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        assert_eq!(record.task.status, TaskStatus::Working);
        assert_eq!(record.owner_id, "owner-1");
        assert_eq!(record.request_method, "tools/call");
        assert_eq!(record.task.poll_interval, Some(5000));
    }

    #[tokio::test]
    async fn create_applies_default_ttl() {
        let store = test_store();
        let record = store.create("owner-1", "tools/call", None).await.unwrap();
        // Default TTL from StoreConfig is 3_600_000 (1 hour)
        assert_eq!(record.task.ttl, Some(3_600_000));
        assert!(record.expires_at.is_some());
    }

    #[tokio::test]
    async fn create_uses_explicit_ttl() {
        let store = test_store();
        let record = store
            .create("owner-1", "tools/call", Some(30_000))
            .await
            .unwrap();
        assert_eq!(record.task.ttl, Some(30_000));
    }

    #[tokio::test]
    async fn create_rejects_ttl_above_max() {
        let store = test_store();
        let result = store
            .create("owner-1", "tools/call", Some(100_000_000))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("TTL"),
            "error should mention TTL: {err}"
        );
    }

    #[tokio::test]
    async fn create_rejects_anonymous_when_disabled() {
        let store = InMemoryTaskStore::new(); // allow_anonymous = false (default)
        let result = store.create("local", "tools/call", None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("anonymous access"));
    }

    #[tokio::test]
    async fn create_rejects_empty_owner_when_anonymous_disabled() {
        let store = InMemoryTaskStore::new();
        let result = store.create("", "tools/call", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn create_allows_anonymous_when_enabled() {
        let store = test_store();
        let result = store.create("local", "tools/call", None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn create_enforces_max_tasks_per_owner() {
        let store = store_with_max_tasks(3);
        for i in 0..3 {
            store
                .create("owner-1", &format!("tools/call-{i}"), None)
                .await
                .unwrap();
        }
        let result = store.create("owner-1", "tools/call-extra", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskError::ResourceExhausted { suggested_action } => {
                assert!(suggested_action.is_some());
            }
            other => panic!("expected ResourceExhausted, got: {other}"),
        }
    }

    #[tokio::test]
    async fn create_max_tasks_scoped_to_owner() {
        let store = store_with_max_tasks(2);
        // Owner A fills their quota
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-a", "tools/call", None).await.unwrap();
        // Owner B can still create
        let result = store.create("owner-b", "tools/call", None).await;
        assert!(result.is_ok());
    }

    // --- Get tests ---

    #[tokio::test]
    async fn get_returns_created_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let fetched = store
            .get(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert_eq!(fetched.task.task_id, created.task.task_id);
    }

    #[tokio::test]
    async fn get_returns_not_found_for_missing_task() {
        let store = test_store();
        let result = store.get("nonexistent-id", "owner-1").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn get_returns_not_found_on_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        // Owner B tries to access Owner A's task
        let result = store.get(&created.task.task_id, "owner-b").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn get_returns_expired_task() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force the task to be expired
        let mut entry = store.tasks.get_mut(&created.task.task_id).unwrap();
        entry.value_mut().expires_at =
            Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        drop(entry);

        // Get should still succeed (expired tasks are readable)
        let fetched = store
            .get(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert!(fetched.is_expired());
    }

    // --- Update status tests ---

    #[tokio::test]
    async fn update_status_valid_transition() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let updated = store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                Some("Done".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(updated.task.status, TaskStatus::Completed);
        assert_eq!(updated.task.status_message.as_deref(), Some("Done"));
    }

    #[tokio::test]
    async fn update_status_invalid_transition() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        // Complete the task first
        store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
            )
            .await
            .unwrap();
        // Try to transition from terminal state
        let result = store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Working,
                None,
            )
            .await;
        assert!(matches!(result, Err(TaskError::InvalidTransition { .. })));
    }

    #[tokio::test]
    async fn update_status_rejects_expired_task() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force expiry
        let mut entry = store.tasks.get_mut(&created.task.task_id).unwrap();
        entry.value_mut().expires_at =
            Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        drop(entry);

        let result = store
            .update_status(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
            )
            .await;
        assert!(matches!(result, Err(TaskError::Expired { .. })));
    }

    #[tokio::test]
    async fn update_status_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store
            .update_status(
                &created.task.task_id,
                "owner-b",
                TaskStatus::Completed,
                None,
            )
            .await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Set variables tests ---

    #[tokio::test]
    async fn set_variables_upserts_values() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), json!("value1"));
        vars.insert("key2".to_string(), json!(42));

        let updated = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();
        assert_eq!(updated.variables.get("key1").unwrap(), &json!("value1"));
        assert_eq!(updated.variables.get("key2").unwrap(), &json!(42));
    }

    #[tokio::test]
    async fn set_variables_null_deletes_key() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // Set initial variable
        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), json!("value1"));
        store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();

        // Delete via null
        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), Value::Null);
        let updated = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();
        assert!(updated.variables.get("key1").is_none());
    }

    #[tokio::test]
    async fn set_variables_rejects_oversized_payload() {
        let store = InMemoryTaskStore::new()
            .with_config(StoreConfig {
                max_variable_size_bytes: 100,
                ..StoreConfig::default()
            })
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        let mut vars = HashMap::new();
        vars.insert("big".to_string(), json!("x".repeat(200)));
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(matches!(
            result,
            Err(TaskError::VariableSizeExceeded { .. })
        ));
    }

    #[tokio::test]
    async fn set_variables_checks_merged_size() {
        let store = InMemoryTaskStore::new()
            .with_config(StoreConfig {
                max_variable_size_bytes: 100,
                ..StoreConfig::default()
            })
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // First set: small enough
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!("x".repeat(30)));
        store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await
            .unwrap();

        // Second set: combined exceeds limit
        let mut vars = HashMap::new();
        vars.insert("b".to_string(), json!("y".repeat(60)));
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(matches!(
            result,
            Err(TaskError::VariableSizeExceeded { .. })
        ));

        // Verify original variables are unchanged (clone-check-commit)
        let record = store
            .get(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert!(record.variables.contains_key("a"));
        assert!(!record.variables.contains_key("b"));
    }

    #[tokio::test]
    async fn set_variables_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let mut vars = HashMap::new();
        vars.insert("key".to_string(), json!("val"));
        let result = store
            .set_variables(&created.task.task_id, "owner-b", vars)
            .await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn set_variables_rejects_expired() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force expiry
        let mut entry = store.tasks.get_mut(&created.task.task_id).unwrap();
        entry.value_mut().expires_at =
            Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        drop(entry);

        let mut vars = HashMap::new();
        vars.insert("key".to_string(), json!("val"));
        let result = store
            .set_variables(&created.task.task_id, "owner-1", vars)
            .await;
        assert!(matches!(result, Err(TaskError::Expired { .. })));
    }

    // --- Set result tests ---

    #[tokio::test]
    async fn set_result_stores_value() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        store
            .set_result(&created.task.task_id, "owner-1", json!({"answer": 42}))
            .await
            .unwrap();

        let record = store
            .get(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert_eq!(record.result, Some(json!({"answer": 42})));
    }

    #[tokio::test]
    async fn set_result_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store
            .set_result(&created.task.task_id, "owner-b", json!("data"))
            .await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Get result tests ---

    #[tokio::test]
    async fn get_result_from_completed_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // Complete with result
        store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
                json!({"output": "done"}),
            )
            .await
            .unwrap();

        let result = store
            .get_result(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert_eq!(result, json!({"output": "done"}));
    }

    #[tokio::test]
    async fn get_result_not_ready_for_working_task() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let result = store
            .get_result(&created.task.task_id, "owner-1")
            .await;
        assert!(matches!(result, Err(TaskError::NotReady { .. })));
    }

    #[tokio::test]
    async fn get_result_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        store
            .complete_with_result(
                &created.task.task_id,
                "owner-a",
                TaskStatus::Completed,
                None,
                json!("result"),
            )
            .await
            .unwrap();
        let result = store
            .get_result(&created.task.task_id, "owner-b")
            .await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Complete with result tests ---

    #[tokio::test]
    async fn complete_with_result_atomic() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        let completed = store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                Some("All done".to_string()),
                json!({"data": true}),
            )
            .await
            .unwrap();

        assert_eq!(completed.task.status, TaskStatus::Completed);
        assert_eq!(
            completed.task.status_message.as_deref(),
            Some("All done")
        );
        assert_eq!(completed.result, Some(json!({"data": true})));
    }

    #[tokio::test]
    async fn complete_with_result_rejects_invalid_transition() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();

        // Complete first
        store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Completed,
                None,
                json!("first"),
            )
            .await
            .unwrap();

        // Try again from terminal state
        let result = store
            .complete_with_result(
                &created.task.task_id,
                "owner-1",
                TaskStatus::Failed,
                None,
                json!("second"),
            )
            .await;
        assert!(matches!(result, Err(TaskError::InvalidTransition { .. })));
    }

    // --- List tests ---

    #[tokio::test]
    async fn list_scoped_to_owner() {
        let store = test_store();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-b", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "owner-a".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(page.tasks.len(), 2);
        assert!(page.tasks.iter().all(|t| t.owner_id == "owner-a"));
    }

    #[tokio::test]
    async fn list_sorted_newest_first() {
        let store = test_store();
        let first = store.create("owner-1", "tools/call", None).await.unwrap();

        // Ensure second task has a strictly later timestamp
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let second = store.create("owner-1", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(page.tasks.len(), 2);
        // Newest first
        assert!(page.tasks[0].task.created_at >= page.tasks[1].task.created_at);
        // The second task was created later so should appear first
        assert_eq!(page.tasks[1].task.task_id, first.task.task_id);
        assert_eq!(page.tasks[0].task.task_id, second.task.task_id);
    }

    #[tokio::test]
    async fn list_pagination() {
        let store = test_store();

        // Create 5 tasks
        let mut task_ids = Vec::new();
        for _ in 0..5 {
            let record = store.create("owner-1", "tools/call", None).await.unwrap();
            task_ids.push(record.task.task_id);
        }

        // First page (limit 2)
        let page1 = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: None,
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page1.tasks.len(), 2);
        assert!(page1.next_cursor.is_some());

        // Second page using cursor
        let page2 = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: page1.next_cursor.clone(),
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page2.tasks.len(), 2);
        assert!(page2.next_cursor.is_some());

        // Third page (last)
        let page3 = store
            .list(ListTasksOptions {
                owner_id: "owner-1".to_string(),
                cursor: page2.next_cursor.clone(),
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page3.tasks.len(), 1);
        assert!(page3.next_cursor.is_none());
    }

    #[tokio::test]
    async fn list_empty_for_unknown_owner() {
        let store = test_store();
        store.create("owner-a", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "owner-b".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();
        assert!(page.tasks.is_empty());
        assert!(page.next_cursor.is_none());
    }

    // --- Cancel tests ---

    #[tokio::test]
    async fn cancel_transitions_to_cancelled() {
        let store = test_store();
        let created = store.create("owner-1", "tools/call", None).await.unwrap();
        let cancelled = store
            .cancel(&created.task.task_id, "owner-1")
            .await
            .unwrap();
        assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn cancel_owner_mismatch() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store.cancel(&created.task.task_id, "owner-b").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    // --- Cleanup expired tests ---

    #[tokio::test]
    async fn cleanup_expired_removes_expired_tasks() {
        let store = test_store();
        let created = store
            .create("owner-1", "tools/call", Some(60_000))
            .await
            .unwrap();

        // Force expiry
        let mut entry = store.tasks.get_mut(&created.task.task_id).unwrap();
        entry.value_mut().expires_at =
            Some(chrono::Utc::now() - chrono::Duration::seconds(10));
        drop(entry);

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);
        assert_eq!(store.tasks.len(), 0);
    }

    #[tokio::test]
    async fn cleanup_expired_keeps_non_expired() {
        let store = test_store();
        store
            .create("owner-1", "tools/call", Some(3_600_000))
            .await
            .unwrap();
        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
        assert_eq!(store.tasks.len(), 1);
    }

    // --- Config accessor test ---

    #[tokio::test]
    async fn config_returns_store_config() {
        let store = InMemoryTaskStore::new().with_config(StoreConfig {
            max_variable_size_bytes: 999,
            ..StoreConfig::default()
        });
        assert_eq!(store.config().max_variable_size_bytes, 999);
    }
}
