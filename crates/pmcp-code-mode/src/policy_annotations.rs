//! Policy annotation parser for Cedar policies.
//!
//! This module parses rustdoc-style annotations from Cedar policy comments
//! to extract metadata for UI display and policy management.
//!
//! ## Annotation Format
//!
//! ```cedar
//! /// @title Allow Write Operations
//! /// @description Permits adding and updating states in the policy database.
//! /// These operations are considered safe for automated execution.
//! /// @category write
//! /// @risk medium
//! /// @editable true
//! permit(
//!   principal,
//!   action == Action::"executeMutation",
//!   resource
//! ) when {
//!   resource.mutation in ["addState", "updateState"]
//! };
//! ```
//!
//! ## Supported Annotations
//!
//! | Annotation | Required | Description |
//! |------------|----------|-------------|
//! | `@title` | Yes | Short display name for the policy |
//! | `@description` | Yes | Multi-line description (continuation lines without @) |
//! | `@category` | Yes | One of: read, write, delete, fields, admin |
//! | `@risk` | Yes | One of: low, medium, high, critical |
//! | `@editable` | No | Whether admins can modify (default: true) |
//! | `@reason` | No | Why the policy exists or is non-editable |
//! | `@author` | No | Who created or last modified |
//! | `@modified` | No | ISO date of last modification |
//!
//! ## Category Mapping
//!
//! The unified categories work across all server types:
//! - `read`: Queries (GraphQL), GET (OpenAPI), SELECT (SQL)
//! - `write`: Create/update mutations, POST/PUT/PATCH, INSERT/UPDATE
//! - `delete`: Delete mutations, DELETE, DELETE/TRUNCATE
//! - `admin`: Introspection, schema access, DDL
//! - `fields`: Field-level access control
//!
//! Legacy category names are still supported for parsing: `queries` → `read`,
//! `mutations` → `write`, `introspection` → `admin`.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Unified policy category for grouping in the UI.
/// Works consistently across GraphQL, OpenAPI, and SQL servers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PolicyCategory {
    /// Read operations (Query, GET, SELECT)
    #[default]
    Read,
    /// Write operations (create/update mutations, POST/PUT/PATCH, INSERT/UPDATE)
    Write,
    /// Delete operations (delete mutations, DELETE, DELETE/TRUNCATE)
    Delete,
    /// Field-level access policies
    Fields,
    /// Administrative operations (introspection, DDL, schema changes)
    Admin,
}

impl fmt::Display for PolicyCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyCategory::Read => write!(f, "read"),
            PolicyCategory::Write => write!(f, "write"),
            PolicyCategory::Delete => write!(f, "delete"),
            PolicyCategory::Fields => write!(f, "fields"),
            PolicyCategory::Admin => write!(f, "admin"),
        }
    }
}

impl FromStr for PolicyCategory {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            // Unified names
            "read" | "reads" => Ok(PolicyCategory::Read),
            "write" | "writes" => Ok(PolicyCategory::Write),
            "delete" | "deletes" => Ok(PolicyCategory::Delete),
            "fields" | "field" | "paths" => Ok(PolicyCategory::Fields),
            "admin" | "safety" | "limits" => Ok(PolicyCategory::Admin),
            // Legacy GraphQL names (backward compatibility)
            "queries" | "query" => Ok(PolicyCategory::Read),
            "mutations" | "mutation" => Ok(PolicyCategory::Write),
            "introspection" => Ok(PolicyCategory::Admin),
            _ => Err(()),
        }
    }
}

/// Risk level for visual indication in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PolicyRiskLevel {
    /// Low risk - typically read-only operations
    #[default]
    Low,
    /// Medium risk - safe mutations
    Medium,
    /// High risk - sensitive operations
    High,
    /// Critical risk - destructive or admin operations
    Critical,
}

impl fmt::Display for PolicyRiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyRiskLevel::Low => write!(f, "low"),
            PolicyRiskLevel::Medium => write!(f, "medium"),
            PolicyRiskLevel::High => write!(f, "high"),
            PolicyRiskLevel::Critical => write!(f, "critical"),
        }
    }
}

impl FromStr for PolicyRiskLevel {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(PolicyRiskLevel::Low),
            "medium" => Ok(PolicyRiskLevel::Medium),
            "high" => Ok(PolicyRiskLevel::High),
            "critical" => Ok(PolicyRiskLevel::Critical),
            _ => Err(()),
        }
    }
}

