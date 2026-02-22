//! Ergonomic wrapper for task operations in tool handlers.
//!
//! [`TaskContext`] scopes all operations to a single task, hiding task IDs,
//! owner IDs, and store internals from tool handler code. It provides typed
//! variable accessors, convenient status transitions, and delegates to the
//! underlying [`TaskStore`] for all persistence.
//!
//! # Design
//!
//! `TaskContext` is `Clone + Send + Sync` because it wraps an
//! `Arc<dyn TaskStore>` (which is `Send + Sync`) and two `String` fields.
//! Cloning a context is cheap and produces a handle to the same underlying
//! task and store.
//!
//! # Examples
//!
//! ```
//! use std::sync::Arc;
//! use pmcp_tasks::context::TaskContext;
//! use pmcp_tasks::store::memory::InMemoryTaskStore;
//! use pmcp_tasks::store::TaskStore;
//! use pmcp_tasks::security::TaskSecurityConfig;
//! use serde_json::json;
//!
//! # tokio::runtime::Runtime::new().unwrap().block_on(async {
//! let store = Arc::new(
//!     InMemoryTaskStore::new()
//!         .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
//! );
//!
//! // Create a task through the store
//! let record = store.create("local", "tools/call", None).await.unwrap();
//! let task_id = record.task.task_id.clone();
//!
//! // Wrap in TaskContext for ergonomic access
//! let ctx = TaskContext::new(store.clone(), task_id, "local".to_string());
//!
//! // Set and read typed variables
//! ctx.set_variable("progress", json!(42)).await.unwrap();
//! let progress = ctx.get_i64("progress").await.unwrap();
//! assert_eq!(progress, Some(42));
//!
//! // Complete the task with a result
//! ctx.complete(json!({"answer": 42})).await.unwrap();
//! let record = ctx.get().await.unwrap();
//! assert_eq!(record.task.status, pmcp_tasks::TaskStatus::Completed);
//! # });
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::domain::TaskRecord;
use crate::error::TaskError;
use crate::store::TaskStore;
use crate::types::task::TaskStatus;

/// Ergonomic wrapper that scopes all task operations to a single task.
///
/// Tool handlers receive a `TaskContext` instead of raw task IDs, store
/// references, and owner strings. All methods delegate to the underlying
/// [`TaskStore`], passing the correct `task_id` and `owner_id` automatically.
///
/// # Thread Safety
///
/// `TaskContext` is `Clone + Send + Sync` because:
/// - `Arc<dyn TaskStore>` is `Send + Sync` (the trait requires it)
/// - `String` is `Send + Sync`
///
/// Cloning is cheap (Arc ref-count bump + String clone) and produces a
/// handle to the same underlying task.
///
/// # Variable Accessors
///
/// Typed accessors ([`get_string`](Self::get_string), [`get_i64`](Self::get_i64),
/// [`get_f64`](Self::get_f64), [`get_bool`](Self::get_bool)) return `Ok(None)`
/// when the key is absent *or* when the stored value cannot be converted to
/// the requested type. This is intentional: type mismatches are not errors
/// in the task variable model.
///
/// For arbitrary types, use [`get_typed`](Self::get_typed) which deserializes
/// via `serde_json::from_value`.
///
/// # Status Transitions
///
/// Convenience methods ([`complete`](Self::complete), [`fail`](Self::fail),
/// [`require_input`](Self::require_input), [`resume`](Self::resume),
/// [`cancel`](Self::cancel)) delegate to the store's state machine validation.
/// Invalid transitions return [`TaskError::InvalidTransition`].
#[derive(Clone)]
pub struct TaskContext {
    store: Arc<dyn TaskStore>,
    task_id: String,
    owner_id: String,
}

