use anyhow::{bail, Result};
use std::collections::HashMap;
use std::sync::Arc;

use super::r#trait::DeploymentTarget;

/// Registry for managing deployment targets
pub struct TargetRegistry {
    targets: HashMap<String, Arc<dyn DeploymentTarget>>,
}

impl TargetRegistry {
    /// Create a new registry with all available targets
    pub fn new() -> Self {
        let mut registry = Self {
            targets: HashMap::new(),
        };

        // Register built-in targets
        registry.register(Arc::new(super::targets::AwsLambdaTarget::new()));
        registry.register(Arc::new(super::targets::CloudflareTarget::new()));

        registry
    }

    /// Register a deployment target
    pub fn register(&mut self, target: Arc<dyn DeploymentTarget>) {
        let id = target.id().to_string();
        self.targets.insert(id, target);
    }

    /// Get a target by ID
    pub fn get(&self, id: &str) -> Result<Arc<dyn DeploymentTarget>> {
        self.targets
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Unknown deployment target: {}", id))
    }

    /// List all registered targets
    pub fn list(&self) -> Vec<Arc<dyn DeploymentTarget>> {
        self.targets.values().cloned().collect()
    }

    /// List available targets (have prerequisites installed)
    pub async fn list_available(&self) -> Vec<Arc<dyn DeploymentTarget>> {
        let mut available = Vec::new();
        for target in self.targets.values() {
            if target.is_available().await.unwrap_or(false) {
                available.push(target.clone());
            }
        }
        available
    }

    /// Get the default target (currently aws-lambda)
    pub fn default_target(&self) -> Result<Arc<dyn DeploymentTarget>> {
        self.get("aws-lambda")
    }

    /// Check if a target exists
    pub fn has(&self, id: &str) -> bool {
        self.targets.contains_key(id)
    }

    /// Get target count
    pub fn count(&self) -> usize {
        self.targets.len()
    }
}

impl Default for TargetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockTarget {
        id: String,
        name: String,
    }

    #[async_trait]
    impl DeploymentTarget for MockTarget {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            "Mock target for testing"
        }

        async fn is_available(&self) -> Result<bool> {
            Ok(true)
        }

        async fn prerequisites(&self) -> Vec<String> {
            vec![]
        }

        async fn init(&self, _config: &crate::deployment::config::DeployConfig) -> Result<()> {
            Ok(())
        }

        async fn build(
            &self,
            _config: &crate::deployment::config::DeployConfig,
        ) -> Result<crate::deployment::r#trait::BuildArtifact> {
            unimplemented!()
        }

        async fn deploy(
            &self,
            _config: &crate::deployment::config::DeployConfig,
            _artifact: crate::deployment::r#trait::BuildArtifact,
        ) -> Result<crate::deployment::r#trait::DeploymentOutputs> {
            unimplemented!()
        }

        async fn destroy(
            &self,
            _config: &crate::deployment::config::DeployConfig,
            _clean: bool,
        ) -> Result<()> {
            unimplemented!()
        }

        async fn outputs(
            &self,
            _config: &crate::deployment::config::DeployConfig,
        ) -> Result<crate::deployment::r#trait::DeploymentOutputs> {
            unimplemented!()
        }

        async fn logs(
            &self,
            _config: &crate::deployment::config::DeployConfig,
            _tail: bool,
            _lines: usize,
        ) -> Result<()> {
            unimplemented!()
        }

        async fn metrics(
            &self,
            _config: &crate::deployment::config::DeployConfig,
            _period: &str,
        ) -> Result<crate::deployment::r#trait::MetricsData> {
            unimplemented!()
        }

        async fn secrets(
            &self,
            _config: &crate::deployment::config::DeployConfig,
            _action: crate::deployment::r#trait::SecretsAction,
        ) -> Result<()> {
            unimplemented!()
        }

        async fn test(
            &self,
            _config: &crate::deployment::config::DeployConfig,
            _verbose: bool,
        ) -> Result<crate::deployment::r#trait::TestResults> {
            unimplemented!()
        }

        async fn rollback(
            &self,
            _config: &crate::deployment::config::DeployConfig,
            _version: Option<&str>,
        ) -> Result<()> {
            unimplemented!()
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = TargetRegistry::new();
        let target = Arc::new(MockTarget {
            id: "test".to_string(),
            name: "Test Target".to_string(),
        });

        registry.register(target.clone());
        assert!(registry.has("test"));
        assert_eq!(registry.count(), 1);

        let retrieved = registry.get("test").unwrap();
        assert_eq!(retrieved.id(), "test");
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let registry = TargetRegistry::new();
        assert!(registry.get("nonexistent").is_err());
    }
}
