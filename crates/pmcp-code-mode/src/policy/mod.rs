//! Policy evaluation framework for Code Mode.
//!
//! This module provides a trait-based abstraction for policy evaluation,
//! allowing different backends (AVP, local Cedar, etc.) to be used
//! interchangeably.
//!
//! # Important
//! The [`NoopPolicyEvaluator`] is provided for **testing and local development only**.
//! Production servers MUST implement [`PolicyEvaluator`] with a real authorization backend.

pub mod types;

#[cfg(feature = "cedar")]
pub mod cedar;

pub use types::*;

/// Error type for policy evaluation.
#[derive(Debug, thiserror::Error)]
pub enum PolicyEvaluationError {
    #[error("Policy configuration error: {0}")]
    ConfigError(String),

    #[error("Policy evaluation error: {0}")]
    EvaluationError(String),

    #[error("Authorization denied: {0}")]
    Denied(String),
}

/// Trait for policy evaluation backends.
///
/// Implementations can use different backends:
/// - `AvpPolicyEvaluator` (in mcp-server-common): Uses AWS AVP
/// - `CedarPolicyEvaluator` (in this crate): Uses local Cedar engine
/// - Custom implementations for testing or other policy engines
#[async_trait::async_trait]
pub trait PolicyEvaluator: Send + Sync {
    /// Evaluate a GraphQL operation against policies.
    async fn evaluate_operation(
        &self,
        operation: &OperationEntity,
        server_config: &ServerConfigEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError>;

    /// Evaluate a JavaScript script against policies (OpenAPI Code Mode).
    /// Default: denies all scripts (override for OpenAPI support).
    #[cfg(feature = "openapi-code-mode")]
    async fn evaluate_script(
        &self,
        _script: &ScriptEntity,
        _server: &OpenAPIServerEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        Ok(AuthorizationDecision {
            allowed: false,
            determining_policies: vec!["default_deny_scripts".to_string()],
            errors: vec!["Script evaluation not supported by this policy evaluator".to_string()],
        })
    }

    /// Evaluate a SQL statement against policies (SQL Code Mode).
    /// Default: denies all statements (override for SQL support).
    #[cfg(feature = "sql-code-mode")]
    async fn evaluate_statement(
        &self,
        _statement: &StatementEntity,
        _server: &SqlServerEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        Ok(AuthorizationDecision {
            allowed: false,
            determining_policies: vec!["default_deny_statements".to_string()],
            errors: vec![
                "SQL statement evaluation not supported by this policy evaluator".to_string(),
            ],
        })
    }

    /// Batch evaluation (default: sequential).
    async fn batch_evaluate(
        &self,
        requests: Vec<(OperationEntity, ServerConfigEntity)>,
    ) -> Result<Vec<AuthorizationDecision>, PolicyEvaluationError> {
        let mut results = Vec::with_capacity(requests.len());
        for (op, config) in &requests {
            results.push(self.evaluate_operation(op, config).await?);
        }
        Ok(results)
    }

    /// Whether this evaluator is configured and ready.
    fn is_configured(&self) -> bool {
        true
    }

    /// Human-readable name for logging.
    fn name(&self) -> &str;
}

/// Always-allow policy evaluator for **testing and local development ONLY**.
///
/// # WARNING: Not for Production Use
///
/// Returns `allowed: true` for all policy evaluations, completely bypassing
/// access control. Using this in production disables the entire policy layer.
///
/// For production, implement [`PolicyEvaluator`] with your authorization backend
/// (e.g., `CedarPolicyEvaluator` behind the `cedar` feature, or a custom
/// implementation calling AWS Verified Permissions).
///
/// # Example
///
/// ```rust
/// use pmcp_code_mode::{NoopPolicyEvaluator, PolicyEvaluator};
///
/// // Test-only usage
/// let evaluator = NoopPolicyEvaluator::new();
/// assert_eq!(evaluator.name(), "noop");
/// ```
pub struct NoopPolicyEvaluator;

impl NoopPolicyEvaluator {
    /// Create a new no-op policy evaluator.
    ///
    /// # Warning
    /// This evaluator allows ALL operations. Only use in tests or local development.
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopPolicyEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PolicyEvaluator for NoopPolicyEvaluator {
    async fn evaluate_operation(
        &self,
        _operation: &OperationEntity,
        _server_config: &ServerConfigEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        Ok(AuthorizationDecision {
            allowed: true,
            determining_policies: vec!["noop_allow_all".to_string()],
            errors: vec![],
        })
    }

    #[cfg(feature = "openapi-code-mode")]
    async fn evaluate_script(
        &self,
        _script: &ScriptEntity,
        _server: &OpenAPIServerEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        Ok(AuthorizationDecision {
            allowed: true,
            determining_policies: vec!["noop_allow_all_scripts".to_string()],
            errors: vec![],
        })
    }

    #[cfg(feature = "sql-code-mode")]
    async fn evaluate_statement(
        &self,
        _statement: &StatementEntity,
        _server: &SqlServerEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        Ok(AuthorizationDecision {
            allowed: true,
            determining_policies: vec!["noop_allow_all_statements".to_string()],
            errors: vec![],
        })
    }

    fn name(&self) -> &str {
        "noop"
    }
}

#[cfg(test)]
mod noop_tests {
    use super::*;

    #[tokio::test]
    async fn noop_evaluator_allows_all_operations() {
        let evaluator = NoopPolicyEvaluator::new();
        let operation = OperationEntity {
            id: "test-op".to_string(),
            operation_type: "query".to_string(),
            operation_name: "GetUsers".to_string(),
            root_fields: ["users"].iter().map(|s| s.to_string()).collect(),
            accessed_types: ["User"].iter().map(|s| s.to_string()).collect(),
            accessed_fields: ["User.id", "User.name"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            depth: 2,
            field_count: 2,
            estimated_cost: 2,
            has_introspection: false,
            accesses_sensitive_data: false,
            sensitive_categories: std::collections::HashSet::new(),
        };
        let config = ServerConfigEntity::default();
        let result = evaluator
            .evaluate_operation(&operation, &config)
            .await
            .unwrap();
        assert!(result.allowed);
        assert_eq!(result.determining_policies, vec!["noop_allow_all"]);
    }

    #[test]
    fn noop_evaluator_name() {
        let evaluator = NoopPolicyEvaluator::new();
        assert_eq!(evaluator.name(), "noop");
    }

    #[test]
    fn noop_evaluator_default() {
        let evaluator = NoopPolicyEvaluator::default();
        assert_eq!(evaluator.name(), "noop");
    }
}