impl TaskContext {
    /// Creates a new `TaskContext` scoped to a specific task.
    ///
    /// # Arguments
    ///
    /// * `store` - The task store backend (shared via `Arc`).
    /// * `task_id` - The unique identifier of the task.
    /// * `owner_id` - The owner identity for access control.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use pmcp_tasks::context::TaskContext;
    /// use pmcp_tasks::store::memory::InMemoryTaskStore;
    ///
    /// let store = Arc::new(InMemoryTaskStore::new());
    /// let ctx = TaskContext::new(
    ///     store,
    ///     "task-123".to_string(),
    ///     "owner-abc".to_string(),
    /// );
    /// assert_eq!(ctx.task_id(), "task-123");
    /// assert_eq!(ctx.owner_id(), "owner-abc");
    /// ```
    pub fn new(store: Arc<dyn TaskStore>, task_id: String, owner_id: String) -> Self {
        Self {
            store,
            task_id,
            owner_id,
        }
    }

    /// Returns the task ID this context is scoped to.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// Returns the owner ID this context operates under.
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    // --- Task retrieval ---

    /// Retrieves the current state of the task from the store.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist or the
    /// owner does not match.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use pmcp_tasks::context::TaskContext;
    /// # use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// # use pmcp_tasks::store::TaskStore;
    /// # use pmcp_tasks::security::TaskSecurityConfig;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// let store = Arc::new(
    ///     InMemoryTaskStore::new()
    ///         .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    /// );
    /// let record = store.create("local", "tools/call", None).await.unwrap();
    /// let ctx = TaskContext::new(store, record.task.task_id.clone(), "local".to_string());
    ///
    /// let fetched = ctx.get().await.unwrap();
    /// assert_eq!(fetched.task.task_id, record.task.task_id);
    /// # });
    /// ```
    pub async fn get(&self) -> Result<TaskRecord, TaskError> {
        self.store.get(&self.task_id, &self.owner_id).await
    }

    // --- Variable accessors ---

