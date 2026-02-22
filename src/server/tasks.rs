//! Task routing trait for MCP Tasks integration.
//!
//! This trait provides the integration point for task-enabled servers
//! without creating a circular dependency with the `pmcp-tasks` crate.
//! The `pmcp-tasks` crate implements [`TaskRouter`] and provides the
//! concrete task lifecycle management, while `pmcp` defines the
//! contract here so that [`ServerCoreBuilder`](super::builder::ServerCoreBuilder)
//! can accept a task router without depending on `pmcp-tasks`.

use async_trait::async_trait;
use serde_json::Value;

use crate::error::Result;

/// Trait for routing MCP task requests.
///
/// This trait is implemented by `pmcp-tasks` to handle task lifecycle
/// operations without requiring `pmcp` to depend on `pmcp-tasks`.
///
/// All params and return values use `serde_json::Value` to avoid
/// circular crate dependencies. The implementing crate parses these
/// into strongly-typed structs (e.g., `TaskGetParams`, `CreateTaskResult`).
#[async_trait]
pub trait TaskRouter: Send + Sync {
    /// Handle a task-augmented `tools/call` request.
    ///
    /// When a client sends a `tools/call` request with a `task` field,
    /// the server delegates to this method instead of calling the tool
    /// handler directly. The router creates a task, spawns the tool
    /// execution, and returns a `CreateTaskResult` as `Value`.
    ///
    /// Returns the `CreateTaskResult` serialized as `Value`.
    async fn handle_task_call(
        &self,
        tool_name: &str,
        arguments: Value,
        task_params: Value,
        owner_id: &str,
        progress_token: Option<Value>,
    ) -> Result<Value>;

    /// Handle `tasks/get` request.
    ///
    /// Returns the task status for the given task ID.
    async fn handle_tasks_get(&self, params: Value, owner_id: &str) -> Result<Value>;

    /// Handle `tasks/result` request.
    ///
    /// Returns the task result (content) for a completed task.
    async fn handle_tasks_result(&self, params: Value, owner_id: &str) -> Result<Value>;

    /// Handle `tasks/list` request.
    ///
    /// Returns a list of tasks visible to the given owner.
    async fn handle_tasks_list(&self, params: Value, owner_id: &str) -> Result<Value>;

    /// Handle `tasks/cancel` request.
    ///
    /// Requests cancellation of the given task.
    async fn handle_tasks_cancel(&self, params: Value, owner_id: &str) -> Result<Value>;

    /// Resolve owner ID from authentication context fields.
    ///
    /// The owner ID determines task visibility and access control.
    /// Implementations typically derive this from the OAuth subject,
    /// client ID, or session ID (in order of preference).
    fn resolve_owner(
        &self,
        subject: Option<&str>,
        client_id: Option<&str>,
        session_id: Option<&str>,
    ) -> String;

    /// Check if a tool requires task augmentation (`taskSupport: required`).
    ///
    /// When a tool has `execution.taskSupport == "required"`, the client
    /// must send a `task` field with the `tools/call` request.
    fn tool_requires_task(&self, tool_name: &str, tool_execution: Option<&Value>) -> bool;

    /// Get the server task capabilities as a `Value` for `experimental.tasks`.
    ///
    /// This is inserted into the server's capabilities during initialization
    /// so clients know the server supports the tasks protocol extension.
    fn task_capabilities(&self) -> Value;
}
