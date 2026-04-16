//! Cedar schema and policy validation for Code Mode.
//!
//! This module provides compile-time validation of Cedar schemas and policies
//! using the cedar-policy crate. It ensures that:
//!
//! 1. Schemas are valid Cedar schemas
//! 2. Policy templates parse correctly
//! 3. Templates type-check against their target schemas
//!
//! ## Schema Architecture
//!
//! Each server type has its own Cedar schema with:
//! - **Unified actions**: Read, Write, Delete, Admin (same across all types)
//! - **Server-specific principal**: Operation (GraphQL), Script (OpenAPI), Statement (SQL)
//! - **Server-specific attributes**: Each principal type has domain-specific attributes
//!
//! ## Template Organization
//!
//! Templates are organized by what attributes they depend on:
//! - **Shared templates**: Only use unified actions, no principal attributes
//! - **GraphQL templates**: Use Operation-specific attributes (depth, accessedFields, etc.)
//! - **OpenAPI templates**: Use Script-specific attributes (totalApiCalls, hasUnboundedLoop, etc.)
//! - **SQL templates**: Use Statement-specific attributes (tables, columns, etc.)

/// GraphQL Code Mode Cedar schema in JSON format.
///
/// Uses unified actions (Read/Write/Delete/Admin) with GraphQL-specific
/// Operation principal and Server resource.
pub const GRAPHQL_CEDAR_SCHEMA: &str = r#"{
    "CodeMode": {
        "entityTypes": {
            "Operation": {
                "shape": {
                    "type": "Record",
                    "attributes": {
                        "operationType": { "type": "String", "required": true },
                        "operationName": { "type": "String", "required": false },
                        "rootFields": { "type": "Set", "element": { "type": "String" } },
                        "accessedTypes": { "type": "Set", "element": { "type": "String" } },
                        "accessedFields": { "type": "Set", "element": { "type": "String" } },
                        "depth": { "type": "Long", "required": true },
                        "fieldCount": { "type": "Long", "required": true },
                        "estimatedCost": { "type": "Long", "required": true },
                        "hasIntrospection": { "type": "Boolean", "required": true },
                        "accessesSensitiveData": { "type": "Boolean", "required": true },
                        "sensitiveCategories": { "type": "Set", "element": { "type": "String" } }
                    }
                }
            },
            "Server": {
                "shape": {
                    "type": "Record",
                    "attributes": {
                        "serverId": { "type": "String", "required": true },
                        "serverType": { "type": "String", "required": true },
                        "maxDepth": { "type": "Long", "required": true },
                        "maxFieldCount": { "type": "Long", "required": true },
                        "maxCost": { "type": "Long", "required": true },
                        "maxApiCalls": { "type": "Long", "required": true },
                        "allowWrite": { "type": "Boolean", "required": true },
                        "allowDelete": { "type": "Boolean", "required": true },
                        "allowAdmin": { "type": "Boolean", "required": true },
                        "blockedOperations": { "type": "Set", "element": { "type": "String" } },
                        "allowedOperations": { "type": "Set", "element": { "type": "String" } },
                        "blockedFields": { "type": "Set", "element": { "type": "String" } }
                    }
                }
            }
        },
        "actions": {
            "Read": {
                "appliesTo": {
                    "principalTypes": ["Operation"],
                    "resourceTypes": ["Server"]
                }
            },
            "Write": {
                "appliesTo": {
                    "principalTypes": ["Operation"],
                    "resourceTypes": ["Server"]
                }
            },
            "Delete": {
                "appliesTo": {
                    "principalTypes": ["Operation"],
                    "resourceTypes": ["Server"]
                }
            },
            "Admin": {
                "appliesTo": {
                    "principalTypes": ["Operation"],
                    "resourceTypes": ["Server"]
                }
            }
        }
    }
}"#;

