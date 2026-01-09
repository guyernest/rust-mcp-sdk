//! MCP Deployment Metadata Extraction and Injection
//!
//! This module extracts metadata from MCP server configurations and injects it
//! into deployment artifacts (CDK context, CloudFormation metadata).
//!
//! # Architecture
//!
//! cargo-pmcp's responsibility is metadata extraction and injection.
//! Platforms (like pmcp.run) handle actual resource provisioning based on this metadata.
//!
//! # Supported Formats
//!
//! 1. **Built-in servers** (pmcp-run style):
//!    - `builtin-manifest.toml` → server type, config path
//!    - Instance config (e.g., `instances/server.toml`) → secrets, tools
//!
//! 2. **Template-generated servers**:
//!    - `.pmcp/template-info.toml` → template provenance
//!    - Server config → secrets, parameters
//!
//! 3. **Custom servers**:
//!    - `pmcp.toml` or `.pmcp/config.toml` → manual declarations
//!
//! # Metadata Schema
//!
//! Uses `mcp:` namespace for standardized metadata:
//! - `mcp:version` - Metadata schema version
//! - `mcp:serverType` - Server type (graphql-api, openapi-api, custom)
//! - `mcp:serverId` - Unique server identifier
//! - `mcp:resources` - Required secrets, parameters, permissions
//! - `mcp:capabilities` - Tools, resources, prompts the server provides

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// MCP metadata version
pub const MCP_METADATA_VERSION: &str = "1.0";

// ============================================================================
// Core Metadata Types
// ============================================================================

/// Complete MCP server metadata for deployment.
///
/// This structure is extracted from server configuration files and injected
/// into deployment artifacts for platforms to consume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMetadata {
    /// Metadata schema version (always "1.0" for now)
    pub version: String,

    /// Server type (e.g., "graphql-api", "openapi-api", "custom")
    pub server_type: String,

    /// Unique server identifier
    pub server_id: String,

    /// Template that generated this server (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,

    /// Version of the template used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template_version: Option<String>,

    /// Required resources (secrets, parameters, permissions)
    pub resources: ResourceRequirements,

    /// Server capabilities (tools, resources, prompts)
    pub capabilities: ServerCapabilities,
}

/// Resource requirements for the server.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Required secrets (sensitive values)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretRequirement>,

    /// Required parameters (non-sensitive configuration)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<ParameterRequirement>,

    /// Required permissions (network access, AWS services, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PermissionRequirement>,
}

/// A required secret for the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRequirement {
    /// Secret name (e.g., "STATE_POLICIES_API_KEY")
    pub name: String,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Whether the server requires this secret to function
    pub required: bool,

    /// Environment variable name the server code expects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,

    /// URL where developers can obtain this secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obtain_url: Option<String>,
}

/// A required parameter (non-sensitive configuration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterRequirement {
    /// Parameter name
    pub name: String,

    /// Parameter value (if configured)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,

    /// Environment variable name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A required permission for the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequirement {
    /// Permission identifier
    pub id: String,

    /// Permission type (outbound_https, s3_read, dynamodb, etc.)
    #[serde(rename = "type")]
    pub permission_type: String,

    /// Specific targets (URLs, ARN patterns, etc.)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,

    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Server capabilities advertised via MCP.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// List of tool names this server provides
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,

    /// Whether server provides MCP resources
    #[serde(default)]
    pub resources: bool,

    /// List of prompt names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prompts: Vec<String>,

    /// Whether server supports composition (server-to-server calls)
    #[serde(default)]
    pub composition: bool,
}

// ============================================================================
// Built-in Manifest Types (pmcp-run format)
// These structs are used for TOML deserialization. Fields may not be directly
// read but are required for serde to properly deserialize the config files.
// ============================================================================

