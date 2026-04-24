//! Domain types for policy evaluation.
//!
//! These types represent the entities used in Cedar policy evaluation.
//! They are pure domain types with no AWS SDK dependency.

#[cfg(feature = "openapi-code-mode")]
use crate::config::OperationRegistry;
use crate::graphql::GraphQLQueryInfo;
use std::collections::HashSet;

/// Server configuration for policy evaluation.
///
/// Uses unified attribute names that match the Cedar schema:
/// - `allow_write`, `allow_delete`, `allow_admin` (unified action flags)
/// - `blocked_operations`, `allowed_operations` (unified operation lists)
#[derive(Debug, Clone)]
pub struct ServerConfigEntity {
    /// Server ID
    pub server_id: String,

    /// Server type (e.g., "graphql")
    pub server_type: String,

    /// Whether write operations (mutations) are allowed
    pub allow_write: bool,

    /// Whether delete operations are allowed
    pub allow_delete: bool,

    /// Whether admin operations (introspection) are allowed
    pub allow_admin: bool,

    /// Allowed operation names (allowlist mode)
    pub allowed_operations: HashSet<String>,

    /// Blocked operation names (blocklist mode)
    pub blocked_operations: HashSet<String>,

    /// Maximum query depth
    pub max_depth: u32,

    /// Maximum field count
    pub max_field_count: u32,

    /// Maximum estimated cost
    pub max_cost: u32,

    /// Maximum API calls (for compatibility with unified schema)
    pub max_api_calls: u32,

    /// Fields that should be blocked
    pub blocked_fields: HashSet<String>,

    /// Allowed sensitive data categories
    pub allowed_sensitive_categories: HashSet<String>,
}

impl Default for ServerConfigEntity {
    fn default() -> Self {
        Self {
            server_id: "unknown".to_string(),
            server_type: "graphql".to_string(),
            allow_write: false,
            allow_delete: false,
            allow_admin: false,
            allowed_operations: HashSet::new(),
            blocked_operations: HashSet::new(),
            max_depth: 10,
            max_field_count: 100,
            max_cost: 1000,
            max_api_calls: 50,
            blocked_fields: HashSet::new(),
            allowed_sensitive_categories: HashSet::new(),
        }
    }
}

/// Operation entity for policy evaluation.
#[derive(Debug, Clone)]
pub struct OperationEntity {
    /// Unique ID for this operation
    pub id: String,

    /// Operation type: "query", "mutation", or "subscription"
    pub operation_type: String,

    /// Operation name (if provided)
    pub operation_name: String,

    /// Root fields accessed
    pub root_fields: HashSet<String>,

    /// Types accessed
    pub accessed_types: HashSet<String>,

    /// Fields accessed (Type.field format)
    pub accessed_fields: HashSet<String>,

    /// Query nesting depth
    pub depth: u32,

    /// Total field count
    pub field_count: u32,

    /// Estimated query cost
    pub estimated_cost: u32,

    /// Whether introspection is used
    pub has_introspection: bool,

    /// Whether sensitive data is accessed
    pub accesses_sensitive_data: bool,

    /// Sensitive data categories accessed
    pub sensitive_categories: HashSet<String>,
}

impl OperationEntity {
    /// Create from GraphQL query info.
    pub fn from_query_info(query_info: &GraphQLQueryInfo) -> Self {
        use crate::graphql::GraphQLOperationType;

        let operation_type = match query_info.operation_type {
            GraphQLOperationType::Query => "query",
            GraphQLOperationType::Mutation => "mutation",
            GraphQLOperationType::Subscription => "subscription",
        };

        Self {
            id: query_info
                .operation_name
                .clone()
                .unwrap_or_else(|| "anonymous".to_string()),
            operation_type: operation_type.to_string(),
            operation_name: query_info.operation_name.clone().unwrap_or_default(),
            root_fields: query_info.root_fields.iter().cloned().collect(),
            accessed_types: query_info.types_accessed.iter().cloned().collect(),
            accessed_fields: query_info.fields_accessed.iter().cloned().collect(),
            depth: query_info.max_depth as u32,
            field_count: query_info.fields_accessed.len() as u32,
            estimated_cost: query_info.fields_accessed.len() as u32,
            has_introspection: query_info.has_introspection,
            accesses_sensitive_data: false,
            sensitive_categories: HashSet::new(),
        }
    }
}

