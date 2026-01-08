//! Provider registry for managing secret providers.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::config::{SecretTarget, SecretsConfig};
use super::error::{SecretError, SecretResult};
use super::provider::SecretProvider;
use super::providers::{AwsSecretProvider, LocalSecretProvider, PmcpRunSecretProvider};

/// Registry of available secret providers.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn SecretProvider>>,
}

impl ProviderRegistry {
    /// Create a new registry with default providers.
    pub fn new(project_root: &Path, config: &SecretsConfig) -> Self {
        let mut registry = Self {
            providers: HashMap::new(),
        };

        // Register local provider (always available)
        let local = LocalSecretProvider::new(config.get_secrets_dir(project_root));
        registry.register(Arc::new(local));

        // Register pmcp.run provider
        // Note: org_id is not needed - backend derives it from server ID
        let pmcp = PmcpRunSecretProvider::new(config.providers.pmcp.api_url.clone());
        registry.register(Arc::new(pmcp));

        // Register AWS provider (stubbed if aws-secrets feature not enabled)
        let aws = AwsSecretProvider::new(
            config.providers.aws.region.clone(),
            config.providers.aws.profile.clone(),
            config.providers.aws.secret_prefix.clone(),
        );
        registry.register(Arc::new(aws));

        registry
    }

    /// Register a provider.
    pub fn register(&mut self, provider: Arc<dyn SecretProvider>) {
        self.providers.insert(provider.id().to_string(), provider);
    }

    /// Get a provider by ID.
    pub fn get(&self, id: &str) -> SecretResult<Arc<dyn SecretProvider>> {
        self.providers
            .get(id)
            .cloned()
            .ok_or_else(|| SecretError::ProviderNotAvailable {
                provider: id.to_string(),
                reason: format!("Provider '{}' is not registered", id),
            })
    }

    /// Get a provider for a target.
    pub fn get_for_target(&self, target: SecretTarget) -> SecretResult<Arc<dyn SecretProvider>> {
        let id = match target {
            SecretTarget::Pmcp => "pmcp",
            SecretTarget::Aws => "aws",
            SecretTarget::Local => "local",
            SecretTarget::Gcp => {
                return Err(SecretError::ProviderNotAvailable {
                    provider: "gcp".to_string(),
                    reason: "GCP Secret Manager provider not yet implemented".to_string(),
                })
            },
            SecretTarget::Cloudflare => {
                return Err(SecretError::ProviderNotAvailable {
                    provider: "cloudflare".to_string(),
                    reason: "Cloudflare Workers secrets provider not yet implemented".to_string(),
                })
            },
        };

        self.get(id)
    }

    /// List all registered providers.
    pub fn list(&self) -> Vec<Arc<dyn SecretProvider>> {
        self.providers.values().cloned().collect()
    }

    /// Check health of all providers.
    pub async fn check_all_health(&self) -> Vec<(String, super::provider::ProviderHealth)> {
        let mut results = Vec::new();

        for (id, provider) in &self.providers {
            let health = match provider.health_check().await {
                Ok(h) => h,
                Err(e) => super::provider::ProviderHealth::unavailable(e.to_string()),
            };
            results.push((id.clone(), health));
        }

        results
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        // Create with default config and current directory
        let cwd = std::env::current_dir().unwrap_or_default();
        let config = SecretsConfig::default();
        Self::new(&cwd, &config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_registry_default_providers() {
        let temp_dir = TempDir::new().unwrap();
        let config = SecretsConfig::default();
        let registry = ProviderRegistry::new(temp_dir.path(), &config);

        // Local provider should always be available
        assert!(registry.get("local").is_ok());

        // pmcp provider should be registered
        assert!(registry.get("pmcp").is_ok());
    }

    #[test]
    fn test_registry_get_for_target() {
        let temp_dir = TempDir::new().unwrap();
        let config = SecretsConfig::default();
        let registry = ProviderRegistry::new(temp_dir.path(), &config);

        assert!(registry.get_for_target(SecretTarget::Local).is_ok());
        assert!(registry.get_for_target(SecretTarget::Pmcp).is_ok());

        // GCP not implemented yet
        assert!(registry.get_for_target(SecretTarget::Gcp).is_err());
    }
}