/// Built-in server manifest (servers/{name}/builtin-manifest.toml)
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BuiltinManifest {
    pub server: BuiltinServerSection,

    #[serde(default)]
    pub features: BuiltinFeaturesSection,

    #[serde(default)]
    #[allow(dead_code)]
    pub build: BuiltinBuildSection,

    #[serde(default)]
    pub resources: Option<BuiltinResourcesSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BuiltinServerSection {
    /// Server type (e.g., "graphql-api", "openapi-api")
    #[serde(rename = "type")]
    pub server_type: String,

    /// Path to the instance configuration file
    pub config: String,

    /// Optional template provenance
    #[serde(default)]
    pub template: Option<TemplateProvenance>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TemplateProvenance {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct BuiltinFeaturesSection {
    /// Parse secrets from the config file
    #[serde(default)]
    pub secrets_from_config: bool,

    /// Enable composition permissions
    #[serde(default)]
    pub composition_enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BuiltinBuildSection {
    #[serde(default)]
    pub pre_build: Vec<String>,

    #[serde(default)]
    pub lambda_crate: Option<String>,
}

/// Optional explicit resource declarations in manifest
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct BuiltinResourcesSection {
    #[serde(default)]
    pub parameters: Vec<ParameterDefinition>,

    #[serde(default)]
    pub permissions: Vec<PermissionDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ParameterDefinition {
    pub name: String,
    pub env_var: Option<String>,
    pub value: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PermissionDefinition {
    pub id: String,
    #[serde(rename = "type")]
    pub permission_type: String,
    #[serde(default)]
    pub targets: Vec<String>,
    pub description: Option<String>,
}

// ============================================================================
// Instance Config Types (instances/*.toml)
// These structs are used for TOML deserialization. Fields may not be directly
// read but are required for serde to properly deserialize the config files.
// ============================================================================

/// Instance configuration file (e.g., instances/state-policies.toml)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct InstanceConfig {
    pub server: InstanceServerSection,

    #[serde(default)]
    pub secrets: SecretsSection,

    #[serde(default)]
    pub backend: Option<BackendSection>,

    #[serde(default)]
    pub tools: Vec<ToolDefinition>,

    #[serde(default)]
    pub observability: Option<ObservabilitySection>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct InstanceServerSection {
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SecretsSection {
    #[serde(default)]
    pub provider: Option<String>,

    #[serde(default)]
    pub definitions: Vec<SecretDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SecretDefinition {
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
    pub obtain_url: Option<String>,
    pub env_var: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct BackendSection {
    pub endpoint: Option<String>,
    #[serde(default)]
    pub schema: Option<SchemaSection>,
    #[serde(default)]
    pub auth: Option<AuthSection>,
    #[serde(default)]
    pub http: Option<HttpSection>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct SchemaSection {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct AuthSection {
    #[serde(rename = "type")]
    pub auth_type: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub query_params: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct HttpSection {
    pub timeout_seconds: Option<u32>,
    pub retries: Option<u32>,
    pub retry_backoff_ms: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    // Other fields omitted - we only need the name for capabilities
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub(crate) struct ObservabilitySection {
    pub log_level: Option<String>,
    pub log_requests: Option<bool>,
    pub enable_metrics: Option<bool>,
    pub enable_tracing: Option<bool>,
}

// ============================================================================
// Metadata Extraction
// ============================================================================

impl McpMetadata {
    /// Extract metadata from a project directory.
    ///
    /// Checks for configuration files in this order:
    /// 1. `builtin-manifest.toml` (pmcp-run built-in servers)
    /// 2. `.pmcp/template-info.toml` (template-generated servers)
    /// 3. `pmcp.toml` (custom servers)
    ///
    /// Falls back to minimal metadata if no config is found.
    pub fn extract(project_root: &Path) -> Result<Self> {
        // Check for builtin-manifest.toml (pmcp-run format)
        let manifest_path = project_root.join("builtin-manifest.toml");
        if manifest_path.exists() {
            return Self::from_builtin_manifest(&manifest_path, project_root);
        }

        // Check for .pmcp/template-info.toml
        let template_info_path = project_root.join(".pmcp/template-info.toml");
        if template_info_path.exists() {
            return Self::from_template_info(&template_info_path, project_root);
        }

        // Check for pmcp.toml
        let pmcp_toml_path = project_root.join("pmcp.toml");
        if pmcp_toml_path.exists() {
            return Self::from_pmcp_toml(&pmcp_toml_path);
        }

        // Check for .pmcp/config.toml
        let pmcp_config_path = project_root.join(".pmcp/config.toml");
        if pmcp_config_path.exists() {
            return Self::from_pmcp_toml(&pmcp_config_path);
        }

        // Fall back to minimal metadata from Cargo.toml
        Self::default_from_cargo(project_root)
    }

    /// Extract metadata from a builtin-manifest.toml file.
    fn from_builtin_manifest(manifest_path: &Path, project_root: &Path) -> Result<Self> {
        let manifest_content = std::fs::read_to_string(manifest_path)
            .context("Failed to read builtin-manifest.toml")?;

        let manifest: BuiltinManifest =
            toml::from_str(&manifest_content).context("Failed to parse builtin-manifest.toml")?;

        // Resolve the config path relative to the manifest
        let manifest_dir = manifest_path.parent().unwrap_or(project_root);
        let config_path = manifest_dir.join(&manifest.server.config);

        // Parse the instance config
        let instance_config = if config_path.exists() {
            let config_content =
                std::fs::read_to_string(&config_path).context("Failed to read instance config")?;
            Some(
                toml::from_str::<InstanceConfig>(&config_content)
                    .context("Failed to parse instance config")?,
            )
        } else {
            None
        };

        // Extract server info
        let server_id = instance_config
            .as_ref()
            .map(|c| c.server.name.clone())
            .unwrap_or_else(|| {
                project_root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            });

        // Extract secrets
        let secrets = if manifest.features.secrets_from_config {
            instance_config
                .as_ref()
                .map(|c| {
                    c.secrets
                        .definitions
                        .iter()
                        .map(|s| SecretRequirement {
                            name: s.name.clone(),
                            description: s.description.clone(),
                            required: s.required,
                            env_var: s.env_var.clone(),
                            obtain_url: s.obtain_url.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            vec![]
        };

        // Extract tools for capabilities
        let tools = instance_config
            .as_ref()
            .map(|c| c.tools.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default();

        // Extract parameters and permissions from manifest if present
        let parameters = manifest
            .resources
            .as_ref()
            .map(|r| {
                r.parameters
                    .iter()
                    .map(|p| ParameterRequirement {
                        name: p.name.clone(),
                        value: p.value.clone(),
                        env_var: p.env_var.clone(),
                        description: p.description.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let permissions = manifest
            .resources
            .as_ref()
            .map(|r| {
                r.permissions
                    .iter()
                    .map(|p| PermissionRequirement {
                        id: p.id.clone(),
                        permission_type: p.permission_type.clone(),
                        targets: p.targets.clone(),
                        description: p.description.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Extract template provenance
        let (template_id, template_version) = manifest
            .server
            .template
            .as_ref()
            .map(|t| (Some(t.id.clone()), Some(t.version.clone())))
            .unwrap_or((None, None));

        Ok(Self {
            version: MCP_METADATA_VERSION.to_string(),
            server_type: manifest.server.server_type,
            server_id,
            template_id,
            template_version,
            resources: ResourceRequirements {
                secrets,
                parameters,
                permissions,
            },
            capabilities: ServerCapabilities {
                tools,
                resources: false,
                prompts: vec![],
                composition: manifest.features.composition_enabled,
            },
        })
    }

    /// Extract metadata from .pmcp/template-info.toml
    fn from_template_info(template_info_path: &Path, project_root: &Path) -> Result<Self> {
        // For now, treat template-info the same as builtin manifest
        // This can be extended later for different format
        let content = std::fs::read_to_string(template_info_path)
            .context("Failed to read template-info.toml")?;

        // Try to parse as builtin manifest format first
        if toml::from_str::<BuiltinManifest>(&content).is_ok() {
            return Self::from_builtin_manifest(template_info_path, project_root);
        }

        // Fall back to minimal metadata
        Self::default_from_cargo(project_root)
    }

    /// Extract metadata from pmcp.toml (custom servers)
    fn from_pmcp_toml(pmcp_toml_path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(pmcp_toml_path).context("Failed to read pmcp.toml")?;

        // Try to parse as instance config format
        if let Ok(config) = toml::from_str::<InstanceConfig>(&content) {
            let secrets = config
                .secrets
                .definitions
                .iter()
                .map(|s| SecretRequirement {
                    name: s.name.clone(),
                    description: s.description.clone(),
                    required: s.required,
                    env_var: s.env_var.clone(),
                    obtain_url: s.obtain_url.clone(),
                })
                .collect();

            let tools = config.tools.iter().map(|t| t.name.clone()).collect();

            return Ok(Self {
                version: MCP_METADATA_VERSION.to_string(),
                server_type: "custom".to_string(),
                server_id: config.server.name,
                template_id: None,
                template_version: None,
                resources: ResourceRequirements {
                    secrets,
                    parameters: vec![],
                    permissions: vec![],
                },
                capabilities: ServerCapabilities {
                    tools,
                    resources: false,
                    prompts: vec![],
                    composition: false,
                },
            });
        }

        // If parsing fails, return minimal metadata
        let server_id = pmcp_toml_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            version: MCP_METADATA_VERSION.to_string(),
            server_type: "custom".to_string(),
            server_id,
            template_id: None,
            template_version: None,
            resources: ResourceRequirements::default(),
            capabilities: ServerCapabilities::default(),
        })
    }

    /// Create minimal metadata from Cargo.toml
    fn default_from_cargo(project_root: &Path) -> Result<Self> {
        let cargo_toml_path = project_root.join("Cargo.toml");
        let server_id = if cargo_toml_path.exists() {
            let content = std::fs::read_to_string(&cargo_toml_path)?;
            // Simple extraction of package name
            content
                .lines()
                .find(|l| l.starts_with("name"))
                .and_then(|l| l.split('=').nth(1))
                .map(|n| n.trim().trim_matches('"').to_string())
                .unwrap_or_else(|| "unknown".to_string())
        } else {
            project_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string()
        };

        Ok(Self {
            version: MCP_METADATA_VERSION.to_string(),
            server_type: "custom".to_string(),
            server_id,
            template_id: None,
            template_version: None,
            resources: ResourceRequirements::default(),
            capabilities: ServerCapabilities::default(),
        })
    }

    /// Convert metadata to CDK context arguments.
    ///
    /// Returns a vector of `-c key='value'` arguments for CDK commands.
    /// JSON values are single-quoted to prevent shell interpretation.
    pub fn to_cdk_context(&self) -> Vec<String> {
        let mut context = vec![
            format!("-c mcp:version={}", self.version),
            format!("-c mcp:serverType={}", self.server_type),
            format!("-c mcp:serverId={}", self.server_id),
        ];

        if let Some(ref template_id) = self.template_id {
            context.push(format!("-c mcp:templateId={}", template_id));
        }

        if let Some(ref template_version) = self.template_version {
            context.push(format!("-c mcp:templateVersion={}", template_version));
        }

        // Serialize resources as JSON with single quotes for shell safety
        if let Ok(resources_json) = serde_json::to_string(&self.resources) {
            // Single-quote the JSON to prevent shell interpretation of special chars
            context.push(format!("-c 'mcp:resources={}'", resources_json));
        }

        // Serialize capabilities as JSON with single quotes for shell safety
        if let Ok(capabilities_json) = serde_json::to_string(&self.capabilities) {
            context.push(format!("-c 'mcp:capabilities={}'", capabilities_json));
        }

        context
    }

    /// Convert metadata to CloudFormation-style metadata object.
    ///
    /// This format is used in CloudFormation template Metadata section.
    #[allow(dead_code)]
    pub fn to_cloudformation_metadata(&self) -> serde_json::Value {
        let mut metadata = serde_json::json!({
            "mcp:version": self.version,
            "mcp:serverType": self.server_type,
            "mcp:serverId": self.server_id,
            "mcp:resources": self.resources,
            "mcp:capabilities": self.capabilities,
        });

        if let Some(ref template_id) = self.template_id {
            metadata["mcp:templateId"] = serde_json::json!(template_id);
        }

        if let Some(ref template_version) = self.template_version {
            metadata["mcp:templateVersion"] = serde_json::json!(template_version);
        }

        metadata
    }

    /// Check if metadata has any secrets declared.
    #[allow(dead_code)]
    pub fn has_secrets(&self) -> bool {
        !self.resources.secrets.is_empty()
    }

    /// Get the list of required secret names.
    #[allow(dead_code)]
    pub fn required_secret_names(&self) -> Vec<&str> {
        self.resources
            .secrets
            .iter()
            .filter(|s| s.required)
            .map(|s| s.name.as_str())
            .collect()
    }
}

// ============================================================================
// Display Implementation
// ============================================================================

impl std::fmt::Display for McpMetadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "MCP Server Metadata v{}", self.version)?;
        writeln!(f, "  Server ID: {}", self.server_id)?;
        writeln!(f, "  Server Type: {}", self.server_type)?;

        if let Some(ref template_id) = self.template_id {
            writeln!(f, "  Template: {}", template_id)?;
        }

        if !self.resources.secrets.is_empty() {
            writeln!(f, "  Secrets: {}", self.resources.secrets.len())?;
            for secret in &self.resources.secrets {
                let required = if secret.required { "*" } else { "" };
                writeln!(f, "    - {}{}", secret.name, required)?;
            }
        }

        if !self.capabilities.tools.is_empty() {
            writeln!(f, "  Tools: {}", self.capabilities.tools.join(", "))?;
        }

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_builtin_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let manifest_path = temp_dir.path().join("builtin-manifest.toml");
        let config_path = temp_dir.path().join("config.toml");

        // Write builtin manifest
        let manifest_content = r#"
[server]
type = "graphql-api"
config = "config.toml"

[features]
secrets_from_config = true
composition_enabled = false
"#;
        std::fs::write(&manifest_path, manifest_content).unwrap();

        // Write instance config
        let config_content = r#"
[server]
name = "test-server"
version = "1.0.0"
description = "A test server"

[secrets]
provider = "auto"

[[secrets.definitions]]
name = "API_KEY"
description = "API key for authentication"
required = true
obtain_url = "https://example.com/keys"

[[tools]]
name = "test-tool"
description = "A test tool"
"#;
        std::fs::write(&config_path, config_content).unwrap();

        let metadata = McpMetadata::extract(temp_dir.path()).unwrap();

        assert_eq!(metadata.version, "1.0");
        assert_eq!(metadata.server_type, "graphql-api");
        assert_eq!(metadata.server_id, "test-server");
        assert_eq!(metadata.resources.secrets.len(), 1);
        assert_eq!(metadata.resources.secrets[0].name, "API_KEY");
        assert!(metadata.resources.secrets[0].required);
        assert_eq!(metadata.capabilities.tools, vec!["test-tool"]);
        assert!(!metadata.capabilities.composition);
    }

    #[test]
    fn test_to_cdk_context() {
        let metadata = McpMetadata {
            version: "1.0".to_string(),
            server_type: "graphql-api".to_string(),
            server_id: "test-server".to_string(),
            template_id: Some("types/graphql".to_string()),
            template_version: Some("1.0.0".to_string()),
            resources: ResourceRequirements {
                secrets: vec![SecretRequirement {
                    name: "API_KEY".to_string(),
                    description: Some("Test key".to_string()),
                    required: true,
                    env_var: Some("API_KEY".to_string()),
                    obtain_url: None,
                }],
                parameters: vec![],
                permissions: vec![],
            },
            capabilities: ServerCapabilities {
                tools: vec!["test-tool".to_string()],
                resources: false,
                prompts: vec![],
                composition: false,
            },
        };

        let context = metadata.to_cdk_context();

        assert!(context.iter().any(|c| c.contains("mcp:version=1.0")));
        assert!(context
            .iter()
            .any(|c| c.contains("mcp:serverType=graphql-api")));
        assert!(context
            .iter()
            .any(|c| c.contains("mcp:serverId=test-server")));
        assert!(context
            .iter()
            .any(|c| c.contains("mcp:templateId=types/graphql")));
    }

    #[test]
    fn test_to_cloudformation_metadata() {
        let metadata = McpMetadata {
            version: "1.0".to_string(),
            server_type: "custom".to_string(),
            server_id: "my-server".to_string(),
            template_id: None,
            template_version: None,
            resources: ResourceRequirements::default(),
            capabilities: ServerCapabilities::default(),
        };

        let cf_metadata = metadata.to_cloudformation_metadata();

        assert_eq!(cf_metadata["mcp:version"], "1.0");
        assert_eq!(cf_metadata["mcp:serverType"], "custom");
        assert_eq!(cf_metadata["mcp:serverId"], "my-server");
    }

    #[test]
    fn test_default_from_cargo() {
        let temp_dir = TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");

        let cargo_content = r#"
[package]
name = "my-mcp-server"
version = "0.1.0"
"#;
        std::fs::write(&cargo_toml, cargo_content).unwrap();

        let metadata = McpMetadata::extract(temp_dir.path()).unwrap();

        assert_eq!(metadata.server_type, "custom");
        assert_eq!(metadata.server_id, "my-mcp-server");
    }
}
