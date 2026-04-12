//! Policy evaluation for Code Mode.
//!
//! This module provides a trait-based abstraction for policy evaluation,
//! allowing different backends (AVP, local Cedar, etc.) to be used
//! interchangeably.

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
