//! SDK-level task store trait and in-memory implementation.
//!
//! This module provides `TaskStore`, the core trait for task lifecycle
//! management within the SDK, and `InMemoryTaskStore`, a thread-safe
//! in-memory implementation suitable for development and testing.
//!
//! # Architecture
//!
//! The SDK defines the trait and a dev/test implementation. Production
//! backends (DynamoDB, Redis) live in the `pmcp-tasks` crate. This
//! follows the TypeScript SDK pattern where task store interfaces and
//! an in-memory implementation are part of core.
//!
//! # Differences from `pmcp-tasks`
//!
//! The SDK `TaskStore` trait is intentionally simplified compared to
//! the `pmcp-tasks` `TaskStore` (see <https://docs.rs/pmcp-tasks/latest/pmcp_tasks/store/trait.TaskStore.html>):
//! - No `set_variables` / `get_result` / `set_result` / `complete_with_result`
//! - No `request_method` parameter on `create`
//! - Returns `Task` (wire type) instead of `TaskRecord`
//!
//! These PMCP extensions remain in `pmcp-tasks`. The SDK trait covers
//! the core MCP spec operations only.
//!
//! # Recommended usage: expose a tool as an MCP Task
//!
//! The clean, correct-by-construction pattern (Phase 101) is: register a
//! task-capable tool (a [`TypedTool`](crate::server::typed_tool::TypedTool)
//! marked [`with_task_support(TaskSupport::Required)`](crate::types::ToolExecution::with_task_support))
//! plus a [`TaskStore`](crate::server::task_store::TaskStore) on
//! [`ServerCoreBuilder`](crate::server::builder::ServerCoreBuilder),
//! then let the SDK serve `tasks/get`, `tasks/result`, `tasks/list`, and
//! `tasks/cancel` typed. You never hand-write any `tasks/*` wire JSON — the SDK
//! serializes from the typed structs — and the store mints the task id.
//!
//! Registering a store via [`task_store`](crate::server::builder::ServerCoreBuilder::task_store)
//! also auto-advertises the `tasks` capability in `initialize` (a
//! [`TaskSupport::Required`](crate::types::tools::TaskSupport::Required) tool
//! with NO store makes [`build()`](crate::server::builder::ServerCoreBuilder::build)
//! return an error, never a hollow capability).
//!
//! ```no_run
//! use std::sync::Arc;
//! use pmcp::server::builder::ServerCoreBuilder;
//! use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
//! use pmcp::server::typed_tool::TypedTool;
//! use pmcp::types::{TaskSupport, ToolExecution};
//!
//! # fn build() -> pmcp::Result<()> {
//! let task_tool = TypedTool::new_with_schema(
//!     "summarize",
//!     serde_json::json!({ "type": "object" }),
//!     |_args: serde_json::Value, _extra| {
//!         Box::pin(async { Ok(serde_json::json!({ "status": "completed" })) })
//!     },
//! )
//! .with_description("Summarize asynchronously as an MCP Task")
//! .with_execution(ToolExecution::new().with_task_support(TaskSupport::Required));
//!
//! let store = Arc::new(InMemoryTaskStore::new()) as Arc<dyn TaskStore>;
//! let server = ServerCoreBuilder::new()
//!     .name("my-server")
//!     .version("1.0.0")
//!     .tool("summarize", task_tool)
//!     .task_store(store) // presence of a store auto-advertises the `tasks` capability
//!     .build()?;
//! # let _ = server;
//! # Ok(())
//! # }
//! ```
//!
//! The task path currently lives on
//! [`ServerCoreBuilder`](crate::server::builder::ServerCoreBuilder) /
//! [`ServerCore`](crate::server::core::ServerCore); the high-level
//! `pmcp::Server` (and `StreamableHttpServer`) does not yet carry a
//! [`TaskStore`](crate::server::task_store::TaskStore). See
//! `examples/s45_tool_as_task_lifecycle.rs` for the full client round-trip
//! (`initialize → call(task) → tasks/get poll → tasks/result`).
//!
//! Note: [`with_task_store(Arc<dyn TaskRouter>)`](crate::server::builder::ServerCoreBuilder::with_task_store)
//! is the LEGACY experimental (`pmcp-tasks`) path — prefer
//! [`task_store(...)`](crate::server::builder::ServerCoreBuilder::task_store) +
//! `with_task_support`.
//!
//! # Examples
//!
//! ```no_run
//! use pmcp::server::task_store::{InMemoryTaskStore, TaskStore, StoreConfig};
//!
//! # async fn example() {
//! let store = InMemoryTaskStore::new();
//! let task = store.create("session-abc", None).await.unwrap();
//! assert_eq!(task.status, pmcp::types::tasks::TaskStatus::Working);
//! # }
//! ```

