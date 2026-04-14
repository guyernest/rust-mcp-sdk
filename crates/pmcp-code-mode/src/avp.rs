//! Amazon Verified Permissions (AVP) policy evaluator for Code Mode.
//!
//! Provides [`AvpPolicyEvaluator`] — an implementation of [`PolicyEvaluator`] backed by
//! AWS Verified Permissions. Supports both GraphQL (`evaluate_operation`) and JavaScript
//! (`evaluate_script`) policy evaluation.
//!
//! # Usage
//!
//! ```rust,ignore
//! use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator};
//! use std::sync::Arc;
//!
//! // Construct from POLICY_STORE_ID env var (injected by pmcp.run platform)
//! let config = AvpConfig {
//!     policy_store_id: std::env::var("POLICY_STORE_ID").unwrap(),
//!     region: None, // uses default AWS region
//! };
//! let client = AvpClient::new(config).await?;
//! let evaluator = Arc::new(AvpPolicyEvaluator::new(client));
//! ```
//!
//! # Feature Gate
//!
//! This module requires the `avp` feature:
//! ```toml
//! pmcp-code-mode = { version = "0.4.0", features = ["avp"] }
//! ```

use aws_config::BehaviorVersion;
use aws_sdk_verifiedpermissions::{
    types::{ActionIdentifier, AttributeValue, EntitiesDefinition, EntityIdentifier, EntityItem},
    Client,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::policy::{
    AuthorizationDecision, OperationEntity, PolicyEvaluationError, PolicyEvaluator,
    ServerConfigEntity,
};

#[cfg(feature = "openapi-code-mode")]
use crate::policy::{normalize_operation_format, OpenAPIServerEntity, ScriptEntity};

/// Configuration for the AVP client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvpConfig {
    /// The AVP Policy Store ID for this server.
    pub policy_store_id: String,

    /// AWS region (optional, uses default if not set).
    #[serde(default)]
    pub region: Option<String>,
}

impl Default for AvpConfig {
    fn default() -> Self {
        Self {
            policy_store_id: String::new(),
            region: None,
        }
    }
}

/// Error type for AVP operations.
#[derive(Debug, thiserror::Error)]
pub enum AvpError {
    #[error("AVP configuration error: {0}")]
    ConfigError(String),

    #[error("AVP SDK error: {0}")]
    SdkError(String),

    #[error("Authorization denied: {0}")]
    Denied(String),
}

/// AVP client for Code Mode policy evaluation.
///
/// Wraps the AWS SDK `verifiedpermissions::Client` and provides typed methods
/// for authorizing GraphQL operations and JavaScript scripts against Cedar
/// policies managed in AWS Verified Permissions.
#[derive(Clone)]
pub struct AvpClient {
    client: Client,
    policy_store_id: String,
}

impl AvpClient {
    /// Create a new AVP client.
    ///
    /// # Errors
    ///
    /// Returns [`AvpError::ConfigError`] if the policy store ID is empty.
    pub async fn new(config: AvpConfig) -> Result<Self, AvpError> {
        if config.policy_store_id.is_empty() {
            return Err(AvpError::ConfigError(
                "Policy store ID is required".to_string(),
            ));
        }

        let aws_config = if let Some(region) = &config.region {
            aws_config::defaults(BehaviorVersion::latest())
                .region(aws_config::Region::new(region.clone()))
                .load()
                .await
        } else {
            aws_config::load_defaults(BehaviorVersion::latest()).await
        };

        let client = Client::new(&aws_config);

        Ok(Self {
            client,
            policy_store_id: config.policy_store_id,
        })
    }

