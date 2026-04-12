//! Schema Exposure Architecture for MCP Built-in Servers.
//!
//! This module implements the Three-Layer Schema Model:
//! - Layer 1: Source Schema (original API specification)
//! - Layer 2: Exposure Policies (what gets exposed)
//! - Layer 3: Derived Schemas (computed views)
//!
//! See SCHEMA_EXPOSURE_ARCHITECTURE.md for full design documentation.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// Re-export GraphQLOperationType from the existing graphql module
pub use crate::graphql::GraphQLOperationType;

// ============================================================================
// LAYER 1: SOURCE SCHEMA
// ============================================================================

/// Schema format identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaFormat {
    /// OpenAPI 3.x REST APIs
    OpenAPI3,
    /// GraphQL APIs
    GraphQL,
    /// SQL databases (future)
    Sql,
    /// AsyncAPI event-driven APIs (future)
    AsyncAPI,
}

/// Where the schema came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SchemaSource {
    /// Schema embedded in config file
    Embedded { path: String },
    /// Schema fetched from remote URL
    Remote {
        url: String,
        #[serde(default)]
        refresh_interval_seconds: Option<u64>,
    },
    /// Schema discovered via introspection
    Introspection { endpoint: String },
}

/// Metadata about the source schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaMetadata {
    /// Where the schema came from
    pub source: SchemaSource,

    /// When it was last fetched/updated (Unix timestamp)
    #[serde(default)]
    pub last_updated: Option<i64>,

    /// Content hash for change detection (SHA-256)
    #[serde(default)]
    pub content_hash: Option<String>,

    /// Schema version (if available from spec)
    #[serde(default)]
    pub version: Option<String>,

    /// Title from the spec
    #[serde(default)]
    pub title: Option<String>,
}

/// Operation category for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationCategory {
    /// Read operations (GET, Query, SELECT)
    Read,
    /// Create operations (POST create, Mutation create, INSERT)
    Create,
    /// Update operations (PUT/PATCH, Mutation update, UPDATE)
    Update,
    /// Delete operations (DELETE, Mutation delete, DELETE)
    Delete,
    /// Administrative operations
    Admin,
    /// Internal/debug operations
    Internal,
}

impl OperationCategory {
    /// Returns true if this is a read-only category.
    pub fn is_read_only(&self) -> bool {
        matches!(self, OperationCategory::Read)
    }

    /// Returns true if this is a write category (create or update).
    pub fn is_write(&self) -> bool {
        matches!(self, OperationCategory::Create | OperationCategory::Update)
    }

    /// Returns true if this is a delete category.
    pub fn is_delete(&self) -> bool {
        matches!(self, OperationCategory::Delete)
    }
}

/// Risk level for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationRiskLevel {
    /// Read-only, no side effects
    Safe,
    /// Creates data, generally reversible
    Low,
    /// Modifies data, potentially reversible
    Medium,
    /// Deletes data, difficult to reverse
    High,
    /// System-wide impact, irreversible
    Critical,
}

impl Default for OperationRiskLevel {
    fn default() -> Self {
        Self::Medium
    }
}

/// Normalized operation model that works across all schema formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Unique identifier for this operation.
    /// - OpenAPI: operationId or "{method} {path}"
    /// - GraphQL: "{Type}.{field}" (e.g., "Query.users", "Mutation.createUser")
    /// - SQL: "{action}_{table}" (e.g., "select_users", "insert_orders")
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Description of what the operation does.
    #[serde(default)]
    pub description: Option<String>,

    /// Operation category.
    pub category: OperationCategory,

    /// Whether this is a read-only operation.
    pub is_read_only: bool,

    /// Risk level for UI hints.
    #[serde(default)]
    pub risk_level: OperationRiskLevel,

    /// Tags/categories for grouping.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Format-specific details.
    pub details: OperationDetails,
}

impl Operation {
    /// Create a new operation with minimal required fields.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        category: OperationCategory,
    ) -> Self {
        let is_read_only = category.is_read_only();
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            category,
            is_read_only,
            risk_level: if is_read_only {
                OperationRiskLevel::Safe
            } else if category.is_delete() {
                OperationRiskLevel::High
            } else {
                OperationRiskLevel::Low
            },
            tags: Vec::new(),
            details: OperationDetails::Unknown,
        }
    }

    /// Check if this operation matches a pattern.
    /// Patterns support glob-style wildcards: * (any characters)
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        match &self.details {
            OperationDetails::OpenAPI { method, path, .. } => {
                // Pattern format: "METHOD /path/*" or "* /path/*"
                let endpoint = format!("{} {}", method.to_uppercase(), path);
                pattern_matches(pattern, &endpoint) || pattern_matches(pattern, &self.id)
            }
            OperationDetails::GraphQL {
                operation_type,
                field_name,
                ..
            } => {
                // Pattern format: "Type.field*" or "*.field"
                let full_name = format!("{:?}.{}", operation_type, field_name);
                pattern_matches(pattern, &full_name) || pattern_matches(pattern, &self.id)
            }
            OperationDetails::Sql {
                statement_type,
                table,
                ..
            } => {
                // Pattern format: "action table" or "* table"
                let full_name = format!("{:?} {}", statement_type, table);
                pattern_matches(pattern, &full_name.to_lowercase())
                    || pattern_matches(pattern, &self.id)
            }
            OperationDetails::Unknown => pattern_matches(pattern, &self.id),
        }
    }
}