    /// Gets a raw JSON variable by key.
    ///
    /// Returns `Ok(None)` if the key does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    pub async fn get_variable(&self, key: &str) -> Result<Option<Value>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).cloned())
    }

    /// Gets a string variable by key.
    ///
    /// Returns `Ok(None)` if the key does not exist or the value is not a
    /// JSON string.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use pmcp_tasks::context::TaskContext;
    /// # use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// # use pmcp_tasks::store::TaskStore;
    /// # use pmcp_tasks::security::TaskSecurityConfig;
    /// # use serde_json::json;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # let store = Arc::new(InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true)));
    /// # let record = store.create("local", "tools/call", None).await.unwrap();
    /// # let ctx = TaskContext::new(store, record.task.task_id.clone(), "local".to_string());
    /// ctx.set_variable("name", json!("Alice")).await.unwrap();
    /// assert_eq!(ctx.get_string("name").await.unwrap(), Some("Alice".to_string()));
    /// assert_eq!(ctx.get_string("missing").await.unwrap(), None);
    /// # });
    /// ```
    pub async fn get_string(&self, key: &str) -> Result<Option<String>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record
            .variables
            .get(key)
            .and_then(|v| v.as_str().map(|s| s.to_string())))
    }

    /// Gets an integer variable by key.
    ///
    /// Returns `Ok(None)` if the key does not exist or the value is not a
    /// JSON number representable as `i64`.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    pub async fn get_i64(&self, key: &str) -> Result<Option<i64>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_i64()))
    }

    /// Gets a floating-point variable by key.
    ///
    /// Returns `Ok(None)` if the key does not exist or the value is not a
    /// JSON number representable as `f64`.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    pub async fn get_f64(&self, key: &str) -> Result<Option<f64>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_f64()))
    }

    /// Gets a boolean variable by key.
    ///
    /// Returns `Ok(None)` if the key does not exist or the value is not a
    /// JSON boolean.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    pub async fn get_bool(&self, key: &str) -> Result<Option<bool>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.get(key).and_then(|v| v.as_bool()))
    }

    /// Gets a typed variable by key, deserializing from JSON.
    ///
    /// Uses [`serde_json::from_value`] to convert the stored JSON value
    /// into the requested type. Returns `Ok(None)` if the key does not
    /// exist or deserialization fails.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Any type implementing [`DeserializeOwned`].
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use pmcp_tasks::context::TaskContext;
    /// # use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// # use pmcp_tasks::store::TaskStore;
    /// # use pmcp_tasks::security::TaskSecurityConfig;
    /// # use serde_json::json;
    /// # use serde::Deserialize;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # let store = Arc::new(InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true)));
    /// # let record = store.create("local", "tools/call", None).await.unwrap();
    /// # let ctx = TaskContext::new(store, record.task.task_id.clone(), "local".to_string());
    /// #[derive(Debug, Deserialize, PartialEq)]
    /// struct Point { x: i32, y: i32 }
    ///
    /// ctx.set_variable("point", json!({"x": 10, "y": 20})).await.unwrap();
    /// let point: Option<Point> = ctx.get_typed("point").await.unwrap();
    /// assert_eq!(point, Some(Point { x: 10, y: 20 }));
    /// # });
    /// ```
    pub async fn get_typed<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> Result<Option<T>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record
            .variables
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok()))
    }

    // --- Variable mutators ---

    /// Sets a single variable on the task.
    ///
    /// Wraps the key-value pair into a map and delegates to
    /// [`TaskStore::set_variables`].
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    /// Returns [`TaskError::VariableSizeExceeded`] if the resulting variable
    /// payload exceeds the configured limit.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use pmcp_tasks::context::TaskContext;
    /// # use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// # use pmcp_tasks::store::TaskStore;
    /// # use pmcp_tasks::security::TaskSecurityConfig;
    /// # use serde_json::json;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # let store = Arc::new(InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true)));
    /// # let record = store.create("local", "tools/call", None).await.unwrap();
    /// # let ctx = TaskContext::new(store, record.task.task_id.clone(), "local".to_string());
    /// ctx.set_variable("counter", json!(1)).await.unwrap();
    /// # });
    /// ```
    pub async fn set_variable(
        &self,
        key: impl Into<String>,
        value: Value,
    ) -> Result<TaskRecord, TaskError> {
        let mut vars = HashMap::new();
        vars.insert(key.into(), value);
        self.store
            .set_variables(&self.task_id, &self.owner_id, vars)
            .await
    }

    /// Sets multiple variables on the task in a single operation.
    ///
    /// Delegates directly to [`TaskStore::set_variables`] with merge semantics:
    /// existing keys are overwritten, new keys are added, and `null` values
    /// delete keys.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    /// Returns [`TaskError::VariableSizeExceeded`] if the resulting variable
    /// payload exceeds the configured limit.
    pub async fn set_variables(
        &self,
        variables: HashMap<String, Value>,
    ) -> Result<TaskRecord, TaskError> {
        self.store
            .set_variables(&self.task_id, &self.owner_id, variables)
            .await
    }

    /// Returns all variables currently stored on the task.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    pub async fn variables(&self) -> Result<HashMap<String, Value>, TaskError> {
        let record = self.store.get(&self.task_id, &self.owner_id).await?;
        Ok(record.variables.clone())
    }

    /// Deletes a variable by setting it to `null`.
    ///
    /// Uses null-deletion semantics: the key is removed from the variable
    /// store when the store processes the `null` value.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::NotFound`] if the task does not exist.
    pub async fn delete_variable(
        &self,
        key: impl Into<String>,
    ) -> Result<TaskRecord, TaskError> {
        let mut vars = HashMap::new();
        vars.insert(key.into(), Value::Null);
        self.store
            .set_variables(&self.task_id, &self.owner_id, vars)
            .await
    }

    // --- Status transition convenience methods ---

    /// Completes the task with a result value.
    ///
    /// Uses [`TaskStore::complete_with_result`] for atomic status transition
    /// and result storage. Both the status change to `Completed` and the
    /// result value are applied in a single operation.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::InvalidTransition`] if the task is in a terminal state.
    /// Returns [`TaskError::Expired`] if the task has expired.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use pmcp_tasks::context::TaskContext;
    /// # use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// # use pmcp_tasks::store::TaskStore;
    /// # use pmcp_tasks::security::TaskSecurityConfig;
    /// # use serde_json::json;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # let store = Arc::new(InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true)));
    /// # let record = store.create("local", "tools/call", None).await.unwrap();
    /// # let ctx = TaskContext::new(store, record.task.task_id.clone(), "local".to_string());
    /// let completed = ctx.complete(json!({"output": "done"})).await.unwrap();
    /// assert_eq!(completed.task.status, pmcp_tasks::TaskStatus::Completed);
    /// assert_eq!(completed.result, Some(json!({"output": "done"})));
    /// # });
    /// ```
    pub async fn complete(&self, result: Value) -> Result<TaskRecord, TaskError> {
        self.store
            .complete_with_result(
                &self.task_id,
                &self.owner_id,
                TaskStatus::Completed,
                None,
                result,
            )
            .await
    }

    /// Transitions the task to `Failed` with a message.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::InvalidTransition`] if the task is in a terminal state.
    /// Returns [`TaskError::Expired`] if the task has expired.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use pmcp_tasks::context::TaskContext;
    /// # use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// # use pmcp_tasks::store::TaskStore;
    /// # use pmcp_tasks::security::TaskSecurityConfig;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # let store = Arc::new(InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true)));
    /// # let record = store.create("local", "tools/call", None).await.unwrap();
    /// # let ctx = TaskContext::new(store, record.task.task_id.clone(), "local".to_string());
    /// let failed = ctx.fail("connection timeout").await.unwrap();
    /// assert_eq!(failed.task.status, pmcp_tasks::TaskStatus::Failed);
    /// assert_eq!(failed.task.status_message.as_deref(), Some("connection timeout"));
    /// # });
    /// ```
    pub async fn fail(&self, message: impl Into<String>) -> Result<TaskRecord, TaskError> {
        self.store
            .update_status(
                &self.task_id,
                &self.owner_id,
                TaskStatus::Failed,
                Some(message.into()),
            )
            .await
    }

    /// Transitions the task to `InputRequired` with a prompt message.
    ///
    /// Use this when the task needs additional input from the client before
    /// it can proceed.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::InvalidTransition`] if the task is in a terminal state.
    /// Returns [`TaskError::Expired`] if the task has expired.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use pmcp_tasks::context::TaskContext;
    /// # use pmcp_tasks::store::memory::InMemoryTaskStore;
    /// # use pmcp_tasks::store::TaskStore;
    /// # use pmcp_tasks::security::TaskSecurityConfig;
    /// # tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// # let store = Arc::new(InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true)));
    /// # let record = store.create("local", "tools/call", None).await.unwrap();
    /// # let ctx = TaskContext::new(store, record.task.task_id.clone(), "local".to_string());
    /// let waiting = ctx.require_input("Please provide your API key").await.unwrap();
    /// assert_eq!(waiting.task.status, pmcp_tasks::TaskStatus::InputRequired);
    /// # });
    /// ```
    pub async fn require_input(
        &self,
        message: impl Into<String>,
    ) -> Result<TaskRecord, TaskError> {
        self.store
            .update_status(
                &self.task_id,
                &self.owner_id,
                TaskStatus::InputRequired,
                Some(message.into()),
            )
            .await
    }

    /// Resumes the task from `InputRequired` back to `Working`.
    ///
    /// Call this after the client has provided the requested input and
    /// the tool handler is ready to continue processing.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::InvalidTransition`] if the task is not in
    /// `InputRequired` state (or is in a terminal state).
    /// Returns [`TaskError::Expired`] if the task has expired.
    pub async fn resume(&self) -> Result<TaskRecord, TaskError> {
        self.store
            .update_status(
                &self.task_id,
                &self.owner_id,
                TaskStatus::Working,
                None,
            )
            .await
    }

    /// Cancels the task.
    ///
    /// Delegates to [`TaskStore::cancel`] which transitions the task to
    /// `Cancelled` status.
    ///
    /// # Errors
    ///
    /// Returns [`TaskError::InvalidTransition`] if the task is already in
    /// a terminal state.
    /// Returns [`TaskError::Expired`] if the task has expired.
    pub async fn cancel(&self) -> Result<TaskRecord, TaskError> {
        self.store.cancel(&self.task_id, &self.owner_id).await
    }
}

impl std::fmt::Debug for TaskContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskContext")
            .field("task_id", &self.task_id)
            .field("owner_id", &self.owner_id)
            .finish_non_exhaustive()
    }
}