/// Authorization decision from policy evaluation.
#[derive(Debug, Clone)]
pub struct AuthorizationDecision {
    /// Whether the operation is allowed
    pub allowed: bool,

    /// Policy IDs that determined the decision
    pub determining_policies: Vec<String>,

    /// Error messages (if any)
    pub errors: Vec<String>,
}

/// Script entity for policy evaluation (OpenAPI Code Mode).
///
/// Unlike GraphQL's single Operation entity, OpenAPI Code Mode validates
/// JavaScript scripts that can contain multiple API calls with loops and logic.
#[cfg(feature = "openapi-code-mode")]
#[derive(Debug, Clone)]
pub struct ScriptEntity {
    /// Unique ID for this script validation
    pub id: String,

    /// Script type: "read_only", "mixed", or "write_only"
    pub script_type: String,

    /// Whether script contains any write operations (POST/PUT/PATCH/DELETE)
    pub has_writes: bool,

    /// Whether script contains DELETE operations
    pub has_deletes: bool,

    /// Total number of API calls in the script
    pub total_api_calls: u32,

    /// Number of GET calls
    pub read_calls: u32,

    /// Number of POST/PUT/PATCH calls
    pub write_calls: u32,

    /// Number of DELETE calls
    pub delete_calls: u32,

    /// Set of all paths accessed
    pub accessed_paths: HashSet<String>,

    /// Set of all HTTP methods used
    pub accessed_methods: HashSet<String>,

    /// Normalized path patterns (IDs replaced with *)
    pub path_patterns: HashSet<String>,

    /// Called operations in "METHOD:pathPattern" format for allowlist/blocklist matching
    pub called_operations: HashSet<String>,

    /// Maximum loop iterations (from .slice() bounds)
    pub loop_iterations: u32,

    /// Maximum nesting depth in the AST
    pub nesting_depth: u32,

    /// Script length in characters
    pub script_length: u32,

    /// Whether script accesses sensitive paths (/admin, /internal, etc.)
    pub accesses_sensitive_path: bool,

    /// Whether script has an unbounded loop
    pub has_unbounded_loop: bool,

    /// Whether script uses dynamic path interpolation
    pub has_dynamic_path: bool,

    /// Whether script has a @returns output declaration
    pub has_output_declaration: bool,

    /// Fields declared in the @returns annotation
    pub output_fields: HashSet<String>,

    /// Whether script uses spread operators in output (potential field leakage)
    pub has_spread_in_output: bool,
}