/// Parsed policy metadata from Cedar doc comments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyMetadata {
    /// AVP policy ID
    pub id: String,

    /// Short display name (@title)
    pub title: String,

    /// Longer description (@description), may be multi-line
    pub description: String,

    /// Policy category for grouping (@category)
    pub category: PolicyCategory,

    /// Risk level for visual indication (@risk)
    pub risk: PolicyRiskLevel,

    /// Whether administrators can modify this policy (@editable)
    pub editable: bool,

    /// Reason for the policy or why it's non-editable (@reason)
    pub reason: Option<String>,

    /// Who created or last modified (@author)
    pub author: Option<String>,

    /// ISO date of last modification (@modified)
    pub modified: Option<String>,

    /// The full Cedar policy text
    pub raw_cedar: String,

    /// Whether this is a baseline policy (cannot be deleted)
    pub is_baseline: bool,

    /// Policy template ID (for template-linked policies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
}

/// Infer category and risk from Cedar policy content when annotations are missing.
///
/// This analyzes the Cedar action clause to determine:
/// - Category: based on `CodeMode::Action::"Read"/"Write"/"Delete"`
/// - Risk: based on policy effect (permit vs forbid) and action type
pub fn infer_category_and_risk_from_cedar(cedar: &str) -> (PolicyCategory, PolicyRiskLevel) {
    let cedar_lower = cedar.to_lowercase();

    // Determine category from action clause
    let category = if cedar.contains("Action::\"Delete\"") || cedar.contains("Action::\"delete\"") {
        PolicyCategory::Delete
    } else if cedar.contains("Action::\"Write\"") || cedar.contains("Action::\"write\"") {
        PolicyCategory::Write
    } else if cedar.contains("Action::\"Read\"") || cedar.contains("Action::\"read\"") {
        PolicyCategory::Read
    } else if cedar.contains("Action::\"Admin\"")
        || cedar.contains("Action::\"admin\"")
        || cedar.contains("Action::\"Introspection\"")
    {
        PolicyCategory::Admin
    } else {
        // Check for generic patterns
        if cedar_lower.contains("delete") {
            PolicyCategory::Delete
        } else if cedar_lower.contains("write") || cedar_lower.contains("mutation") {
            PolicyCategory::Write
        } else {
            PolicyCategory::Read
        }
    };

    // Determine risk based on effect and category
    let _is_forbid = cedar_lower.trim_start().starts_with("forbid");
    let is_permit = cedar_lower.trim_start().starts_with("permit");

    let risk = match category {
        PolicyCategory::Delete => {
            if is_permit {
                PolicyRiskLevel::High // Permitting deletes is high risk
            } else {
                PolicyRiskLevel::Low // Forbidding deletes is protective
            }
        },
        PolicyCategory::Write => {
            if is_permit {
                PolicyRiskLevel::Medium // Permitting writes is medium risk
            } else {
                PolicyRiskLevel::Low // Forbidding writes is protective
            }
        },
        PolicyCategory::Admin => {
            if is_permit {
                PolicyRiskLevel::High // Permitting admin is high risk
            } else {
                PolicyRiskLevel::Medium // Forbidding admin is protective but important
            }
        },
        PolicyCategory::Read => PolicyRiskLevel::Low, // Reads are generally low risk
        PolicyCategory::Fields => PolicyRiskLevel::Medium, // Field restrictions are medium
    };

    (category, risk)
}

impl Default for PolicyMetadata {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            description: String::new(),
            category: PolicyCategory::default(),
            risk: PolicyRiskLevel::default(),
            editable: true,
            reason: None,
            author: None,
            modified: None,
            raw_cedar: String::new(),
            is_baseline: false,
            template_id: None,
        }
    }
}

impl PolicyMetadata {
    /// Create a new PolicyMetadata with the given ID and Cedar content.
    pub fn new(id: impl Into<String>, cedar: impl Into<String>) -> Self {
        let cedar = cedar.into();
        let mut metadata = parse_policy_annotations(&cedar, &id.into());
        metadata.raw_cedar = cedar;
        metadata
    }