/// Format-specific operation details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "lowercase")]
pub enum OperationDetails {
    /// OpenAPI operation details.
    #[serde(rename = "openapi")]
    OpenAPI {
        method: String,
        path: String,
        #[serde(default)]
        parameters: Vec<OperationParameter>,
        #[serde(default)]
        has_request_body: bool,
    },

    /// GraphQL operation details.
    #[serde(rename = "graphql")]
    GraphQL {
        operation_type: GraphQLOperationType,
        field_name: String,
        #[serde(default)]
        arguments: Vec<OperationParameter>,
        #[serde(default)]
        return_type: Option<String>,
    },

    /// SQL operation details.
    #[serde(rename = "sql")]
    Sql {
        statement_type: SqlStatementType,
        table: String,
        #[serde(default)]
        columns: Vec<String>,
    },

    /// Unknown/generic operation.
    Unknown,
}

/// SQL statement type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SqlStatementType {
    Select,
    Insert,
    Update,
    Delete,
}

/// Operation parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationParameter {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub required: bool,
    #[serde(default)]
    pub param_type: Option<String>,
}

// ============================================================================
// LAYER 2: EXPOSURE POLICIES
// ============================================================================

/// Complete exposure policy configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpExposurePolicy {
    /// Operations NEVER exposed via MCP (highest priority).
    #[serde(default)]
    pub global_blocklist: GlobalBlocklist,

    /// Policy for MCP tool exposure.
    #[serde(default)]
    pub tools: ToolExposurePolicy,

    /// Policy for Code Mode exposure.
    #[serde(default)]
    pub code_mode: CodeModeExposurePolicy,
}

/// Global blocklist - these operations are never exposed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalBlocklist {
    /// Blocked operation IDs (exact match).
    #[serde(default)]
    pub operations: HashSet<String>,

    /// Blocked patterns (glob matching).
    /// - OpenAPI: "METHOD /path/*" or "* /path/*"
    /// - GraphQL: "Type.field*" or "*.field"
    /// - SQL: "action table" or "* table"
    #[serde(default)]
    pub patterns: HashSet<String>,

    /// Blocked categories.
    #[serde(default)]
    pub categories: HashSet<OperationCategory>,

    /// Blocked risk levels.
    #[serde(default)]
    pub risk_levels: HashSet<OperationRiskLevel>,
}

impl GlobalBlocklist {
    /// Check if an operation is blocked by this blocklist.
    pub fn is_blocked(&self, operation: &Operation) -> Option<FilterReason> {
        // Check exact operation ID match
        if self.operations.contains(&operation.id) {
            return Some(FilterReason::GlobalBlocklistOperation {
                operation_id: operation.id.clone(),
            });
        }

        // Check pattern matches
        for pattern in &self.patterns {
            if operation.matches_pattern(pattern) {
                return Some(FilterReason::GlobalBlocklistPattern {
                    pattern: pattern.clone(),
                });
            }
        }

        // Check category
        if self.categories.contains(&operation.category) {
            return Some(FilterReason::GlobalBlocklistCategory {
                category: operation.category,
            });
        }

        // Check risk level
        if self.risk_levels.contains(&operation.risk_level) {
            return Some(FilterReason::GlobalBlocklistRiskLevel {
                level: operation.risk_level,
            });
        }

        None
    }
}

/// Tool exposure policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolExposurePolicy {
    /// Exposure mode.
    #[serde(default)]
    pub mode: ExposureMode,

    /// Operations to include (for allowlist mode).
    #[serde(default)]
    pub allowlist: HashSet<String>,

    /// Operations to exclude (for blocklist mode).
    #[serde(default)]
    pub blocklist: HashSet<String>,

    /// Per-operation customization.
    #[serde(default)]
    pub overrides: HashMap<String, ToolOverride>,
}

impl ToolExposurePolicy {
    /// Check if an operation is allowed by this policy.
    pub fn is_allowed(&self, operation: &Operation) -> Option<FilterReason> {
        // Check blocklist first (always applied)
        if self.blocklist.contains(&operation.id) {
            return Some(FilterReason::ToolBlocklist);
        }

        // Check patterns in blocklist
        for pattern in &self.blocklist {
            if pattern.contains('*') && operation.matches_pattern(pattern) {
                return Some(FilterReason::ToolBlocklistPattern {
                    pattern: pattern.clone(),
                });
            }
        }

        match self.mode {
            ExposureMode::AllowAll => None,
            ExposureMode::DenyAll => Some(FilterReason::ToolDenyAllMode),
            ExposureMode::Allowlist => {
                // Check if in allowlist
                if self.allowlist.contains(&operation.id) {
                    return None;
                }
                // Check patterns in allowlist
                for pattern in &self.allowlist {
                    if pattern.contains('*') && operation.matches_pattern(pattern) {
                        return None;
                    }
                }
                Some(FilterReason::ToolNotInAllowlist)
            }
            ExposureMode::Blocklist => None, // Already checked blocklist above
        }
    }
}

