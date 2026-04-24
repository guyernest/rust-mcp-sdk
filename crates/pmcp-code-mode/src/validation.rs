//! Validation pipeline for Code Mode.
//!
//! The pipeline validates code through multiple stages:
//! 1. Parse (syntax check)
//! 2. Policy evaluation (PolicyEvaluator trait or basic config checks)
//! 3. Security analysis
//! 4. Explanation generation
//! 5. Token generation

use crate::config::CodeModeConfig;
#[cfg(feature = "openapi-code-mode")]
use crate::config::OperationRegistry;
use crate::explanation::{ExplanationGenerator, TemplateExplanationGenerator};
use crate::graphql::{GraphQLQueryInfo, GraphQLValidator};
use crate::policy::{OperationEntity, PolicyEvaluator};
use crate::token::{compute_context_hash, HmacTokenGenerator, TokenGenerator, TokenSecret};
use crate::types::{
    PolicyViolation, TokenError, UnifiedAction, ValidationError, ValidationMetadata,
    ValidationResult,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

#[cfg(feature = "openapi-code-mode")]
use crate::javascript::{JavaScriptCodeInfo, JavaScriptValidator};

/// Static flag to ensure the "no policy evaluator" warning is only logged once per process.
static NO_POLICY_WARNING_LOGGED: AtomicBool = AtomicBool::new(false);

/// Build the `Vec<PolicyViolation>` for a denied authorization decision.
///
/// Flows all available diagnostic information from the decision into violations
/// so the client sees *why* a request was denied:
///
/// - Each `determining_policies` entry becomes a `policy` violation naming that policy
/// - Each `decision.errors` entry becomes a `policy_error` violation surfacing the
///   underlying Cedar/AVP error (e.g., "entity does not conform to schema")
/// - If both lists are empty (the canonical "default-deny: no Permit matched" case)
///   a synthetic `default_deny` violation is injected with context about the
///   server_id and action, so the client has *something* to debug against
///
/// Without this, default-deny would surface as `valid:false, violations:[]` —
/// impossible to debug from the client.
fn build_policy_violations(
    decision: &crate::policy::AuthorizationDecision,
    server_id: &str,
    action: impl std::fmt::Display,
    denied_subject: &str,
) -> Vec<PolicyViolation> {
    let capacity = decision.determining_policies.len() + decision.errors.len() + 1;
    let mut violations: Vec<PolicyViolation> = Vec::with_capacity(capacity);

    for policy_id in &decision.determining_policies {
        violations.push(PolicyViolation::new(
            "policy",
            policy_id.clone(),
            format!("Policy denied the {}", denied_subject),
        ));
    }

    for err in &decision.errors {
        violations.push(PolicyViolation::new(
            "policy_error",
            "evaluation_error",
            err.clone(),
        ));
    }

    if violations.is_empty() {
        violations.push(PolicyViolation::new(
            "policy",
            "default_deny",
            format!(
                "Authorization default-deny: no Permit policy matched for \
                 server_id={server_id} action={action}. Check that Cedar \
                 policies exist for this server and that server_id is set correctly."
            ),
        ));
    }

    violations
}

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
    #[cfg(feature = "openapi-code-mode")]
    operation_registry: OperationRegistry,
    token_generator: T,
    explanation_generator: E,
    policy_evaluator: Option<Arc<dyn PolicyEvaluator>>,
}

impl ValidationPipeline<HmacTokenGenerator, TemplateExplanationGenerator> {
    /// Create a new validation pipeline with default generators.
    ///
    /// **Warning**: This constructor does not configure a policy evaluator.
    /// Only basic config checks will be performed.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::SecretTooShort`] if the token secret is shorter
    /// than [`HmacTokenGenerator::MIN_SECRET_LEN`] (16 bytes).
    pub fn new(
        mut config: CodeModeConfig,
        token_secret: impl Into<Vec<u8>>,
    ) -> Result<Self, TokenError> {
        if config.enabled {
            warn_no_policy_configured();
        }

        config.resolve_server_id();

        #[cfg(feature = "openapi-code-mode")]
        let operation_registry = OperationRegistry::from_entries(&config.operations);

        Ok(Self {
            graphql_validator: GraphQLValidator::default(),
            #[cfg(feature = "openapi-code-mode")]
            javascript_validator: JavaScriptValidator::default()
                .with_sdk_operations(config.sdk_operations.clone()),
            #[cfg(feature = "openapi-code-mode")]
            operation_registry,
            token_generator: HmacTokenGenerator::new_from_bytes(token_secret)?,
            explanation_generator: TemplateExplanationGenerator::new(),
            policy_evaluator: None,
            config,
        })
    }