    /// Check if a GraphQL operation is authorized.
    pub async fn is_authorized(
        &self,
        operation: &OperationEntity,
        server_config: &ServerConfigEntity,
    ) -> Result<AuthorizationDecision, AvpError> {
        let entities = self.build_entities(operation, server_config);

        let action_id = if operation.has_introspection {
            "Admin"
        } else {
            match operation.operation_type.as_str() {
                "mutation" => {
                    let op_name = operation.operation_name.to_lowercase();
                    if op_name.starts_with("delete")
                        || op_name.starts_with("remove")
                        || op_name.starts_with("purge")
                    {
                        "Delete"
                    } else {
                        "Write"
                    }
                },
                "subscription" => "Write",
                _ => "Read",
            }
        };

        let response = self
            .client
            .is_authorized()
            .policy_store_id(&self.policy_store_id)
            .principal(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Operation")
                    .entity_id(&operation.id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .action(
                ActionIdentifier::builder()
                    .action_type("CodeMode::Action")
                    .action_id(action_id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .resource(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Server")
                    .entity_id(&server_config.server_id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .entities(entities)
            .send()
            .await
            .map_err(|e| AvpError::SdkError(e.to_string()))?;

        Ok(self.parse_response(&response))
    }

    /// Generic authorization check using raw entity types and attributes.
    ///
    /// Use this when your server type has a unique Cedar schema that doesn't
    /// match the typed entity structs (`OperationEntity`, `ScriptEntity`, etc.).
    pub async fn is_authorized_raw(
        &self,
        principal_type: &str,
        principal_id: &str,
        action_type: &str,
        action_id: &str,
        resource_type: &str,
        resource_id: &str,
        entities: Vec<EntityItem>,
    ) -> Result<AuthorizationDecision, AvpError> {
        let response = self
            .client
            .is_authorized()
            .policy_store_id(&self.policy_store_id)
            .principal(
                EntityIdentifier::builder()
                    .entity_type(principal_type)
                    .entity_id(principal_id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .action(
                ActionIdentifier::builder()
                    .action_type(action_type)
                    .action_id(action_id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .resource(
                EntityIdentifier::builder()
                    .entity_type(resource_type)
                    .entity_id(resource_id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .entities(EntitiesDefinition::EntityList(entities))
            .send()
            .await
            .map_err(|e| AvpError::SdkError(e.to_string()))?;

        Ok(self.parse_response(&response))
    }

    /// Batch authorization for multiple operations (chunks of 30 per API limit).
    pub async fn batch_is_authorized(
        &self,
        requests: Vec<(OperationEntity, ServerConfigEntity)>,
    ) -> Result<Vec<AuthorizationDecision>, AvpError> {
        let mut results = Vec::new();

        for chunk in requests.chunks(30) {
            let batch_items: Vec<_> = chunk
                .iter()
                .map(|(op, config)| {
                    let action_id = Self::determine_action_id(op);

                    aws_sdk_verifiedpermissions::types::BatchIsAuthorizedInputItem::builder()
                        .principal(
                            EntityIdentifier::builder()
                                .entity_type("CodeMode::Operation")
                                .entity_id(&op.id)
                                .build()
                                .expect("valid entity identifier"),
                        )
                        .action(
                            ActionIdentifier::builder()
                                .action_type("CodeMode::Action")
                                .action_id(action_id)
                                .build()
                                .expect("valid action identifier"),
                        )
                        .resource(
                            EntityIdentifier::builder()
                                .entity_type("CodeMode::Server")
                                .entity_id(&config.server_id)
                                .build()
                                .expect("valid entity identifier"),
                        )
                        .build()
                })
                .collect();

            let mut all_entities = Vec::new();
            for (op, config) in chunk {
                all_entities.push(self.build_operation_entity(op));
                all_entities.push(self.build_server_config_entity(config));
            }

            let response = self
                .client
                .batch_is_authorized()
                .policy_store_id(&self.policy_store_id)
                .set_requests(Some(batch_items))
                .entities(EntitiesDefinition::EntityList(all_entities))
                .send()
                .await
                .map_err(|e| AvpError::SdkError(e.to_string()))?;

            for result in response.results() {
                let allowed =
                    result.decision() == &aws_sdk_verifiedpermissions::types::Decision::Allow;
                results.push(AuthorizationDecision {
                    allowed,
                    determining_policies: result
                        .determining_policies()
                        .iter()
                        .map(|p| p.policy_id().to_string())
                        .collect(),
                    errors: result
                        .errors()
                        .iter()
                        .map(|e| e.error_description().to_string())
                        .collect(),
                });
            }
        }

        Ok(results)
    }

    fn determine_action_id(op: &OperationEntity) -> &'static str {
        if op.has_introspection {
            "Admin"
        } else {
            match op.operation_type.as_str() {
                "mutation" => {
                    let op_name = op.operation_name.to_lowercase();
                    if op_name.starts_with("delete")
                        || op_name.starts_with("remove")
                        || op_name.starts_with("purge")
                    {
                        "Delete"
                    } else {
                        "Write"
                    }
                },
                "subscription" => "Write",
                _ => "Read",
            }
        }
    }

    fn parse_response(
        &self,
        response: &aws_sdk_verifiedpermissions::operation::is_authorized::IsAuthorizedOutput,
    ) -> AuthorizationDecision {
        let allowed = response.decision() == &aws_sdk_verifiedpermissions::types::Decision::Allow;
        AuthorizationDecision {
            allowed,
            determining_policies: response
                .determining_policies()
                .iter()
                .map(|p| p.policy_id().to_string())
                .collect(),
            errors: response
                .errors()
                .iter()
                .map(|e| e.error_description().to_string())
                .collect(),
        }
    }

    fn build_entities(
        &self,
        operation: &OperationEntity,
        server_config: &ServerConfigEntity,
    ) -> EntitiesDefinition {
        EntitiesDefinition::EntityList(vec![
            self.build_operation_entity(operation),
            self.build_server_config_entity(server_config),
        ])
    }

    fn build_operation_entity(&self, operation: &OperationEntity) -> EntityItem {
        let mut attrs: HashMap<String, AttributeValue> = HashMap::new();
        attrs.insert(
            "operationType".into(),
            AttributeValue::String(operation.operation_type.clone()),
        );
        attrs.insert(
            "operationName".into(),
            AttributeValue::String(operation.operation_name.clone()),
        );
        attrs.insert("depth".into(), AttributeValue::Long(operation.depth as i64));
        attrs.insert(
            "fieldCount".into(),
            AttributeValue::Long(operation.field_count as i64),
        );
        attrs.insert(
            "estimatedCost".into(),
            AttributeValue::Long(operation.estimated_cost as i64),
        );
        attrs.insert(
            "hasIntrospection".into(),
            AttributeValue::Boolean(operation.has_introspection),
        );
        attrs.insert(
            "accessesSensitiveData".into(),
            AttributeValue::Boolean(operation.accesses_sensitive_data),
        );
        attrs.insert(
            "rootFields".into(),
            Self::string_set(&operation.root_fields),
        );
        attrs.insert(
            "accessedTypes".into(),
            Self::string_set(&operation.accessed_types),
        );
        attrs.insert(
            "accessedFields".into(),
            Self::string_set(&operation.accessed_fields),
        );
        attrs.insert(
            "sensitiveCategories".into(),
            Self::string_set(&operation.sensitive_categories),
        );

        EntityItem::builder()
            .identifier(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Operation")
                    .entity_id(&operation.id)
                    .build()
                    .expect("valid entity identifier"),
            )
            .set_attributes(Some(attrs))
            .build()
    }

    fn build_server_config_entity(&self, config: &ServerConfigEntity) -> EntityItem {
        let mut attrs: HashMap<String, AttributeValue> = HashMap::new();
        attrs.insert(
            "serverId".into(),
            AttributeValue::String(config.server_id.clone()),
        );
        attrs.insert(
            "serverType".into(),
            AttributeValue::String(config.server_type.clone()),
        );
        attrs.insert(
            "allowWrite".into(),
            AttributeValue::Boolean(config.allow_write),
        );
        attrs.insert(
            "allowDelete".into(),
            AttributeValue::Boolean(config.allow_delete),
        );
        attrs.insert(
            "allowAdmin".into(),
            AttributeValue::Boolean(config.allow_admin),
        );
        attrs.insert(
            "maxDepth".into(),
            AttributeValue::Long(config.max_depth as i64),
        );
        attrs.insert(
            "maxFieldCount".into(),
            AttributeValue::Long(config.max_field_count as i64),
        );
        attrs.insert(
            "maxCost".into(),
            AttributeValue::Long(config.max_cost as i64),
        );
        attrs.insert(
            "maxApiCalls".into(),
            AttributeValue::Long(config.max_api_calls as i64),
        );
        attrs.insert(
            "allowedOperations".into(),
            Self::string_set(&config.allowed_operations),
        );
        attrs.insert(
            "blockedOperations".into(),
            Self::string_set(&config.blocked_operations),
        );
        attrs.insert(
            "blockedFields".into(),
            Self::string_set(&config.blocked_fields),
        );

        EntityItem::builder()
            .identifier(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Server")
                    .entity_id(&config.server_id)
                    .build()
                    .expect("valid entity identifier"),
            )
            .set_attributes(Some(attrs))
            .build()
    }

    fn string_set(set: &HashSet<String>) -> AttributeValue {
        AttributeValue::Set(
            set.iter()
                .map(|s| AttributeValue::String(s.clone()))
                .collect(),
        )
    }
}

// ============================================================================
// OpenAPI Code Mode Support (Script-based validation)
// ============================================================================

#[cfg(feature = "openapi-code-mode")]
impl AvpClient {
    /// Check if a JavaScript script is authorized (OpenAPI Code Mode).
    pub async fn is_script_authorized(
        &self,
        script: &ScriptEntity,
        server: &OpenAPIServerEntity,
    ) -> Result<AuthorizationDecision, AvpError> {
        let entities = EntitiesDefinition::EntityList(vec![
            self.build_script_entity(script),
            self.build_openapi_server_entity(server),
        ]);

        let response = self
            .client
            .is_authorized()
            .policy_store_id(&self.policy_store_id)
            .principal(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Script")
                    .entity_id(&script.id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .action(
                ActionIdentifier::builder()
                    .action_type("CodeMode::Action")
                    .action_id(script.action())
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .resource(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Server")
                    .entity_id(&server.server_id)
                    .build()
                    .map_err(|e| AvpError::SdkError(e.to_string()))?,
            )
            .entities(entities)
            .send()
            .await
            .map_err(|e| AvpError::SdkError(e.to_string()))?;

        Ok(self.parse_response(&response))
    }

    fn build_script_entity(&self, script: &ScriptEntity) -> EntityItem {
        let mut attrs: HashMap<String, AttributeValue> = HashMap::new();
        attrs.insert(
            "scriptType".into(),
            AttributeValue::String(script.script_type.clone()),
        );
        attrs.insert(
            "hasWrites".into(),
            AttributeValue::Boolean(script.has_writes),
        );
        attrs.insert(
            "hasDeletes".into(),
            AttributeValue::Boolean(script.has_deletes),
        );
        attrs.insert(
            "accessesSensitivePath".into(),
            AttributeValue::Boolean(script.accesses_sensitive_path),
        );
        attrs.insert(
            "hasUnboundedLoop".into(),
            AttributeValue::Boolean(script.has_unbounded_loop),
        );
        attrs.insert(
            "hasDynamicPath".into(),
            AttributeValue::Boolean(script.has_dynamic_path),
        );
        attrs.insert(
            "totalApiCalls".into(),
            AttributeValue::Long(script.total_api_calls as i64),
        );
        attrs.insert(
            "readCalls".into(),
            AttributeValue::Long(script.read_calls as i64),
        );
        attrs.insert(
            "writeCalls".into(),
            AttributeValue::Long(script.write_calls as i64),
        );
        attrs.insert(
            "deleteCalls".into(),
            AttributeValue::Long(script.delete_calls as i64),
        );
        attrs.insert(
            "loopIterations".into(),
            AttributeValue::Long(script.loop_iterations as i64),
        );
        attrs.insert(
            "nestingDepth".into(),
            AttributeValue::Long(script.nesting_depth as i64),
        );
        attrs.insert(
            "scriptLength".into(),
            AttributeValue::Long(script.script_length as i64),
        );
        attrs.insert(
            "accessedPaths".into(),
            Self::string_set(&script.accessed_paths),
        );
        attrs.insert(
            "accessedMethods".into(),
            Self::string_set(&script.accessed_methods),
        );
        attrs.insert(
            "pathPatterns".into(),
            Self::string_set(&script.path_patterns),
        );
        attrs.insert(
            "calledOperations".into(),
            Self::string_set(&script.called_operations),
        );
        attrs.insert(
            "hasOutputDeclaration".into(),
            AttributeValue::Boolean(script.has_output_declaration),
        );
        attrs.insert(
            "outputFields".into(),
            Self::string_set(&script.output_fields),
        );
        attrs.insert(
            "hasSpreadInOutput".into(),
            AttributeValue::Boolean(script.has_spread_in_output),
        );

        EntityItem::builder()
            .identifier(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Script")
                    .entity_id(&script.id)
                    .build()
                    .expect("valid entity identifier"),
            )
            .set_attributes(Some(attrs))
            .build()
    }

    fn build_openapi_server_entity(&self, server: &OpenAPIServerEntity) -> EntityItem {
        let mut attrs: HashMap<String, AttributeValue> = HashMap::new();
        attrs.insert(
            "serverId".into(),
            AttributeValue::String(server.server_id.clone()),
        );
        attrs.insert(
            "serverType".into(),
            AttributeValue::String(server.server_type.clone()),
        );
        attrs.insert(
            "allowWrite".into(),
            AttributeValue::Boolean(server.allow_write),
        );
        attrs.insert(
            "allowDelete".into(),
            AttributeValue::Boolean(server.allow_delete),
        );
        attrs.insert(
            "allowAdmin".into(),
            AttributeValue::Boolean(server.allow_admin),
        );
        attrs.insert(
            "writeMode".into(),
            AttributeValue::String(server.write_mode.clone()),
        );
        attrs.insert(
            "maxDepth".into(),
            AttributeValue::Long(server.max_depth as i64),
        );
        attrs.insert(
            "maxCost".into(),
            AttributeValue::Long(server.max_cost as i64),
        );
        attrs.insert(
            "maxApiCalls".into(),
            AttributeValue::Long(server.max_api_calls as i64),
        );
        attrs.insert(
            "maxLoopIterations".into(),
            AttributeValue::Long(server.max_loop_iterations as i64),
        );
        attrs.insert(
            "maxScriptLength".into(),
            AttributeValue::Long(server.max_script_length as i64),
        );
        attrs.insert(
            "maxNestingDepth".into(),
            AttributeValue::Long(server.max_nesting_depth as i64),
        );
        attrs.insert(
            "executionTimeoutSeconds".into(),
            AttributeValue::Long(server.execution_timeout_seconds as i64),
        );
        attrs.insert(
            "allowedOperations".into(),
            AttributeValue::Set(
                server
                    .allowed_operations
                    .iter()
                    .map(|s| AttributeValue::String(normalize_operation_format(s)))
                    .collect(),
            ),
        );
        attrs.insert(
            "blockedOperations".into(),
            AttributeValue::Set(
                server
                    .blocked_operations
                    .iter()
                    .map(|s| AttributeValue::String(normalize_operation_format(s)))
                    .collect(),
            ),
        );
        attrs.insert(
            "allowedMethods".into(),
            Self::string_set(&server.allowed_methods),
        );
        attrs.insert(
            "blockedMethods".into(),
            Self::string_set(&server.blocked_methods),
        );
        attrs.insert(
            "allowedPathPatterns".into(),
            Self::string_set(&server.allowed_path_patterns),
        );
        attrs.insert(
            "blockedPathPatterns".into(),
            Self::string_set(&server.blocked_path_patterns),
        );
        attrs.insert(
            "sensitivePathPatterns".into(),
            Self::string_set(&server.sensitive_path_patterns),
        );
        attrs.insert(
            "autoApproveReadOnly".into(),
            AttributeValue::Boolean(server.auto_approve_read_only),
        );
        attrs.insert(
            "maxApiCallsForAutoApprove".into(),
            AttributeValue::Long(server.max_api_calls_for_auto_approve as i64),
        );
        attrs.insert(
            "internalBlockedFields".into(),
            Self::string_set(&server.internal_blocked_fields),
        );
        attrs.insert(
            "outputBlockedFields".into(),
            Self::string_set(&server.output_blocked_fields),
        );
        attrs.insert(
            "requireOutputDeclaration".into(),
            AttributeValue::Boolean(server.require_output_declaration),
        );

        EntityItem::builder()
            .identifier(
                EntityIdentifier::builder()
                    .entity_type("CodeMode::Server")
                    .entity_id(&server.server_id)
                    .build()
                    .expect("valid entity identifier"),
            )
            .set_attributes(Some(attrs))
            .build()
    }
}

/// AVP-based policy evaluator implementing the [`PolicyEvaluator`] trait.
///
/// This wraps [`AvpClient`] and provides the standard `PolicyEvaluator` interface
/// for use with `ValidationPipeline` and `#[derive(CodeMode)]`.
///
/// # Example
///
/// ```rust,ignore
/// use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator, NoopPolicyEvaluator};
/// use std::sync::Arc;
///
/// // Runtime selection based on environment
/// let evaluator: Arc<dyn PolicyEvaluator> = match std::env::var("POLICY_STORE_ID") {
///     Ok(store_id) => Arc::new(AvpPolicyEvaluator::new(
///         AvpClient::new(AvpConfig { policy_store_id: store_id, region: None }).await?
///     )),
///     Err(_) => Arc::new(NoopPolicyEvaluator::new()),
/// };
/// ```
pub struct AvpPolicyEvaluator {
    client: AvpClient,
}

impl AvpPolicyEvaluator {
    /// Create a new AVP policy evaluator.
    pub fn new(client: AvpClient) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl PolicyEvaluator for AvpPolicyEvaluator {
    async fn evaluate_operation(
        &self,
        operation: &OperationEntity,
        server_config: &ServerConfigEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        self.client
            .is_authorized(operation, server_config)
            .await
            .map_err(|e| PolicyEvaluationError::EvaluationError(e.to_string()))
    }

    #[cfg(feature = "openapi-code-mode")]
    async fn evaluate_script(
        &self,
        script: &ScriptEntity,
        server: &OpenAPIServerEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        self.client
            .is_script_authorized(script, server)
            .await
            .map_err(|e| PolicyEvaluationError::EvaluationError(e.to_string()))
    }

    fn name(&self) -> &str {
        "avp"
    }
}
