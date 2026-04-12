//! Local Cedar policy evaluator.
//!
//! Provides in-process Cedar policy evaluation using the `cedar-policy` crate.
//! This enables local agent projects to get real Cedar policy enforcement
//! without an AWS account.

use super::types::{AuthorizationDecision, OperationEntity, ServerConfigEntity};
use super::PolicyEvaluationError;
use cedar_policy::{
    Authorizer, Context, Entities, Entity, EntityId, EntityTypeName, EntityUid, PolicySet, Request,
    Schema,
};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

/// Configuration for the local Cedar policy evaluator.
#[derive(Debug, Clone)]
pub struct CedarPolicyConfig {
    /// Cedar schema in JSON format
    pub schema_json: serde_json::Value,
    /// Cedar policies (id, description, policy_text)
    pub policies: Vec<(String, String, String)>,
}

/// Local Cedar policy evaluator.
///
/// Uses the `cedar-policy` crate for in-process policy evaluation.
pub struct CedarPolicyEvaluator {
    authorizer: Authorizer,
    policy_set: PolicySet,
    schema: Schema,
}

impl CedarPolicyEvaluator {
    /// Create a new evaluator from config.
    pub fn new(config: CedarPolicyConfig) -> Result<Self, PolicyEvaluationError> {
        let schema_json = serde_json::to_string(&config.schema_json).map_err(|e| {
            PolicyEvaluationError::ConfigError(format!("Invalid schema JSON: {}", e))
        })?;

        let schema = Schema::from_json_str(&schema_json).map_err(|e| {
            PolicyEvaluationError::ConfigError(format!("Invalid Cedar schema: {}", e))
        })?;

        let mut policy_set = PolicySet::new();
        for (id, _description, policy_text) in &config.policies {
            let policy = cedar_policy::Policy::parse(
                Some(cedar_policy::PolicyId::from_str(id).unwrap()),
                policy_text,
            )
            .map_err(|e| {
                PolicyEvaluationError::ConfigError(format!("Invalid policy '{}': {}", id, e))
            })?;
            policy_set.add(policy).map_err(|e| {
                PolicyEvaluationError::ConfigError(format!("Duplicate policy '{}': {}", id, e))
            })?;
        }

        Ok(Self {
            authorizer: Authorizer::new(),
            policy_set,
            schema,
        })
    }

    /// Create a default evaluator for GraphQL Code Mode using built-in schemas and policies.
    pub fn graphql_default() -> Result<Self, PolicyEvaluationError> {
        let schema_json = super::types::get_code_mode_schema_json();
        let baseline = super::types::get_baseline_policies();

        let policies = baseline
            .into_iter()
            .map(|(id, desc, text)| (id.to_string(), desc.to_string(), text.to_string()))
            .collect();

        Self::new(CedarPolicyConfig {
            schema_json,
            policies,
        })
    }

    fn build_operation_entity(&self, operation: &OperationEntity) -> Entity {
        let uid = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("CodeMode::Operation").expect("valid type name"),
            EntityId::from_str(&operation.id).expect("valid entity id"),
        );

        let mut attrs: HashMap<String, cedar_policy::RestrictedExpression> = HashMap::new();

        attrs.insert(
            "operationType".to_string(),
            cedar_policy::RestrictedExpression::new_string(operation.operation_type.clone()),
        );
        attrs.insert(
            "operationName".to_string(),
            cedar_policy::RestrictedExpression::new_string(operation.operation_name.clone()),
        );
        attrs.insert(
            "depth".to_string(),
            cedar_policy::RestrictedExpression::new_long(operation.depth as i64),
        );
        attrs.insert(
            "fieldCount".to_string(),
            cedar_policy::RestrictedExpression::new_long(operation.field_count as i64),
        );
        attrs.insert(
            "estimatedCost".to_string(),
            cedar_policy::RestrictedExpression::new_long(operation.estimated_cost as i64),
        );
        attrs.insert(
            "hasIntrospection".to_string(),
            cedar_policy::RestrictedExpression::new_bool(operation.has_introspection),
        );
        attrs.insert(
            "accessesSensitiveData".to_string(),
            cedar_policy::RestrictedExpression::new_bool(operation.accesses_sensitive_data),
        );

        // Set attributes
        attrs.insert(
            "rootFields".to_string(),
            Self::string_set_expr(&operation.root_fields),
        );
        attrs.insert(
            "accessedTypes".to_string(),
            Self::string_set_expr(&operation.accessed_types),
        );
        attrs.insert(
            "accessedFields".to_string(),
            Self::string_set_expr(&operation.accessed_fields),
        );
        attrs.insert(
            "sensitiveCategories".to_string(),
            Self::string_set_expr(&operation.sensitive_categories),
        );

