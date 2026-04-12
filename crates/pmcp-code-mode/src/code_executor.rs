//! High-level code execution trait for MCP servers.
//!
//! This module provides the [`CodeExecutor`] trait, which is the primary public API
//! for implementing code execution in MCP servers. It replaces the internal
//! `HttpExecutor`, `SdkExecutor`, and `McpExecutor` traits for external server
//! developers (those traits remain available for advanced use behind the
//! `js-runtime` feature flag).

use crate::types::ExecutionError;

/// High-level trait for executing validated code.
///
/// Implementations handle the execution of code that has already passed
/// validation and token verification. This is the primary public API
/// for code execution -- it replaces the internal `HttpExecutor`,
/// `SdkExecutor`, and `McpExecutor` traits for external server developers.
///
/// # Execution Patterns
///
/// The four supported patterns (all implemented via this single trait):
/// - **Pattern A (SQL):** Direct SQL execution, no JS runtime
/// - **Pattern B (JS+HTTP):** JavaScript plan compiled and executed via HTTP calls
/// - **Pattern C (JS+SDK):** JavaScript plan executed via AWS SDK calls
/// - **Pattern D (JS+MCP):** JavaScript plan executed via MCP tool calls
///
/// # API Stability Note
///
/// **\[Addresses divergent review concern: CodeExecutor trait surface area\]**
/// This v0.1.0 API uses a simple `(code, variables)` signature per D-04.
/// A future v0.2.0 may add an `ExecutionContext` parameter carrying timeout,
/// cancellation token, and request metadata. The `(code, variables)` signature
/// will be preserved as a default-method wrapper for backward compatibility.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp_code_mode::{CodeExecutor, ExecutionError};
/// use serde_json::Value;
///
/// struct MyExecutor { /* database pool, http client, etc. */ }
///
/// #[pmcp_code_mode::async_trait]
/// impl CodeExecutor for MyExecutor {
///     async fn execute(
///         &self,
///         code: &str,
///         variables: Option<&Value>,
///     ) -> Result<Value, ExecutionError> {
///         // Execute validated code against your backend
///         todo!()
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait CodeExecutor: Send + Sync {
    /// Execute validated code and return the result.
    ///
    /// `code` has already passed validation and token verification.
    /// `variables` are optional user-provided parameters (e.g., GraphQL variables).
    ///
    /// Implementations should NOT re-verify the token -- that is handled
    /// by the Code Mode framework before calling this method.
    async fn execute(
        &self,
        code: &str,
        variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError>;
}

// NOTE: PlanExecutor blanket impl deferred -- PlanExecutor::execute requires &mut self,
// which is incompatible with CodeExecutor's &self. Wrapping in Mutex adds complexity
// without clear benefit. External servers implement CodeExecutor directly.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct EchoExecutor;

    #[async_trait::async_trait]
    impl CodeExecutor for EchoExecutor {
        async fn execute(
            &self,
            code: &str,
            variables: Option<&serde_json::Value>,
        ) -> Result<serde_json::Value, ExecutionError> {
            Ok(json!({
                "code": code,
                "variables": variables,
            }))
        }
    }

    #[tokio::test]
    async fn code_executor_echo() {
        let executor = EchoExecutor;
        let result = executor.execute("SELECT 1", None).await.unwrap();
        assert_eq!(result["code"], "SELECT 1");
    }

    #[tokio::test]
    async fn code_executor_with_variables() {
        let executor = EchoExecutor;
        let vars = json!({"limit": 10});
        let result = executor
            .execute("query { users }", Some(&vars))
            .await
            .unwrap();
        assert_eq!(result["variables"]["limit"], 10);
    }

    #[tokio::test]
    async fn code_executor_returns_error() {
        struct FailingExecutor;

        #[async_trait::async_trait]
        impl CodeExecutor for FailingExecutor {
            async fn execute(
                &self,
                _code: &str,
                _variables: Option<&serde_json::Value>,
            ) -> Result<serde_json::Value, ExecutionError> {
                Err(ExecutionError::BackendError(
                    "database unavailable".to_string(),
                ))
            }
        }

        let executor = FailingExecutor;
        let result = executor.execute("SELECT 1", None).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("database unavailable"));
    }

    // Compile-time test: CodeExecutor requires Send + Sync
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _code_executor_is_send_sync() {
        _assert_send_sync::<EchoExecutor>();
    }
}