    /// Create a new validation pipeline from a [`TokenSecret`].
    ///
    /// Convenience constructor for production callers and derive macro generated
    /// code. Callers never need to call `expose_secret()` directly.
    ///
    /// **Security note**: Internally this creates an intermediate `Vec<u8>` copy
    /// of the secret bytes that is **not** zeroized on drop. For maximum security,
    /// prefer [`TokenSecret::from_env`] which minimizes secret copies. This
    /// limitation will be addressed in a future version by adding a
    /// `HmacTokenGenerator::from_secret_ref` constructor.
    ///
    /// **Warning**: This constructor does not configure a policy evaluator.
    /// Only basic config checks will be performed.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::SecretTooShort`] if the token secret is shorter
    /// than [`HmacTokenGenerator::MIN_SECRET_LEN`] (16 bytes).
    pub fn from_token_secret(
        config: CodeModeConfig,
        secret: &TokenSecret,
    ) -> Result<Self, TokenError> {
        Self::new(config, secret.expose_secret().to_vec())
    }

    /// Create a new validation pipeline with a policy evaluator.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::SecretTooShort`] if the token secret is shorter
    /// than [`HmacTokenGenerator::MIN_SECRET_LEN`] (16 bytes).
    pub fn with_policy_evaluator(
        mut config: CodeModeConfig,
        token_secret: impl Into<Vec<u8>>,
        evaluator: Arc<dyn PolicyEvaluator>,
    ) -> Result<Self, TokenError> {
        config.resolve_server_id();
        if config.server_id.is_none() {
            tracing::warn!(
                target: "code_mode",
                "CodeModeConfig.server_id is not set — AVP/Cedar authorization will use 'unknown' \
                 as the resource entity ID and will likely default-deny silently. \
                 Set server_id in config.toml, or the PMCP_SERVER_ID or AWS_LAMBDA_FUNCTION_NAME env var."
            );
        }

        #[cfg(feature = "openapi-code-mode")]
        let operation_registry = OperationRegistry::from_entries(&config.operations);

        Ok(Self {
            graphql_validator: GraphQLValidator::default(),
            #[cfg(feature = "openapi-code-mode")]
            javascript_validator: JavaScriptValidator::default()
                .with_sdk_operations(config.sdk_operations.clone()),
            #[cfg(feature = "openapi-code-mode")]
            operation_registry,
            token_generator: HmacTokenGenerator::new_from_bytes(token_secret)?,
            explanation_generator: TemplateExplanationGenerator::new(),
            policy_evaluator: Some(evaluator),
            config,
        })
    }

    /// Create a pipeline from a [`TokenSecret`] with an `Arc` policy evaluator.
    ///
    /// Used by derive macro generated code where the policy evaluator is
    /// stored as `Arc<dyn PolicyEvaluator>` on the parent struct.
    ///
    /// # Errors
    ///
    /// Returns [`TokenError::SecretTooShort`] if the token secret is shorter
    /// than [`HmacTokenGenerator::MIN_SECRET_LEN`] (16 bytes).
    pub fn from_token_secret_with_policy(
        config: CodeModeConfig,
        secret: &TokenSecret,
        evaluator: Arc<dyn PolicyEvaluator>,
    ) -> Result<Self, TokenError> {
        Self::with_policy_evaluator(config, secret.expose_secret().to_vec(), evaluator)
    }
}

impl<T: TokenGenerator, E: ExplanationGenerator> ValidationPipeline<T, E> {
    /// Create a pipeline with custom generators.
    pub fn with_generators(
        mut config: CodeModeConfig,
        token_generator: T,
        explanation_generator: E,
    ) -> Self {
        config.resolve_server_id();

        #[cfg(feature = "openapi-code-mode")]
        let operation_registry = OperationRegistry::from_entries(&config.operations);

        Self {
            graphql_validator: GraphQLValidator::default(),
            #[cfg(feature = "openapi-code-mode")]
            javascript_validator: JavaScriptValidator::default()
                .with_sdk_operations(config.sdk_operations.clone()),
            #[cfg(feature = "openapi-code-mode")]
            operation_registry,
            token_generator,
            explanation_generator,
            policy_evaluator: None,
            config,
        }
    }