use async_trait::async_trait;
use dashmap::DashMap;
use std::time::Instant;

use crate::types::tasks::{Task, TaskStatus};
use crate::types::CallToolResult;

// ---------------------------------------------------------------------------
// TaskStoreError
// ---------------------------------------------------------------------------

/// Errors returned by [`TaskStore`] operations.
#[derive(Debug)]
pub enum TaskStoreError {
    /// The requested task was not found (or belongs to a different owner).
    NotFound {
        /// The task ID that was looked up.
        task_id: String,
    },
    /// The requested state transition is invalid per the MCP state machine.
    InvalidTransition {
        /// The task ID.
        task_id: String,
        /// Current status.
        from: TaskStatus,
        /// Attempted target status.
        to: TaskStatus,
    },
    /// The task has expired (TTL elapsed).
    Expired {
        /// The task ID.
        task_id: String,
    },
    /// An internal error occurred.
    Internal {
        /// Human-readable error message.
        message: String,
    },
}

impl std::fmt::Display for TaskStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { task_id } => write!(f, "task not found: {task_id}"),
            Self::InvalidTransition { task_id, from, to } => {
                write!(f, "invalid transition for task {task_id}: {from} -> {to}")
            },
            Self::Expired { task_id } => write!(f, "task expired: {task_id}"),
            Self::Internal { message } => write!(f, "internal error: {message}"),
        }
    }
}

impl std::error::Error for TaskStoreError {}