/// Code Mode exposure policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeModeExposurePolicy {
    /// Policy for read operations.
    #[serde(default)]
    pub reads: MethodExposurePolicy,

    /// Policy for write operations (create/update).
    #[serde(default)]
    pub writes: MethodExposurePolicy,

    /// Policy for delete operations.
    #[serde(default)]
    pub deletes: MethodExposurePolicy,

    /// Additional blocklist (applies on top of method policies).
    #[serde(default)]
    pub blocklist: HashSet<String>,
}

impl CodeModeExposurePolicy {
    /// Check if an operation is allowed by this policy.
    pub fn is_allowed(&self, operation: &Operation) -> Option<FilterReason> {
        // Check additional blocklist first
        if self.blocklist.contains(&operation.id) {
            return Some(FilterReason::CodeModeBlocklist);
        }

        // Check patterns in blocklist
        for pattern in &self.blocklist {
            if pattern.contains('*') && operation.matches_pattern(pattern) {
                return Some(FilterReason::CodeModeBlocklistPattern {
                    pattern: pattern.clone(),
                });
            }
        }

        // Get the appropriate method policy
        let method_policy = self.get_method_policy(operation);
        method_policy.is_allowed(operation)
    }

    /// Get the method policy for an operation based on its category.
    fn get_method_policy(&self, operation: &Operation) -> &MethodExposurePolicy {
        match operation.category {
            OperationCategory::Read => &self.reads,
            OperationCategory::Delete => &self.deletes,
            OperationCategory::Create | OperationCategory::Update => &self.writes,
            OperationCategory::Admin | OperationCategory::Internal => &self.writes,
        }
    }
}

/// Per-method-type exposure policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MethodExposurePolicy {
    /// Exposure mode.
    #[serde(default)]
    pub mode: ExposureMode,

    /// Operations to include (for allowlist mode).
    /// Can be operation IDs or patterns.
    #[serde(default)]
    pub allowlist: HashSet<String>,

    /// Operations to exclude (for blocklist mode).
    #[serde(default)]
    pub blocklist: HashSet<String>,
}

impl MethodExposurePolicy {
    /// Check if an operation is allowed by this policy.
    pub fn is_allowed(&self, operation: &Operation) -> Option<FilterReason> {
        // Check blocklist first
        if self.blocklist.contains(&operation.id) {
            return Some(FilterReason::MethodBlocklist {
                method_type: Self::method_type_name(operation),
            });
        }

        // Check patterns in blocklist
        for pattern in &self.blocklist {
            if pattern.contains('*') && operation.matches_pattern(pattern) {
                return Some(FilterReason::MethodBlocklistPattern {
                    method_type: Self::method_type_name(operation),
                    pattern: pattern.clone(),
                });
            }
        }

        match self.mode {
            ExposureMode::AllowAll => None,
            ExposureMode::DenyAll => Some(FilterReason::MethodDenyAllMode {
                method_type: Self::method_type_name(operation),
            }),
            ExposureMode::Allowlist => {
                // Check if in allowlist
                if self.allowlist.contains(&operation.id) {
                    return None;
                }
                // Check patterns in allowlist
                for pattern in &self.allowlist {
                    if pattern.contains('*') && operation.matches_pattern(pattern) {
                        return None;
                    }
                }
                Some(FilterReason::MethodNotInAllowlist {
                    method_type: Self::method_type_name(operation),
                })
            }
            ExposureMode::Blocklist => None, // Already checked blocklist above
        }
    }

    fn method_type_name(operation: &Operation) -> String {
        match operation.category {
            OperationCategory::Read => "reads".to_string(),
            OperationCategory::Delete => "deletes".to_string(),
            _ => "writes".to_string(),
        }
    }
}

/// Exposure mode determines how allowlist/blocklist are interpreted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExposureMode {
    /// All operations exposed except those in blocklist.
    #[default]
    AllowAll,
    /// No operations exposed (allowlist/blocklist ignored).
    DenyAll,
    /// Only operations in allowlist are exposed (blocklist still applies).
    Allowlist,
    /// All operations exposed except those in blocklist (same as AllowAll).
    Blocklist,
}

/// Per-operation customization for tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolOverride {
    /// Custom tool name (instead of operation ID).
    #[serde(default)]
    pub name: Option<String>,

    /// Custom description.
    #[serde(default)]
    pub description: Option<String>,

    /// Mark as dangerous (requires confirmation in Claude).
    #[serde(default)]
    pub dangerous: bool,

    /// Hide from tool list but still callable.
    #[serde(default)]
    pub hidden: bool,
}

// ============================================================================
// LAYER 3: DERIVED SCHEMAS
// ============================================================================

/// Derived schema for a specific exposure context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivedSchema {
    /// Operations included in this derived view.
    pub operations: Vec<Operation>,

    /// Human-readable documentation (for MCP resources).
    pub documentation: String,

    /// Derivation metadata (for audit).
    pub metadata: DerivationMetadata,
}

impl DerivedSchema {
    /// Get an operation by ID.
    pub fn get_operation(&self, id: &str) -> Option<&Operation> {
        self.operations.iter().find(|op| op.id == id)
    }