#[cfg(feature = "openapi-code-mode")]
impl ScriptEntity {
    /// Build from JavaScript code analysis.
    pub fn from_javascript_info(
        info: &crate::javascript::JavaScriptCodeInfo,
        sensitive_patterns: &[String],
        registry: Option<&OperationRegistry>,
    ) -> Self {
        use crate::javascript::HttpMethod;

        let mut accessed_paths = HashSet::new();
        let mut accessed_methods = HashSet::new();
        let mut path_patterns = HashSet::new();
        let mut called_operations = HashSet::new();
        let mut read_calls = 0u32;
        let mut write_calls = 0u32;
        let mut delete_calls = 0u32;
        let mut has_dynamic_path = false;
        let mut accesses_sensitive_path = false;

        for api_call in &info.api_calls {
            accessed_paths.insert(api_call.path.clone());
            let method_str = format!("{:?}", api_call.method).to_uppercase();
            accessed_methods.insert(method_str.clone());

            // Normalize path to pattern
            let pattern = normalize_path_to_pattern(&api_call.path);
            path_patterns.insert(pattern.clone());

            // Build called operation string: use canonical ID from registry if available,
            // fall back to METHOD:/path format when no registry entry matches.
            let op_id = registry
                .and_then(|r| r.lookup(&api_call.path))
                .map(|id| id.to_string())
                .unwrap_or_else(|| format!("{}:{}", method_str, pattern));
            called_operations.insert(op_id);

            // Count by declared category (from [[code_mode.operations]]) when available,
            // fall back to HTTP method when no registry entry or no category declared.
            let call_category = registry.and_then(|r| r.lookup_category(&api_call.path));
            match call_category {
                Some("read") => read_calls += 1,
                Some("delete") => delete_calls += 1,
                Some("write" | "admin") => write_calls += 1,
                Some(_) => write_calls += 1,
                None => match api_call.method {
                    HttpMethod::Get | HttpMethod::Head | HttpMethod::Options => read_calls += 1,
                    HttpMethod::Delete => delete_calls += 1,
                    _ => write_calls += 1,
                },
            }

            // Track dynamic paths
            if api_call.is_dynamic_path {
                has_dynamic_path = true;
            }

            // Check for sensitive path access
            let path_lower = api_call.path.to_lowercase();
            for pattern in sensitive_patterns {
                if path_lower.contains(&pattern.to_lowercase()) {
                    accesses_sensitive_path = true;
                    break;
                }
            }
        }

        // Determine script type
        let has_writes = write_calls > 0 || delete_calls > 0;
        let has_reads = read_calls > 0;
        let script_type = match (has_reads, has_writes) {
            (true, false) => "read_only",
            (false, true) => "write_only",
            (true, true) => "mixed",
            (false, false) => "empty",
        };

        Self {
            id: info
                .api_calls
                .first()
                .map(|c| format!("{}:{}", format!("{:?}", c.method).to_uppercase(), c.path))
                .unwrap_or_else(|| "script".to_string()),
            script_type: script_type.to_string(),
            has_writes,
            has_deletes: delete_calls > 0,
            total_api_calls: info.api_calls.len() as u32,
            read_calls,
            write_calls,
            delete_calls,
            accessed_paths,
            accessed_methods,
            path_patterns,
            called_operations,
            loop_iterations: 0,
            nesting_depth: info.max_depth as u32,
            script_length: 0,
            accesses_sensitive_path,
            has_unbounded_loop: !info.all_loops_bounded && info.loop_count > 0,
            has_dynamic_path,
            has_output_declaration: info.output_declaration.has_declaration,
            output_fields: info.output_declaration.declared_fields.clone(),
            has_spread_in_output: info.output_declaration.has_spread_risk
                || info.has_output_spread_risk,
        }
    }

    /// Get the policy action for this script using unified action model.
    pub fn action(&self) -> &'static str {
        match self.script_type.as_str() {
            "read_only" | "empty" => "Read",
            "write_only" | "mixed" => {
                if self.has_deletes {
                    "Delete"
                } else {
                    "Write"
                }
            },
            _ => "Read",
        }
    }
}

/// Check whether a path segment looks like a UUID (8-4-4-4-12 hex pattern).
#[cfg(feature = "openapi-code-mode")]
fn is_uuid_like(segment: &str) -> bool {
    if segment.len() != 36 {
        return false;
    }
    let parts: Vec<&str> = segment.split('-').collect();
    matches!(parts.as_slice(), [a, b, c, d, e]
        if a.len() == 8 && b.len() == 4 && c.len() == 4
        && d.len() == 4 && e.len() == 12
        && segment.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '-'))
}

