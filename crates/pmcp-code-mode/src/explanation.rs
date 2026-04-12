//! Template-based explanation generation for Code Mode.
//!
//! MVP uses templates. Full implementation will use server-side LLM.

use crate::graphql::{GraphQLOperationType, GraphQLQueryInfo};
use crate::types::{RiskLevel, SecurityAnalysis};

/// Trait for generating human-readable explanations.
pub trait ExplanationGenerator: Send + Sync {
    /// Generate an explanation for a GraphQL query.
    fn explain_graphql(&self, query_info: &GraphQLQueryInfo, security: &SecurityAnalysis)
        -> String;
}

/// Template-based explanation generator for MVP.
pub struct TemplateExplanationGenerator;

impl Default for TemplateExplanationGenerator {
    fn default() -> Self {
        Self
    }
}

impl TemplateExplanationGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl ExplanationGenerator for TemplateExplanationGenerator {
    fn explain_graphql(
        &self,
        query_info: &GraphQLQueryInfo,
        security: &SecurityAnalysis,
    ) -> String {
        let mut parts = Vec::new();

        // Operation type description
        let op_desc = match query_info.operation_type {
            GraphQLOperationType::Query => "This query will read",
            GraphQLOperationType::Mutation => "This mutation will modify",
            GraphQLOperationType::Subscription => "This subscription will watch",
        };

        // What data is being accessed
        let types: Vec<&str> = security
            .tables_accessed
            .iter()
            .map(|s| s.as_str())
            .collect();
        let types_desc = if types.is_empty() {
            "data".to_string()
        } else if types.len() == 1 {
            format!("{} data", types[0])
        } else {
            let last = types.last().unwrap();
            let rest = &types[..types.len() - 1];
            format!("{} and {} data", rest.join(", "), last)
        };

        parts.push(format!("{} {}.", op_desc, types_desc));

        // Fields being accessed
        let fields: Vec<&str> = security
            .fields_accessed
            .iter()
            .map(|s| s.as_str())
            .collect();
        if !fields.is_empty() {
            let field_count = fields.len();
            if field_count <= 5 {
                parts.push(format!("Fields: {}.", fields.join(", ")));
            } else {
                parts.push(format!(
                    "Accessing {} fields including: {}.",
                    field_count,
                    fields[..5].join(", ")
                ));
            }
        }

        // Security warnings
        let sensitive_issues: Vec<_> = security
            .potential_issues
            .iter()
            .filter(|i| i.is_sensitive())
            .collect();

        if !sensitive_issues.is_empty() {
            parts.push("⚠️ This query accesses potentially sensitive data.".to_string());
        }

        // Complexity notes
        if query_info.max_depth > 3 {
            parts.push(format!(
                "Query has {} levels of nesting.",
                query_info.max_depth
            ));
        }

        // Risk level summary
        let risk = security.assess_risk();
        let risk_desc = match risk {
            RiskLevel::Low => "Risk: LOW (read-only, no sensitive data)".to_string(),
            RiskLevel::Medium => "Risk: MEDIUM (may access sensitive data)".to_string(),
            RiskLevel::High => "Risk: HIGH (modifies multiple records)".to_string(),
            RiskLevel::Critical => "Risk: CRITICAL (requires admin approval)".to_string(),
        };
        parts.push(risk_desc);

        parts.join(" ")
    }
}

/// Generate a simple description for auto-approval messages.
#[allow(dead_code)]
pub fn auto_approval_message(risk_level: RiskLevel) -> &'static str {
    match risk_level {
        RiskLevel::Low => "Auto-approved: low-risk read-only query",
        RiskLevel::Medium => "Auto-approved: medium-risk query (configured to allow)",
        RiskLevel::High => "Auto-approved: high-risk query (configured to allow)",
        RiskLevel::Critical => "Auto-approved: critical-risk query (configured to allow)",
    }
}

/// Generate a denial message when mutations are not allowed.
#[allow(dead_code)]
pub fn mutations_not_allowed_message() -> &'static str {
    "Mutations are not enabled for this server. Only read-only queries are allowed in Code Mode."
}

/// Generate a message when code mode is disabled.
#[allow(dead_code)]
pub fn code_mode_disabled_message() -> &'static str {
    "Code Mode is not enabled for this server. Use the standard tools instead."
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn sample_query_info() -> GraphQLQueryInfo {
        GraphQLQueryInfo {
            operation_type: GraphQLOperationType::Query,
            operation_name: Some("GetUsers".to_string()),
            root_fields: vec!["users".to_string()],
            types_accessed: {
                let mut set = HashSet::new();
                set.insert("User".to_string());
                set
            },
            fields_accessed: {
                let mut set = HashSet::new();
                set.insert("id".to_string());
                set.insert("name".to_string());
                set.insert("email".to_string());
                set
            },
            has_variables: false,
            variable_names: vec![],
            max_depth: 2,
            has_fragments: false,
            fragment_names: vec![],
            has_introspection: false,
        }
    }

    fn sample_security() -> SecurityAnalysis {
        let info = sample_query_info();
        SecurityAnalysis {
            is_read_only: true,
            tables_accessed: info.types_accessed,
            fields_accessed: info.fields_accessed,
            has_aggregation: false,
            has_subqueries: false,
            estimated_complexity: crate::types::Complexity::Low,
            potential_issues: vec![],
            estimated_rows: None,
        }
    }

    #[test]
    fn test_basic_explanation() {
        let generator = TemplateExplanationGenerator::new();
        let info = sample_query_info();
        let security = sample_security();

        let explanation = generator.explain_graphql(&info, &security);

        assert!(explanation.contains("read"));
        assert!(explanation.contains("User"));
        assert!(explanation.contains("Risk: LOW"));
    }

    #[test]
    fn test_mutation_explanation() {
        let generator = TemplateExplanationGenerator::new();
        let mut info = sample_query_info();
        info.operation_type = GraphQLOperationType::Mutation;

        let mut security = sample_security();
        security.is_read_only = false;

        let explanation = generator.explain_graphql(&info, &security);

        assert!(explanation.contains("modify"));
    }
}