/// OpenAPI Code Mode Cedar schema in JSON format.
///
/// Uses unified actions (Read/Write/Delete/Admin) with OpenAPI-specific
/// Script principal and Server resource.
pub const OPENAPI_CEDAR_SCHEMA: &str = r#"{
    "CodeMode": {
        "entityTypes": {
            "Script": {
                "shape": {
                    "type": "Record",
                    "attributes": {
                        "scriptType": { "type": "String", "required": true },
                        "hasWrites": { "type": "Boolean", "required": true },
                        "hasDeletes": { "type": "Boolean", "required": true },
                        "totalApiCalls": { "type": "Long", "required": true },
                        "readCalls": { "type": "Long", "required": true },
                        "writeCalls": { "type": "Long", "required": true },
                        "deleteCalls": { "type": "Long", "required": true },
                        "accessedPaths": { "type": "Set", "element": { "type": "String" } },
                        "accessedMethods": { "type": "Set", "element": { "type": "String" } },
                        "pathPatterns": { "type": "Set", "element": { "type": "String" } },
                        "calledOperations": { "type": "Set", "element": { "type": "String" } },
                        "loopIterations": { "type": "Long", "required": true },
                        "nestingDepth": { "type": "Long", "required": true },
                        "scriptLength": { "type": "Long", "required": true },
                        "accessesSensitivePath": { "type": "Boolean", "required": true },
                        "hasUnboundedLoop": { "type": "Boolean", "required": true },
                        "hasDynamicPath": { "type": "Boolean", "required": true },
                        "outputFields": { "type": "Set", "element": { "type": "String" } },
                        "hasOutputDeclaration": { "type": "Boolean", "required": true },
                        "hasSpreadInOutput": { "type": "Boolean", "required": true }
                    }
                }
            },
            "Server": {
                "shape": {
                    "type": "Record",
                    "attributes": {
                        "serverId": { "type": "String", "required": true },
                        "serverType": { "type": "String", "required": true },
                        "writeMode": { "type": "String", "required": true },
                        "maxDepth": { "type": "Long", "required": true },
                        "maxCost": { "type": "Long", "required": true },
                        "maxApiCalls": { "type": "Long", "required": true },
                        "maxLoopIterations": { "type": "Long", "required": true },
                        "maxScriptLength": { "type": "Long", "required": true },
                        "maxNestingDepth": { "type": "Long", "required": true },
                        "executionTimeoutSeconds": { "type": "Long", "required": true },
                        "allowWrite": { "type": "Boolean", "required": true },
                        "allowDelete": { "type": "Boolean", "required": true },
                        "allowAdmin": { "type": "Boolean", "required": true },
                        "blockedOperations": { "type": "Set", "element": { "type": "String" } },
                        "allowedOperations": { "type": "Set", "element": { "type": "String" } },
                        "blockedFields": { "type": "Set", "element": { "type": "String" } },
                        "allowedMethods": { "type": "Set", "element": { "type": "String" } },
                        "blockedMethods": { "type": "Set", "element": { "type": "String" } },
                        "allowedPathPatterns": { "type": "Set", "element": { "type": "String" } },
                        "blockedPathPatterns": { "type": "Set", "element": { "type": "String" } },
                        "sensitivePathPatterns": { "type": "Set", "element": { "type": "String" } },
                        "autoApproveReadOnly": { "type": "Boolean", "required": true },
                        "maxApiCallsForAutoApprove": { "type": "Long", "required": true },
                        "internalBlockedFields": { "type": "Set", "element": { "type": "String" } },
                        "outputBlockedFields": { "type": "Set", "element": { "type": "String" } },
                        "requireOutputDeclaration": { "type": "Boolean", "required": true }
                    }
                }
            }
        },
        "actions": {
            "Read": {
                "appliesTo": {
                    "principalTypes": ["Script"],
                    "resourceTypes": ["Server"]
                }
            },
            "Write": {
                "appliesTo": {
                    "principalTypes": ["Script"],
                    "resourceTypes": ["Server"]
                }
            },
            "Delete": {
                "appliesTo": {
                    "principalTypes": ["Script"],
                    "resourceTypes": ["Server"]
                }
            },
            "Admin": {
                "appliesTo": {
                    "principalTypes": ["Script"],
                    "resourceTypes": ["Server"]
                }
            }
        }
    }
}"#;

