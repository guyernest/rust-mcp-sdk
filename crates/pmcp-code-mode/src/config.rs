//! Code Mode configuration.

use crate::types::{RiskLevel, ValidationError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

/// Resolve a `server_id` from environment variables.
///
/// Checks, in order:
/// 1. `PMCP_SERVER_ID`
/// 2. `AWS_LAMBDA_FUNCTION_NAME` (Lambda runtime)
///
/// Returns `None` if neither is set. Empty strings are treated as unset.
///
/// This is the same resolution chain used by
/// [`CodeModeConfig::resolve_server_id`] — exposed as a free function so tests
/// and non-pipeline code can share it.
pub fn resolve_server_id_from_env() -> Option<String> {
    let candidate = std::env::var("PMCP_SERVER_ID")
        .ok()
        .or_else(|| std::env::var("AWS_LAMBDA_FUNCTION_NAME").ok())?;
    if candidate.is_empty() {
        None
    } else {
        Some(candidate)
    }
}

/// A single declared operation in Code Mode configuration.
/// Maps a raw API path to a canonical plain-name ID for Cedar policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationEntry {
    /// Canonical operation ID (plain name, no method prefix).
    /// This is what appears in Cedar policy calledOperations.
    pub id: String,

    /// Action category for AVP action routing.
    /// Values: "read", "write", "delete", "admin"
    pub category: String,

    /// Human-readable description for admin UI and LLM context.
    #[serde(default)]
    pub description: String,

    /// Raw API path this ID maps to (e.g., "/getCostAnomalies").
    /// Used to match against api_call.path from JavaScript analysis.
    #[serde(default)]
    pub path: Option<String>,
}

/// Registry built from [[code_mode.operations]] config entries.
/// Maps raw paths to canonical operation IDs and categories.
#[derive(Debug, Clone, Default)]
pub struct OperationRegistry {
    path_to_id: HashMap<String, String>,
    path_to_category: HashMap<String, String>,
}

impl OperationRegistry {
    pub fn from_entries(entries: &[OperationEntry]) -> Self {
        let mut path_to_id = HashMap::with_capacity(entries.len());
        let mut path_to_category = HashMap::with_capacity(entries.len());
        for entry in entries {
            if let Some(ref path) = entry.path {
                path_to_id.insert(path.clone(), entry.id.clone());
                if !entry.category.is_empty() {
                    path_to_category.insert(path.clone(), entry.category.clone());
                }
            }
        }
        Self {
            path_to_id,
            path_to_category,
        }
    }

    pub fn lookup(&self, path: &str) -> Option<&str> {
        self.path_to_id.get(path).map(|s| s.as_str())
    }

    /// Look up the declared category for a path (e.g., "read", "write", "delete", "admin").
    /// Returns `None` if the path has no registry entry or no category declared.
    pub fn lookup_category(&self, path: &str) -> Option<&str> {
        self.path_to_category.get(path).map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.path_to_id.is_empty()
    }
}