    /// Set the policy evaluator for this pipeline.
    pub fn set_policy_evaluator(&mut self, evaluator: Arc<dyn PolicyEvaluator>) {
        self.policy_evaluator = Some(evaluator);
    }

    /// Check if a policy evaluator is configured.
    pub fn has_policy_evaluator(&self) -> bool {
        self.policy_evaluator.is_some()
    }

    /// Check mutation and query authorization against config (blocklists, allowlists).
    ///
    /// This is the authorization logic shared between the sync and async validation paths.
    /// It uses the already-parsed `query_info` to avoid re-parsing.
    fn check_config_authorization(
        &self,
        query_info: &GraphQLQueryInfo,
        start: Instant,
    ) -> Option<ValidationResult> {
        // Mutation authorization checks
        if !query_info.operation_type.is_read_only() {
            let mutation_name = query_info.root_fields.first().cloned().unwrap_or_default();

            if !self.config.blocked_mutations.is_empty()
                && self.config.blocked_mutations.contains(&mutation_name)
            {
                return Some(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "blocked_mutation",
                        &format!("Mutation '{}' is blocked for this server", mutation_name),
                    )
                    .with_suggestion("This mutation is in the blocklist and cannot be executed")],
                    self.build_metadata(query_info, start.elapsed().as_millis() as u64),
                ));
            }

            if !self.config.allowed_mutations.is_empty() {
                if !self.config.allowed_mutations.contains(&mutation_name) {
                    return Some(ValidationResult::failure(
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
                        self.build_metadata(query_info, start.elapsed().as_millis() as u64),
                    ));
                }
            } else if !self.config.allow_mutations {
                return Some(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "allow_mutations",
                        "Mutations are not enabled for this server",
                    )
                    .with_suggestion("Only read-only queries are allowed")],
                    self.build_metadata(query_info, start.elapsed().as_millis() as u64),
                ));
            }
        }

        // Query (read) authorization checks -- mirrors mutation enforcement above
        if query_info.operation_type.is_read_only() {
            let query_name = query_info.root_fields.first().cloned().unwrap_or_default();

            if !self.config.blocked_queries.is_empty()
                && self.config.blocked_queries.contains(&query_name)
            {
                return Some(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "blocked_query",
                        &format!("Query '{}' is blocked for this server", query_name),
                    )
                    .with_suggestion("This query is in the blocklist and cannot be executed")],
                    self.build_metadata(query_info, start.elapsed().as_millis() as u64),
                ));
            }

            if !self.config.allowed_queries.is_empty()
                && !self.config.allowed_queries.contains(&query_name)
            {
                return Some(ValidationResult::failure(
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
                    self.build_metadata(query_info, start.elapsed().as_millis() as u64),
                ));
            }
        }

        None
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

        // Config-based authorization checks (mutation blocklist/allowlist, query blocklist/allowlist)
        if let Some(failure) = self.check_config_authorization(&query_info, start) {
            return Ok(failure);
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
                let op_type_str = format!("{:?}", query_info.operation_type);
                let action =
                    UnifiedAction::from_graphql(&op_type_str, query_info.operation_name.as_deref());
                let violations = build_policy_violations(
                    &decision,
                    self.config.server_id(),
                    action,
                    "operation",
                );

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
            // Reuse already-parsed query_info instead of re-parsing via validate_graphql_query
            if let Some(failure) = self.check_config_authorization(&query_info, start) {
                return Ok(failure);
            }
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

    /// Validate JavaScript code for OpenAPI Code Mode (sync, no policy evaluation).
    ///
    /// Runs config-level checks only. For policy evaluation (Cedar/AVP), use
    /// [`validate_javascript_code_async`] instead. Retained for backward
    /// compatibility with callers that don't need policy enforcement.
    #[cfg(feature = "openapi-code-mode")]
    pub fn validate_javascript_code(
        &self,
        code: &str,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        let start = Instant::now();
        let code_info = self.validate_js_preamble(code)?;
        if let Some(failure) = self.check_js_config_authorization(&code_info, start) {
            return Ok(failure);
        }
        self.complete_js_validation(code, &code_info, context, start)
    }

    /// Validate JavaScript code with async policy evaluation.
    ///
    /// Mirrors [`validate_graphql_query_async`] but for JavaScript/OpenAPI:
    /// 1. Parse JS via SWC + config-level checks (shared with sync version)
    /// 2. Policy evaluation via [`PolicyEvaluator::evaluate_script`] (async, fail-closed)
    /// 3. Security analysis + token generation
    ///
    /// When no policy evaluator is configured, falls back to config-only checks.
    #[cfg(feature = "openapi-code-mode")]
    pub async fn validate_javascript_code_async(
        &self,
        code: &str,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        use crate::policy::types::ScriptEntity;

        let start = Instant::now();
        let code_info = self.validate_js_preamble(code)?;
        if let Some(failure) = self.check_js_config_authorization(&code_info, start) {
            return Ok(failure);
        }

        // Policy evaluation via evaluate_script (mirrors GraphQL's evaluate_operation)
        if let Some(ref evaluator) = self.policy_evaluator {
            let sensitive_patterns: Vec<String> =
                self.config.openapi_blocked_paths.iter().cloned().collect();
            let registry_ref = if self.operation_registry.is_empty() {
                None
            } else {
                Some(&self.operation_registry)
            };
            let script_entity =
                ScriptEntity::from_javascript_info(&code_info, &sensitive_patterns, registry_ref);
            let server_entity = self.config.to_openapi_server_entity();

            let decision = evaluator
                .evaluate_script(&script_entity, &server_entity)
                .await
                .map_err(|e| {
                    ValidationError::InternalError(format!("Policy evaluation error: {}", e))
                })?;

            if !decision.allowed {
                let violations = build_policy_violations(
                    &decision,
                    self.config.server_id(),
                    script_entity.action(),
                    "script",
                );

                return Ok(ValidationResult::failure(
                    violations,
                    self.build_js_metadata(&code_info, start.elapsed().as_millis() as u64),
                ));
            }
        }

        self.complete_js_validation(code, &code_info, context, start)
    }

    /// Shared JavaScript preamble: enabled check, length check, parse.
    #[cfg(feature = "openapi-code-mode")]
    fn validate_js_preamble(&self, code: &str) -> Result<JavaScriptCodeInfo, ValidationError> {
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

        self.javascript_validator.validate(code)
    }

    /// Config-level authorization checks for JavaScript code.
    ///
    /// Returns `Some(failure)` if a config check denied the code,
    /// `None` if all checks pass. Shared between sync and async paths
    /// (mirrors `check_config_authorization` for GraphQL).
    #[cfg(feature = "openapi-code-mode")]
    fn check_js_config_authorization(
        &self,
        code_info: &JavaScriptCodeInfo,
        start: Instant,
    ) -> Option<ValidationResult> {
        if code_info.is_read_only {
            return None;
        }

        for method in &code_info.methods_used {
            if !self.config.openapi_blocked_writes.is_empty()
                && self.config.openapi_blocked_writes.contains(method)
            {
                return Some(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "blocked_method",
                        &format!("HTTP method '{}' is blocked for this server", method),
                    )
                    .with_suggestion("This method is in the blocklist and cannot be used")],
                    self.build_js_metadata(code_info, start.elapsed().as_millis() as u64),
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
            return Some(ValidationResult::failure(
                vec![PolicyViolation::new(
                    "code_mode",
                    "allow_mutations",
                    "Write HTTP methods (POST, PUT, DELETE, PATCH) are not enabled for this server",
                )
                .with_suggestion("Only read-only methods (GET, HEAD, OPTIONS) are allowed. Contact your administrator to enable write operations.")],
                self.build_js_metadata(code_info, start.elapsed().as_millis() as u64),
            ));
        }

        None
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

    /// Validate a SQL statement using basic config checks only (no policy evaluator).
    ///
    /// For policy evaluation (Cedar/AVP), use [`validate_sql_query_async`] instead.
    #[cfg(feature = "sql-code-mode")]
    pub fn validate_sql_query(
        &self,
        sql: &str,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        let start = Instant::now();
        let info = self.validate_sql_preamble(sql)?;
        if let Some(failure) = self.check_sql_config_authorization(&info, start) {
            return Ok(failure);
        }
        self.complete_sql_validation(sql, &info, context, start)
    }

    /// Validate a SQL statement with async policy evaluation.
    ///
    /// Mirrors [`validate_graphql_query_async`] and [`validate_javascript_code_async`]:
    /// 1. Parse SQL via sqlparser + config-level checks (shared with sync version)
    /// 2. Policy evaluation via [`PolicyEvaluator::evaluate_statement`] (async, fail-closed)
    /// 3. Security analysis + token generation
    ///
    /// When no policy evaluator is configured, falls back to config-only checks.
    #[cfg(feature = "sql-code-mode")]
    pub async fn validate_sql_query_async(
        &self,
        sql: &str,
        context: &ValidationContext,
    ) -> Result<ValidationResult, ValidationError> {
        use crate::policy::StatementEntity;

        let start = Instant::now();
        let info = self.validate_sql_preamble(sql)?;
        if let Some(failure) = self.check_sql_config_authorization(&info, start) {
            return Ok(failure);
        }

        if let Some(ref evaluator) = self.policy_evaluator {
            let statement_entity = StatementEntity::from_sql_info(&info);
            let server_entity = self.config.to_sql_server_entity();

            let decision = evaluator
                .evaluate_statement(&statement_entity, &server_entity)
                .await
                .map_err(|e| {
                    ValidationError::InternalError(format!("Policy evaluation error: {}", e))
                })?;

            if !decision.allowed {
                let violations = build_policy_violations(
                    &decision,
                    self.config.server_id(),
                    statement_entity.action(),
                    "SQL statement",
                );

                return Ok(ValidationResult::failure(
                    violations,
                    self.build_sql_metadata(&info, start.elapsed().as_millis() as u64),
                ));
            }
        } else {
            warn_no_policy_configured();
        }

        self.complete_sql_validation(sql, &info, context, start)
    }

    /// Shared SQL preamble: enabled check, length check, parse.
    #[cfg(feature = "sql-code-mode")]
    fn validate_sql_preamble(
        &self,
        sql: &str,
    ) -> Result<crate::sql::SqlStatementInfo, ValidationError> {
        if !self.config.enabled {
            return Err(ValidationError::ConfigError(
                "Code Mode is not enabled for this server".into(),
            ));
        }

        if sql.len() > self.config.max_query_length {
            return Err(ValidationError::SecurityError {
                message: format!(
                    "SQL length {} exceeds maximum {}",
                    sql.len(),
                    self.config.max_query_length
                ),
                issue: crate::types::SecurityIssueType::HighComplexity,
            });
        }

        let validator = crate::sql::SqlValidator::new();
        validator.validate(sql)
    }

    /// Config-level authorization checks for SQL.
    ///
    /// Returns `Some(failure)` if a config check denied the statement,
    /// `None` if all checks pass.
    #[cfg(feature = "sql-code-mode")]
    fn check_sql_config_authorization(
        &self,
        info: &crate::sql::SqlStatementInfo,
        start: Instant,
    ) -> Option<ValidationResult> {
        use crate::sql::SqlStatementType;

        let stype = info.statement_type.as_str();

        // Statement-type blocklist
        if self.config.sql_blocked_statements.contains(stype) {
            return Some(ValidationResult::failure(
                vec![PolicyViolation::new(
                    "code_mode",
                    "blocked_statement",
                    format!("Statement type '{}' is blocked for this server", stype),
                )],
                self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
            ));
        }

        // Statement-type allowlist
        if !self.config.sql_allowed_statements.is_empty()
            && !self.config.sql_allowed_statements.contains(stype)
        {
            return Some(ValidationResult::failure(
                vec![PolicyViolation::new(
                    "code_mode",
                    "statement_not_allowed",
                    format!("Statement type '{}' is not in the allowlist", stype),
                )],
                self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
            ));
        }

        // Global action flags
        match info.statement_type {
            SqlStatementType::Select => {
                if !self.config.sql_reads_enabled {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "reads_disabled",
                            "SELECT statements are not enabled for this server",
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
            },
            SqlStatementType::Insert | SqlStatementType::Update => {
                if !self.config.sql_allow_writes {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "writes_disabled",
                            "INSERT/UPDATE statements are not enabled for this server",
                        )
                        .with_suggestion("Contact your administrator to enable sql_allow_writes.")],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
                // WHERE requirement applies to UPDATE only — INSERTs never have WHERE.
                if matches!(info.statement_type, SqlStatementType::Update)
                    && self.config.sql_require_where_on_writes
                    && !info.has_where
                {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "missing_where",
                            format!("{} without WHERE clause is not allowed", info.verb),
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
            },
            SqlStatementType::Delete => {
                if !self.config.sql_allow_deletes {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "deletes_disabled",
                            "DELETE statements are not enabled for this server",
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
                if self.config.sql_require_where_on_writes && !info.has_where {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "missing_where",
                            "DELETE without WHERE clause is not allowed",
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
            },
            SqlStatementType::Ddl => {
                if !self.config.sql_allow_ddl {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "ddl_disabled",
                            "DDL (CREATE/ALTER/DROP/GRANT/REVOKE) is not enabled for this server",
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
            },
            SqlStatementType::Other => {
                return Some(ValidationResult::failure(
                    vec![PolicyViolation::new(
                        "code_mode",
                        "unsupported_statement",
                        format!("Statement type '{}' is not supported", info.verb),
                    )],
                    self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                ));
            },
        }

        // Table-level blocklist
        if !self.config.sql_blocked_tables.is_empty() {
            for table in &info.tables {
                if self.config.sql_blocked_tables.contains(table) {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "blocked_table",
                            format!("Table '{}' is blocked for this server", table),
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
            }
        }

        // Table-level allowlist
        if !self.config.sql_allowed_tables.is_empty() {
            for table in &info.tables {
                if !self.config.sql_allowed_tables.contains(table) {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "table_not_allowed",
                            format!("Table '{}' is not in the allowlist", table),
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
            }
        }

        // Column-level blocklist
        if !self.config.sql_blocked_columns.is_empty() {
            for col in &info.columns {
                if self.config.sql_blocked_columns.contains(col) {
                    return Some(ValidationResult::failure(
                        vec![PolicyViolation::new(
                            "code_mode",
                            "blocked_column",
                            format!("Column '{}' is blocked for this server", col),
                        )],
                        self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
                    ));
                }
            }
        }

        // Structural limits
        if info.join_count > self.config.sql_max_joins {
            return Some(ValidationResult::failure(
                vec![PolicyViolation::new(
                    "code_mode",
                    "excessive_joins",
                    format!(
                        "Query has {} JOINs, exceeds limit of {}",
                        info.join_count, self.config.sql_max_joins
                    ),
                )],
                self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
            ));
        }

        if info.estimated_rows > self.config.sql_max_rows {
            return Some(ValidationResult::failure(
                vec![PolicyViolation::new(
                    "code_mode",
                    "excessive_rows",
                    format!(
                        "Estimated rows {} exceeds limit of {}",
                        info.estimated_rows, self.config.sql_max_rows
                    ),
                )],
                self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
            ));
        }

        None
    }

    /// Complete SQL validation after config/policy checks pass.
    #[cfg(feature = "sql-code-mode")]
    fn complete_sql_validation(
        &self,
        sql: &str,
        info: &crate::sql::SqlStatementInfo,
        context: &ValidationContext,
        start: Instant,
    ) -> Result<ValidationResult, ValidationError> {
        let validator = crate::sql::SqlValidator::new();
        let security_analysis = validator.analyze_security(info);
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
                self.build_sql_metadata(info, start.elapsed().as_millis() as u64),
            ));
        }

        let context_hash = context.context_hash();
        let token = self.token_generator.generate(
            sql,
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

        let explanation = self.generate_sql_explanation(info, &security_analysis);
        let metadata = self.build_sql_metadata(info, start.elapsed().as_millis() as u64);

        let mut result = ValidationResult::success(explanation, risk_level, token_string, metadata);

        for issue in &security_analysis.potential_issues {
            if !issue.is_critical() {
                result.warnings.push(issue.message.clone());
            }
        }

        Ok(result)
    }

    /// Build metadata from SQL statement info.
    #[cfg(feature = "sql-code-mode")]
    fn build_sql_metadata(
        &self,
        info: &crate::sql::SqlStatementInfo,
        validation_time_ms: u64,
    ) -> ValidationMetadata {
        let inferred = UnifiedAction::from_sql(info.statement_type.as_str());
        let action = UnifiedAction::resolve(inferred, &self.config.action_tags, &info.verb);

        ValidationMetadata {
            is_read_only: info.statement_type.is_read_only(),
            estimated_rows: Some(info.estimated_rows),
            accessed_types: info.tables.iter().cloned().collect(),
            accessed_fields: info.columns.iter().cloned().collect(),
            has_aggregation: info.has_aggregation,
            code_type: Some(if info.statement_type.is_read_only() {
                crate::types::CodeType::SqlQuery
            } else {
                crate::types::CodeType::SqlMutation
            }),
            action: Some(action),
            validation_time_ms,
        }
    }

    /// Generate a human-readable explanation for a SQL statement.
    #[cfg(feature = "sql-code-mode")]
    fn generate_sql_explanation(
        &self,
        info: &crate::sql::SqlStatementInfo,
        security_analysis: &crate::types::SecurityAnalysis,
    ) -> String {
        let mut parts = Vec::new();

        let verb_phrase = match info.statement_type.as_str() {
            "SELECT" => "This query reads data",
            "INSERT" => "This statement inserts rows",
            "UPDATE" => "This statement updates rows",
            "DELETE" => "This statement deletes rows",
            "DDL" => "This statement changes schema or permissions",
            _ => "This statement",
        };

        let tables_phrase = if info.tables.is_empty() {
            String::new()
        } else {
            let mut ts: Vec<&String> = info.tables.iter().collect();
            ts.sort();
            format!(
                " in table(s): {}",
                ts.into_iter().cloned().collect::<Vec<_>>().join(", ")
            )
        };

        parts.push(format!("{}{}.", verb_phrase, tables_phrase));

        if info.has_where {
            parts.push("Filtered with WHERE clause.".to_string());
        }
        if info.has_limit {
            parts.push(format!("Limited to {} rows.", info.estimated_rows));
        }
        if info.join_count > 0 {
            parts.push(format!("Uses {} JOIN(s).", info.join_count));
        }
        if info.subquery_count > 0 {
            parts.push(format!("Contains {} subquer(ies).", info.subquery_count));
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
        ValidationPipeline::new(CodeModeConfig::enabled(), b"test-secret-key!".to_vec()).unwrap()
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

        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
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
        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
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
        config.blocked_queries.insert("users".to_string());

        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
        let ctx = test_context();

        let result = pipeline
            .validate_graphql_query("query { users { id } }", &ctx)
            .unwrap();

        assert!(!result.is_valid);
        assert!(result.violations.iter().any(|v| v.rule == "blocked_query"));
    }

    #[test]
    fn test_allowed_queries_enforced() {
        let mut config = CodeModeConfig::enabled();
        config.allowed_queries.insert("orders".to_string());

        let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
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

    // ============================================================================
    // SQL Code Mode tests
    // ============================================================================

    #[cfg(feature = "sql-code-mode")]
    mod sql_tests {
        use super::*;

        fn sql_pipeline() -> ValidationPipeline {
            ValidationPipeline::new(CodeModeConfig::enabled(), b"test-secret-key!".to_vec())
                .unwrap()
        }

        #[test]
        fn validates_select() {
            let pipeline = sql_pipeline();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query("SELECT id, name FROM users LIMIT 10", &ctx)
                .unwrap();

            assert!(result.is_valid);
            assert!(result.approval_token.is_some());
        }

        #[test]
        fn rejects_insert_when_writes_disabled() {
            let pipeline = sql_pipeline();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query("INSERT INTO users (id, name) VALUES (1, 'Alice')", &ctx)
                .unwrap();

            assert!(!result.is_valid);
            assert!(result
                .violations
                .iter()
                .any(|v| v.rule == "writes_disabled"));
        }

        #[test]
        fn permits_insert_when_writes_enabled() {
            let mut config = CodeModeConfig::enabled();
            config.sql_allow_writes = true;
            let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query("INSERT INTO users (id, name) VALUES (1, 'Alice')", &ctx)
                .unwrap();

            assert!(result.is_valid);
        }

        #[test]
        fn rejects_update_without_where_by_default() {
            let mut config = CodeModeConfig::enabled();
            config.sql_allow_writes = true;
            let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query("UPDATE users SET active = 0", &ctx)
                .unwrap();

            assert!(!result.is_valid);
            assert!(result.violations.iter().any(|v| v.rule == "missing_where"));
        }

        #[test]
        fn rejects_blocked_table() {
            let mut config = CodeModeConfig::enabled();
            config.sql_blocked_tables.insert("secrets".to_string());
            let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query("SELECT * FROM secrets LIMIT 10", &ctx)
                .unwrap();

            assert!(!result.is_valid);
            assert!(result.violations.iter().any(|v| v.rule == "blocked_table"));
        }

        #[test]
        fn rejects_non_allowlisted_table() {
            let mut config = CodeModeConfig::enabled();
            config.sql_allowed_tables.insert("users".to_string());
            let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
            let ctx = test_context();

            // "orders" is not in the allowlist
            let result = pipeline
                .validate_sql_query("SELECT id FROM orders LIMIT 10", &ctx)
                .unwrap();

            assert!(!result.is_valid);
            assert!(result
                .violations
                .iter()
                .any(|v| v.rule == "table_not_allowed"));
        }

        #[test]
        fn rejects_blocked_column() {
            let mut config = CodeModeConfig::enabled();
            config.sql_blocked_columns.insert("password".to_string());
            let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query("SELECT id, password FROM users LIMIT 10", &ctx)
                .unwrap();

            assert!(!result.is_valid);
            assert!(result.violations.iter().any(|v| v.rule == "blocked_column"));
        }

        #[test]
        fn rejects_ddl_by_default() {
            let pipeline = sql_pipeline();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query("CREATE TABLE foo (id INT)", &ctx)
                .unwrap();

            assert!(!result.is_valid);
            assert!(result.violations.iter().any(|v| v.rule == "ddl_disabled"));
        }

        #[test]
        fn rejects_syntax_error() {
            let pipeline = sql_pipeline();
            let ctx = test_context();

            let result = pipeline.validate_sql_query("SELEC id FRM users", &ctx);

            assert!(matches!(result, Err(ValidationError::ParseError { .. })));
        }

        struct FixedDenyEvaluator {
            errors: Vec<String>,
        }

        #[async_trait::async_trait]
        impl PolicyEvaluator for FixedDenyEvaluator {
            async fn evaluate_operation(
                &self,
                _op: &crate::policy::OperationEntity,
                _cfg: &crate::policy::ServerConfigEntity,
            ) -> Result<crate::policy::AuthorizationDecision, crate::policy::PolicyEvaluationError>
            {
                Ok(crate::policy::AuthorizationDecision {
                    allowed: false,
                    determining_policies: vec![],
                    errors: self.errors.clone(),
                })
            }

            #[cfg(feature = "sql-code-mode")]
            async fn evaluate_statement(
                &self,
                _stmt: &crate::policy::StatementEntity,
                _server: &crate::policy::SqlServerEntity,
            ) -> Result<crate::policy::AuthorizationDecision, crate::policy::PolicyEvaluationError>
            {
                Ok(crate::policy::AuthorizationDecision {
                    allowed: false,
                    determining_policies: vec![],
                    errors: self.errors.clone(),
                })
            }

            fn name(&self) -> &str {
                "fixed-deny-test"
            }
        }

        fn sql_pipeline_with_evaluator(evaluator: Arc<dyn PolicyEvaluator>) -> ValidationPipeline {
            let mut config = CodeModeConfig::enabled();
            config.server_id = Some("test-server".to_string());
            ValidationPipeline::with_policy_evaluator(
                config,
                b"test-secret-key!".to_vec(),
                evaluator,
            )
            .unwrap()
        }

        #[tokio::test]
        async fn default_deny_produces_synthetic_violation() {
            let evaluator =
                Arc::new(FixedDenyEvaluator { errors: vec![] }) as Arc<dyn PolicyEvaluator>;
            let pipeline = sql_pipeline_with_evaluator(evaluator);
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query_async("SELECT id FROM users LIMIT 10", &ctx)
                .await
                .unwrap();

            assert!(!result.is_valid);
            let default_deny = result
                .violations
                .iter()
                .find(|v| v.rule == "default_deny")
                .expect("expected a synthetic default_deny violation");
            assert!(default_deny.message.contains("test-server"));
            assert!(default_deny.message.contains("Read"));
        }

        #[tokio::test]
        async fn policy_errors_flow_to_violations() {
            let evaluator = Arc::new(FixedDenyEvaluator {
                errors: vec!["schema validation: missing required attribute X".to_string()],
            }) as Arc<dyn PolicyEvaluator>;
            let pipeline = sql_pipeline_with_evaluator(evaluator);
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query_async("SELECT id FROM users LIMIT 10", &ctx)
                .await
                .unwrap();

            assert!(!result.is_valid);
            let policy_error = result
                .violations
                .iter()
                .find(|v| v.rule == "evaluation_error")
                .expect("expected a policy_error violation");
            assert!(policy_error.message.contains("schema validation"));
        }

        #[test]
        fn rejects_excessive_joins() {
            let mut config = CodeModeConfig::enabled();
            config.sql_max_joins = 1;
            let pipeline = ValidationPipeline::new(config, b"test-secret-key!".to_vec()).unwrap();
            let ctx = test_context();

            let result = pipeline
                .validate_sql_query(
                    "SELECT u.id FROM users u \
                     JOIN orders o ON u.id = o.user_id \
                     JOIN items i ON o.id = i.order_id LIMIT 10",
                    &ctx,
                )
                .unwrap();

            assert!(!result.is_valid);
            assert!(result
                .violations
                .iter()
                .any(|v| v.rule == "excessive_joins"));
        }
    }
}