/// SQL Code Mode Cedar schema in JSON format.
///
/// Uses unified actions (Read/Write/Delete/Admin) with SQL-specific
/// Statement principal and Server resource.
pub const SQL_CEDAR_SCHEMA: &str = r#"{
    "CodeMode": {
        "entityTypes": {
            "Statement": {
                "shape": {
                    "type": "Record",
                    "attributes": {
                        "statementType": { "type": "String", "required": true },
                        "tables": { "type": "Set", "element": { "type": "String" } },
                        "columns": { "type": "Set", "element": { "type": "String" } },
                        "hasWhere": { "type": "Boolean", "required": true },
                        "hasLimit": { "type": "Boolean", "required": true },
                        "hasOrderBy": { "type": "Boolean", "required": true },
                        "estimatedRows": { "type": "Long", "required": true },
                        "joinCount": { "type": "Long", "required": true },
                        "subqueryCount": { "type": "Long", "required": true }
                    }
                }
            },
            "Server": {
                "shape": {
                    "type": "Record",
                    "attributes": {
                        "serverId": { "type": "String", "required": true },
                        "serverType": { "type": "String", "required": true },
                        "maxRows": { "type": "Long", "required": true },
                        "maxJoins": { "type": "Long", "required": true },
                        "allowWrite": { "type": "Boolean", "required": true },
                        "allowDelete": { "type": "Boolean", "required": true },
                        "allowAdmin": { "type": "Boolean", "required": true },
                        "blockedOperations": { "type": "Set", "element": { "type": "String" } },
                        "allowedOperations": { "type": "Set", "element": { "type": "String" } },
                        "blockedTables": { "type": "Set", "element": { "type": "String" } },
                        "blockedColumns": { "type": "Set", "element": { "type": "String" } }
                    }
                }
            }
        },
        "actions": {
            "Read": {
                "appliesTo": {
                    "principalTypes": ["Statement"],
                    "resourceTypes": ["Server"]
                }
            },
            "Write": {
                "appliesTo": {
                    "principalTypes": ["Statement"],
                    "resourceTypes": ["Server"]
                }
            },
            "Delete": {
                "appliesTo": {
                    "principalTypes": ["Statement"],
                    "resourceTypes": ["Server"]
                }
            },
            "Admin": {
                "appliesTo": {
                    "principalTypes": ["Statement"],
                    "resourceTypes": ["Server"]
                }
            }
        }
    }
}"#;

/// Policy template definition for validation.
#[derive(Debug, Clone)]
pub struct PolicyTemplate {
    /// Template ID (e.g., "PermitAllReads")
    pub id: &'static str,
    /// Description for documentation
    pub description: &'static str,
    /// Cedar policy statement (with ?principal and ?resource placeholders)
    pub statement: &'static str,
    /// Server types this template is valid for
    pub valid_for: &'static [&'static str],
}

/// Shared templates that work across all server types.
///
/// These templates only use unified actions and don't reference
/// any server-specific principal or resource attributes.
pub const SHARED_TEMPLATES: &[PolicyTemplate] = &[
    PolicyTemplate {
        id: "PermitAllReads",
        description: "Permits all read operations.",
        statement: r#"permit(
    principal,
    action == CodeMode::Action::"Read",
    resource
);"#,
        valid_for: &["graphql-api", "openapi-api", "sql-api"],
    },
    PolicyTemplate {
        id: "PermitWritesWhenEnabled",
        description: "Permits write operations when allowWrite is true.",
        statement: r#"permit(
    principal,
    action == CodeMode::Action::"Write",
    resource
) when {
    resource.allowWrite == true
};"#,
        valid_for: &["graphql-api", "openapi-api", "sql-api"],
    },
    PolicyTemplate {
        id: "PermitDeletesWhenEnabled",
        description: "Permits delete operations when allowDelete is true.",
        statement: r#"permit(
    principal,
    action == CodeMode::Action::"Delete",
    resource
) when {
    resource.allowDelete == true
};"#,
        valid_for: &["graphql-api", "openapi-api", "sql-api"],
    },
    PolicyTemplate {
        id: "PermitAdminWhenEnabled",
        description: "Permits admin operations when allowAdmin is true.",
        statement: r#"permit(
    principal,
    action == CodeMode::Action::"Admin",
    resource
) when {
    resource.allowAdmin == true
};"#,
        valid_for: &["graphql-api", "openapi-api", "sql-api"],
    },
    PolicyTemplate {
        id: "ForbidAllDeletes",
        description: "Forbids all delete operations.",
        statement: r#"forbid(
    principal,
    action == CodeMode::Action::"Delete",
    resource
);"#,
        valid_for: &["graphql-api", "openapi-api", "sql-api"],
    },
];