    /// Check if an operation is included.
    pub fn contains(&self, id: &str) -> bool {
        self.operations.iter().any(|op| op.id == id)
    }

    /// Get all operation IDs.
    pub fn operation_ids(&self) -> HashSet<String> {
        self.operations.iter().map(|op| op.id.clone()).collect()
    }
}

/// Metadata about schema derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationMetadata {
    /// Context (tools or code_mode).
    pub context: String,

    /// When the schema was derived (Unix timestamp).
    pub derived_at: i64,

    /// Source schema hash (for cache invalidation).
    pub source_hash: String,

    /// Policy hash (for cache invalidation).
    pub policy_hash: String,

    /// Combined hash for caching.
    pub cache_key: String,

    /// What was filtered and why.
    pub filtered: Vec<FilteredOperation>,

    /// Statistics.
    pub stats: DerivationStats,
}

/// An operation that was filtered during derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilteredOperation {
    /// Operation that was filtered.
    pub operation_id: String,

    /// Operation name for display.
    pub operation_name: String,

    /// Why it was filtered.
    pub reason: FilterReason,

    /// Which policy caused the filter.
    pub policy: String,
}

/// Why an operation was filtered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FilterReason {
    /// Blocked by global blocklist (exact operation ID match).
    GlobalBlocklistOperation { operation_id: String },

    /// Blocked by global blocklist (pattern match).
    GlobalBlocklistPattern { pattern: String },

    /// Blocked by global blocklist (category).
    GlobalBlocklistCategory { category: OperationCategory },

    /// Blocked by global blocklist (risk level).
    GlobalBlocklistRiskLevel { level: OperationRiskLevel },

    /// Blocked by tool blocklist.
    ToolBlocklist,

    /// Blocked by tool blocklist pattern.
    ToolBlocklistPattern { pattern: String },

    /// Not in tool allowlist.
    ToolNotInAllowlist,

    /// Tool policy is deny_all.
    ToolDenyAllMode,

    /// Blocked by code mode blocklist.
    CodeModeBlocklist,

    /// Blocked by code mode blocklist pattern.
    CodeModeBlocklistPattern { pattern: String },

    /// Blocked by method blocklist.
    MethodBlocklist { method_type: String },

    /// Blocked by method blocklist pattern.
    MethodBlocklistPattern {
        method_type: String,
        pattern: String,
    },

    /// Not in method allowlist.
    MethodNotInAllowlist { method_type: String },

    /// Method policy is deny_all.
    MethodDenyAllMode { method_type: String },
}

impl FilterReason {
    /// Get a human-readable description of the filter reason.
    pub fn description(&self) -> String {
        match self {
            FilterReason::GlobalBlocklistOperation { operation_id } => {
                format!("Operation '{}' is in the global blocklist", operation_id)
            }
            FilterReason::GlobalBlocklistPattern { pattern } => {
                format!("Matches global blocklist pattern '{}'", pattern)
            }
            FilterReason::GlobalBlocklistCategory { category } => {
                format!("Category '{:?}' is blocked globally", category)
            }
            FilterReason::GlobalBlocklistRiskLevel { level } => {
                format!("Risk level '{:?}' is blocked globally", level)
            }
            FilterReason::ToolBlocklist => "Operation is in the tool blocklist".to_string(),
            FilterReason::ToolBlocklistPattern { pattern } => {
                format!("Matches tool blocklist pattern '{}'", pattern)
            }
            FilterReason::ToolNotInAllowlist => {
                "Operation is not in the tool allowlist".to_string()
            }
            FilterReason::ToolDenyAllMode => "Tool exposure is set to deny_all".to_string(),
            FilterReason::CodeModeBlocklist => {
                "Operation is in the Code Mode blocklist".to_string()
            }
            FilterReason::CodeModeBlocklistPattern { pattern } => {
                format!("Matches Code Mode blocklist pattern '{}'", pattern)
            }
            FilterReason::MethodBlocklist { method_type } => {
                format!("Operation is in the {} blocklist", method_type)
            }
            FilterReason::MethodBlocklistPattern {
                method_type,
                pattern,
            } => {
                format!("Matches {} blocklist pattern '{}'", method_type, pattern)
            }
            FilterReason::MethodNotInAllowlist { method_type } => {
                format!("Operation is not in the {} allowlist", method_type)
            }
            FilterReason::MethodDenyAllMode { method_type } => {
                format!("{} exposure is set to deny_all", method_type)
            }
        }
    }
}

/// Statistics about schema derivation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DerivationStats {
    /// Total operations in source.
    pub source_total: usize,

    /// Operations in derived schema.
    pub derived_total: usize,

    /// Operations filtered.
    pub filtered_total: usize,

    /// Breakdown by filter reason type.
    pub filtered_by_reason: HashMap<String, usize>,
}

// ============================================================================
// PATTERN MATCHING
// ============================================================================

/// Check if a pattern matches a string using glob-style wildcards.
/// Supports: * (match any characters)
pub fn pattern_matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.to_lowercase();
    let text = text.to_lowercase();

    // Simple glob matching with * wildcard
    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 1 {
        // No wildcards, exact match
        return pattern == text;
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if i == 0 {
            // First part must match at the start
            if !text.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            // Last part must match at the end
            if !text[pos..].ends_with(part) {
                return false;
            }
        } else {
            // Middle parts can match anywhere after current position
            match text[pos..].find(part) {
                Some(found) => pos += found + part.len(),
                None => return false,
            }
        }
    }

    true
}