impl From<TaskStoreError> for crate::error::Error {
    fn from(err: TaskStoreError) -> Self {
        match &err {
            TaskStoreError::NotFound { .. } => Self::not_found(err.to_string()),
            TaskStoreError::InvalidTransition { .. } => Self::validation(err.to_string()),
            // Expired uses NotFound to avoid leaking existence of expired tasks
            TaskStoreError::Expired { .. } => Self::not_found(err.to_string()),
            TaskStoreError::Internal { .. } => Self::internal(err.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// StoreConfig
// ---------------------------------------------------------------------------

/// Configuration for the SDK task store.
///
/// Controls TTL defaults, poll intervals, and per-owner task limits.
///
/// # Defaults
///
/// | Setting                  | Default    | Description           |
/// |--------------------------|------------|-----------------------|
/// | `default_ttl_ms`         | 3,600,000  | 1 hour                |
/// | `max_ttl_ms`             | 86,400,000 | 24 hours              |
/// | `default_poll_interval_ms` | 5,000    | 5 seconds             |
/// | `max_tasks_per_owner`    | 100        | Per-owner task limit  |
///
/// # Examples
///
/// ```
/// use pmcp::server::task_store::StoreConfig;
///
/// let config = StoreConfig::default();
/// assert_eq!(config.default_ttl_ms, Some(3_600_000));
/// assert_eq!(config.max_ttl_ms, Some(86_400_000));
/// assert_eq!(config.default_poll_interval_ms, 5000);
/// assert_eq!(config.max_tasks_per_owner, 100);
/// ```
#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// Default TTL in milliseconds. Applied when `create()` receives `None`.
    pub default_ttl_ms: Option<u64>,
    /// Maximum allowed TTL in milliseconds. `None` means no upper bound.
    pub max_ttl_ms: Option<u64>,
    /// Default polling interval suggested to clients, in milliseconds.
    pub default_poll_interval_ms: u64,
    /// Maximum number of active tasks per owner.
    pub max_tasks_per_owner: usize,
}

impl Default for StoreConfig {
    fn default() -> Self {
        Self {
            default_ttl_ms: Some(3_600_000), // 1 hour
            max_ttl_ms: Some(86_400_000),    // 24 hours
            default_poll_interval_ms: 5000,  // 5 seconds
            max_tasks_per_owner: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// TaskStore trait
// ---------------------------------------------------------------------------

/// Core trait for MCP task lifecycle management.
///
/// Implementations must be `Send + Sync` for concurrent access from
/// multiple request handlers.
///
/// # Recommended usage
///
/// To expose a tool as an async MCP Task, register a task-capable
/// [`TypedTool`](crate::server::typed_tool::TypedTool) plus an implementation
/// of this trait (e.g. [`InMemoryTaskStore`]) on
/// [`ServerCoreBuilder::task_store`](crate::server::builder::ServerCoreBuilder::task_store);
/// the SDK then serves `tasks/get`, `tasks/result`, `tasks/list`, and
/// `tasks/cancel` typed from the store — you never hand-write `tasks/*` wire
/// JSON, and the store mints the task id. See the module-level docs and
/// `examples/s45_tool_as_task_lifecycle.rs` for the full pattern.
///
/// # Owner Isolation
///
/// All methods that access a specific task require an `owner_id`. If the
/// task belongs to a different owner, the store returns
/// [`TaskStoreError::NotFound`] (never revealing that the task exists
/// but belongs to someone else).
#[async_trait]
pub trait TaskStore: Send + Sync {
    /// Create a new task in the `Working` state.
    ///
    /// If `ttl` is `None`, the store's `default_ttl_ms` is applied.
    async fn create(&self, owner_id: &str, ttl: Option<u64>) -> Result<Task, TaskStoreError>;

    /// Retrieve a task by ID, scoped to the given owner.
    async fn get(&self, task_id: &str, owner_id: &str) -> Result<Task, TaskStoreError>;

    /// Transition a task to a new status with an optional status message.
    ///
    /// Validates the transition against the MCP state machine before applying.
    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        message: Option<String>,
    ) -> Result<Task, TaskStoreError>;

    /// List tasks for an owner with optional cursor-based pagination.
    ///
    /// Returns `(tasks, next_cursor)`. If `next_cursor` is `None`, there
    /// are no more results.
    async fn list(
        &self,
        owner_id: &str,
        cursor: Option<&str>,
    ) -> Result<(Vec<Task>, Option<String>), TaskStoreError>;

    /// Cancel a task (transition to `Cancelled`).
    async fn cancel(&self, task_id: &str, owner_id: &str) -> Result<Task, TaskStoreError>;

    /// Remove expired tasks. Returns the count of tasks removed.
    async fn cleanup_expired(&self) -> Result<usize, TaskStoreError>;

    /// Returns a reference to the store's configuration.
    fn config(&self) -> &StoreConfig;

    /// Persist the terminal [`CallToolResult`]
    /// for a completed task, scoped to `owner_id`.
    ///
    /// This is an **additive** trait method with a default implementation, so
    /// existing out-of-tree [`TaskStore`] implementations keep compiling. The
    /// default returns [`TaskStoreError::Internal`] to signal — explicitly,
    /// never silently — that the store does not persist terminal results.
    /// Stores that DO persist results (e.g. [`InMemoryTaskStore`]) override
    /// this method and also override [`TaskStore::supports_results`] to return
    /// `true`.
    ///
    /// Implementations MUST scope the write by `owner_id` (mirroring
    /// [`TaskStore::get`] / [`TaskStore::cancel`]) so one owner cannot set a
    /// result on another owner's task.
    ///
    /// # Errors
    ///
    /// The default implementation always returns [`TaskStoreError::Internal`]
    /// ("store does not support terminal results"). Overriding implementations
    /// return [`TaskStoreError::NotFound`] when the task does not exist or
    /// belongs to a different owner.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
    /// use pmcp::types::CallToolResult;
    /// use pmcp::types::Content;
    ///
    /// # async fn example() {
    /// let store = InMemoryTaskStore::new();
    /// let task = store.create("owner-1", None).await.unwrap();
    /// let result = CallToolResult::new(vec![Content::text("done")]);
    /// store
    ///     .set_result(&task.task_id, "owner-1", result)
    ///     .await
    ///     .unwrap();
    /// # }
    /// ```
    async fn set_result(
        &self,
        task_id: &str,
        _owner_id: &str,
        _result: crate::types::CallToolResult,
    ) -> Result<(), TaskStoreError> {
        let _ = task_id;
        Err(TaskStoreError::Internal {
            message: "store does not support terminal results".to_string(),
        })
    }

    /// Retrieve the persisted terminal
    /// [`CallToolResult`] for a task, scoped to
    /// `owner_id`.
    ///
    /// This is an **additive** trait method with a default implementation. The
    /// default returns [`TaskStoreError::NotFound`] — a store that does not
    /// persist results has none to return. Stores that persist results
    /// override this method.
    ///
    /// Implementations MUST return [`TaskStoreError::NotFound`] (never a
    /// distinct error) on owner mismatch, so the existence of another owner's
    /// task is never revealed. A task that exists but has no stored result yet
    /// (still pending) also returns [`TaskStoreError::NotFound`]; the dispatch
    /// layer turns that signal into a specified "not completed" error.
    ///
    /// # Errors
    ///
    /// Returns [`TaskStoreError::NotFound`] when no result is available for the
    /// task under the given owner (task absent, owner mismatch, or pending).
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp::server::task_store::{InMemoryTaskStore, TaskStore};
    /// use pmcp::types::{CallToolResult, Content};
    ///
    /// # async fn example() {
    /// let store = InMemoryTaskStore::new();
    /// let task = store.create("owner-1", None).await.unwrap();
    /// let result = CallToolResult::new(vec![Content::text("done")]);
    /// store
    ///     .set_result(&task.task_id, "owner-1", result)
    ///     .await
    ///     .unwrap();
    ///
    /// let fetched = store.get_result(&task.task_id, "owner-1").await.unwrap();
    /// assert_eq!(fetched.content.len(), 1);
    /// # }
    /// ```
    async fn get_result(
        &self,
        task_id: &str,
        _owner_id: &str,
    ) -> Result<crate::types::CallToolResult, TaskStoreError> {
        Err(TaskStoreError::NotFound {
            task_id: task_id.to_string(),
        })
    }

    /// Whether this store persists terminal results
    /// (i.e. [`TaskStore::set_result`] / [`TaskStore::get_result`] are real).
    ///
    /// Defaults to `false`. The dispatch layer consults this before serving the
    /// store-result path, so a store that cannot persist results falls through
    /// to the [`TaskRouter`](crate::server::tasks::TaskRouter) instead of
    /// silently dropping or serving empty results.
    fn supports_results(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Internal TaskRecord
// ---------------------------------------------------------------------------

/// Internal record wrapping a [`Task`] with owner and expiration metadata.
///
/// The `result` field holds the terminal [`CallToolResult`] for a completed
/// task. It lives on this INTERNAL record (never on the wire [`Task`], whose
/// shape is locked) so it is purged together with the task by
/// [`InMemoryTaskStore::cleanup_expired`] — no separate unexpiring map.
#[derive(Debug)]
struct TaskRecord {
    task: Task,
    owner_id: String,
    expires_at: Option<Instant>,
    result: Option<CallToolResult>,
}

// ---------------------------------------------------------------------------
// InMemoryTaskStore
// ---------------------------------------------------------------------------

/// Thread-safe in-memory task store using [`DashMap`].
///
/// Suitable for development and testing. For production use, see the
/// `pmcp-tasks` crate which provides `DynamoDB` and Redis backends.
///
/// # Examples
///
/// ```
/// use pmcp::server::task_store::{InMemoryTaskStore, StoreConfig};
///
/// let store = InMemoryTaskStore::with_config(StoreConfig {
///     default_poll_interval_ms: 3000,
///     ..StoreConfig::default()
/// });
/// ```
#[derive(Debug)]
pub struct InMemoryTaskStore {
    records: DashMap<String, TaskRecord>,
    config: StoreConfig,
}

impl InMemoryTaskStore {
    /// Create an in-memory task store with default configuration.
    pub fn new() -> Self {
        Self {
            records: DashMap::new(),
            config: StoreConfig::default(),
        }
    }

    /// Create an in-memory task store with custom configuration.
    pub fn with_config(config: StoreConfig) -> Self {
        Self {
            records: DashMap::new(),
            config,
        }
    }

    /// Validate owner and expiration for a task record.
    fn validate_access(
        record: &TaskRecord,
        task_id: &str,
        owner_id: &str,
    ) -> Result<(), TaskStoreError> {
        if record.owner_id != owner_id {
            return Err(TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            });
        }
        if let Some(expires_at) = record.expires_at {
            if Instant::now() > expires_at {
                return Err(TaskStoreError::Expired {
                    task_id: task_id.to_string(),
                });
            }
        }
        Ok(())
    }
}

impl Default for InMemoryTaskStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TaskStore for InMemoryTaskStore {
    async fn create(&self, owner_id: &str, ttl: Option<u64>) -> Result<Task, TaskStoreError> {
        // Enforce max_tasks_per_owner (excludes expired tasks)
        let now = Instant::now();
        let owner_count = self
            .records
            .iter()
            .filter(|entry| {
                let v = entry.value();
                v.owner_id == owner_id && v.expires_at.is_none_or(|e| now <= e)
            })
            .count();
        if owner_count >= self.config.max_tasks_per_owner {
            return Err(TaskStoreError::Internal {
                message: format!(
                    "owner {owner_id} has reached the maximum of {} tasks",
                    self.config.max_tasks_per_owner
                ),
            });
        }

        let task_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        let now_str = now.to_rfc3339();

        let effective_ttl = ttl.or(self.config.default_ttl_ms);

        // Clamp to max_ttl_ms if configured
        let effective_ttl = match (effective_ttl, self.config.max_ttl_ms) {
            (Some(t), Some(max)) if t > max => Some(max),
            (t, _) => t,
        };

        let expires_at =
            effective_ttl.map(|ms| Instant::now() + std::time::Duration::from_millis(ms));

        let task = Task::new(&task_id, TaskStatus::Working)
            .with_timestamps(&now_str, &now_str)
            .with_poll_interval(self.config.default_poll_interval_ms);

        let task = if let Some(ttl_val) = effective_ttl {
            task.with_ttl(ttl_val)
        } else {
            task
        };

        let record = TaskRecord {
            task: task.clone(),
            owner_id: owner_id.to_string(),
            expires_at,
            result: None,
        };

        self.records.insert(task_id, record);
        Ok(task)
    }

    async fn get(&self, task_id: &str, owner_id: &str) -> Result<Task, TaskStoreError> {
        let entry = self
            .records
            .get(task_id)
            .ok_or_else(|| TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            })?;
        Self::validate_access(entry.value(), task_id, owner_id)?;
        Ok(entry.value().task.clone())
    }

    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        status: TaskStatus,
        message: Option<String>,
    ) -> Result<Task, TaskStoreError> {
        let mut entry = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            })?;

        let record = entry.value_mut();
        Self::validate_access(record, task_id, owner_id)?;

        // Validate state machine transition
        if !record.task.status.can_transition_to(&status) {
            return Err(TaskStoreError::InvalidTransition {
                task_id: task_id.to_string(),
                from: record.task.status,
                to: status,
            });
        }

        let now_str = chrono::Utc::now().to_rfc3339();
        record.task.status = status;
        record.task.last_updated_at = now_str;
        record.task.status_message = message;

        Ok(record.task.clone())
    }

    async fn list(
        &self,
        owner_id: &str,
        cursor: Option<&str>,
    ) -> Result<(Vec<Task>, Option<String>), TaskStoreError> {
        const PAGE_SIZE: usize = 20;
        let now = Instant::now();
        let mut tasks: Vec<Task> = self
            .records
            .iter()
            .filter(|entry| {
                let v = entry.value();
                v.owner_id == owner_id && v.expires_at.is_none_or(|e| now <= e)
            })
            .map(|entry| entry.value().task.clone())
            .collect();

        // Sort by created_at descending (newest first)
        tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        // Apply cursor-based pagination (cursor = task_id of last item)
        if let Some(cursor_id) = cursor {
            if let Some(pos) = tasks.iter().position(|t| t.task_id == cursor_id) {
                tasks = tasks.into_iter().skip(pos + 1).collect();
            }
        }

        if tasks.len() > PAGE_SIZE {
            let next_cursor = tasks[PAGE_SIZE - 1].task_id.clone();
            tasks.truncate(PAGE_SIZE);
            Ok((tasks, Some(next_cursor)))
        } else {
            Ok((tasks, None))
        }
    }

    async fn cancel(&self, task_id: &str, owner_id: &str) -> Result<Task, TaskStoreError> {
        self.update_status(task_id, owner_id, TaskStatus::Cancelled, None)
            .await
    }

    async fn cleanup_expired(&self) -> Result<usize, TaskStoreError> {
        let now = Instant::now();
        let before = self.records.len();
        self.records
            .retain(|_, record| record.expires_at.is_none_or(|e| now <= e));
        Ok(before - self.records.len())
    }

    fn config(&self) -> &StoreConfig {
        &self.config
    }

    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: CallToolResult,
    ) -> Result<(), TaskStoreError> {
        let mut entry = self
            .records
            .get_mut(task_id)
            .ok_or_else(|| TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            })?;
        let record = entry.value_mut();
        Self::validate_access(record, task_id, owner_id)?;
        record.result = Some(result);
        Ok(())
    }

