//! GraphQL-specific validation for Code Mode.

use crate::types::{
    CodeType, Complexity, SecurityAnalysis, SecurityIssue, SecurityIssueType, ValidationError,
};
use graphql_parser::query::{Definition, Document, OperationDefinition, Selection, SelectionSet};
use std::collections::HashSet;

/// Information extracted from a parsed GraphQL query.
#[derive(Debug, Clone, Default)]
pub struct GraphQLQueryInfo {
    /// The operation type (query, mutation, subscription)
    pub operation_type: GraphQLOperationType,

    /// Name of the operation (if named)
    pub operation_name: Option<String>,

    /// Root fields being queried
    pub root_fields: Vec<String>,

    /// All types accessed in the query
    pub types_accessed: HashSet<String>,

    /// All fields accessed in the query
    pub fields_accessed: HashSet<String>,

    /// Whether the query has variables
    pub has_variables: bool,

    /// Variable names
    pub variable_names: Vec<String>,

    /// Maximum depth of the query
    pub max_depth: usize,

    /// Whether query has fragments
    pub has_fragments: bool,

    /// Fragment names used
    pub fragment_names: Vec<String>,

    /// Whether the query contains introspection fields (__schema, __type)
    pub has_introspection: bool,
}

/// GraphQL operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum GraphQLOperationType {
    #[default]
    Query,
    Mutation,
    Subscription,
}

impl GraphQLOperationType {
    /// Whether this operation is read-only.
    pub fn is_read_only(&self) -> bool {
        matches!(self, GraphQLOperationType::Query)
    }
}

/// GraphQL query validator.
pub struct GraphQLValidator {
    /// Known sensitive field patterns (e.g., "password", "ssn", "creditCard")
    sensitive_fields: Vec<String>,

    /// Maximum allowed query depth
    max_depth: usize,

    /// Maximum allowed complexity
    max_complexity: usize,
}

impl Default for GraphQLValidator {
    fn default() -> Self {
        Self {
            sensitive_fields: vec![
                "password".into(),
                "ssn".into(),
                "socialSecurityNumber".into(),
                "creditCard".into(),
                "creditCardNumber".into(),
                "apiKey".into(),
                "secret".into(),
                "token".into(),
            ],
            max_depth: 10,
            max_complexity: 100,
        }
    }
}

impl GraphQLValidator {
    /// Create a new validator with custom settings.
    pub fn new(sensitive_fields: Vec<String>, max_depth: usize, max_complexity: usize) -> Self {
        Self {
            sensitive_fields: sensitive_fields
                .into_iter()
                .map(|s| s.to_lowercase())
                .collect(),
            max_depth,
            max_complexity,
        }
    }

    /// Parse and validate a GraphQL query.
    pub fn validate(&self, query: &str) -> Result<GraphQLQueryInfo, ValidationError> {
        // Parse the query
        let document = graphql_parser::parse_query::<&str>(query).map_err(|e| {
            ValidationError::ParseError {
                message: e.to_string(),
                line: 0,
                column: 0,
            }
        })?;

        // Extract query information
        let info = self.extract_query_info(&document)?;

        // Validate depth
        if info.max_depth > self.max_depth {
            return Err(ValidationError::SecurityError {
                message: format!(
                    "Query depth {} exceeds maximum allowed depth {}",
                    info.max_depth, self.max_depth
                ),
                issue: SecurityIssueType::DeepNesting,
            });
        }

        Ok(info)
    }