/// Configuration for Code Mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeModeConfig {
    /// Whether Code Mode is enabled for this server
    #[serde(default)]
    pub enabled: bool,

    // ========================================================================
    // GraphQL-specific settings
    // ========================================================================
    /// Whether to allow mutations (MVP: false)
    #[serde(default)]
    pub allow_mutations: bool,

    /// Allowed mutation names (whitelist). If empty and allow_mutations=true, all are allowed.
    #[serde(default)]
    pub allowed_mutations: HashSet<String>,

    /// Blocked mutation names (blacklist). Always blocked even if allow_mutations=true.
    #[serde(default)]
    pub blocked_mutations: HashSet<String>,

    /// Whether to allow introspection queries
    #[serde(default)]
    pub allow_introspection: bool,

    /// Fields that should never be returned (Type.field format) - GraphQL
    #[serde(default)]
    pub blocked_fields: HashSet<String>,

    /// Allowed query names (whitelist). If empty and mode is allowlist, none are allowed.
    #[serde(default)]
    pub allowed_queries: HashSet<String>,

    /// Blocked query names (blocklist). Always blocked even if reads enabled.
    #[serde(default)]
    pub blocked_queries: HashSet<String>,

    // ========================================================================
    // OpenAPI-specific settings
    // ========================================================================
    /// Whether read operations (GET) are enabled (default: true)
    #[serde(default = "default_true")]
    pub openapi_reads_enabled: bool,

    /// Whether write operations (POST, PUT, PATCH) are allowed globally
    #[serde(default)]
    pub openapi_allow_writes: bool,

    /// Allowed write operations (operationId or "METHOD /path")
    #[serde(default)]
    pub openapi_allowed_writes: HashSet<String>,

    /// Blocked write operations
    #[serde(default)]
    pub openapi_blocked_writes: HashSet<String>,

    /// Whether delete operations (DELETE) are allowed globally
    #[serde(default)]
    pub openapi_allow_deletes: bool,

    /// Allowed delete operations (operationId or "METHOD /path")
    #[serde(default)]
    pub openapi_allowed_deletes: HashSet<String>,

    /// Blocked paths (glob patterns like "/admin/*")
    #[serde(default)]
    pub openapi_blocked_paths: HashSet<String>,

    /// Fields that are stripped from API responses entirely (no access)
    #[serde(default)]
    pub openapi_internal_blocked_fields: HashSet<String>,

    /// Fields that can be used internally but not in script output
    #[serde(default)]
    pub openapi_output_blocked_fields: HashSet<String>,

    /// Whether scripts must declare their return type with @returns
    #[serde(default)]
    pub openapi_require_output_declaration: bool,

    // ========================================================================
    // SQL-specific settings
    //
    // SQL fields accept both their prefixed name (`sql_allow_writes`) and the
    // unprefixed natural form (`allow_writes`). Downstream SQL servers can use
    // the unprefixed names in their `[code_mode]` block without a manual
    // conversion layer:
    //
    //     [code_mode]
    //     reads_enabled = true    # same as sql_reads_enabled
    //     allow_writes = false    # same as sql_allow_writes
    //     blocked_tables = ["secrets"]
    //     max_rows = 5000
    // ========================================================================
    /// Whether SELECT statements are enabled (default: true).
    #[serde(default = "default_true", alias = "reads_enabled")]
    pub sql_reads_enabled: bool,

    /// Whether INSERT/UPDATE/MERGE statements are allowed globally.
    #[serde(default, alias = "allow_writes")]
    pub sql_allow_writes: bool,

    /// Whether DELETE/TRUNCATE statements are allowed globally.
    #[serde(default, alias = "allow_deletes")]
    pub sql_allow_deletes: bool,

    /// Whether DDL (CREATE/ALTER/DROP/GRANT/REVOKE) is allowed globally.
    /// Default is `false` — DDL is almost never appropriate for LLM-generated code.
    #[serde(default, alias = "allow_ddl")]
    pub sql_allow_ddl: bool,

    /// Allowed statement types ("SELECT"/"INSERT"/"UPDATE"/"DELETE"/"DDL").
    /// If non-empty, only statement types in this set are allowed.
    #[serde(default, alias = "allowed_statements")]
    pub sql_allowed_statements: HashSet<String>,

    /// Blocked statement types. Always blocked even if globally allowed.
    #[serde(default, alias = "blocked_statements")]
    pub sql_blocked_statements: HashSet<String>,

    /// Tables that are always forbidden (blocklist mode).
    #[serde(default, alias = "blocked_tables")]
    pub sql_blocked_tables: HashSet<String>,

    /// If non-empty, only these tables can be accessed (allowlist mode).
    #[serde(default, alias = "allowed_tables")]
    pub sql_allowed_tables: HashSet<String>,

    /// Columns that may not be referenced in any statement (e.g., `password`, `ssn`).
    #[serde(default, alias = "blocked_columns")]
    pub sql_blocked_columns: HashSet<String>,

    /// Maximum row-count estimate allowed (based on LIMIT or default estimate).
    #[serde(default = "default_sql_max_rows", alias = "max_rows")]
    pub sql_max_rows: u64,

    /// Maximum number of JOINs in a single statement.
    #[serde(default = "default_sql_max_joins", alias = "max_joins")]
    pub sql_max_joins: u32,

    /// Whether to require a WHERE clause for UPDATE/DELETE statements.
    #[serde(default = "default_true", alias = "require_where_on_writes")]
    pub sql_require_where_on_writes: bool,

    // ========================================================================
    // Common settings
    // ========================================================================
    /// Action tags to override inferred actions for specific operations.
    #[serde(default)]
    pub action_tags: HashMap<String, String>,

    /// Maximum query depth
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,

    /// Maximum field count per query
    #[serde(default = "default_max_field_count")]
    pub max_field_count: u32,

    /// Maximum estimated query cost
    #[serde(default = "default_max_cost")]
    pub max_cost: u32,

    /// Allowed sensitive data categories
    #[serde(default)]
    pub allowed_sensitive_categories: HashSet<String>,

    /// Token time-to-live in seconds
    #[serde(default = "default_token_ttl")]
    pub token_ttl_seconds: i64,

    /// Risk levels that can be auto-approved without human confirmation
    #[serde(default = "default_auto_approve_levels")]
    pub auto_approve_levels: Vec<RiskLevel>,

    /// Maximum query length in characters
    #[serde(default = "default_max_query_length")]
    pub max_query_length: usize,

    /// Maximum result rows to return
    #[serde(default = "default_max_result_rows")]
    pub max_result_rows: usize,

    /// Query execution timeout in seconds
    #[serde(default = "default_query_timeout")]
    pub query_timeout_seconds: u32,

    /// Server ID for token generation
    #[serde(default)]
    pub server_id: Option<String>,

    // ========================================================================
    // SDK-backed settings
    // ========================================================================
    /// Allowed SDK operation names for SDK-backed Code Mode.
    /// When non-empty, Code Mode uses SDK dispatch instead of HTTP.
    /// Operations are validated at compile time — unlisted names are rejected.
    #[serde(default)]
    pub sdk_operations: HashSet<String>,

    /// Declared operations for plain-name ID mapping in Cedar entities.
    /// Parsed from [[code_mode.operations]] TOML sections.
    /// When non-empty, ScriptEntity calledOperations uses IDs from the registry
    /// built from these entries. Unregistered paths fall back to METHOD:/path.
    #[serde(default)]
    pub operations: Vec<OperationEntry>,
}