    /// Check if this policy has all required annotations.
    pub fn validate(&self) -> Result<(), Vec<PolicyValidationError>> {
        let mut errors = Vec::new();

        if self.title.is_empty() {
            errors.push(PolicyValidationError::MissingAnnotation(
                "@title".to_string(),
            ));
        }

        if self.description.is_empty() {
            errors.push(PolicyValidationError::MissingAnnotation(
                "@description".to_string(),
            ));
        }

        // Category and risk have defaults, so we check if they were explicitly set
        // by checking if the raw Cedar contains the annotations
        if !self.raw_cedar.contains("@category") {
            errors.push(PolicyValidationError::MissingAnnotation(
                "@category".to_string(),
            ));
        }

        if !self.raw_cedar.contains("@risk") {
            errors.push(PolicyValidationError::MissingAnnotation(
                "@risk".to_string(),
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Validation error for policy annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyValidationError {
    /// A required annotation is missing
    MissingAnnotation(String),
    /// An annotation has an invalid value
    InvalidAnnotation { annotation: String, message: String },
    /// Cedar syntax error
    CedarSyntaxError { line: Option<u32>, message: String },
}

impl fmt::Display for PolicyValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyValidationError::MissingAnnotation(ann) => {
                write!(f, "Missing required annotation: {}", ann)
            },
            PolicyValidationError::InvalidAnnotation {
                annotation,
                message,
            } => {
                write!(f, "Invalid {}: {}", annotation, message)
            },
            PolicyValidationError::CedarSyntaxError { line, message } => {
                if let Some(line) = line {
                    write!(f, "Cedar syntax error at line {}: {}", line, message)
                } else {
                    write!(f, "Cedar syntax error: {}", message)
                }
            },
        }
    }
}

/// Parse Cedar policy annotations from doc comments.
///
/// # Example
///
/// ```ignore
/// use pmcp_code_mode::policy_annotations::parse_policy_annotations;
///
/// let cedar = r#"
/// /// @title Allow Queries
/// /// @description Permits all read-only queries.
/// /// @category queries
/// /// @risk low
/// permit(principal, action, resource);
/// "#;
///
/// let metadata = parse_policy_annotations(cedar, "policy-123");
/// assert_eq!(metadata.title, "Allow Queries");
/// assert_eq!(metadata.description, "Permits all read-only queries.");
/// ```
pub fn parse_policy_annotations(cedar: &str, policy_id: &str) -> PolicyMetadata {
    let mut metadata = PolicyMetadata {
        id: policy_id.to_string(),
        raw_cedar: cedar.to_string(),
        ..Default::default()
    };

    let mut in_description = false;
    let mut found_category = false;
    let mut found_risk = false;

    for line in cedar.lines() {
        let line = line.trim();
        in_description = process_annotation_line(
            line,
            &mut metadata,
            in_description,
            &mut found_category,
            &mut found_risk,
        );
    }

    metadata.description = metadata.description.trim().to_string();
    apply_inferred_category_and_risk(&mut metadata, cedar, found_category, found_risk);
    metadata
}

/// Process a single trimmed source line for `parse_policy_annotations`. Returns the
/// updated `in_description` flag (true iff we are still inside an `@description` block).
fn process_annotation_line(
    line: &str,
    metadata: &mut PolicyMetadata,
    in_description: bool,
    found_category: &mut bool,
    found_risk: &mut bool,
) -> bool {
    if let Some(content) = line.strip_prefix("/// @") {
        return apply_at_annotation(content, metadata, found_category, found_risk);
    }
    if let Some(content) = line.strip_prefix("/// ") {
        if in_description {
            append_description_line(metadata, content);
        }
        return in_description;
    }
    if line == "///" {
        if in_description {
            metadata.description.push_str("\n\n");
        }
        return in_description;
    }
    // Non-comment line — stop parsing description.
    false
}

/// Apply a single `/// @key value` annotation. Returns the new `in_description` state
/// (true only when the annotation is `@description`).
fn apply_at_annotation(
    content: &str,
    metadata: &mut PolicyMetadata,
    found_category: &mut bool,
    found_risk: &mut bool,
) -> bool {
    let Some((key, value)) = content.split_once(' ') else {
        return false;
    };
    let value = value.trim();
    match key.to_lowercase().as_str() {
        "title" => {
            apply_title(metadata, value);
            false
        },
        "description" => {
            metadata.description = value.to_string();
            true
        },
        "category" => {
            metadata.category = value.parse().unwrap_or_default();
            *found_category = true;
            false
        },
        "risk" => {
            metadata.risk = value.parse().unwrap_or_default();
            *found_risk = true;
            false
        },
        "editable" => {
            metadata.editable = value.eq_ignore_ascii_case("true");
            false
        },
        "reason" => {
            metadata.reason = Some(value.to_string());
            false
        },
        "author" => {
            metadata.author = Some(value.to_string());
            false
        },
        "modified" => {
            metadata.modified = Some(value.to_string());
            false
        },
        _ => false, // Unknown annotation, ignore
    }
}

/// Apply the `@title` annotation. `Baseline:` titles flag the policy as immutable.
fn apply_title(metadata: &mut PolicyMetadata, value: &str) {
    metadata.title = value.to_string();
    if value.starts_with("Baseline:") {
        metadata.is_baseline = true;
        metadata.editable = false;
    }
}

/// Append a continuation line to the in-progress `@description`.
fn append_description_line(metadata: &mut PolicyMetadata, content: &str) {
    if !metadata.description.is_empty() {
        metadata.description.push('\n');
    }
    metadata.description.push_str(content);
}

/// If `@category` and/or `@risk` annotations were absent, infer them from the policy body.
fn apply_inferred_category_and_risk(
    metadata: &mut PolicyMetadata,
    cedar: &str,
    found_category: bool,
    found_risk: bool,
) {
    if found_category && found_risk {
        return;
    }
    let (inferred_category, inferred_risk) = infer_category_and_risk_from_cedar(cedar);
    if !found_category {
        metadata.category = inferred_category;
    }
    if !found_risk {
        metadata.risk = inferred_risk;
    }
}

/// Generate Cedar policy text with annotations from metadata.
///
/// This creates a properly formatted Cedar policy with doc comments.
pub fn generate_policy_cedar(metadata: &PolicyMetadata, policy_body: &str) -> String {
    let mut lines = Vec::new();

    // Title
    lines.push(format!("/// @title {}", metadata.title));

    // Description (handle multi-line)
    for (i, desc_line) in metadata.description.lines().enumerate() {
        if i == 0 {
            lines.push(format!("/// @description {}", desc_line));
        } else if desc_line.is_empty() {
            lines.push("///".to_string());
        } else {
            lines.push(format!("/// {}", desc_line));
        }
    }

    // Category and risk
    lines.push(format!("/// @category {}", metadata.category));
    lines.push(format!("/// @risk {}", metadata.risk));

    // Optional annotations
    if !metadata.editable {
        lines.push("/// @editable false".to_string());
    }

    if let Some(ref reason) = metadata.reason {
        lines.push(format!("/// @reason {}", reason));
    }

    if let Some(ref author) = metadata.author {
        lines.push(format!("/// @author {}", author));
    }

    if let Some(ref modified) = metadata.modified {
        lines.push(format!("/// @modified {}", modified));
    }

    // Add the policy body
    lines.push(policy_body.to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_policy() {
        let cedar = r#"/// @title Allow Queries
/// @description Permits all read-only queries.
/// @category read
/// @risk low
permit(principal, action, resource);"#;

        let metadata = parse_policy_annotations(cedar, "policy-123");

        assert_eq!(metadata.id, "policy-123");
        assert_eq!(metadata.title, "Allow Queries");
        assert_eq!(metadata.description, "Permits all read-only queries.");
        assert_eq!(metadata.category, PolicyCategory::Read);
        assert_eq!(metadata.risk, PolicyRiskLevel::Low);
        assert!(metadata.editable);
        assert!(!metadata.is_baseline);
    }

    #[test]
    fn test_parse_legacy_category_names() {
        // Test that legacy category names are parsed correctly
        let cedar = r#"/// @title Allow Queries
/// @description Permits all read-only queries.
/// @category queries
/// @risk low
permit(principal, action, resource);"#;

        let metadata = parse_policy_annotations(cedar, "policy-legacy");
        assert_eq!(metadata.category, PolicyCategory::Read);

        let cedar2 = r#"/// @title Block Mutations
/// @description Blocks write operations.
/// @category mutations
/// @risk high
forbid(principal, action, resource);"#;

        let metadata2 = parse_policy_annotations(cedar2, "policy-legacy2");
        assert_eq!(metadata2.category, PolicyCategory::Write);
    }

    #[test]
    fn test_parse_multiline_description() {
        let cedar = r#"/// @title Block Mutations
/// @description Prevents execution of dangerous mutations.
/// This is a critical security policy.
///
/// Do not modify without approval.
/// @category write
/// @risk critical
/// @editable false
/// @reason Security compliance
forbid(principal, action, resource);"#;

        let metadata = parse_policy_annotations(cedar, "policy-456");

        assert_eq!(metadata.title, "Block Mutations");
        assert!(metadata.description.contains("Prevents execution"));
        assert!(metadata.description.contains("Do not modify"));
        assert_eq!(metadata.category, PolicyCategory::Write);
        assert_eq!(metadata.risk, PolicyRiskLevel::Critical);
        assert!(!metadata.editable);
        assert_eq!(metadata.reason, Some("Security compliance".to_string()));
    }

    #[test]
    fn test_parse_baseline_policy() {
        let cedar = r#"/// @title Baseline: Allow Read-Only Queries
/// @description Core functionality for Code Mode.
/// @category read
/// @risk low
permit(principal, action, resource);"#;

        let metadata = parse_policy_annotations(cedar, "baseline-1");

        assert!(metadata.is_baseline);
        assert!(!metadata.editable);
    }

    #[test]
    fn test_validate_missing_annotations() {
        let metadata = PolicyMetadata {
            id: "test".to_string(),
            title: "".to_string(), // Missing
            description: "Has description".to_string(),
            raw_cedar: "permit(principal, action, resource);".to_string(),
            ..Default::default()
        };

        let result = metadata.validate();
        assert!(result.is_err());

        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e,
            PolicyValidationError::MissingAnnotation(s) if s == "@title"
        )));
    }