    /// Extract information from a parsed document.
    fn extract_query_info<'a>(
        &self,
        document: &'a Document<'a, &'a str>,
    ) -> Result<GraphQLQueryInfo, ValidationError> {
        let mut info = GraphQLQueryInfo::default();
        let mut found_operation = false;

        for definition in &document.definitions {
            match definition {
                Definition::Operation(op) => {
                    if found_operation {
                        return Err(ValidationError::ParseError {
                            message: "Multiple operations not supported".into(),
                            line: 0,
                            column: 0,
                        });
                    }
                    found_operation = true;
                    self.extract_operation_info(op, &mut info)?;
                },
                Definition::Fragment(frag) => {
                    info.has_fragments = true;
                    info.fragment_names.push(frag.name.to_string());
                },
            }
        }

        if !found_operation {
            return Err(ValidationError::ParseError {
                message: "No operation found in query".into(),
                line: 0,
                column: 0,
            });
        }

        Ok(info)
    }

    /// Extract information from an operation.
    fn extract_operation_info<'a>(
        &self,
        op: &'a OperationDefinition<'a, &'a str>,
        info: &mut GraphQLQueryInfo,
    ) -> Result<(), ValidationError> {
        match op {
            OperationDefinition::Query(q) => {
                info.operation_type = GraphQLOperationType::Query;
                info.operation_name = q.name.map(|s| s.to_string());
                info.has_variables = !q.variable_definitions.is_empty();
                info.variable_names = q
                    .variable_definitions
                    .iter()
                    .map(|v| v.name.to_string())
                    .collect();
                self.extract_selection_set(&q.selection_set, info, 1)?;
            },
            OperationDefinition::Mutation(m) => {
                info.operation_type = GraphQLOperationType::Mutation;
                info.operation_name = m.name.map(|s| s.to_string());
                info.has_variables = !m.variable_definitions.is_empty();
                info.variable_names = m
                    .variable_definitions
                    .iter()
                    .map(|v| v.name.to_string())
                    .collect();
                self.extract_selection_set(&m.selection_set, info, 1)?;
            },
            OperationDefinition::Subscription(s) => {
                info.operation_type = GraphQLOperationType::Subscription;
                info.operation_name = s.name.map(|s| s.to_string());
                info.has_variables = !s.variable_definitions.is_empty();
                info.variable_names = s
                    .variable_definitions
                    .iter()
                    .map(|v| v.name.to_string())
                    .collect();
                self.extract_selection_set(&s.selection_set, info, 1)?;
            },
            OperationDefinition::SelectionSet(ss) => {
                // Anonymous query
                info.operation_type = GraphQLOperationType::Query;
                self.extract_selection_set(ss, info, 1)?;
            },
        }
        Ok(())
    }

    /// Extract information from a selection set.
    fn extract_selection_set<'a>(
        &self,
        selection_set: &'a SelectionSet<'a, &'a str>,
        info: &mut GraphQLQueryInfo,
        depth: usize,
    ) -> Result<(), ValidationError> {
        info.max_depth = info.max_depth.max(depth);

        for selection in &selection_set.items {
            match selection {
                Selection::Field(field) => {
                    let field_name = field.name.to_string();

                    // Check for introspection fields
                    if field_name.starts_with("__") {
                        info.has_introspection = true;
                    }

                    // Track root fields
                    if depth == 1 {
                        info.root_fields.push(field_name.clone());
                    }

                    // Track all fields
                    info.fields_accessed.insert(field_name.clone());

                    // Check for type name hints in field name (e.g., "users" -> "User")
                    // This is a heuristic - real implementation would use schema
                    if depth == 1 {
                        let type_name = field_name_to_type(&field_name);
                        info.types_accessed.insert(type_name);
                    }

                    // Recurse into nested selections
                    self.extract_selection_set(&field.selection_set, info, depth + 1)?;
                },
                Selection::FragmentSpread(spread) => {
                    info.fragment_names.push(spread.fragment_name.to_string());
                },
                Selection::InlineFragment(inline) => {
                    if let Some(type_cond) = &inline.type_condition {
                        info.types_accessed.insert(type_cond.to_string());
                    }
                    self.extract_selection_set(&inline.selection_set, info, depth + 1)?;
                },
            }
        }

        Ok(())
    }

    /// Perform security analysis on query info.
    pub fn analyze_security(&self, info: &GraphQLQueryInfo) -> SecurityAnalysis {
        let mut analysis = SecurityAnalysis {
            is_read_only: info.operation_type.is_read_only(),
            tables_accessed: info.types_accessed.clone(),
            fields_accessed: info.fields_accessed.clone(),
            has_aggregation: false,
            has_subqueries: info.max_depth > 3,
            estimated_complexity: self.estimate_complexity(info),
            potential_issues: Vec::new(),
            estimated_rows: None,
        };

        // Check for sensitive fields
        for field in &info.fields_accessed {
            let field_lower = field.to_lowercase();
            if self
                .sensitive_fields
                .iter()
                .any(|s| field_lower.contains(s))
            {
                analysis.potential_issues.push(SecurityIssue::new(
                    SecurityIssueType::SensitiveFields,
                    format!("Query accesses potentially sensitive field: {}", field),
                ));
            }
        }

        // Check for deep nesting
        if info.max_depth > 5 {
            analysis.potential_issues.push(SecurityIssue::new(
                SecurityIssueType::DeepNesting,
                format!("Query has deep nesting (depth: {})", info.max_depth),
            ));
        }

        // Check for high complexity
        if matches!(analysis.estimated_complexity, Complexity::High) {
            analysis.potential_issues.push(SecurityIssue::new(
                SecurityIssueType::HighComplexity,
                "Query has high complexity",
            ));
        }

        analysis
    }

    /// Estimate query complexity.
    fn estimate_complexity(&self, info: &GraphQLQueryInfo) -> Complexity {
        let field_count = info.fields_accessed.len();
        let type_count = info.types_accessed.len();
        let depth = info.max_depth;

        // Simple heuristic based on fields, types, and depth
        let complexity_score = field_count + (type_count * 2) + (depth * depth);

        if complexity_score > self.max_complexity {
            Complexity::High
        } else if complexity_score > self.max_complexity / 2 {
            Complexity::Medium
        } else {
            Complexity::Low
        }
    }

    /// Convert query info to CodeType.
    pub fn to_code_type(&self, info: &GraphQLQueryInfo) -> CodeType {
        match info.operation_type {
            GraphQLOperationType::Query => CodeType::GraphQLQuery,
            GraphQLOperationType::Mutation => CodeType::GraphQLMutation,
            GraphQLOperationType::Subscription => CodeType::GraphQLQuery, // Treat as query for now
        }
    }
}