impl Default for CodeModeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            // GraphQL
            allow_mutations: false,
            allowed_mutations: HashSet::new(),
            blocked_mutations: HashSet::new(),
            allow_introspection: false,
            blocked_fields: HashSet::new(),
            allowed_queries: HashSet::new(),
            blocked_queries: HashSet::new(),
            // OpenAPI
            openapi_reads_enabled: true,
            openapi_allow_writes: false,
            openapi_allowed_writes: HashSet::new(),
            openapi_blocked_writes: HashSet::new(),
            openapi_allow_deletes: false,
            openapi_allowed_deletes: HashSet::new(),
            openapi_blocked_paths: HashSet::new(),
            openapi_internal_blocked_fields: HashSet::new(),
            openapi_output_blocked_fields: HashSet::new(),
            openapi_require_output_declaration: false,
            // SQL
            sql_reads_enabled: true,
            sql_allow_writes: false,
            sql_allow_deletes: false,
            sql_allow_ddl: false,
            sql_allowed_statements: HashSet::new(),
            sql_blocked_statements: HashSet::new(),
            sql_blocked_tables: HashSet::new(),
            sql_allowed_tables: HashSet::new(),
            sql_blocked_columns: HashSet::new(),
            sql_max_rows: default_sql_max_rows(),
            sql_max_joins: default_sql_max_joins(),
            sql_require_where_on_writes: true,
            // Common
            action_tags: HashMap::new(),
            max_depth: default_max_depth(),
            max_field_count: default_max_field_count(),
            max_cost: default_max_cost(),
            allowed_sensitive_categories: HashSet::new(),
            token_ttl_seconds: default_token_ttl(),
            auto_approve_levels: default_auto_approve_levels(),
            max_query_length: default_max_query_length(),
            max_result_rows: default_max_result_rows(),
            query_timeout_seconds: default_query_timeout(),
            server_id: None,
            // SDK
            sdk_operations: HashSet::new(),
            operations: Vec::new(),
        }
    }
}