/// Normalize a path to a pattern by replacing numeric/UUID segments with *.
#[cfg(feature = "openapi-code-mode")]
pub fn normalize_path_to_pattern(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if segment.chars().all(|c| c.is_ascii_digit()) || is_uuid_like(segment) {
                "*"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Normalize an operation string to the canonical "METHOD:/path" format.
#[cfg(feature = "openapi-code-mode")]
pub fn normalize_operation_format(op: &str) -> String {
    let trimmed = op.trim();

    let (method, path) = if let Some(idx) = trimmed.find(':') {
        let potential_path = trimmed[idx + 1..].trim();
        if potential_path.starts_with('/') {
            let method = trimmed[..idx].trim();
            (method, potential_path)
        } else {
            return trimmed.to_string();
        }
    } else if let Some(idx) = trimmed.find(' ') {
        let method = trimmed[..idx].trim();
        let path = trimmed[idx + 1..].trim();
        (method, path)
    } else {
        return trimmed.to_string();
    };

    let method_upper = method.to_uppercase();

    let normalized_path = path
        .split('/')
        .map(|segment| {
            if segment.starts_with('{') && segment.ends_with('}') {
                "*"
            } else if segment.starts_with(':') {
                "*"
            } else if segment.chars().all(|c| c.is_ascii_digit()) {
                "*"
            } else if is_uuid_like(segment) {
                "*"
            } else {
                segment
            }
        })
        .collect::<Vec<_>>()
        .join("/");

    format!("{}:{}", method_upper, normalized_path)
}

/// Server configuration for OpenAPI Code Mode.
#[cfg(feature = "openapi-code-mode")]
#[derive(Debug, Clone)]
pub struct OpenAPIServerEntity {
    pub server_id: String,
    pub server_type: String,

    // Unified action flags
    pub allow_write: bool,
    pub allow_delete: bool,
    pub allow_admin: bool,

    // Write mode: "allow_all", "deny_all", "allowlist", "blocklist"
    pub write_mode: String,

    // Unified limits
    pub max_depth: u32,
    pub max_cost: u32,
    pub max_api_calls: u32,

    // OpenAPI-specific limits
    pub max_loop_iterations: u32,
    pub max_script_length: u32,
    pub max_nesting_depth: u32,
    pub execution_timeout_seconds: u32,

    // Unified operation lists
    pub allowed_operations: HashSet<String>,
    pub blocked_operations: HashSet<String>,

    // OpenAPI-specific method controls
    pub allowed_methods: HashSet<String>,
    pub blocked_methods: HashSet<String>,
    pub allowed_path_patterns: HashSet<String>,
    pub blocked_path_patterns: HashSet<String>,
    pub sensitive_path_patterns: HashSet<String>,

    // Auto-approval settings
    pub auto_approve_read_only: bool,
    pub max_api_calls_for_auto_approve: u32,

    // Field control (two-tier blocklist)
    pub internal_blocked_fields: HashSet<String>,
    pub output_blocked_fields: HashSet<String>,
    pub require_output_declaration: bool,
}

#[cfg(feature = "openapi-code-mode")]
impl Default for OpenAPIServerEntity {
    fn default() -> Self {
        Self {
            server_id: "unknown".to_string(),
            server_type: "openapi".to_string(),
            allow_write: false,
            allow_delete: false,
            allow_admin: false,
            write_mode: "deny_all".to_string(),
            max_depth: 10,
            max_cost: 1000,
            max_api_calls: 50,
            max_loop_iterations: 100,
            max_script_length: 10000,
            max_nesting_depth: 10,
            execution_timeout_seconds: 30,
            allowed_operations: HashSet::new(),
            blocked_operations: HashSet::new(),
            allowed_methods: HashSet::new(),
            blocked_methods: HashSet::new(),
            allowed_path_patterns: HashSet::new(),
            blocked_path_patterns: ["/admin".into(), "/internal".into()].into_iter().collect(),
            sensitive_path_patterns: ["/admin".into(), "/internal".into(), "/debug".into()]
                .into_iter()
                .collect(),
            auto_approve_read_only: true,
            max_api_calls_for_auto_approve: 10,
            internal_blocked_fields: HashSet::new(),
            output_blocked_fields: HashSet::new(),
            require_output_declaration: false,
        }
    }
}

/// SQL statement entity for policy evaluation (SQL Code Mode).
///
/// Mirrors the `Statement` entity in `SQL_CEDAR_SCHEMA` —
/// see `cedar_validation.rs` for the schema definition.
#[cfg(feature = "sql-code-mode")]
#[derive(Debug, Clone)]
pub struct StatementEntity {
    /// Unique ID for this statement validation.
    pub id: String,

    /// Statement type: "SELECT", "INSERT", "UPDATE", "DELETE", "DDL", "OTHER".
    pub statement_type: String,

    /// Tables referenced by the statement.
    pub tables: HashSet<String>,

    /// Columns referenced by the statement. `*` for wildcards.
    pub columns: HashSet<String>,

    /// Whether the statement has a WHERE clause.
    pub has_where: bool,

    /// Whether the statement has a LIMIT clause.
    pub has_limit: bool,

    /// Whether the statement has an ORDER BY clause.
    pub has_order_by: bool,

    /// Estimated rows affected.
    pub estimated_rows: u64,

    /// Number of JOIN clauses.
    pub join_count: u32,

    /// Number of nested subqueries.
    pub subquery_count: u32,
}

#[cfg(feature = "sql-code-mode")]
impl StatementEntity {
    /// Build from [`SqlStatementInfo`](crate::sql::SqlStatementInfo).
    pub fn from_sql_info(info: &crate::sql::SqlStatementInfo) -> Self {
        Self {
            id: format!(
                "{}:{}",
                info.statement_type.as_str(),
                first_or_default(&info.tables)
            ),
            statement_type: info.statement_type.as_str().to_string(),
            tables: info.tables.clone(),
            columns: info.columns.clone(),
            has_where: info.has_where,
            has_limit: info.has_limit,
            has_order_by: info.has_order_by,
            estimated_rows: info.estimated_rows,
            join_count: info.join_count,
            subquery_count: info.subquery_count,
        }
    }

    /// Get the Cedar action for this statement using unified action model.
    pub fn action(&self) -> &'static str {
        match self.statement_type.as_str() {
            "SELECT" => "Read",
            "INSERT" | "UPDATE" => "Write",
            "DELETE" => "Delete",
            "DDL" => "Admin",
            _ => "Read",
        }
    }
}

/// Helper for building a deterministic statement ID.
#[cfg(feature = "sql-code-mode")]
fn first_or_default(set: &HashSet<String>) -> String {
    let mut names: Vec<&String> = set.iter().collect();
    names.sort();
    names
        .first()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "statement".to_string())
}

