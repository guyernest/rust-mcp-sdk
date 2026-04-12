//! Validation pipeline for Code Mode.
//!
//! The pipeline validates code through multiple stages:
//! 1. Parse (syntax check)
//! 2. Policy evaluation (PolicyEvaluator trait or basic config checks)
//! 3. Security analysis
//! 4. Explanation generation
//! 5. Token generation

use crate::config::CodeModeConfig;
use crate::explanation::{ExplanationGenerator, TemplateExplanationGenerator};
use crate::graphql::{GraphQLQueryInfo, GraphQLValidator};
use crate::policy::{OperationEntity, PolicyEvaluator};
use crate::token::{compute_context_hash, HmacTokenGenerator, TokenGenerator, TokenSecret};
use crate::types::{
    PolicyViolation, UnifiedAction, ValidationError, ValidationMetadata, ValidationResult,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[cfg(feature = "openapi-code-mode")]
use crate::javascript::{JavaScriptCodeInfo, JavaScriptValidator};

/// Static flag to ensure the "no policy evaluator" warning is only logged once per process.
static NO_POLICY_WARNING_LOGGED: AtomicBool = AtomicBool::new(false);

/// Log a warning when Code Mode is enabled without a policy evaluator.
fn warn_no_policy_configured() {
    if !NO_POLICY_WARNING_LOGGED.swap(true, Ordering::SeqCst) {
        tracing::warn!(
            target: "code_mode",
            "CODE MODE SECURITY WARNING: Code Mode is enabled but no policy evaluator \
            is configured. Only basic config checks (allow_mutations, max_depth, etc.) will be \
            performed. This provides NO real authorization policy evaluation. \
            For production deployments, configure a policy evaluator (AVP or local Cedar)."
        );
    }
}

/// Context for validation (user, session, schema).
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// User ID from access token
    pub user_id: String,

    /// MCP session ID
    pub session_id: String,

    /// Schema hash for context binding
    pub schema_hash: String,

    /// Permissions hash for context binding
    pub permissions_hash: String,
}

impl ValidationContext {
    /// Create a new validation context.
    pub fn new(
        user_id: impl Into<String>,
        session_id: impl Into<String>,
        schema_hash: impl Into<String>,
        permissions_hash: impl Into<String>,
    ) -> Self {
        Self {
            user_id: user_id.into(),
            session_id: session_id.into(),
            schema_hash: schema_hash.into(),
            permissions_hash: permissions_hash.into(),
        }
    }

    /// Compute the combined context hash.
    pub fn context_hash(&self) -> String {
        compute_context_hash(&self.schema_hash, &self.permissions_hash)
    }
}

/// The validation pipeline that orchestrates all validation stages.
pub struct ValidationPipeline<
    T: TokenGenerator = HmacTokenGenerator,
    E: ExplanationGenerator = TemplateExplanationGenerator,
> {
    config: CodeModeConfig,
    graphql_validator: GraphQLValidator,
    #[cfg(feature = "openapi-code-mode")]
    javascript_validator: JavaScriptValidator,
    token_generator: T,
    explanation_generator: E,
    policy_evaluator: Option<Box<dyn PolicyEvaluator>>,
}

impl ValidationPipeline<HmacTokenGenerator, TemplateExplanationGenerator> {
    /// Create a new validation pipeline with default generators.
    ///
    /// **Warning**: This constructor does not configure a policy evaluator.
    /// Only basic config checks will be performed.
    pub fn new(config: CodeModeConfig, token_secret: impl Into<Vec<u8>>) -> Self {
        if config.enabled {
            warn_no_policy_configured();
        }

        Self {
            graphql_validator: GraphQLValidator::default(),
            #[cfg(feature = "openapi-code-mode")]
            javascript_validator: JavaScriptValidator::default()
                .with_sdk_operations(config.sdk_operations.clone()),
            token_generator: HmacTokenGenerator::new_from_bytes(token_secret),
            explanation_generator: TemplateExplanationGenerator::new(),
            policy_evaluator: None,
            config,
        }
    }