        Entity::new(uid, attrs, HashSet::new()).expect("valid entity")
    }

    fn build_server_entity(&self, config: &ServerConfigEntity) -> Entity {
        let uid = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("CodeMode::Server").expect("valid type name"),
            EntityId::from_str(&config.server_id).expect("valid entity id"),
        );

        let mut attrs: HashMap<String, cedar_policy::RestrictedExpression> = HashMap::new();

        attrs.insert(
            "serverId".to_string(),
            cedar_policy::RestrictedExpression::new_string(config.server_id.clone()),
        );
        attrs.insert(
            "serverType".to_string(),
            cedar_policy::RestrictedExpression::new_string(config.server_type.clone()),
        );
        attrs.insert(
            "allowWrite".to_string(),
            cedar_policy::RestrictedExpression::new_bool(config.allow_write),
        );
        attrs.insert(
            "allowDelete".to_string(),
            cedar_policy::RestrictedExpression::new_bool(config.allow_delete),
        );
        attrs.insert(
            "allowAdmin".to_string(),
            cedar_policy::RestrictedExpression::new_bool(config.allow_admin),
        );
        attrs.insert(
            "maxDepth".to_string(),
            cedar_policy::RestrictedExpression::new_long(config.max_depth as i64),
        );
        attrs.insert(
            "maxCost".to_string(),
            cedar_policy::RestrictedExpression::new_long(config.max_cost as i64),
        );
        attrs.insert(
            "maxApiCalls".to_string(),
            cedar_policy::RestrictedExpression::new_long(config.max_api_calls as i64),
        );

        attrs.insert(
            "allowedOperations".to_string(),
            Self::string_set_expr(&config.allowed_operations),
        );
        attrs.insert(
            "blockedOperations".to_string(),
            Self::string_set_expr(&config.blocked_operations),
        );
        attrs.insert(
            "blockedFields".to_string(),
            Self::string_set_expr(&config.blocked_fields),
        );

        Entity::new(uid, attrs, HashSet::new()).expect("valid entity")
    }

    fn string_set_expr(set: &HashSet<String>) -> cedar_policy::RestrictedExpression {
        cedar_policy::RestrictedExpression::new_set(
            set.iter()
                .map(|s| cedar_policy::RestrictedExpression::new_string(s.clone())),
        )
    }
}

#[async_trait::async_trait]
impl super::PolicyEvaluator for CedarPolicyEvaluator {
    async fn evaluate_operation(
        &self,
        operation: &OperationEntity,
        server_config: &ServerConfigEntity,
    ) -> Result<AuthorizationDecision, PolicyEvaluationError> {
        let principal = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("CodeMode::Operation").expect("valid type name"),
            EntityId::from_str(&operation.id).expect("valid entity id"),
        );

        // Determine action
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

        let action = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("CodeMode::Action").expect("valid type name"),
            EntityId::from_str(action_id).expect("valid entity id"),
        );

        let resource = EntityUid::from_type_name_and_id(
            EntityTypeName::from_str("CodeMode::Server").expect("valid type name"),
            EntityId::from_str(&server_config.server_id).expect("valid entity id"),
        );

        let op_entity = self.build_operation_entity(operation);
        let server_entity = self.build_server_entity(server_config);

        let entities = Entities::from_entities([op_entity, server_entity], Some(&self.schema))
            .map_err(|e| PolicyEvaluationError::EvaluationError(format!("Entity error: {}", e)))?;

        let context = Context::from_pairs([
            (
                "serverId".to_string(),
                cedar_policy::RestrictedExpression::new_string(server_config.server_id.clone()),
            ),
            (
                "serverType".to_string(),
                cedar_policy::RestrictedExpression::new_string(server_config.server_type.clone()),
            ),
        ])
        .map_err(|e| PolicyEvaluationError::EvaluationError(format!("Context error: {}", e)))?;

        let request = Request::new(principal, action, resource, context, Some(&self.schema))
            .map_err(|e| PolicyEvaluationError::EvaluationError(format!("Request error: {}", e)))?;

        let response = self
            .authorizer
            .is_authorized(&request, &self.policy_set, &entities);

        let allowed = matches!(response.decision(), cedar_policy::Decision::Allow);

        let determining_policies: Vec<String> = response
            .diagnostics()
            .reason()
            .map(|p| p.to_string())
            .collect();

        let errors: Vec<String> = response
            .diagnostics()
            .errors()
            .map(|e| e.to_string())
            .collect();

        Ok(AuthorizationDecision {
            allowed,
            determining_policies,
            errors,
        })
    }

    fn name(&self) -> &str {
        "cedar-local"
    }
}

#[cfg(test)]
mod tests {
    use super::super::PolicyEvaluator;
    use super::*;

    #[tokio::test]
    async fn test_cedar_evaluator_permits_reads() {
        let evaluator = CedarPolicyEvaluator::graphql_default().unwrap();

        let operation = OperationEntity {
            id: "test-query".to_string(),
            operation_type: "query".to_string(),
            operation_name: "GetUsers".to_string(),
            root_fields: ["users".to_string()].into_iter().collect(),
            accessed_types: ["User".to_string()].into_iter().collect(),
            accessed_fields: ["User.id".to_string(), "User.name".to_string()]
                .into_iter()
                .collect(),
            depth: 2,
            field_count: 2,
            estimated_cost: 2,
            has_introspection: false,
            accesses_sensitive_data: false,
            sensitive_categories: HashSet::new(),
        };

        let server_config = ServerConfigEntity::default();

        let decision = evaluator
            .evaluate_operation(&operation, &server_config)
            .await
            .unwrap();
        assert!(
            decision.allowed,
            "Read queries should be permitted by default"
        );
    }

    #[tokio::test]
    async fn test_cedar_evaluator_denies_mutations_when_disabled() {
        let evaluator = CedarPolicyEvaluator::graphql_default().unwrap();

        let operation = OperationEntity {
            id: "test-mutation".to_string(),
            operation_type: "mutation".to_string(),
            operation_name: "CreateUser".to_string(),
            root_fields: ["createUser".to_string()].into_iter().collect(),
            accessed_types: ["User".to_string()].into_iter().collect(),
            accessed_fields: ["User.id".to_string()].into_iter().collect(),
            depth: 1,
            field_count: 1,
            estimated_cost: 1,
            has_introspection: false,
            accesses_sensitive_data: false,
            sensitive_categories: HashSet::new(),
        };

        let server_config = ServerConfigEntity {
            allow_write: false,
            ..Default::default()
        };

        let decision = evaluator
            .evaluate_operation(&operation, &server_config)
            .await
            .unwrap();
        assert!(
            !decision.allowed,
            "Mutations should be denied when allow_write is false"
        );
    }
}