/// Server configuration for SQL Code Mode.
///
/// Fields use `sql_*` config prefixes externally so DBA administrators
/// can set "this is a SQL server's config" vocabulary in `config.toml`.
/// Field names here drop the prefix for concision in policy code.
#[cfg(feature = "sql-code-mode")]
#[derive(Debug, Clone)]
pub struct SqlServerEntity {
    pub server_id: String,
    pub server_type: String,

    // Unified action flags
    pub allow_write: bool,
    pub allow_delete: bool,
    pub allow_admin: bool,

    // SQL-specific limits
    pub max_rows: u64,
    pub max_joins: u32,

    // Unified operation lists (statement-type level, e.g., "SELECT"/"INSERT")
    pub allowed_operations: HashSet<String>,
    pub blocked_operations: HashSet<String>,

    // SQL-specific table/column controls
    pub blocked_tables: HashSet<String>,
    pub blocked_columns: HashSet<String>,
    pub allowed_tables: HashSet<String>,
}

#[cfg(feature = "sql-code-mode")]
impl Default for SqlServerEntity {
    fn default() -> Self {
        Self {
            server_id: "unknown".to_string(),
            server_type: "sql".to_string(),
            allow_write: false,
            allow_delete: false,
            allow_admin: false,
            max_rows: 10_000,
            max_joins: 5,
            allowed_operations: HashSet::new(),
            blocked_operations: HashSet::new(),
            blocked_tables: HashSet::new(),
            blocked_columns: HashSet::new(),
            allowed_tables: HashSet::new(),
        }
    }
}