    #[test]
    fn test_generate_policy_cedar() {
        let metadata = PolicyMetadata {
            id: "test".to_string(),
            title: "Allow Writes".to_string(),
            description: "Permits safe write operations.\nAdd operations to the list.".to_string(),
            category: PolicyCategory::Write,
            risk: PolicyRiskLevel::Medium,
            editable: true,
            reason: None,
            author: Some("admin".to_string()),
            modified: Some("2024-01-15".to_string()),
            raw_cedar: String::new(),
            is_baseline: false,
            template_id: None,
        };

        let body = r#"permit(
  principal,
  action == Action::"executeMutation",
  resource
);"#;

        let cedar = generate_policy_cedar(&metadata, body);

        assert!(cedar.contains("/// @title Allow Writes"));
        assert!(cedar.contains("/// @description Permits safe write operations."));
        assert!(cedar.contains("/// Add operations to the list."));
        assert!(cedar.contains("/// @category write"));
        assert!(cedar.contains("/// @risk medium"));
        assert!(cedar.contains("/// @author admin"));
        assert!(cedar.contains("/// @modified 2024-01-15"));
        assert!(cedar.contains("permit("));
    }

    #[test]
    fn test_policy_category_parsing() {
        // Unified names (singular)
        assert_eq!(
            "read".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Read
        );
        assert_eq!(
            "write".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Write
        );
        assert_eq!(
            "delete".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Delete
        );
        assert_eq!(
            "FIELDS".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Fields
        );
        assert_eq!(
            "admin".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Admin
        );
        // Unified names (plural - used by OpenAPI policies)
        assert_eq!(
            "reads".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Read
        );
        assert_eq!(
            "writes".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Write
        );
        assert_eq!(
            "deletes".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Delete
        );
        // OpenAPI-specific categories
        assert_eq!(
            "paths".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Fields
        );
        assert_eq!(
            "safety".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Admin
        );
        assert_eq!(
            "limits".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Admin
        );
        // Legacy GraphQL names map to unified
        assert_eq!(
            "queries".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Read
        );
        assert_eq!(
            "mutation".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Write
        );
        assert_eq!(
            "introspection".parse::<PolicyCategory>().unwrap(),
            PolicyCategory::Admin
        );
        assert!("unknown".parse::<PolicyCategory>().is_err());
    }

    #[test]
    fn test_policy_risk_parsing() {
        assert_eq!(
            "low".parse::<PolicyRiskLevel>().unwrap(),
            PolicyRiskLevel::Low
        );
        assert_eq!(
            "CRITICAL".parse::<PolicyRiskLevel>().unwrap(),
            PolicyRiskLevel::Critical
        );
        assert!("unknown".parse::<PolicyRiskLevel>().is_err());
    }
}