/// Wrapper for deserializing the `[code_mode]` section from a full TOML config file.
/// The file may contain other sections (`[server]`, `[[tools]]`, etc.) which are ignored.
#[derive(Deserialize)]
struct TomlWrapper {
    #[serde(default)]
    code_mode: CodeModeConfig,
}

impl CodeModeConfig {
    /// Parse `CodeModeConfig` from a full TOML config string.
    ///
    /// Extracts the `[code_mode]` section (including `[[code_mode.operations]]`)
    /// and ignores all other sections. This is the recommended way for external
    /// servers to build their config from `config.toml`:
    ///
    /// ```rust,ignore
    /// const CONFIG_TOML: &str = include_str!("../../config.toml");
    ///
    /// let config = CodeModeConfig::from_toml(CONFIG_TOML)
    ///     .expect("Invalid code_mode section in config.toml");
    /// ```
    ///
    /// If the TOML has no `[code_mode]` section, returns `CodeModeConfig::default()`.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        let wrapper: TomlWrapper = toml::from_str(toml_str)?;
        Ok(wrapper.code_mode)
    }

    /// Create a new config with Code Mode enabled.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Returns true if this config enables SDK-backed Code Mode.
    pub fn is_sdk_mode(&self) -> bool {
        !self.sdk_operations.is_empty()
    }

    /// Check if a risk level should be auto-approved.
    pub fn should_auto_approve(&self, risk_level: RiskLevel) -> bool {
        self.auto_approve_levels.contains(&risk_level)
    }

    /// Get the server ID, falling back to a default.
    ///
    /// **Note:** The `"unknown"` fallback produces silent AVP default-deny failures
    /// (no Cedar policy matches a server_id of `"unknown"`). Prefer
    /// [`resolve_server_id`](Self::resolve_server_id) to auto-fill from environment,
    /// or [`require_server_id`](Self::require_server_id) to fail fast.
    pub fn server_id(&self) -> &str {
        self.server_id.as_deref().unwrap_or("unknown")
    }

    /// Auto-resolve `server_id` from environment if not already set.
    ///
    /// Resolution order:
    /// 1. `self.server_id` (if already set, e.g., from TOML) — no change
    /// 2. `PMCP_SERVER_ID` env var
    /// 3. `AWS_LAMBDA_FUNCTION_NAME` env var (Lambda runtime)
    /// 4. Left as `None` — caller is responsible for handling
    ///
    /// [`ValidationPipeline`](crate::ValidationPipeline) constructors call this
    /// automatically, so wrappers rarely need to invoke it directly.
    pub fn resolve_server_id(&mut self) {
        if self.server_id.is_some() {
            return;
        }
        self.server_id = resolve_server_id_from_env();
    }

    /// Return the `server_id`, or an error if not resolved.
    ///
    /// Use this in production code paths that require AVP authorization —
    /// it fails fast with a clear message instead of letting `"unknown"`
    /// reach AVP and produce a silent default-deny.
    pub fn require_server_id(&self) -> Result<&str, ValidationError> {
        self.server_id.as_deref().ok_or_else(|| {
            ValidationError::ConfigError(
                "server_id is not set. Set it in config.toml, PMCP_SERVER_ID env var, \
                 or AWS_LAMBDA_FUNCTION_NAME (Lambda). Without it, AVP authorization \
                 will default-deny silently."
                    .into(),
            )
        })
    }

    /// Convert to ServerConfigEntity for policy evaluation.
    pub fn to_server_config_entity(&self) -> crate::policy::ServerConfigEntity {
        crate::policy::ServerConfigEntity {
            server_id: self.server_id().to_string(),
            server_type: "graphql".to_string(),
            allow_write: self.allow_mutations,
            allow_delete: self.allow_mutations,
            allow_admin: self.allow_introspection,
            allowed_operations: self.allowed_mutations.clone(),
            blocked_operations: self.blocked_mutations.clone(),
            max_depth: self.max_depth,
            max_field_count: self.max_field_count,
            max_cost: self.max_cost,
            max_api_calls: 50,
            blocked_fields: self.blocked_fields.clone(),
            allowed_sensitive_categories: self.allowed_sensitive_categories.clone(),
        }
    }

    /// Convert to OpenAPIServerEntity for policy evaluation (OpenAPI Code Mode).
    #[cfg(feature = "openapi-code-mode")]
    pub fn to_openapi_server_entity(&self) -> crate::policy::OpenAPIServerEntity {
        let mut allowed_operations = self.openapi_allowed_writes.clone();
        allowed_operations.extend(self.openapi_allowed_deletes.clone());

        let write_mode = if !self.openapi_allow_writes {
            "deny_all"
        } else if !self.openapi_allowed_writes.is_empty() {
            "allowlist"
        } else if !self.openapi_blocked_writes.is_empty() {
            "blocklist"
        } else {
            "allow_all"
        };

        crate::policy::OpenAPIServerEntity {
            server_id: self.server_id().to_string(),
            server_type: "openapi".to_string(),
            allow_write: self.openapi_allow_writes,
            allow_delete: self.openapi_allow_deletes,
            allow_admin: false,
            write_mode: write_mode.to_string(),
            max_depth: self.max_depth,
            max_cost: self.max_cost,
            max_api_calls: 50,
            max_loop_iterations: 100,
            max_script_length: self.max_query_length as u32,
            max_nesting_depth: self.max_depth,
            execution_timeout_seconds: self.query_timeout_seconds,
            allowed_operations,
            blocked_operations: self.openapi_blocked_writes.clone(),
            allowed_methods: HashSet::new(),
            blocked_methods: HashSet::new(),
            allowed_path_patterns: HashSet::new(),
            blocked_path_patterns: self.openapi_blocked_paths.clone(),
            sensitive_path_patterns: self.openapi_blocked_paths.clone(),
            auto_approve_read_only: self.openapi_reads_enabled,
            max_api_calls_for_auto_approve: 10,
            internal_blocked_fields: self.openapi_internal_blocked_fields.clone(),
            output_blocked_fields: self.openapi_output_blocked_fields.clone(),
            require_output_declaration: self.openapi_require_output_declaration,
        }
    }

    /// Convert to `SqlServerEntity` for policy evaluation (SQL Code Mode).
    #[cfg(feature = "sql-code-mode")]
    pub fn to_sql_server_entity(&self) -> crate::policy::SqlServerEntity {
        crate::policy::SqlServerEntity {
            server_id: self.server_id().to_string(),
            server_type: "sql".to_string(),
            allow_write: self.sql_allow_writes,
            allow_delete: self.sql_allow_deletes,
            allow_admin: self.sql_allow_ddl,
            max_rows: self.sql_max_rows,
            max_joins: self.sql_max_joins,
            allowed_operations: self.sql_allowed_statements.clone(),
            blocked_operations: self.sql_blocked_statements.clone(),
            blocked_tables: self.sql_blocked_tables.clone(),
            blocked_columns: self.sql_blocked_columns.clone(),
            allowed_tables: self.sql_allowed_tables.clone(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_token_ttl() -> i64 {
    300 // 5 minutes
}

fn default_auto_approve_levels() -> Vec<RiskLevel> {
    vec![RiskLevel::Low]
}

fn default_max_query_length() -> usize {
    10000
}

fn default_max_result_rows() -> usize {
    10000
}

fn default_query_timeout() -> u32 {
    30
}

fn default_max_depth() -> u32 {
    10
}

fn default_max_field_count() -> u32 {
    100
}

fn default_max_cost() -> u32 {
    1000
}

fn default_sql_max_rows() -> u64 {
    10_000
}

fn default_sql_max_joins() -> u32 {
    5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CodeModeConfig::default();
        assert!(!config.enabled);
        assert!(!config.allow_mutations);
        assert_eq!(config.token_ttl_seconds, 300);
        assert_eq!(config.auto_approve_levels, vec![RiskLevel::Low]);
    }

    #[test]
    fn test_enabled_config() {
        let config = CodeModeConfig::enabled();
        assert!(config.enabled);
    }

    #[test]
    fn test_auto_approve() {
        let config = CodeModeConfig::default();
        assert!(config.should_auto_approve(RiskLevel::Low));
        assert!(!config.should_auto_approve(RiskLevel::Medium));
        assert!(!config.should_auto_approve(RiskLevel::High));
        assert!(!config.should_auto_approve(RiskLevel::Critical));
    }

    #[test]
    fn test_operation_registry_from_entries() {
        let entries = vec![
            OperationEntry {
                id: "getCostAnomalies".to_string(),
                category: "read".to_string(),
                description: "Get cost anomalies".to_string(),
                path: Some("/getCostAnomalies".to_string()),
            },
            OperationEntry {
                id: "listInstances".to_string(),
                category: "read".to_string(),
                description: "List EC2 instances".to_string(),
                path: Some("/listInstances".to_string()),
            },
        ];
        let registry = OperationRegistry::from_entries(&entries);
        assert_eq!(
            registry.lookup("/getCostAnomalies"),
            Some("getCostAnomalies")
        );
        assert_eq!(registry.lookup("/listInstances"), Some("listInstances"));
    }

    #[test]
    fn test_operation_registry_lookup_unregistered() {
        let entries = vec![OperationEntry {
            id: "getCostAnomalies".to_string(),
            category: "read".to_string(),
            description: String::new(),
            path: Some("/getCostAnomalies".to_string()),
        }];
        let registry = OperationRegistry::from_entries(&entries);
        assert_eq!(registry.lookup("/unknownPath"), None);
        assert_eq!(registry.lookup(""), None);
    }

    #[test]
    fn test_operation_registry_lookup_category() {
        let entries = vec![
            OperationEntry {
                id: "getCostAnomalies".to_string(),
                category: "read".to_string(),
                description: String::new(),
                path: Some("/getCostAnomalies".to_string()),
            },
            OperationEntry {
                id: "deleteReservation".to_string(),
                category: "delete".to_string(),
                description: String::new(),
                path: Some("/deleteReservation".to_string()),
            },
            OperationEntry {
                id: "updateBudget".to_string(),
                category: "write".to_string(),
                description: String::new(),
                path: Some("/updateBudget".to_string()),
            },
        ];
        let registry = OperationRegistry::from_entries(&entries);
        assert_eq!(registry.lookup_category("/getCostAnomalies"), Some("read"));
        assert_eq!(
            registry.lookup_category("/deleteReservation"),
            Some("delete")
        );
        assert_eq!(registry.lookup_category("/updateBudget"), Some("write"));
        assert_eq!(registry.lookup_category("/unknownPath"), None);
    }

    #[test]
    fn test_operation_registry_empty_category_excluded() {
        let entries = vec![OperationEntry {
            id: "legacyOp".to_string(),
            category: String::new(), // empty = not declared
            description: String::new(),
            path: Some("/legacyOp".to_string()),
        }];
        let registry = OperationRegistry::from_entries(&entries);
        // ID lookup still works
        assert_eq!(registry.lookup("/legacyOp"), Some("legacyOp"));
        // Category lookup returns None for empty category
        assert_eq!(registry.lookup_category("/legacyOp"), None);
    }

    #[test]
    fn test_operation_registry_is_empty() {
        let empty_registry = OperationRegistry::from_entries(&[]);
        assert!(empty_registry.is_empty());

        let entries = vec![OperationEntry {
            id: "op1".to_string(),
            category: "read".to_string(),
            description: String::new(),
            path: Some("/op1".to_string()),
        }];
        let registry = OperationRegistry::from_entries(&entries);
        assert!(!registry.is_empty());
    }

    #[test]
    fn test_operation_entry_deserialization() {
        let toml_str = r#"
id = "getCostAnomalies"
category = "read"
description = "Get cost anomalies"
path = "/getCostAnomalies"
"#;
        let entry: OperationEntry =
            toml::from_str(toml_str).expect("Failed to deserialize OperationEntry");
        assert_eq!(entry.id, "getCostAnomalies");
        assert_eq!(entry.category, "read");
        assert_eq!(entry.description, "Get cost anomalies");
        assert_eq!(entry.path, Some("/getCostAnomalies".to_string()));
    }

    #[test]
    fn test_code_mode_config_with_operations() {
        let toml_str = r#"
enabled = true

[[operations]]
id = "getCostAnomalies"
category = "read"
description = "Get cost anomalies"
path = "/getCostAnomalies"

[[operations]]
id = "listInstances"
category = "read"
path = "/listInstances"
"#;
        let config: CodeModeConfig = toml::from_str(toml_str).expect("Failed to deserialize");
        assert!(config.enabled);
        assert_eq!(config.operations.len(), 2);
        assert_eq!(config.operations[0].id, "getCostAnomalies");
        assert_eq!(config.operations[1].id, "listInstances");
    }

    #[test]
    fn test_code_mode_config_without_operations_defaults_to_empty() {
        let toml_str = r#"
enabled = true
"#;
        let config: CodeModeConfig = toml::from_str(toml_str).expect("Failed to deserialize");
        assert!(config.enabled);
        assert!(config.operations.is_empty());
    }

    #[test]
    fn test_from_toml_extracts_code_mode_section() {
        let toml_str = r#"
[server]
name = "cost-coach"
type = "openapi-api"

[code_mode]
enabled = true
token_ttl_seconds = 600
server_id = "cost-coach"

[[code_mode.operations]]
id = "getCostAndUsage"
category = "read"
description = "Historical cost and usage data"
path = "/getCostAndUsage"

[[code_mode.operations]]
id = "getCostAnomalies"
category = "read"
description = "Cost anomalies detected by AWS"
path = "/getCostAnomalies"

[[tools]]
name = "some_tool"
"#;
        let config = CodeModeConfig::from_toml(toml_str).expect("Failed to parse");
        assert!(config.enabled);
        assert_eq!(config.token_ttl_seconds, 600);
        assert_eq!(config.server_id, Some("cost-coach".to_string()));
        assert_eq!(config.operations.len(), 2);
        assert_eq!(config.operations[0].id, "getCostAndUsage");
        assert_eq!(config.operations[1].id, "getCostAnomalies");
        assert_eq!(
            config.operations[0].path,
            Some("/getCostAndUsage".to_string())
        );
    }

    #[test]
    fn test_from_toml_missing_code_mode_returns_default() {
        let toml_str = r#"
[server]
name = "some-server"
"#;
        let config = CodeModeConfig::from_toml(toml_str).expect("Failed to parse");
        assert!(!config.enabled);
        assert!(config.operations.is_empty());
        assert_eq!(config.token_ttl_seconds, 300); // default
    }

    // =========================================================================
    // server_id resolution tests
    //
    // These tests mutate process-wide env vars. Cargo parallelizes tests across
    // threads in the same process, so a shared Mutex serializes them — without
    // this, set_var/remove_var in one test would race with another.
    // =========================================================================

    use std::sync::Mutex;
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn acquire() -> Self {
            let lock = ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::env::remove_var("PMCP_SERVER_ID");
            std::env::remove_var("AWS_LAMBDA_FUNCTION_NAME");
            Self { _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var("PMCP_SERVER_ID");
            std::env::remove_var("AWS_LAMBDA_FUNCTION_NAME");
        }
    }

    #[test]
    fn resolve_server_id_from_explicit_config_takes_precedence() {
        let _g = EnvGuard::acquire();
        std::env::set_var("PMCP_SERVER_ID", "from-env");

        let mut config = CodeModeConfig {
            server_id: Some("from-config".to_string()),
            ..Default::default()
        };
        config.resolve_server_id();

        assert_eq!(config.server_id.as_deref(), Some("from-config"));
    }

    #[test]
    fn resolve_server_id_from_pmcp_env() {
        let _g = EnvGuard::acquire();
        std::env::set_var("PMCP_SERVER_ID", "my-server");

        let mut config = CodeModeConfig::default();
        config.resolve_server_id();

        assert_eq!(config.server_id.as_deref(), Some("my-server"));
    }

    #[test]
    fn resolve_server_id_from_lambda_env() {
        let _g = EnvGuard::acquire();
        std::env::set_var("AWS_LAMBDA_FUNCTION_NAME", "my-lambda-fn");

        let mut config = CodeModeConfig::default();
        config.resolve_server_id();

        assert_eq!(config.server_id.as_deref(), Some("my-lambda-fn"));
    }

    #[test]
    fn resolve_server_id_pmcp_wins_over_lambda() {
        let _g = EnvGuard::acquire();
        std::env::set_var("PMCP_SERVER_ID", "explicit");
        std::env::set_var("AWS_LAMBDA_FUNCTION_NAME", "lambda-fn");

        let mut config = CodeModeConfig::default();
        config.resolve_server_id();

        assert_eq!(config.server_id.as_deref(), Some("explicit"));
    }

    #[test]
    fn resolve_server_id_leaves_none_when_unset() {
        let _g = EnvGuard::acquire();
        let mut config = CodeModeConfig::default();
        config.resolve_server_id();
        assert!(config.server_id.is_none());
    }

    #[test]
    fn require_server_id_errors_when_unset() {
        let config = CodeModeConfig::default();
        let result = config.require_server_id();
        assert!(matches!(result, Err(ValidationError::ConfigError(_))));
    }

    #[test]
    fn require_server_id_returns_value_when_set() {
        let config = CodeModeConfig {
            server_id: Some("my-server".to_string()),
            ..Default::default()
        };
        assert_eq!(config.require_server_id().unwrap(), "my-server");
    }

    #[test]
    fn resolve_server_id_from_env_free_fn_treats_empty_as_unset() {
        let _g = EnvGuard::acquire();
        std::env::set_var("PMCP_SERVER_ID", "");
        assert_eq!(resolve_server_id_from_env(), None);
    }

    // =========================================================================
    // SQL TOML DX tests (serde aliases)
    // =========================================================================

    #[test]
    fn sql_config_accepts_unprefixed_toml_names() {
        let toml_str = r#"
enabled = true
allow_writes = true
allow_deletes = true
allow_ddl = true
allowed_tables = ["users", "orders"]
blocked_tables = ["secrets"]
blocked_columns = ["password", "ssn"]
max_rows = 5000
max_joins = 3
require_where_on_writes = false
"#;
        let config: CodeModeConfig =
            toml::from_str(toml_str).expect("Failed to deserialize with unprefixed aliases");

        assert!(config.enabled);
        assert!(config.sql_allow_writes);
        assert!(config.sql_allow_deletes);
        assert!(config.sql_allow_ddl);
        assert!(config.sql_allowed_tables.contains("users"));
        assert!(config.sql_allowed_tables.contains("orders"));
        assert!(config.sql_blocked_tables.contains("secrets"));
        assert!(config.sql_blocked_columns.contains("password"));
        assert_eq!(config.sql_max_rows, 5000);
        assert_eq!(config.sql_max_joins, 3);
        assert!(!config.sql_require_where_on_writes);
    }

    #[test]
    fn sql_config_accepts_prefixed_toml_names() {
        let toml_str = r#"
enabled = true
sql_allow_writes = true
sql_blocked_tables = ["secrets"]
sql_max_rows = 5000
"#;
        let config: CodeModeConfig =
            toml::from_str(toml_str).expect("Failed to deserialize with prefixed names");

        assert!(config.sql_allow_writes);
        assert!(config.sql_blocked_tables.contains("secrets"));
        assert_eq!(config.sql_max_rows, 5000);
    }
}