/// Get the Cedar schema in JSON format.
///
/// Uses unified action model with Read/Write/Delete/Admin actions.
pub fn get_code_mode_schema_json() -> serde_json::Value {
    let applies_to = serde_json::json!({
        "principalTypes": ["Operation"],
        "resourceTypes": ["Server"],
        "context": {
            "type": "Record",
            "attributes": {
                "serverId": { "type": "String", "required": true },
                "serverType": { "type": "String", "required": true },
                "userId": { "type": "String", "required": false },
                "sessionId": { "type": "String", "required": false }
            }
        }
    });

    serde_json::json!({
        "CodeMode": {
            "entityTypes": {
                "Operation": {
                    "shape": {
                        "type": "Record",
                        "attributes": {
                            "operationType": { "type": "String", "required": true },
                            "operationName": { "type": "String", "required": true },
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
                "Read": { "appliesTo": applies_to },
                "Write": { "appliesTo": applies_to },
                "Delete": { "appliesTo": applies_to },
                "Admin": { "appliesTo": applies_to }
            }
        }
    })
}

/// Get the Cedar schema for OpenAPI Code Mode in JSON format.
#[cfg(feature = "openapi-code-mode")]
pub fn get_openapi_code_mode_schema_json() -> serde_json::Value {
    let applies_to = serde_json::json!({
        "principalTypes": ["Script"],
        "resourceTypes": ["Server"],
        "context": {
            "type": "Record",
            "attributes": {
                "serverId": { "type": "String", "required": true },
                "serverType": { "type": "String", "required": true },
                "userId": { "type": "String", "required": false },
                "sessionId": { "type": "String", "required": false }
            }
        }
    });

    serde_json::json!({
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
                            "allowWrite": { "type": "Boolean", "required": true },
                            "allowDelete": { "type": "Boolean", "required": true },
                            "allowAdmin": { "type": "Boolean", "required": true },
                            "blockedOperations": { "type": "Set", "element": { "type": "String" } },
                            "allowedOperations": { "type": "Set", "element": { "type": "String" } },
                            "blockedFields": { "type": "Set", "element": { "type": "String" } },
                            "maxLoopIterations": { "type": "Long", "required": true },
                            "maxScriptLength": { "type": "Long", "required": true },
                            "maxNestingDepth": { "type": "Long", "required": true },
                            "executionTimeoutSeconds": { "type": "Long", "required": true },
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
                "Read": { "appliesTo": applies_to },
                "Write": { "appliesTo": applies_to },
                "Delete": { "appliesTo": applies_to },
                "Admin": { "appliesTo": applies_to }
            }
        }
    })
}

/// Get baseline Cedar policies for OpenAPI Code Mode.
#[cfg(feature = "openapi-code-mode")]
pub fn get_openapi_baseline_policies() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "permit_reads",
            "Permit all read operations (GET scripts)",
            r#"permit(principal, action == CodeMode::Action::"Read", resource);"#,
        ),
        (
            "permit_writes",
            "Permit write operations (when enabled)",
            r#"permit(principal, action == CodeMode::Action::"Write", resource) when { resource.allowWrite == true };"#,
        ),
        (
            "permit_deletes",
            "Permit delete operations (when enabled)",
            r#"permit(principal, action == CodeMode::Action::"Delete", resource) when { resource.allowDelete == true };"#,
        ),
        (
            "forbid_sensitive_paths",
            "Block scripts accessing sensitive paths",
            r#"forbid(principal, action, resource) when { principal.accessesSensitivePath == true };"#,
        ),
        (
            "forbid_unbounded_loops",
            "Block scripts with unbounded loops",
            r#"forbid(principal, action, resource) when { principal.hasUnboundedLoop == true };"#,
        ),
        (
            "forbid_excessive_api_calls",
            "Enforce API call limit",
            r#"forbid(principal, action, resource) when { principal.totalApiCalls > resource.maxApiCalls };"#,
        ),
        (
            "forbid_excessive_nesting",
            "Enforce nesting depth limit",
            r#"forbid(principal, action, resource) when { principal.nestingDepth > resource.maxNestingDepth };"#,
        ),
        (
            "forbid_output_blocked_fields",
            "Block scripts that return output-blocked fields",
            r#"forbid(principal, action, resource) when { principal.outputFields.containsAny(resource.outputBlockedFields) };"#,
        ),
        (
            "forbid_spread_without_declaration",
            "Block scripts with spread in output when output declaration is required",
            r#"forbid(principal, action, resource) when { principal.hasSpreadInOutput == true && resource.requireOutputDeclaration == true };"#,
        ),
        (
            "forbid_missing_output_declaration",
            "Block scripts without output declaration when required",
            r#"forbid(principal, action, resource) when { principal.hasOutputDeclaration == false && resource.requireOutputDeclaration == true };"#,
        ),
    ]
}

/// Get the baseline Cedar policies.
pub fn get_baseline_policies() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "permit_reads",
            "Permit all read operations (queries)",
            r#"permit(principal, action == CodeMode::Action::"Read", resource);"#,
        ),
        (
            "permit_writes",
            "Permit write operations (when enabled)",
            r#"permit(principal, action == CodeMode::Action::"Write", resource) when { resource.allowWrite == true };"#,
        ),
        (
            "permit_deletes",
            "Permit delete operations (when enabled)",
            r#"permit(principal, action == CodeMode::Action::"Delete", resource) when { resource.allowDelete == true };"#,
        ),
        (
            "permit_admin",
            "Permit admin operations (when enabled)",
            r#"permit(principal, action == CodeMode::Action::"Admin", resource) when { resource.allowAdmin == true };"#,
        ),
        (
            "forbid_blocked_operations",
            "Block operations in blocklist",
            r#"forbid(principal, action, resource) when { resource.blockedOperations.contains(principal.operationName) };"#,
        ),
        (
            "forbid_blocked_fields",
            "Block access to blocked fields",
            r#"forbid(principal, action, resource) when { resource.blockedFields.containsAny(principal.accessedFields) };"#,
        ),
        (
            "forbid_excessive_depth",
            "Enforce maximum query depth",
            r#"forbid(principal, action, resource) when { principal.depth > resource.maxDepth };"#,
        ),
        (
            "forbid_excessive_cost",
            "Enforce maximum query cost",
            r#"forbid(principal, action, resource) when { principal.estimatedCost > resource.maxCost };"#,
        ),
    ]
}

