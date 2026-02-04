//! Secret provider trait and related types.

use async_trait::async_trait;

use super::error::SecretResult;
use super::value::{SecretEntry, SecretMetadata, SecretValue};

/// Capabilities supported by a secret provider.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ProviderCapabilities {
    /// Whether the provider supports versioning
    pub versioning: bool,
    /// Whether the provider supports tags
    pub tags: bool,
    /// Whether the provider supports descriptions
    pub descriptions: bool,
    /// Whether the provider supports binary values
    pub binary_values: bool,
    /// Maximum value size in bytes
    pub max_value_size: usize,
    /// Whether the provider supports hierarchical names (e.g., "api/openai/key")
    pub hierarchical_names: bool,
}

/// Health status of a provider.
#[derive(Debug, Clone)]
pub struct ProviderHealth {
    /// Whether the provider is available and authenticated
    pub available: bool,
    /// Authentication method used
    pub auth_method: Option<String>,
    /// Additional status information
    pub message: Option<String>,
    /// User or account identifier (if available)
    pub user: Option<String>,
}

impl ProviderHealth {
    /// Create a healthy status.
    pub fn healthy(auth_method: impl Into<String>) -> Self {
        Self {
            available: true,
            auth_method: Some(auth_method.into()),
            message: None,
            user: None,
        }
    }

    /// Create a healthy status with user info.
    pub fn healthy_with_user(auth_method: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            available: true,
            auth_method: Some(auth_method.into()),
            message: None,
            user: Some(user.into()),
        }
    }

    /// Create an unavailable status.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            available: false,
            auth_method: None,
            message: Some(message.into()),
            user: None,
        }
    }
}

/// Options for listing secrets.
#[derive(Debug, Clone, Default)]
pub struct ListOptions {
    /// Filter by name pattern (glob syntax)
    pub filter: Option<String>,
    /// Filter by server ID
    pub server_id: Option<String>,
    /// Include metadata in results
    pub include_metadata: bool,
}

/// Result of a list operation.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct ListResult {
    /// List of secrets (values hidden)
    pub secrets: Vec<SecretEntry>,
    /// Total count (if different from secrets.len())
    pub total_count: Option<usize>,
}

/// Options for setting a secret.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SetOptions {
    /// Description for the secret
    pub description: Option<String>,
    /// Tags to attach to the secret
    pub tags: std::collections::HashMap<String, String>,
    /// Fail if secret already exists
    pub no_overwrite: bool,
    /// Server ID for namespacing
    pub server_id: Option<String>,
}

/// Trait for secret storage providers.
///
/// Implementations handle the actual storage and retrieval of secrets
/// from various backends (local files, pmcp.run, AWS Secrets Manager, etc.).
#[async_trait]
pub trait SecretProvider: Send + Sync {
    /// Unique identifier for this provider (e.g., "local", "pmcp", "aws").
    fn id(&self) -> &str;

    /// Human-readable name for this provider.
    fn name(&self) -> &str;

    /// Get the capabilities of this provider.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Validate a secret name for this provider.
    ///
    /// Different providers have different naming restrictions.
    fn validate_name(&self, name: &str) -> SecretResult<()>;

    /// List secrets (values are never returned, only names/metadata).
    async fn list(&self, options: ListOptions) -> SecretResult<ListResult>;

    /// Get a secret value by name.
    ///
    /// The name should include the server prefix (e.g., "chess/ANTHROPIC_API_KEY").
    async fn get(&self, name: &str) -> SecretResult<SecretValue>;

    /// Set a secret value.
    ///
    /// Returns metadata about the created/updated secret.
    async fn set(
        &self,
        name: &str,
        value: SecretValue,
        options: SetOptions,
    ) -> SecretResult<SecretMetadata>;

    /// Delete a secret.
    async fn delete(&self, name: &str, force: bool) -> SecretResult<()>;

    /// Check the health/availability of this provider.
    async fn health_check(&self) -> SecretResult<ProviderHealth>;
}

/// Parse a fully-qualified secret name into (server_id, secret_name).
///
/// Format: `server-id/SECRET_NAME`
///
/// # Examples
/// ```ignore
/// let (server, name) = parse_secret_name("chess/ANTHROPIC_API_KEY");
/// assert_eq!(server, "chess");
/// assert_eq!(name, "ANTHROPIC_API_KEY");
/// ```
pub fn parse_secret_name(full_name: &str) -> SecretResult<(String, String)> {
    match full_name.split_once('/') {
        Some((server_id, secret_name)) if !server_id.is_empty() && !secret_name.is_empty() => {
            Ok((server_id.to_string(), secret_name.to_string()))
        },
        _ => Err(super::error::SecretError::InvalidName {
            name: full_name.to_string(),
            reason: "Secret name must be in format 'server-id/SECRET_NAME'".to_string(),
        }),
    }
}

/// Create a fully-qualified secret name from server ID and secret name.
#[allow(dead_code)]
pub fn make_secret_name(server_id: &str, secret_name: &str) -> String {
    format!("{}/{}", server_id, secret_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_secret_name_valid() {
        let (server, name) = parse_secret_name("chess/ANTHROPIC_API_KEY").unwrap();
        assert_eq!(server, "chess");
        assert_eq!(name, "ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_parse_secret_name_nested() {
        let (server, name) = parse_secret_name("my-server/api/key").unwrap();
        assert_eq!(server, "my-server");
        assert_eq!(name, "api/key");
    }

    #[test]
    fn test_parse_secret_name_invalid_no_slash() {
        let result = parse_secret_name("ANTHROPIC_API_KEY");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_secret_name_invalid_empty_server() {
        let result = parse_secret_name("/ANTHROPIC_API_KEY");
        assert!(result.is_err());
    }

    #[test]
    fn test_make_secret_name() {
        let name = make_secret_name("chess", "ANTHROPIC_API_KEY");
        assert_eq!(name, "chess/ANTHROPIC_API_KEY");
    }
}