    /// Create a new validation pipeline from a [`TokenSecret`].
    ///
    /// This is the **secure entry point** for production callers and derive macro
    /// generated code. The secret is unwrapped internally -- callers never need
    /// to call `expose_secret()` directly.
    ///
    /// **Warning**: This constructor does not configure a policy evaluator.
    /// Only basic config checks will be performed.
    pub fn from_token_secret(config: CodeModeConfig, secret: &TokenSecret) -> Self {
        Self::new(config, secret.expose_secret().to_vec())
    }

    /// Create a new validation pipeline with a policy evaluator.
    pub fn with_policy_evaluator(
        config: CodeModeConfig,
        token_secret: impl Into<Vec<u8>>,
        evaluator: Box<dyn PolicyEvaluator>,
    ) -> Self {
        Self {
            graphql_validator: GraphQLValidator::default(),
            #[cfg(feature = "openapi-code-mode")]
            javascript_validator: JavaScriptValidator::default()
                .with_sdk_operations(config.sdk_operations.clone()),
            token_generator: HmacTokenGenerator::new_from_bytes(token_secret),
            explanation_generator: TemplateExplanationGenerator::new(),
            policy_evaluator: Some(evaluator),
            config,
        }
    }
}

impl<T: TokenGenerator, E: ExplanationGenerator> ValidationPipeline<T, E> {
    /// Create a pipeline with custom generators.
    pub fn with_generators(
        config: CodeModeConfig,
        token_generator: T,
        explanation_generator: E,
    ) -> Self {
        Self {
            graphql_validator: GraphQLValidator::default(),
            #[cfg(feature = "openapi-code-mode")]
            javascript_validator: JavaScriptValidator::default()
                .with_sdk_operations(config.sdk_operations.clone()),
            token_generator,
            explanation_generator,
            policy_evaluator: None,
            config,
        }
    }

    /// Set the policy evaluator for this pipeline.
    pub fn set_policy_evaluator(&mut self, evaluator: Box<dyn PolicyEvaluator>) {
        self.policy_evaluator = Some(evaluator);
    }

    /// Check if a policy evaluator is configured.
    pub fn has_policy_evaluator(&self) -> bool {
        self.policy_evaluator.is_some()
    }