/// Convert a field name to a probable type name.
///
/// e.g., "users" -> "User", "orderItems" -> "OrderItem"
pub(crate) fn field_name_to_type(field_name: &str) -> String {
    // Remove trailing 's' for plurals and capitalize
    let singular = if field_name.ends_with('s') && field_name.len() > 1 {
        &field_name[..field_name.len() - 1]
    } else {
        field_name
    };

    // Capitalize first letter
    let mut c = singular.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query_parsing() {
        let validator = GraphQLValidator::default();
        let query = "query { users { id name email } }";

        let info = validator.validate(query).unwrap();

        assert_eq!(info.operation_type, GraphQLOperationType::Query);
        assert!(info.root_fields.contains(&"users".to_string()));
        assert!(info.fields_accessed.contains("id"));
        assert!(info.fields_accessed.contains("name"));
        assert!(info.fields_accessed.contains("email"));
    }

    #[test]
    fn test_mutation_detection() {
        let validator = GraphQLValidator::default();
        let query = "mutation { createUser(name: \"test\") { id } }";

        let info = validator.validate(query).unwrap();

        assert_eq!(info.operation_type, GraphQLOperationType::Mutation);
        assert!(!info.operation_type.is_read_only());
    }

    #[test]
    fn test_nested_query() {
        let validator = GraphQLValidator::default();
        let query = r#"
            query {
                users {
                    id
                    orders {
                        id
                        items {
                            product {
                                name
                            }
                        }
                    }
                }
            }
        "#;

        let info = validator.validate(query).unwrap();

        assert!(info.max_depth >= 4);
    }

    #[test]
    fn test_sensitive_field_detection() {
        let validator = GraphQLValidator::default();
        let query = "query { users { id name password } }";

        let info = validator.validate(query).unwrap();
        let analysis = validator.analyze_security(&info);

        assert!(analysis
            .potential_issues
            .iter()
            .any(|i| matches!(i.issue_type, SecurityIssueType::SensitiveFields)));
    }

    #[test]
    fn test_variables() {
        let validator = GraphQLValidator::default();
        let query = "query GetUser($id: ID!) { user(id: $id) { name } }";

        let info = validator.validate(query).unwrap();

        assert!(info.has_variables);
        assert!(info.variable_names.contains(&"id".to_string()));
        assert_eq!(info.operation_name, Some("GetUser".to_string()));
    }

    #[test]
    fn test_field_name_to_type() {
        assert_eq!(field_name_to_type("users"), "User");
        assert_eq!(field_name_to_type("orderItems"), "OrderItem");
        assert_eq!(field_name_to_type("user"), "User");
    }
}