/// Get the Cedar schema for SQL Code Mode in JSON format.
///
/// Matches `SQL_CEDAR_SCHEMA` in `cedar_validation.rs`. A schema-sync test
/// in `cedar_validation.rs` enforces this stays aligned.
#[cfg(feature = "sql-code-mode")]
pub fn get_sql_code_mode_schema_json() -> serde_json::Value {
    let applies_to = serde_json::json!({
        "principalTypes": ["Statement"],
        "resourceTypes": ["Server"],
        "context": {
            "type": "Record",
            "attributes": {
                "serverId": { "type": "String", "required": true },
                "serverType": { "type": "String", "required": true },
                "userId": { "type": "String", "required": false },
                "sessionId": { "type": "String", "required": false }
            }
        }
    });

    serde_json::json!({
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
                "Read": { "appliesTo": applies_to },
                "Write": { "appliesTo": applies_to },
                "Delete": { "appliesTo": applies_to },
                "Admin": { "appliesTo": applies_to }
            }
        }
    })
}

/// Get baseline Cedar policies for SQL Code Mode.
#[cfg(feature = "sql-code-mode")]
pub fn get_sql_baseline_policies() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "permit_reads",
            "Permit all SELECT statements",
            r#"permit(principal, action == CodeMode::Action::"Read", resource);"#,
        ),
        (
            "permit_writes",
            "Permit INSERT/UPDATE when enabled",
            r#"permit(principal, action == CodeMode::Action::"Write", resource) when { resource.allowWrite == true };"#,
        ),
        (
            "permit_deletes",
            "Permit DELETE when enabled",
            r#"permit(principal, action == CodeMode::Action::"Delete", resource) when { resource.allowDelete == true };"#,
        ),
        (
            "permit_admin",
            "Permit DDL when enabled",
            r#"permit(principal, action == CodeMode::Action::"Admin", resource) when { resource.allowAdmin == true };"#,
        ),
        (
            "forbid_blocked_tables",
            "Block statements touching blocked tables",
            r#"forbid(principal, action, resource) when { principal.tables.containsAny(resource.blockedTables) };"#,
        ),
        (
            "forbid_blocked_columns",
            "Block statements touching blocked columns",
            r#"forbid(principal, action, resource) when { principal.columns.containsAny(resource.blockedColumns) };"#,
        ),
        (
            "forbid_excessive_rows",
            "Enforce row-count limit",
            r#"forbid(principal, action, resource) when { principal.estimatedRows > resource.maxRows };"#,
        ),
        (
            "forbid_excessive_joins",
            "Enforce JOIN-count limit",
            r#"forbid(principal, action, resource) when { principal.joinCount > resource.maxJoins };"#,
        ),
    ]
}

#[cfg(all(test, feature = "openapi-code-mode"))]
mod tests {
    use super::*;
    use crate::config::{OperationEntry, OperationRegistry};
    use crate::javascript::{ApiCall, HttpMethod, JavaScriptCodeInfo};

    fn make_api_call(method: HttpMethod, path: &str) -> ApiCall {
        ApiCall {
            method,
            path: path.to_string(),
            is_dynamic_path: false,
            line: 1,
            column: 0,
        }
    }

    fn make_info(calls: Vec<ApiCall>) -> JavaScriptCodeInfo {
        JavaScriptCodeInfo {
            api_calls: calls,
            ..Default::default()
        }
    }