/// GraphQL-specific templates that use Operation attributes.
pub const GRAPHQL_TEMPLATES: &[PolicyTemplate] = &[
    PolicyTemplate {
        id: "ForbidExcessiveDepth",
        description: "Blocks queries exceeding maxDepth.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.depth > resource.maxDepth
};"#,
        valid_for: &["graphql-api"],
    },
    PolicyTemplate {
        id: "ForbidExcessiveFieldCount",
        description: "Blocks queries exceeding maxFieldCount.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.fieldCount > resource.maxFieldCount
};"#,
        valid_for: &["graphql-api"],
    },
    PolicyTemplate {
        id: "ForbidExcessiveCost",
        description: "Blocks queries exceeding maxCost.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.estimatedCost > resource.maxCost
};"#,
        valid_for: &["graphql-api"],
    },
    PolicyTemplate {
        id: "ForbidBlockedFields",
        description: "Blocks queries accessing blocked fields.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.accessedFields.containsAny(resource.blockedFields)
};"#,
        valid_for: &["graphql-api"],
    },
    PolicyTemplate {
        id: "ForbidSensitiveData",
        description: "Blocks queries accessing sensitive data.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.accessesSensitiveData == true
};"#,
        valid_for: &["graphql-api"],
    },
    PolicyTemplate {
        id: "ForbidBlockedOperations",
        description: "Blocks operations in the blocklist.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal has operationName &&
    resource.blockedOperations.contains(principal.operationName)
};"#,
        valid_for: &["graphql-api"],
    },
];

/// OpenAPI-specific templates that use Script attributes.
pub const OPENAPI_TEMPLATES: &[PolicyTemplate] = &[
    PolicyTemplate {
        id: "ForbidExcessiveApiCalls",
        description: "Blocks scripts exceeding maxApiCalls.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.totalApiCalls > resource.maxApiCalls
};"#,
        valid_for: &["openapi-api"],
    },
    PolicyTemplate {
        id: "ForbidExcessiveNesting",
        description: "Blocks scripts exceeding maxNestingDepth.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.nestingDepth > resource.maxNestingDepth
};"#,
        valid_for: &["openapi-api"],
    },
    PolicyTemplate {
        id: "ForbidUnboundedLoops",
        description: "Blocks scripts with unbounded loops.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.hasUnboundedLoop == true
};"#,
        valid_for: &["openapi-api"],
    },
    PolicyTemplate {
        id: "ForbidSensitivePaths",
        description: "Blocks scripts accessing sensitive paths.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.accessesSensitivePath == true
};"#,
        valid_for: &["openapi-api"],
    },
    PolicyTemplate {
        id: "ForbidOutputBlockedFields",
        description: "Blocks scripts that return blocked fields.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.outputFields.containsAny(resource.outputBlockedFields)
};"#,
        valid_for: &["openapi-api"],
    },
    PolicyTemplate {
        id: "ForbidSpreadWithoutDeclaration",
        description: "Blocks scripts with spread when output declaration is required.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.hasSpreadInOutput == true &&
    resource.requireOutputDeclaration == true
};"#,
        valid_for: &["openapi-api"],
    },
];

/// SQL-specific templates that use Statement attributes.
pub const SQL_TEMPLATES: &[PolicyTemplate] = &[
    PolicyTemplate {
        id: "ForbidExcessiveRows",
        description: "Blocks queries exceeding maxRows.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.estimatedRows > resource.maxRows
};"#,
        valid_for: &["sql-api"],
    },
    PolicyTemplate {
        id: "ForbidExcessiveJoins",
        description: "Blocks queries exceeding maxJoins.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.joinCount > resource.maxJoins
};"#,
        valid_for: &["sql-api"],
    },
    PolicyTemplate {
        id: "ForbidBlockedTables",
        description: "Blocks queries accessing blocked tables.",
        statement: r#"forbid(
    principal,
    action,
    resource
) when {
    principal.tables.containsAny(resource.blockedTables)
};"#,
        valid_for: &["sql-api"],
    },
    PolicyTemplate {
        id: "RequireWhereClause",
        description: "Requires WHERE clause for write/delete operations.",
        statement: r#"forbid(
    principal,
    action == CodeMode::Action::"Write",
    resource
) when {
    principal.hasWhere == false
};