    /// Validate a GraphQL query using basic config checks only.
    pub fn validate_graphql_query(
        &self,
        query: &str,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        let start = Instant::now();

        if !self.config.enabled {
            return Err(ValidationError::ConfigError(
                "Code Mode is not enabled for this server".into(),
            ));
        }

        if query.len() > self.config.max_query_length {
            return Err(ValidationError::SecurityError {
                message: format!(
                    "Query length {} exceeds maximum {}",
                    query.len(),
                    self.config.max_query_length
                ),
                issue: crate::types::SecurityIssueType::HighComplexity,
            });
        }

        let query_info = self.graphql_validator.validate(query)?;

        // Mutation authorization checks
        if !query_info.operation_type.is_read_only() {
            let mutation_name = query_info.root_fields.first().cloned().unwrap_or_default();

            if !self.config.blocked_mutations.is_empty()
                && self.config.blocked_mutations.contains(&mutation_name)
            {
                return Ok(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "blocked_mutation",
                        &format!("Mutation '{}' is blocked for this server", mutation_name),
                    )
                    .with_suggestion("This mutation is in the blocklist and cannot be executed")],
                    self.build_metadata(&query_info, start.elapsed().as_millis() as u64),
                ));
            }

            if !self.config.allowed_mutations.is_empty() {
                if !self.config.allowed_mutations.contains(&mutation_name) {
                    return Ok(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "mutation_not_allowed",
                            &format!("Mutation '{}' is not in the allowlist", mutation_name),
                        )
                        .with_suggestion(&format!(
                            "Only these mutations are allowed: {}",
                            self.config
                                .allowed_mutations
                                .iter()
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(", ")
                        ))],
                        self.build_metadata(&query_info, start.elapsed().as_millis() as u64),
                    ));
                }
            } else if !self.config.allow_mutations {
                return Ok(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "allow_mutations",
                        "Mutations are not enabled for this server",
                    )
                    .with_suggestion("Only read-only queries are allowed")],
                    self.build_metadata(&query_info, start.elapsed().as_millis() as u64),
                ));
            }
        }

        // Query (read) authorization checks -- mirrors mutation enforcement above
        if query_info.operation_type.is_read_only() {
            let query_name = query_info.root_fields.first().cloned().unwrap_or_default();

            if !self.config.blocked_queries.is_empty()
                && self.config.blocked_queries.contains(&query_name)
            {
                return Ok(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "blocked_query",
                        &format!("Query '{}' is blocked for this server", query_name),
                    )
                    .with_suggestion("This query is in the blocklist and cannot be executed")],
                    self.build_metadata(&query_info, start.elapsed().as_millis() as u64),
                ));
            }

            if !self.config.allowed_queries.is_empty()
                && !self.config.allowed_queries.contains(&query_name)
            {
                return Ok(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "query_not_allowed",
                        &format!("Query '{}' is not in the allowlist", query_name),
                    )
                    .with_suggestion(&format!(
                        "Only these queries are allowed: {}",
                        self.config
                            .allowed_queries
                            .iter()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ))],
                    self.build_metadata(&query_info, start.elapsed().as_millis() as u64),
                ));
            }
        }

        self.complete_validation(query, &query_info, context, start)
    }

    /// Validate a GraphQL query using a policy evaluator (async).
    pub async fn validate_graphql_query_async(
        &self,
        query: &str,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        let start = Instant::now();

        if !self.config.enabled {
            return Err(ValidationError::ConfigError(
                "Code Mode is not enabled for this server".into(),
            ));
        }

        if query.len() > self.config.max_query_length {
            return Err(ValidationError::SecurityError {
                message: format!(
                    "Query length {} exceeds maximum {}",
                    query.len(),
                    self.config.max_query_length
                ),
                issue: crate::types::SecurityIssueType::HighComplexity,
            });
        }

        let query_info = self.graphql_validator.validate(query)?;

        // Policy evaluation via trait
        if let Some(ref evaluator) = self.policy_evaluator {
            let operation_entity = OperationEntity::from_query_info(&query_info);
            let server_config = self.config.to_server_config_entity();

            let decision = evaluator
                .evaluate_operation(&operation_entity, &server_config)
                .await
                .map_err(|e| {
                    ValidationError::InternalError(format!("Policy evaluation error: {}", e))
                })?;

            if !decision.allowed {
                let violations: Vec<PolicyViolation> = decision
                    .determining_policies
                    .iter()
                    .map(|policy_id| {
                        PolicyViolation::new(
                            "policy",
                            policy_id.clone(),
                            "Policy denied the operation",
                        )
                    })
                    .collect();

                return Ok(ValidationResult::failure(
                    violations,
                    self.build_metadata(&query_info, start.elapsed().as_millis() as u64),
                ));
            }
        } else {
            warn_no_policy_configured();
            tracing::debug!(
                target: "code_mode",
                "Falling back to basic config checks (no policy evaluator configured)"
            );
            return self.validate_graphql_query(query, context);
        }

        self.complete_validation(query, &query_info, context, start)
    }

    /// Complete validation after policy check passes.
    fn complete_validation(
        &self,
        query: &str,
        query_info: &GraphQLQueryInfo,
        context: &ValidationContext,
        start: Instant,
    ) -> Result<ValidationResult, ValidationError> {
        let security_analysis = self.graphql_validator.analyze_security(query_info);
        let risk_level = security_analysis.assess_risk();

        if security_analysis
            .potential_issues
            .iter()
            .any(|i| i.is_critical())
        {
            let violations: Vec<PolicyViolation> = security_analysis
                .potential_issues
                .iter()
                .filter(|i| i.is_critical())
                .map(|i| {
                    PolicyViolation::new("security", format!("{:?}", i.issue_type), &i.message)
                })
                .collect();

            return Ok(ValidationResult::failure(
                violations,
                self.build_metadata(query_info, start.elapsed().as_millis() as u64),
            ));
        }

        let explanation = self
            .explanation_generator
            .explain_graphql(query_info, &security_analysis);

        let context_hash = context.context_hash();
        let token = self.token_generator.generate(
            query,
            &context.user_id,
            &context.session_id,
            self.config.server_id(),
            &context_hash,
            risk_level,
            self.config.token_ttl_seconds,
        );

        let token_string = token.encode().map_err(|e| {
            ValidationError::InternalError(format!("Failed to encode token: {}", e))
        })?;

        let operation_type_str = format!("{:?}", query_info.operation_type).to_lowercase();
        let mutation_name = query_info.operation_name.as_deref();
        let inferred_action = UnifiedAction::from_graphql(&operation_type_str, mutation_name);
        let action = UnifiedAction::resolve(
            inferred_action,
            &self.config.action_tags,
            query_info.operation_name.as_deref().unwrap_or(""),
        );

        let metadata = ValidationMetadata {
            is_read_only: query_info.operation_type.is_read_only(),
            estimated_rows: security_analysis.estimated_rows,
            accessed_types: security_analysis.tables_accessed.iter().cloned().collect(),
            accessed_fields: security_analysis.fields_accessed.iter().cloned().collect(),
            has_aggregation: security_analysis.has_aggregation,
            code_type: Some(self.graphql_validator.to_code_type(query_info)),
            action: Some(action),
            validation_time_ms: start.elapsed().as_millis() as u64,
        };

        let mut result = ValidationResult::success(explanation, risk_level, token_string, metadata);

        for issue in &security_analysis.potential_issues {
            if !issue.is_critical() {
                result.warnings.push(issue.message.clone());
            }
        }

        Ok(result)
    }

    /// Build metadata from query info.
    fn build_metadata(
        &self,
        query_info: &GraphQLQueryInfo,
        validation_time_ms: u64,
    ) -> ValidationMetadata {
        let operation_type_str = format!("{:?}", query_info.operation_type).to_lowercase();
        let mutation_name = query_info.operation_name.as_deref();
        let inferred_action = UnifiedAction::from_graphql(&operation_type_str, mutation_name);
        let action = UnifiedAction::resolve(
            inferred_action,
            &self.config.action_tags,
            query_info.operation_name.as_deref().unwrap_or(""),
        );

        ValidationMetadata {
            is_read_only: query_info.operation_type.is_read_only(),
            estimated_rows: None,
            accessed_types: query_info.types_accessed.iter().cloned().collect(),
            accessed_fields: query_info.fields_accessed.iter().cloned().collect(),
            has_aggregation: false,
            code_type: Some(self.graphql_validator.to_code_type(query_info)),
            action: Some(action),
            validation_time_ms,
        }
    }

    /// Validate JavaScript code for OpenAPI Code Mode.
    #[cfg(feature = "openapi-code-mode")]
    pub fn validate_javascript_code(
        &self,
        code: &str,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        let start = Instant::now();

        if !self.config.enabled {
            return Err(ValidationError::ConfigError(
                "Code Mode is not enabled for this server".into(),
            ));
        }

        if code.len() > self.config.max_query_length {
            return Err(ValidationError::SecurityError {
                message: format!(
                    "Code length {} exceeds maximum {}",
                    code.len(),
                    self.config.max_query_length
                ),
                issue: crate::types::SecurityIssueType::HighComplexity,
            });
        }

        let code_info = self.javascript_validator.validate(code)?;

        if !code_info.is_read_only {
            for method in &code_info.methods_used {
                if !self.config.openapi_blocked_writes.is_empty()
                    && self.config.openapi_blocked_writes.contains(method)
                {
                    return Ok(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "blocked_method",
                            &format!("HTTP method '{}' is blocked for this server", method),
                        )
                        .with_suggestion("This method is in the blocklist and cannot be used")],
                        self.build_js_metadata(&code_info, start.elapsed().as_millis() as u64),
                    ));
                }
            }

            if !self.config.openapi_allowed_writes.is_empty() {
                tracing::debug!(
                    target: "code_mode",
                    "Skipping method-level check - policy evaluator will check operation allowlist ({} entries)",
                    self.config.openapi_allowed_writes.len()
                );
            } else if !self.config.openapi_allow_writes {
                return Ok(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "allow_mutations",
                        "Write HTTP methods (POST, PUT, DELETE, PATCH) are not enabled for this server",
                    )
                    .with_suggestion("Only read-only methods (GET, HEAD, OPTIONS) are allowed. Contact your administrator to enable write operations.")],
                    self.build_js_metadata(&code_info, start.elapsed().as_millis() as u64),
                ));
            }
        }

        self.complete_js_validation(code, &code_info, context, start)
    }

    /// Complete JavaScript validation after policy checks pass.
    #[cfg(feature = "openapi-code-mode")]
    fn complete_js_validation(
        &self,
        code: &str,
        code_info: &JavaScriptCodeInfo,
        context: &ValidationContext,
        start: Instant,
    ) -> Result<ValidationResult, ValidationError> {
        let security_analysis = self.javascript_validator.analyze_security(code_info);
        let risk_level = security_analysis.assess_risk();

        if security_analysis
            .potential_issues
            .iter()
            .any(|i| i.is_critical())
        {
            let violations: Vec<PolicyViolation> = security_analysis
                .potential_issues
                .iter()
                .filter(|i| i.is_critical())
                .map(|i| {
                    PolicyViolation::new("security", format!("{:?}", i.issue_type), &i.message)
                })
                .collect();

            return Ok(ValidationResult::failure(
                violations,
                self.build_js_metadata(code_info, start.elapsed().as_millis() as u64),
            ));
        }

        let explanation = self.generate_js_explanation(code_info, &security_analysis);

        let context_hash = context.context_hash();
        let token = self.token_generator.generate(
            code,
            &context.user_id,
            &context.session_id,
            self.config.server_id(),
            &context_hash,
            risk_level,
            self.config.token_ttl_seconds,
        );

        let token_string = token.encode().map_err(|e| {
            ValidationError::InternalError(format!("Failed to encode token: {}", e))
        })?;

        let metadata = self.build_js_metadata(code_info, start.elapsed().as_millis() as u64);

        let mut result = ValidationResult::success(explanation, risk_level, token_string, metadata);

        for issue in &security_analysis.potential_issues {
            if !issue.is_critical() {
                result.warnings.push(issue.message.clone());
            }
        }

        Ok(result)
    }

    /// Build metadata from JavaScript code info.
    #[cfg(feature = "openapi-code-mode")]
    fn build_js_metadata(
        &self,
        code_info: &JavaScriptCodeInfo,
        validation_time_ms: u64,
    ) -> ValidationMetadata {
        let action = if !code_info.api_calls.is_empty() {
            let mut max_action = UnifiedAction::Read;
            for call in &code_info.api_calls {
                let method_str = format!("{:?}", call.method);
                let inferred = UnifiedAction::from_http_method(&method_str);
                match (&max_action, &inferred) {
                    (UnifiedAction::Read, _) => max_action = inferred,
                    (UnifiedAction::Write, UnifiedAction::Delete | UnifiedAction::Admin) => {
                        max_action = inferred
                    },
                    (UnifiedAction::Delete, UnifiedAction::Admin) => max_action = inferred,
                    _ => {},
                }
            }
            Some(max_action)
        } else if code_info.is_read_only {
            Some(UnifiedAction::Read)
        } else {
            Some(UnifiedAction::Write)
        };

        ValidationMetadata {
            is_read_only: code_info.is_read_only,
            estimated_rows: None,
            accessed_types: code_info.endpoints_accessed.iter().cloned().collect(),
            accessed_fields: code_info.methods_used.iter().cloned().collect(),
            has_aggregation: false,
            code_type: Some(self.javascript_validator.to_code_type(code_info)),
            action,
            validation_time_ms,
        }
    }

    /// Generate a human-readable explanation for JavaScript code.
    #[cfg(feature = "openapi-code-mode")]
    fn generate_js_explanation(
        &self,
        code_info: &JavaScriptCodeInfo,
        security_analysis: &crate::types::SecurityAnalysis,
    ) -> String {
        let mut parts = Vec::new();

        if code_info.is_read_only {
            parts.push("This code will perform read-only API requests.".to_string());
        } else {
            parts.push("This code will perform API requests that may modify data.".to_string());
        }

        if !code_info.api_calls.is_empty() {
            let call_descriptions: Vec<String> = code_info
                .api_calls
                .iter()
                .map(|call| format!("{:?} {}", call.method, call.path))
                .collect();

            if call_descriptions.len() <= 3 {
                parts.push(format!("API calls: {}", call_descriptions.join(", ")));
            } else {
                parts.push(format!(
                    "API calls: {} and {} more",
                    call_descriptions[..2].join(", "),
                    call_descriptions.len() - 2
                ));
            }
        }

        if code_info.loop_count > 0 {
            if code_info.all_loops_bounded {
                parts.push(format!(
                    "Contains {} bounded loop(s).",
                    code_info.loop_count
                ));
            } else {
                parts.push(format!(
                    "Contains {} loop(s) - ensure they are properly bounded.",
                    code_info.loop_count
                ));
            }
        }

        let risk = security_analysis.assess_risk();
        parts.push(format!("Risk: {}", risk));

        parts.join(" ")
    }

    /// Check if a validation result should be auto-approved.
    pub fn should_auto_approve(&self, result: &ValidationResult) -> bool {
        result.is_valid && self.config.should_auto_approve(result.risk_level)
    }

    /// Get the config.
    pub fn config(&self) -> &CodeModeConfig {
        &self.config
    }

    /// Get the token generator.
    pub fn token_generator(&self) -> &T {
        &self.token_generator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RiskLevel;

    fn test_pipeline() -> ValidationPipeline {
        ValidationPipeline::new(CodeModeConfig::enabled(), b"test-secret-key!".to_vec())
    }

    fn test_context() -> ValidationContext {
        ValidationContext::new("user-123", "session-456", "schema-hash", "perms-hash")
    }

    #[test]
    fn test_simple_query_validation() {
        let pipeline = test_pipeline();
        let ctx = test_context();

        let result = pipeline
            .validate_graphql_query("query { users { id name } }", &ctx)
            .unwrap();

        assert!(result.is_valid);
        assert!(result.approval_token.is_some());
        assert_eq!(result.risk_level, RiskLevel::Low);
        assert!(result.explanation.contains("read"));
    }

    #[test]
    fn test_mutation_blocked() {
        let mut config = CodeModeConfig::enabled();
        config.allow_mutations = false;

        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec());
        let ctx = test_context();

        let result = pipeline
            .validate_graphql_query("mutation { createUser(name: \"test\") { id } }", &ctx)
            .unwrap();

        assert!(!result.is_valid);
        assert!(result
            .violations
            .iter()
            .any(|v| v.rule == "allow_mutations"));
    }

    #[test]
    fn test_disabled_code_mode() {
        let config = CodeModeConfig::default();
        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec());
        let ctx = test_context();

        let result = pipeline.validate_graphql_query("query { users { id } }", &ctx);

        assert!(matches!(result, Err(ValidationError::ConfigError(_))));
    }

    #[test]
    fn test_auto_approve_low_risk() {
        let pipeline = test_pipeline();
        let ctx = test_context();

        let result = pipeline
            .validate_graphql_query("query { users { id } }", &ctx)
            .unwrap();

        assert!(pipeline.should_auto_approve(&result));
    }

    #[test]
    fn test_context_hash() {
        let ctx = test_context();
        let hash1 = ctx.context_hash();

        let ctx2 =
            ValidationContext::new("user-123", "session-456", "different-schema", "perms-hash");
        let hash2 = ctx2.context_hash();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_blocked_query_rejected() {
        let mut config = CodeModeConfig::enabled();
        config
            .blocked_queries
            .insert("users".to_string());

        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec());
        let ctx = test_context();

        let result = pipeline
            .validate_graphql_query("query { users { id } }", &ctx)
            .unwrap();

        assert!(!result.is_valid);
        assert!(result
            .violations
            .iter()
            .any(|v| v.rule == "blocked_query"));
    }

    #[test]
    fn test_allowed_queries_enforced() {
        let mut config = CodeModeConfig::enabled();
        config
            .allowed_queries
            .insert("orders".to_string());

        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec());
        let ctx = test_context();

        // "users" is not in the allowlist -- should be rejected
        let result = pipeline
            .validate_graphql_query("query { users { id } }", &ctx)
            .unwrap();

        assert!(!result.is_valid);
        assert!(result
            .violations
            .iter()
            .any(|v| v.rule == "query_not_allowed"));
    }
}