    fn make_registry(entries: &[(&str, &str, &str)]) -> OperationRegistry {
        let entries: Vec<OperationEntry> = entries
            .iter()
            .map(|(id, category, path)| OperationEntry {
                id: id.to_string(),
                category: category.to_string(),
                description: String::new(),
                path: Some(path.to_string()),
            })
            .collect();
        OperationRegistry::from_entries(&entries)
    }

    #[test]
    fn test_category_read_overrides_post_method() {
        let registry = make_registry(&[("getCostAnomalies", "read", "/getCostAnomalies")]);
        let info = make_info(vec![make_api_call(HttpMethod::Post, "/getCostAnomalies")]);

        let entity = ScriptEntity::from_javascript_info(&info, &[], Some(&registry));

        assert_eq!(entity.read_calls, 1);
        assert_eq!(entity.write_calls, 0);
        assert_eq!(entity.script_type, "read_only");
        assert_eq!(entity.action(), "Read");
    }

    #[test]
    fn test_category_write_overrides_get_method() {
        let registry = make_registry(&[("triggerExport", "write", "/triggerExport")]);
        let info = make_info(vec![make_api_call(HttpMethod::Get, "/triggerExport")]);

        let entity = ScriptEntity::from_javascript_info(&info, &[], Some(&registry));

        assert_eq!(entity.write_calls, 1);
        assert_eq!(entity.read_calls, 0);
        assert_eq!(entity.script_type, "write_only");
        assert_eq!(entity.action(), "Write");
    }

    #[test]
    fn test_category_delete_routes_correctly() {
        let registry = make_registry(&[("deleteReservation", "delete", "/deleteReservation")]);
        let info = make_info(vec![make_api_call(HttpMethod::Post, "/deleteReservation")]);

        let entity = ScriptEntity::from_javascript_info(&info, &[], Some(&registry));

        assert_eq!(entity.delete_calls, 1);
        assert!(entity.has_deletes);
        assert_eq!(entity.action(), "Delete");
    }

    #[test]
    fn test_no_registry_falls_back_to_http_method() {
        let info = make_info(vec![
            make_api_call(HttpMethod::Get, "/getCostAnomalies"),
            make_api_call(HttpMethod::Post, "/updateBudget"),
        ]);

        let entity = ScriptEntity::from_javascript_info(&info, &[], None);

        assert_eq!(entity.read_calls, 1);
        assert_eq!(entity.write_calls, 1);
        assert_eq!(entity.script_type, "mixed");
        assert_eq!(entity.action(), "Write");
    }

    #[test]
    fn test_unregistered_path_falls_back_to_http_method() {
        let registry = make_registry(&[("getCostAnomalies", "read", "/getCostAnomalies")]);
        let info = make_info(vec![make_api_call(HttpMethod::Post, "/unknownEndpoint")]);

        let entity = ScriptEntity::from_javascript_info(&info, &[], Some(&registry));

        // POST with no category → write (HTTP method fallback)
        assert_eq!(entity.write_calls, 1);
        assert_eq!(entity.read_calls, 0);
        assert_eq!(entity.script_type, "write_only");
    }

    #[test]
    fn test_mixed_categories_produce_mixed_script() {
        let registry = make_registry(&[
            ("getCostAnomalies", "read", "/getCostAnomalies"),
            ("updateBudget", "write", "/updateBudget"),
        ]);
        let info = make_info(vec![
            make_api_call(HttpMethod::Post, "/getCostAnomalies"),
            make_api_call(HttpMethod::Post, "/updateBudget"),
        ]);

        let entity = ScriptEntity::from_javascript_info(&info, &[], Some(&registry));

        assert_eq!(entity.read_calls, 1);
        assert_eq!(entity.write_calls, 1);
        assert_eq!(entity.script_type, "mixed");
        assert_eq!(entity.action(), "Write");
    }

    #[test]
    fn test_empty_category_falls_back_to_http_method() {
        // category = "" (from #[serde(default)]) → no category → HTTP method fallback
        let registry = make_registry(&[("legacyOp", "", "/legacyOp")]);
        let info = make_info(vec![make_api_call(HttpMethod::Post, "/legacyOp")]);

        let entity = ScriptEntity::from_javascript_info(&info, &[], Some(&registry));

        // POST with empty category → write (HTTP method fallback)
        assert_eq!(entity.write_calls, 1);
        assert_eq!(entity.script_type, "write_only");
    }
}
