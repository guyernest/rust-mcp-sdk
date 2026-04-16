//! Code Mode configuration.

use crate::types::RiskLevel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;

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
    pub fn server_id(&self) -> &str {
        self.server_id.as_deref().unwrap_or("unknown")
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
}