forbid(
    principal,
    action == CodeMode::Action::"Delete",
    resource
) when {
    principal.hasWhere == false
};"#,
        valid_for: &["sql-api"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::field_name_to_type;
    use crate::policy::types::{get_baseline_policies, get_code_mode_schema_json};
    #[cfg(feature = "openapi-code-mode")]
    use crate::policy::types::{get_openapi_baseline_policies, get_openapi_code_mode_schema_json};
    use crate::policy_annotations::{parse_policy_annotations, PolicyCategory, PolicyRiskLevel};
    use crate::schema_exposure::pattern_matches;
    use crate::templates::conditional;
    use cedar_policy::{PolicySet, Schema};

    /// Parse a Cedar schema from JSON.
    fn parse_schema(json: &str) -> Result<Schema, String> {
        Schema::from_json_str(json).map_err(|e| e.to_string())
    }

    /// Parse a Cedar policy.
    fn parse_policy(statement: &str) -> Result<PolicySet, String> {
        statement
            .parse()
            .map_err(|e: cedar_policy::ParseErrors| e.to_string())
    }

    /// Validate a policy against a schema.
    fn validate_policy(schema: &Schema, policy: &PolicySet) -> Result<(), String> {
        let validator = cedar_policy::Validator::new(schema.clone());
        let result = validator.validate(policy, cedar_policy::ValidationMode::Strict);
        if result.validation_passed() {
            Ok(())
        } else {
            let errors: Vec<String> = result.validation_errors().map(|e| e.to_string()).collect();
            Err(errors.join("; "))
        }
    }

    // ==========================================================================
    // SCHEMA VALIDATION TESTS
    // ==========================================================================

    #[test]
    fn test_graphql_schema_is_valid() {
        let result = parse_schema(GRAPHQL_CEDAR_SCHEMA);
        assert!(
            result.is_ok(),
            "GraphQL schema failed to parse: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_openapi_schema_is_valid() {
        let result = parse_schema(OPENAPI_CEDAR_SCHEMA);
        assert!(
            result.is_ok(),
            "OpenAPI schema failed to parse: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_sql_schema_is_valid() {
        let result = parse_schema(SQL_CEDAR_SCHEMA);
        assert!(
            result.is_ok(),
            "SQL schema failed to parse: {:?}",
            result.err()
        );
    }

    // ==========================================================================
    // SHARED TEMPLATE VALIDATION TESTS
    // ==========================================================================

    #[test]
    fn test_shared_templates_parse() {
        for template in SHARED_TEMPLATES {
            let result = parse_policy(template.statement);
            assert!(
                result.is_ok(),
                "Shared template '{}' failed to parse: {:?}",
                template.id,
                result.err()
            );
        }
    }

    #[test]
    fn test_shared_templates_validate_against_graphql_schema() {
        let schema = parse_schema(GRAPHQL_CEDAR_SCHEMA).expect("Schema should parse");
        for template in SHARED_TEMPLATES {
            if template.valid_for.contains(&"graphql-api") {
                let policy = parse_policy(template.statement).expect("Policy should parse");
                let result = validate_policy(&schema, &policy);
                assert!(
                    result.is_ok(),
                    "Shared template '{}' failed GraphQL validation: {:?}",
                    template.id,
                    result.err()
                );
            }
        }
    }

    #[test]
    fn test_shared_templates_validate_against_openapi_schema() {
        let schema = parse_schema(OPENAPI_CEDAR_SCHEMA).expect("Schema should parse");
        for template in SHARED_TEMPLATES {
            if template.valid_for.contains(&"openapi-api") {
                let policy = parse_policy(template.statement).expect("Policy should parse");
                let result = validate_policy(&schema, &policy);
                assert!(
                    result.is_ok(),
                    "Shared template '{}' failed OpenAPI validation: {:?}",
                    template.id,
                    result.err()
                );
            }
        }
    }

    #[test]
    fn test_shared_templates_validate_against_sql_schema() {
        let schema = parse_schema(SQL_CEDAR_SCHEMA).expect("Schema should parse");
        for template in SHARED_TEMPLATES {
            if template.valid_for.contains(&"sql-api") {
                let policy = parse_policy(template.statement).expect("Policy should parse");
                let result = validate_policy(&schema, &policy);
                assert!(
                    result.is_ok(),
                    "Shared template '{}' failed SQL validation: {:?}",
                    template.id,
                    result.err()
                );
            }
        }
    }

    // ==========================================================================
    // GRAPHQL TEMPLATE VALIDATION TESTS
    // ==========================================================================

    #[test]
    fn test_graphql_templates_parse() {
        for template in GRAPHQL_TEMPLATES {
            let result = parse_policy(template.statement);
            assert!(
                result.is_ok(),
                "GraphQL template '{}' failed to parse: {:?}",
                template.id,
                result.err()
            );
        }
    }

    #[test]
    fn test_graphql_templates_validate_against_schema() {
        let schema = parse_schema(GRAPHQL_CEDAR_SCHEMA).expect("Schema should parse");
        for template in GRAPHQL_TEMPLATES {
            let policy = parse_policy(template.statement).expect("Policy should parse");
            let result = validate_policy(&schema, &policy);
            assert!(
                result.is_ok(),
                "GraphQL template '{}' failed validation: {:?}",
                template.id,
                result.err()
            );
        }
    }

    // ==========================================================================
    // OPENAPI TEMPLATE VALIDATION TESTS
    // ==========================================================================

    #[test]
    fn test_openapi_templates_parse() {
        for template in OPENAPI_TEMPLATES {
            let result = parse_policy(template.statement);
            assert!(
                result.is_ok(),
                "OpenAPI template '{}' failed to parse: {:?}",
                template.id,
                result.err()
            );
        }
    }

    #[test]
    fn test_openapi_templates_validate_against_schema() {
        let schema = parse_schema(OPENAPI_CEDAR_SCHEMA).expect("Schema should parse");
        for template in OPENAPI_TEMPLATES {
            let policy = parse_policy(template.statement).expect("Policy should parse");
            let result = validate_policy(&schema, &policy);
            assert!(
                result.is_ok(),
                "OpenAPI template '{}' failed validation: {:?}",
                template.id,
                result.err()
            );
        }
    }

    // ==========================================================================
    // SQL TEMPLATE VALIDATION TESTS
    // ==========================================================================

    #[test]
    fn test_sql_templates_parse() {
        for template in SQL_TEMPLATES {
            let result = parse_policy(template.statement);
            assert!(
                result.is_ok(),
                "SQL template '{}' failed to parse: {:?}",
                template.id,
                result.err()
            );
        }
    }

    #[test]
    fn test_sql_templates_validate_against_schema() {
        let schema = parse_schema(SQL_CEDAR_SCHEMA).expect("Schema should parse");
        for template in SQL_TEMPLATES {
            let policy = parse_policy(template.statement).expect("Policy should parse");
            let result = validate_policy(&schema, &policy);
            assert!(
                result.is_ok(),
                "SQL template '{}' failed validation: {:?}",
                template.id,
                result.err()
            );
        }
    }

    // ==========================================================================
    // CROSS-VALIDATION TESTS (ensure templates DON'T work with wrong schema)
    // ==========================================================================

    #[test]
    fn test_graphql_templates_fail_against_openapi_schema() {
        let schema = parse_schema(OPENAPI_CEDAR_SCHEMA).expect("Schema should parse");
        let template = &GRAPHQL_TEMPLATES[0]; // ForbidExcessiveDepth uses principal.depth
        let policy = parse_policy(template.statement).expect("Policy should parse");
        let result = validate_policy(&schema, &policy);
        // This should fail because Script doesn't have 'depth' attribute
        assert!(
            result.is_err(),
            "GraphQL template '{}' should NOT validate against OpenAPI schema",
            template.id
        );
    }

    #[test]
    fn test_openapi_templates_fail_against_graphql_schema() {
        let schema = parse_schema(GRAPHQL_CEDAR_SCHEMA).expect("Schema should parse");
        let template = &OPENAPI_TEMPLATES[0]; // ForbidExcessiveApiCalls uses principal.totalApiCalls
        let policy = parse_policy(template.statement).expect("Policy should parse");
        let result = validate_policy(&schema, &policy);
        // This should fail because Operation doesn't have 'totalApiCalls' attribute
        assert!(
            result.is_err(),
            "OpenAPI template '{}' should NOT validate against GraphQL schema",
            template.id
        );
    }

    // ==========================================================================
    // ACTION UNIFICATION TESTS
    // ==========================================================================

    #[test]
    fn test_all_schemas_have_unified_actions() {
        let schemas = [
            ("GraphQL", GRAPHQL_CEDAR_SCHEMA),
            ("OpenAPI", OPENAPI_CEDAR_SCHEMA),
            ("SQL", SQL_CEDAR_SCHEMA),
        ];

        let expected_actions = ["Read", "Write", "Delete", "Admin"];

        for (name, schema_json) in schemas {
            let schema_value: serde_json::Value =
                serde_json::from_str(schema_json).expect("Schema should be valid JSON");

            let actions = schema_value["CodeMode"]["actions"]
                .as_object()
                .expect("Schema should have actions");

            for action in &expected_actions {
                assert!(
                    actions.contains_key(*action),
                    "{} schema is missing unified action: {}",
                    name,
                    action
                );
            }
        }
    }

    // ==========================================================================
    // SCHEMA SYNC TESTS (cedar_validation.rs ↔ types.rs must agree)
    // ==========================================================================

    /// Extract sorted attribute names from a Cedar JSON schema entity type.
    fn extract_attrs(schema_json: &serde_json::Value, entity_type: &str) -> Vec<String> {
        let attrs = &schema_json["CodeMode"]["entityTypes"][entity_type]["shape"]["attributes"];
        let mut names: Vec<String> = attrs
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        names.sort();
        names
    }

    #[cfg(feature = "openapi-code-mode")]
    #[test]
    fn test_openapi_schema_sources_in_sync() {
        // cedar_validation.rs (test const — also used by platform for AVP schema provisioning)
        let const_schema: serde_json::Value =
            serde_json::from_str(OPENAPI_CEDAR_SCHEMA).expect("const schema should parse");

        // types.rs (runtime JSON export)
        let runtime_schema = get_openapi_code_mode_schema_json();

        // Server entity attributes must match
        let const_server = extract_attrs(&const_schema, "Server");
        let runtime_server = extract_attrs(&runtime_schema, "Server");
        assert_eq!(
            const_server, runtime_server,
            "OPENAPI_CEDAR_SCHEMA Server attrs != get_openapi_code_mode_schema_json() Server attrs\n\
             const only: {:?}\n\
             runtime only: {:?}",
            const_server
                .iter()
                .filter(|a| !runtime_server.contains(a))
                .collect::<Vec<_>>(),
            runtime_server
                .iter()
                .filter(|a| !const_server.contains(a))
                .collect::<Vec<_>>(),
        );

        // Script entity attributes must match
        let const_script = extract_attrs(&const_schema, "Script");
        let runtime_script = extract_attrs(&runtime_schema, "Script");
        assert_eq!(
            const_script, runtime_script,
            "OPENAPI_CEDAR_SCHEMA Script attrs != get_openapi_code_mode_schema_json() Script attrs\n\
             const only: {:?}\n\
             runtime only: {:?}",
            const_script
                .iter()
                .filter(|a| !runtime_script.contains(a))
                .collect::<Vec<_>>(),
            runtime_script
                .iter()
                .filter(|a| !const_script.contains(a))
                .collect::<Vec<_>>(),
        );
    }

    #[test]
    fn test_graphql_schema_sources_in_sync() {
        // cedar_validation.rs
        let const_schema: serde_json::Value =
            serde_json::from_str(GRAPHQL_CEDAR_SCHEMA).expect("const schema should parse");

        // types.rs
        let runtime_schema = get_code_mode_schema_json();

        // Server entity attributes must match
        let const_server = extract_attrs(&const_schema, "Server");
        let runtime_server = extract_attrs(&runtime_schema, "Server");
        assert_eq!(
            const_server,
            runtime_server,
            "GRAPHQL_CEDAR_SCHEMA Server attrs != get_code_mode_schema_json() Server attrs\n\
             const only: {:?}\n\
             runtime only: {:?}",
            const_server
                .iter()
                .filter(|a| !runtime_server.contains(a))
                .collect::<Vec<_>>(),
            runtime_server
                .iter()
                .filter(|a| !const_server.contains(a))
                .collect::<Vec<_>>(),
        );

        // Operation entity attributes must match
        let const_op = extract_attrs(&const_schema, "Operation");
        let runtime_op = extract_attrs(&runtime_schema, "Operation");
        assert_eq!(
            const_op,
            runtime_op,
            "GRAPHQL_CEDAR_SCHEMA Operation attrs != get_code_mode_schema_json() Operation attrs\n\
             const only: {:?}\n\
             runtime only: {:?}",
            const_op
                .iter()
                .filter(|a| !runtime_op.contains(a))
                .collect::<Vec<_>>(),
            runtime_op
                .iter()
                .filter(|a| !const_op.contains(a))
                .collect::<Vec<_>>(),
        );
    }
}