// ============================================================================
// SCHEMA DERIVER
// ============================================================================

/// Derives schemas from source + policy.
pub struct SchemaDeriver {
    /// Source operations.
    operations: Vec<Operation>,

    /// Exposure policy.
    policy: McpExposurePolicy,

    /// Source schema hash.
    source_hash: String,

    /// Policy hash.
    policy_hash: String,
}

impl SchemaDeriver {
    /// Create a new schema deriver.
    pub fn new(operations: Vec<Operation>, policy: McpExposurePolicy, source_hash: String) -> Self {
        let policy_hash = Self::compute_policy_hash(&policy);
        Self {
            operations,
            policy,
            source_hash,
            policy_hash,
        }
    }

    /// Derive the MCP Tools schema.
    pub fn derive_tools_schema(&self) -> DerivedSchema {
        let mut included = Vec::new();
        let mut filtered = Vec::new();

        for op in &self.operations {
            // Step 1: Check global blocklist (highest priority)
            if let Some(reason) = self.policy.global_blocklist.is_blocked(op) {
                filtered.push(FilteredOperation {
                    operation_id: op.id.clone(),
                    operation_name: op.name.clone(),
                    reason,
                    policy: "global_blocklist".to_string(),
                });
                continue;
            }

            // Step 2: Check tool exposure policy
            if let Some(reason) = self.policy.tools.is_allowed(op) {
                filtered.push(FilteredOperation {
                    operation_id: op.id.clone(),
                    operation_name: op.name.clone(),
                    reason,
                    policy: "tools".to_string(),
                });
                continue;
            }

            // Step 3: Apply overrides and include
            let op = self.apply_tool_overrides(op);
            included.push(op);
        }

        self.build_derived_schema(included, filtered, "tools")
    }

    /// Derive the Code Mode schema.
    pub fn derive_code_mode_schema(&self) -> DerivedSchema {
        let mut included = Vec::new();
        let mut filtered = Vec::new();

        for op in &self.operations {
            // Step 1: Check global blocklist
            if let Some(reason) = self.policy.global_blocklist.is_blocked(op) {
                filtered.push(FilteredOperation {
                    operation_id: op.id.clone(),
                    operation_name: op.name.clone(),
                    reason,
                    policy: "global_blocklist".to_string(),
                });
                continue;
            }

            // Step 2: Check code mode policy
            if let Some(reason) = self.policy.code_mode.is_allowed(op) {
                let policy_name = match op.category {
                    OperationCategory::Read => "code_mode.reads",
                    OperationCategory::Delete => "code_mode.deletes",
                    _ => "code_mode.writes",
                };
                filtered.push(FilteredOperation {
                    operation_id: op.id.clone(),
                    operation_name: op.name.clone(),
                    reason,
                    policy: policy_name.to_string(),
                });
                continue;
            }

            included.push(op.clone());
        }

        self.build_derived_schema(included, filtered, "code_mode")
    }

    /// Check if an operation is allowed in tools.
    pub fn is_tool_allowed(&self, operation_id: &str) -> bool {
        self.operations
            .iter()
            .find(|op| op.id == operation_id)
            .map(|op| {
                self.policy.global_blocklist.is_blocked(op).is_none()
                    && self.policy.tools.is_allowed(op).is_none()
            })
            .unwrap_or(false)
    }

    /// Check if an operation is allowed in code mode.
    pub fn is_code_mode_allowed(&self, operation_id: &str) -> bool {
        self.operations
            .iter()
            .find(|op| op.id == operation_id)
            .map(|op| {
                self.policy.global_blocklist.is_blocked(op).is_none()
                    && self.policy.code_mode.is_allowed(op).is_none()
            })
            .unwrap_or(false)
    }

    /// Get the filter reason for an operation in tools context.
    pub fn get_tool_filter_reason(&self, operation_id: &str) -> Option<FilterReason> {
        self.operations
            .iter()
            .find(|op| op.id == operation_id)
            .and_then(|op| {
                self.policy
                    .global_blocklist
                    .is_blocked(op)
                    .or_else(|| self.policy.tools.is_allowed(op))
            })
    }

    /// Get the filter reason for an operation in code mode context.
    pub fn get_code_mode_filter_reason(&self, operation_id: &str) -> Option<FilterReason> {
        self.operations
            .iter()
            .find(|op| op.id == operation_id)
            .and_then(|op| {
                self.policy
                    .global_blocklist
                    .is_blocked(op)
                    .or_else(|| self.policy.code_mode.is_allowed(op))
            })
    }