    async fn get_result(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<CallToolResult, TaskStoreError> {
        let entry = self
            .records
            .get(task_id)
            .ok_or_else(|| TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            })?;
        Self::validate_access(entry.value(), task_id, owner_id)?;
        // A task that exists but has no stored result yet is "pending" — signal
        // NotFound so the dispatch layer can map it to a specified error.
        entry
            .value()
            .result
            .clone()
            .ok_or_else(|| TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            })
    }

    fn supports_results(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Constructor tests --

    #[test]
    fn new_creates_empty_store() {
        let store = InMemoryTaskStore::new();
        assert!(store.records.is_empty());
    }

    #[test]
    fn default_creates_empty_store() {
        let store = InMemoryTaskStore::default();
        assert!(store.records.is_empty());
    }

    #[test]
    fn with_config_applies_custom_config() {
        let config = StoreConfig {
            default_ttl_ms: Some(1_000),
            max_ttl_ms: Some(2_000),
            default_poll_interval_ms: 500,
            max_tasks_per_owner: 10,
        };
        let store = InMemoryTaskStore::with_config(config);
        assert_eq!(store.config().default_ttl_ms, Some(1_000));
        assert_eq!(store.config().max_ttl_ms, Some(2_000));
        assert_eq!(store.config().default_poll_interval_ms, 500);
        assert_eq!(store.config().max_tasks_per_owner, 10);
    }

    #[test]
    fn store_config_default_values() {
        let config = StoreConfig::default();
        assert_eq!(config.default_ttl_ms, Some(3_600_000));
        assert_eq!(config.max_ttl_ms, Some(86_400_000));
        assert_eq!(config.default_poll_interval_ms, 5000);
        assert_eq!(config.max_tasks_per_owner, 100);
    }

    // -- Create tests --

    #[tokio::test]
    async fn create_returns_working_task() {
        let store = InMemoryTaskStore::new();
        let task = store.create("owner-1", None).await.unwrap();
        assert_eq!(task.status, TaskStatus::Working);
        assert!(!task.task_id.is_empty());
        assert!(!task.created_at.is_empty());
        assert!(!task.last_updated_at.is_empty());
    }

    #[tokio::test]
    async fn create_with_default_ttl() {
        let store = InMemoryTaskStore::new();
        let task = store.create("owner-1", None).await.unwrap();
        // Default TTL from StoreConfig is 3_600_000 (1 hour)
        assert_eq!(task.ttl, Some(3_600_000));
    }

    #[tokio::test]
    async fn create_with_explicit_ttl() {
        let store = InMemoryTaskStore::new();
        let task = store.create("owner-1", Some(60_000)).await.unwrap();
        assert_eq!(task.ttl, Some(60_000));
    }

    #[tokio::test]
    async fn create_clamps_ttl_to_max() {
        let store = InMemoryTaskStore::with_config(StoreConfig {
            max_ttl_ms: Some(10_000),
            ..StoreConfig::default()
        });
        let task = store.create("owner-1", Some(999_999)).await.unwrap();
        assert_eq!(task.ttl, Some(10_000));
    }

    #[tokio::test]
    async fn create_sets_poll_interval() {
        let store = InMemoryTaskStore::with_config(StoreConfig {
            default_poll_interval_ms: 3000,
            ..StoreConfig::default()
        });
        let task = store.create("owner-1", None).await.unwrap();
        assert_eq!(task.poll_interval, Some(3000));
    }

    // -- Get tests --

    #[tokio::test]
    async fn get_returns_created_task() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        let fetched = store.get(&created.task_id, "owner-1").await.unwrap();
        assert_eq!(fetched.task_id, created.task_id);
        assert_eq!(fetched.status, TaskStatus::Working);
    }

    #[tokio::test]
    async fn get_owner_mismatch_returns_not_found() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        let result = store.get(&created.task_id, "owner-2").await;
        assert!(
            matches!(&result, Err(TaskStoreError::NotFound { task_id }) if task_id == &created.task_id),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn get_nonexistent_returns_not_found() {
        let store = InMemoryTaskStore::new();
        let result = store.get("nonexistent", "owner-1").await;
        assert!(matches!(result, Err(TaskStoreError::NotFound { .. })));
    }

    // -- List tests --

    #[tokio::test]
    async fn list_returns_owner_tasks_only() {
        let store = InMemoryTaskStore::new();
        store.create("owner-1", None).await.unwrap();
        store.create("owner-1", None).await.unwrap();
        store.create("owner-2", None).await.unwrap();

        let (tasks, _) = store.list("owner-1", None).await.unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn list_empty_for_unknown_owner() {
        let store = InMemoryTaskStore::new();
        store.create("owner-1", None).await.unwrap();
        let (tasks, _) = store.list("owner-unknown", None).await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn list_sorted_newest_first() {
        let store = InMemoryTaskStore::new();
        let first = store.create("owner-1", None).await.unwrap();
        // Small delay to ensure different timestamps
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        let second = store.create("owner-1", None).await.unwrap();

        let (tasks, _) = store.list("owner-1", None).await.unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].task_id, second.task_id);
        assert_eq!(tasks[1].task_id, first.task_id);
    }

    // -- Cancel tests --

    #[tokio::test]
    async fn cancel_transitions_to_cancelled() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        let cancelled = store.cancel(&created.task_id, "owner-1").await.unwrap();
        assert_eq!(cancelled.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn cancel_completed_task_returns_invalid_transition() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();

        // Complete the task first
        store
            .update_status(
                &created.task_id,
                "owner-1",
                TaskStatus::Completed,
                Some("Done".to_string()),
            )
            .await
            .unwrap();

        // Cancel should fail
        let result = store.cancel(&created.task_id, "owner-1").await;
        assert!(
            matches!(result, Err(TaskStoreError::InvalidTransition { .. })),
            "expected InvalidTransition, got: {result:?}"
        );
    }

    // -- Update status tests --

    #[tokio::test]
    async fn update_status_working_to_completed() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        let updated = store
            .update_status(
                &created.task_id,
                "owner-1",
                TaskStatus::Completed,
                Some("Done".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(updated.status, TaskStatus::Completed);
        assert_eq!(updated.status_message.as_deref(), Some("Done"));
    }

    #[tokio::test]
    async fn update_status_from_terminal_returns_invalid_transition() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();

        // Complete first
        store
            .update_status(&created.task_id, "owner-1", TaskStatus::Completed, None)
            .await
            .unwrap();

        // Try to go back to Working
        let result = store
            .update_status(&created.task_id, "owner-1", TaskStatus::Working, None)
            .await;
        assert!(
            matches!(result, Err(TaskStoreError::InvalidTransition { .. })),
            "expected InvalidTransition, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn update_status_self_transition_rejected() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        let result = store
            .update_status(&created.task_id, "owner-1", TaskStatus::Working, None)
            .await;
        assert!(
            matches!(result, Err(TaskStoreError::InvalidTransition { .. })),
            "expected InvalidTransition, got: {result:?}"
        );
    }

    // -- TTL / expiration tests --

    #[tokio::test]
    async fn task_created_with_explicit_ttl_has_correct_field() {
        let store = InMemoryTaskStore::new();
        let task = store.create("owner-1", Some(60_000)).await.unwrap();
        assert_eq!(task.ttl, Some(60_000));
    }

    #[tokio::test]
    async fn task_created_with_none_ttl_gets_default() {
        let config = StoreConfig {
            default_ttl_ms: Some(120_000),
            ..StoreConfig::default()
        };
        let store = InMemoryTaskStore::with_config(config);
        let task = store.create("owner-1", None).await.unwrap();
        assert_eq!(task.ttl, Some(120_000));
    }

    #[tokio::test]
    async fn cleanup_expired_removes_expired_tasks() {
        let store = InMemoryTaskStore::with_config(StoreConfig {
            default_ttl_ms: Some(1), // 1ms TTL
            ..StoreConfig::default()
        });
        store.create("owner-1", Some(1)).await.unwrap();

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);
        assert!(store.records.is_empty());
    }

    #[tokio::test]
    async fn cleanup_expired_keeps_non_expired() {
        let store = InMemoryTaskStore::new();
        store.create("owner-1", Some(3_600_000)).await.unwrap();
        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
        assert_eq!(store.records.len(), 1);
    }

    #[tokio::test]
    async fn get_expired_task_returns_expired_error() {
        let store = InMemoryTaskStore::with_config(StoreConfig {
            default_ttl_ms: Some(1), // 1ms TTL
            ..StoreConfig::default()
        });
        let created = store.create("owner-1", Some(1)).await.unwrap();

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let result = store.get(&created.task_id, "owner-1").await;
        assert!(
            matches!(result, Err(TaskStoreError::Expired { .. })),
            "expected Expired, got: {result:?}"
        );
    }

    // -- Error display tests --

    #[test]
    fn task_store_error_display_not_found() {
        let err = TaskStoreError::NotFound {
            task_id: "t-123".to_string(),
        };
        assert_eq!(err.to_string(), "task not found: t-123");
    }

    #[test]
    fn task_store_error_display_invalid_transition() {
        let err = TaskStoreError::InvalidTransition {
            task_id: "t-123".to_string(),
            from: TaskStatus::Completed,
            to: TaskStatus::Working,
        };
        let msg = err.to_string();
        assert!(msg.contains("invalid transition"));
        assert!(msg.contains("t-123"));
    }

    #[test]
    fn task_store_error_display_expired() {
        let err = TaskStoreError::Expired {
            task_id: "t-123".to_string(),
        };
        assert_eq!(err.to_string(), "task expired: t-123");
    }

    #[test]
    fn task_store_error_display_internal() {
        let err = TaskStoreError::Internal {
            message: "something broke".to_string(),
        };
        assert_eq!(err.to_string(), "internal error: something broke");
    }

    #[test]
    fn task_store_error_converts_to_sdk_error() {
        let err = TaskStoreError::NotFound {
            task_id: "t-123".to_string(),
        };
        let sdk_err: crate::error::Error = err.into();
        let msg = sdk_err.to_string();
        assert!(msg.contains("task not found: t-123"));
    }

    // -- Max tasks per owner --

    #[tokio::test]
    async fn max_tasks_per_owner_enforced() {
        let store = InMemoryTaskStore::with_config(StoreConfig {
            max_tasks_per_owner: 2,
            ..StoreConfig::default()
        });
        store.create("owner-1", None).await.unwrap();
        store.create("owner-1", None).await.unwrap();
        let result = store.create("owner-1", None).await;
        assert!(
            matches!(result, Err(TaskStoreError::Internal { .. })),
            "expected Internal error for max tasks, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn max_tasks_scoped_to_owner() {
        let store = InMemoryTaskStore::with_config(StoreConfig {
            max_tasks_per_owner: 2,
            ..StoreConfig::default()
        });
        store.create("owner-a", None).await.unwrap();
        store.create("owner-a", None).await.unwrap();
        // Owner B should still be able to create
        let result = store.create("owner-b", None).await;
        assert!(result.is_ok());
    }

    // -- Terminal result (set_result / get_result / supports_results) tests --

    use crate::types::{CallToolResult, Content};

    fn sample_result(text: &str) -> CallToolResult {
        CallToolResult::new(vec![Content::text(text)])
    }

    #[tokio::test]
    async fn set_then_get_result_round_trips() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        store
            .set_result(&created.task_id, "owner-1", sample_result("hello"))
            .await
            .unwrap();
        let fetched = store.get_result(&created.task_id, "owner-1").await.unwrap();
        assert_eq!(fetched.content.len(), 1);
    }

    #[tokio::test]
    async fn get_result_owner_mismatch_returns_not_found() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        store
            .set_result(&created.task_id, "owner-1", sample_result("secret"))
            .await
            .unwrap();
        let result = store.get_result(&created.task_id, "owner-2").await;
        assert!(
            matches!(result, Err(TaskStoreError::NotFound { .. })),
            "cross-owner read must be NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn set_result_owner_mismatch_returns_not_found() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        let result = store
            .set_result(&created.task_id, "owner-2", sample_result("x"))
            .await;
        assert!(
            matches!(result, Err(TaskStoreError::NotFound { .. })),
            "cross-owner set must be NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn get_result_existing_task_no_result_returns_not_found() {
        let store = InMemoryTaskStore::new();
        let created = store.create("owner-1", None).await.unwrap();
        // Task exists but no result was ever set -> pending signal.
        let result = store.get_result(&created.task_id, "owner-1").await;
        assert!(
            matches!(result, Err(TaskStoreError::NotFound { .. })),
            "pending task (no result) must be NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn in_memory_store_supports_results() {
        let store = InMemoryTaskStore::new();
        assert!(store.supports_results());
    }

    #[tokio::test]
    async fn cleanup_expired_drops_result() {
        let store = InMemoryTaskStore::with_config(StoreConfig {
            default_ttl_ms: Some(1),
            ..StoreConfig::default()
        });
        let created = store.create("owner-1", Some(1)).await.unwrap();
        store
            .set_result(&created.task_id, "owner-1", sample_result("ephemeral"))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);
        // Result is gone along with the task (no separate unexpiring map).
        let result = store.get_result(&created.task_id, "owner-1").await;
        assert!(matches!(result, Err(TaskStoreError::NotFound { .. })));
    }

    /// A store that does NOT override the additive result methods — exercises
    /// the explicit-unsupported defaults.
    struct DefaultOnlyStore {
        config: StoreConfig,
    }

    #[async_trait]
    impl TaskStore for DefaultOnlyStore {
        async fn create(&self, _owner_id: &str, _ttl: Option<u64>) -> Result<Task, TaskStoreError> {
            Ok(Task::new("default-only", TaskStatus::Working))
        }
        async fn get(&self, task_id: &str, _owner_id: &str) -> Result<Task, TaskStoreError> {
            Ok(Task::new(task_id, TaskStatus::Working))
        }
        async fn update_status(
            &self,
            task_id: &str,
            _owner_id: &str,
            status: TaskStatus,
            _message: Option<String>,
        ) -> Result<Task, TaskStoreError> {
            Ok(Task::new(task_id, status))
        }
        async fn list(
            &self,
            _owner_id: &str,
            _cursor: Option<&str>,
        ) -> Result<(Vec<Task>, Option<String>), TaskStoreError> {
            Ok((Vec::new(), None))
        }
        async fn cancel(&self, task_id: &str, _owner_id: &str) -> Result<Task, TaskStoreError> {
            Ok(Task::new(task_id, TaskStatus::Cancelled))
        }
        async fn cleanup_expired(&self) -> Result<usize, TaskStoreError> {
            Ok(0)
        }
        fn config(&self) -> &StoreConfig {
            &self.config
        }
        // Deliberately does NOT override set_result/get_result/supports_results.
    }

    #[tokio::test]
    async fn default_impl_store_reports_unsupported() {
        let store = DefaultOnlyStore {
            config: StoreConfig::default(),
        };
        assert!(!store.supports_results());

        let set = store.set_result("t", "owner-1", sample_result("x")).await;
        assert!(
            matches!(set, Err(TaskStoreError::Internal { .. })),
            "default set_result must be an explicit unsupported error, got: {set:?}"
        );

        let get = store.get_result("t", "owner-1").await;
        assert!(
            matches!(get, Err(TaskStoreError::NotFound { .. })),
            "default get_result must be NotFound, got: {get:?}"
        );
    }
}