    /// Find operation ID by HTTP method and path pattern.
    ///
    /// This enables looking up human-readable operationIds (like "updateProduct")
    /// from METHOD:/path patterns (like "PUT:/products/*").
    ///
    /// # Arguments
    /// * `method` - HTTP method (e.g., "PUT", "POST")
    /// * `path_pattern` - Path pattern with wildcards (e.g., "/products/*")
    ///
    /// # Returns
    /// The operationId if a matching operation is found.
    pub fn find_operation_id(&self, method: &str, path_pattern: &str) -> Option<String> {
        let method_upper = method.to_uppercase();
        let normalized_pattern = Self::normalize_path_for_matching(path_pattern);

        for op in &self.operations {
            if let OperationDetails::OpenAPI {
                method: op_method,
                path: op_path,
                ..
            } = &op.details
            {
                if op_method.to_uppercase() == method_upper {
                    let normalized_op_path = Self::normalize_path_for_matching(op_path);
                    if Self::paths_match(&normalized_pattern, &normalized_op_path) {
                        return Some(op.id.clone());
                    }
                }
            }
        }
        None
    }

    /// Get all operations in a format suitable for display to administrators.
    ///
    /// Returns tuples of (operationId, METHOD:/path, description).
    pub fn get_operations_for_allowlist(&self) -> Vec<(String, String, String)> {
        self.operations
            .iter()
            .filter_map(|op| {
                if let OperationDetails::OpenAPI { method, path, .. } = &op.details {
                    let method_path = format!("{}:{}", method.to_uppercase(), path);
                    let description = op.description.clone().unwrap_or_else(|| op.name.clone());
                    Some((op.id.clone(), method_path, description))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Normalize a path for matching by replacing parameter placeholders with *.
    fn normalize_path_for_matching(path: &str) -> String {
        path.split('/')
            .map(|segment| {
                if segment.starts_with('{') && segment.ends_with('}') {
                    "*" // {id} -> *
                } else if segment.starts_with(':') {
                    "*" // :id -> *
                } else if segment == "*" {
                    "*"
                } else {
                    segment
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Check if two normalized paths match.
    fn paths_match(pattern: &str, path: &str) -> bool {
        let pattern_parts: Vec<_> = pattern.split('/').collect();
        let path_parts: Vec<_> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return false;
        }

        for (p, s) in pattern_parts.iter().zip(path_parts.iter()) {
            if *p == "*" || *s == "*" {
                continue; // Wildcard matches anything
            }
            if p != s {
                return false;
            }
        }
        true
    }

    /// Apply tool overrides to an operation.
    fn apply_tool_overrides(&self, op: &Operation) -> Operation {
        let mut op = op.clone();

        if let Some(override_config) = self.policy.tools.overrides.get(&op.id) {
            if let Some(name) = &override_config.name {
                op.name = name.clone();
            }
            if let Some(description) = &override_config.description {
                op.description = Some(description.clone());
            }
            if override_config.dangerous {
                op.risk_level = OperationRiskLevel::High;
            }
        }

        op
    }

    /// Build a derived schema from included and filtered operations.
    fn build_derived_schema(
        &self,
        operations: Vec<Operation>,
        filtered: Vec<FilteredOperation>,
        context: &str,
    ) -> DerivedSchema {
        // Build statistics
        let mut filtered_by_reason: HashMap<String, usize> = HashMap::new();
        for f in &filtered {
            let reason_type = match &f.reason {
                FilterReason::GlobalBlocklistOperation { .. } => "global_blocklist_operation",
                FilterReason::GlobalBlocklistPattern { .. } => "global_blocklist_pattern",
                FilterReason::GlobalBlocklistCategory { .. } => "global_blocklist_category",
                FilterReason::GlobalBlocklistRiskLevel { .. } => "global_blocklist_risk_level",
                FilterReason::ToolBlocklist => "tool_blocklist",
                FilterReason::ToolBlocklistPattern { .. } => "tool_blocklist_pattern",
                FilterReason::ToolNotInAllowlist => "tool_not_in_allowlist",
                FilterReason::ToolDenyAllMode => "tool_deny_all",
                FilterReason::CodeModeBlocklist => "code_mode_blocklist",
                FilterReason::CodeModeBlocklistPattern { .. } => "code_mode_blocklist_pattern",
                FilterReason::MethodBlocklist { .. } => "method_blocklist",
                FilterReason::MethodBlocklistPattern { .. } => "method_blocklist_pattern",
                FilterReason::MethodNotInAllowlist { .. } => "method_not_in_allowlist",
                FilterReason::MethodDenyAllMode { .. } => "method_deny_all",
            };
            *filtered_by_reason
                .entry(reason_type.to_string())
                .or_default() += 1;
        }

        let stats = DerivationStats {
            source_total: self.operations.len(),
            derived_total: operations.len(),
            filtered_total: filtered.len(),
            filtered_by_reason,
        };

        // Generate documentation
        let documentation = self.generate_documentation(&operations, context);

        // Compute cache key
        let cache_key = format!("{}:{}:{}", context, self.source_hash, self.policy_hash);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        DerivedSchema {
            operations,
            documentation,
            metadata: DerivationMetadata {
                context: context.to_string(),
                derived_at: now,
                source_hash: self.source_hash.clone(),
                policy_hash: self.policy_hash.clone(),
                cache_key,
                filtered,
                stats,
            },
        }
    }

    /// Generate human-readable documentation for a derived schema.
    fn generate_documentation(&self, operations: &[Operation], context: &str) -> String {
        let mut doc = String::new();

        if context == "code_mode" {
            doc.push_str("# API Operations Available in Code Mode\n\n");
        } else {
            doc.push_str("# API Operations Available as MCP Tools\n\n");
        }

        doc.push_str(&format!(
            "**{} of {} operations available**\n\n",
            operations.len(),
            self.operations.len()
        ));

        // Group by category
        let reads: Vec<_> = operations
            .iter()
            .filter(|o| o.category == OperationCategory::Read)
            .collect();
        let writes: Vec<_> = operations
            .iter()
            .filter(|o| {
                matches!(
                    o.category,
                    OperationCategory::Create | OperationCategory::Update
                )
            })
            .collect();
        let deletes: Vec<_> = operations
            .iter()
            .filter(|o| o.category == OperationCategory::Delete)
            .collect();

        // Read operations
        doc.push_str(&format!(
            "## Read Operations ({} available)\n\n",
            reads.len()
        ));
        if reads.is_empty() {
            doc.push_str("_No read operations available._\n\n");
        } else {
            for op in reads {
                self.document_operation(&mut doc, op, context);
            }
        }

        // Write operations
        doc.push_str(&format!(
            "\n## Write Operations ({} available)\n\n",
            writes.len()
        ));
        if writes.is_empty() {
            doc.push_str("_No write operations available._\n\n");
        } else {
            for op in writes {
                self.document_operation(&mut doc, op, context);
            }
        }

        // Delete operations
        doc.push_str(&format!(
            "\n## Delete Operations ({} available)\n\n",
            deletes.len()
        ));
        if deletes.is_empty() {
            doc.push_str("_No delete operations available._\n\n");
        } else {
            for op in deletes {
                self.document_operation(&mut doc, op, context);
            }
        }

        doc
    }

    /// Document a single operation.
    fn document_operation(&self, doc: &mut String, op: &Operation, context: &str) {
        match &op.details {
            OperationDetails::OpenAPI { method, path, .. } => {
                if context == "code_mode" {
                    let method_lower = method.to_lowercase();
                    doc.push_str(&format!(
                        "- `api.{}(\"{}\")` - {}\n",
                        method_lower, path, op.name
                    ));
                } else {
                    doc.push_str(&format!("- **{}**: `{} {}`\n", op.name, method, path));
                }
            }
            OperationDetails::GraphQL {
                operation_type,
                field_name,
                ..
            } => {
                doc.push_str(&format!(
                    "- **{}**: `{:?}.{}`\n",
                    op.name, operation_type, field_name
                ));
            }
            OperationDetails::Sql {
                statement_type,
                table,
                ..
            } => {
                doc.push_str(&format!(
                    "- **{}**: `{:?} {}`\n",
                    op.name, statement_type, table
                ));
            }
            OperationDetails::Unknown => {
                doc.push_str(&format!("- **{}** ({})\n", op.name, op.id));
            }
        }

        if let Some(desc) = &op.description {
            doc.push_str(&format!("  {}\n", desc));
        }
    }

    /// Compute a hash of the policy for caching.
    fn compute_policy_hash(policy: &McpExposurePolicy) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash global blocklist
        let mut ops: Vec<_> = policy.global_blocklist.operations.iter().collect();
        ops.sort();
        for op in ops {
            op.hash(&mut hasher);
        }

        let mut patterns: Vec<_> = policy.global_blocklist.patterns.iter().collect();
        patterns.sort();
        for p in patterns {
            p.hash(&mut hasher);
        }

        // Hash tool policy
        format!("{:?}", policy.tools.mode).hash(&mut hasher);
        let mut allowlist: Vec<_> = policy.tools.allowlist.iter().collect();
        allowlist.sort();
        for a in allowlist {
            a.hash(&mut hasher);
        }

        // Hash code mode policy
        format!("{:?}", policy.code_mode.reads.mode).hash(&mut hasher);
        format!("{:?}", policy.code_mode.writes.mode).hash(&mut hasher);
        format!("{:?}", policy.code_mode.deletes.mode).hash(&mut hasher);

        format!("{:016x}", hasher.finish())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        // Exact match
        assert!(pattern_matches("GET /users", "GET /users"));
        assert!(!pattern_matches("GET /users", "POST /users"));

        // Wildcard at end
        assert!(pattern_matches("GET /users/*", "GET /users/123"));
        assert!(pattern_matches("GET /users/*", "GET /users/123/posts"));
        assert!(!pattern_matches("GET /users/*", "GET /posts/123"));

        // Wildcard at start
        assert!(pattern_matches("* /admin/*", "GET /admin/users"));
        assert!(pattern_matches("* /admin/*", "DELETE /admin/config"));

        // Wildcard in middle
        assert!(pattern_matches(
            "GET /users/*/posts",
            "GET /users/123/posts"
        ));

        // Multiple wildcards
        assert!(pattern_matches("*/admin/*", "DELETE /admin/all"));

        // Case insensitive
        assert!(pattern_matches("GET /USERS", "get /users"));
    }

    #[test]
    fn test_global_blocklist() {
        let blocklist = GlobalBlocklist {
            operations: ["factoryReset".to_string()].into_iter().collect(),
            patterns: ["* /admin/*".to_string()].into_iter().collect(),
            categories: [OperationCategory::Internal].into_iter().collect(),
            risk_levels: [OperationRiskLevel::Critical].into_iter().collect(),
        };

        // Blocked by operation ID
        let op = Operation {
            id: "factoryReset".to_string(),
            name: "Factory Reset".to_string(),
            description: None,
            category: OperationCategory::Admin,
            is_read_only: false,
            risk_level: OperationRiskLevel::Critical,
            tags: vec![],
            details: OperationDetails::Unknown,
        };
        assert!(blocklist.is_blocked(&op).is_some());

        // Blocked by pattern
        let op = Operation {
            id: "listAdminUsers".to_string(),
            name: "List Admin Users".to_string(),
            description: None,
            category: OperationCategory::Read,
            is_read_only: true,
            risk_level: OperationRiskLevel::Safe,
            tags: vec![],
            details: OperationDetails::OpenAPI {
                method: "GET".to_string(),
                path: "/admin/users".to_string(),
                parameters: vec![],
                has_request_body: false,
            },
        };
        assert!(blocklist.is_blocked(&op).is_some());

        // Blocked by category
        let op = Operation {
            id: "internalSync".to_string(),
            name: "Internal Sync".to_string(),
            description: None,
            category: OperationCategory::Internal,
            is_read_only: false,
            risk_level: OperationRiskLevel::Low,
            tags: vec![],
            details: OperationDetails::Unknown,
        };
        assert!(blocklist.is_blocked(&op).is_some());

        // Not blocked
        let op = Operation {
            id: "listUsers".to_string(),
            name: "List Users".to_string(),
            description: None,
            category: OperationCategory::Read,
            is_read_only: true,
            risk_level: OperationRiskLevel::Safe,
            tags: vec![],
            details: OperationDetails::OpenAPI {
                method: "GET".to_string(),
                path: "/users".to_string(),
                parameters: vec![],
                has_request_body: false,
            },
        };
        assert!(blocklist.is_blocked(&op).is_none());
    }

    #[test]
    fn test_exposure_modes() {
        // Test AllowAll mode
        let policy = ToolExposurePolicy {
            mode: ExposureMode::AllowAll,
            blocklist: ["blocked".to_string()].into_iter().collect(),
            ..Default::default()
        };

        let allowed_op = Operation::new("allowed", "Allowed", OperationCategory::Read);
        let blocked_op = Operation::new("blocked", "Blocked", OperationCategory::Read);

        assert!(policy.is_allowed(&allowed_op).is_none());
        assert!(policy.is_allowed(&blocked_op).is_some());

        // Test Allowlist mode
        let policy = ToolExposurePolicy {
            mode: ExposureMode::Allowlist,
            allowlist: ["allowed".to_string()].into_iter().collect(),
            ..Default::default()
        };

        assert!(policy.is_allowed(&allowed_op).is_none());
        assert!(policy.is_allowed(&blocked_op).is_some());

        // Test DenyAll mode
        let policy = ToolExposurePolicy {
            mode: ExposureMode::DenyAll,
            ..Default::default()
        };

        assert!(policy.is_allowed(&allowed_op).is_some());
    }

    #[test]
    fn test_schema_deriver() {
        let operations = vec![
            Operation::new("listUsers", "List Users", OperationCategory::Read),
            Operation::new("createUser", "Create User", OperationCategory::Create),
            Operation::new("deleteUser", "Delete User", OperationCategory::Delete),
            Operation::new("factoryReset", "Factory Reset", OperationCategory::Admin),
        ];

        let policy = McpExposurePolicy {
            global_blocklist: GlobalBlocklist {
                operations: ["factoryReset".to_string()].into_iter().collect(),
                ..Default::default()
            },
            tools: ToolExposurePolicy {
                mode: ExposureMode::AllowAll,
                ..Default::default()
            },
            code_mode: CodeModeExposurePolicy {
                reads: MethodExposurePolicy {
                    mode: ExposureMode::AllowAll,
                    ..Default::default()
                },
                writes: MethodExposurePolicy {
                    mode: ExposureMode::Allowlist,
                    allowlist: ["createUser".to_string()].into_iter().collect(),
                    ..Default::default()
                },
                deletes: MethodExposurePolicy {
                    mode: ExposureMode::DenyAll,
                    ..Default::default()
                },
                ..Default::default()
            },
        };

        let deriver = SchemaDeriver::new(operations, policy, "test-hash".to_string());

        // Tools schema should have 3 operations (factoryReset blocked globally)
        let tools = deriver.derive_tools_schema();
        assert_eq!(tools.operations.len(), 3);
        assert!(tools.contains("listUsers"));
        assert!(tools.contains("createUser"));
        assert!(tools.contains("deleteUser"));
        assert!(!tools.contains("factoryReset"));

        // Code mode schema should have 2 operations
        // - listUsers (read, allowed)
        // - createUser (write, in allowlist)
        // - deleteUser (delete, deny_all)
        // - factoryReset (blocked globally)
        let code_mode = deriver.derive_code_mode_schema();
        assert_eq!(code_mode.operations.len(), 2);
        assert!(code_mode.contains("listUsers"));
        assert!(code_mode.contains("createUser"));
        assert!(!code_mode.contains("deleteUser"));
        assert!(!code_mode.contains("factoryReset"));
    }
}
