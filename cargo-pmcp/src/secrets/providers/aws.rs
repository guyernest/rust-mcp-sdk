//! AWS Secrets Manager provider.
//!
//! This module is only compiled when the `aws-secrets` feature is enabled.
//! It provides integration with AWS Secrets Manager for self-hosted deployments.

use async_trait::async_trait;

use crate::secrets::error::{SecretError, SecretResult};
use crate::secrets::provider::{
    parse_secret_name, ListOptions, ListResult, ProviderCapabilities, ProviderHealth,
    SecretProvider, SetOptions,
};
use crate::secrets::value::{SecretMetadata, SecretValue};

/// AWS Secrets Manager provider.
///
/// Stores secrets in AWS Secrets Manager with optional prefix namespacing.
/// Uses the standard AWS credential chain for authentication.
#[allow(dead_code)]
pub struct AwsSecretProvider {
    region: Option<String>,
    profile: Option<String>,
    prefix: Option<String>,
}

impl AwsSecretProvider {
    /// Create a new AWS Secrets Manager provider.
    pub fn new(region: Option<String>, profile: Option<String>, prefix: Option<String>) -> Self {
        Self {
            region,
            profile,
            prefix,
        }
    }

    /// Get the full secret name with prefix.
    #[allow(dead_code)] // Will be used when AWS provider is fully implemented
    fn prefixed_name(&self, name: &str) -> String {
        match &self.prefix {
            Some(prefix) => format!("{}{}", prefix, name),
            None => name.to_string(),
        }
    }
}

#[async_trait]
impl SecretProvider for AwsSecretProvider {
    fn id(&self) -> &str {
        "aws"
    }

    fn name(&self) -> &str {
        "AWS Secrets Manager"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            versioning: true,
            tags: true,
            descriptions: true,
            binary_values: true,
            max_value_size: 64 * 1024, // 64KB
            hierarchical_names: true,
        }
    }

    fn validate_name(&self, name: &str) -> SecretResult<()> {
        // Parse to validate format
        let (server_id, secret_name) = parse_secret_name(name)?;

        // AWS naming pattern: ^[a-zA-Z0-9/_+=.@-]+$
        let valid_chars = |c: char| {
            c.is_ascii_alphanumeric()
                || c == '/'
                || c == '_'
                || c == '+'
                || c == '='
                || c == '.'
                || c == '@'
                || c == '-'
        };

        if !server_id.chars().all(valid_chars) || !secret_name.chars().all(valid_chars) {
            return Err(SecretError::InvalidName {
                name: name.to_string(),
                reason: "AWS secret names can only contain alphanumeric characters and /_+=.@-"
                    .to_string(),
            });
        }

        Ok(())
    }

    async fn list(&self, _options: ListOptions) -> SecretResult<ListResult> {
        // TODO: Implement with aws-sdk-secretsmanager
        Err(SecretError::ProviderError {
            provider: "aws".to_string(),
            message: "AWS Secrets Manager provider not yet implemented. Enable with `cargo pmcp --features aws-secrets`".to_string(),
        })
    }

    async fn get(&self, _name: &str) -> SecretResult<SecretValue> {
        // TODO: Implement with aws-sdk-secretsmanager
        Err(SecretError::ProviderError {
            provider: "aws".to_string(),
            message: "AWS Secrets Manager provider not yet implemented".to_string(),
        })
    }

    async fn set(
        &self,
        _name: &str,
        _value: SecretValue,
        _options: SetOptions,
    ) -> SecretResult<SecretMetadata> {
        // TODO: Implement with aws-sdk-secretsmanager
        Err(SecretError::ProviderError {
            provider: "aws".to_string(),
            message: "AWS Secrets Manager provider not yet implemented".to_string(),
        })
    }

    async fn delete(&self, _name: &str, _force: bool) -> SecretResult<()> {
        // TODO: Implement with aws-sdk-secretsmanager
        Err(SecretError::ProviderError {
            provider: "aws".to_string(),
            message: "AWS Secrets Manager provider not yet implemented".to_string(),
        })
    }

    async fn health_check(&self) -> SecretResult<ProviderHealth> {
        // Check for AWS credentials
        if std::env::var("AWS_ACCESS_KEY_ID").is_ok() || std::env::var("AWS_PROFILE").is_ok() {
            let region = self
                .region
                .clone()
                .or_else(|| std::env::var("AWS_REGION").ok())
                .or_else(|| std::env::var("AWS_DEFAULT_REGION").ok())
                .unwrap_or_else(|| "us-east-1".to_string());

            Ok(ProviderHealth::healthy_with_user(
                "AWS credentials",
                format!("region: {}", region),
            ))
        } else {
            Ok(ProviderHealth::unavailable(
                "AWS credentials not configured. Set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY or AWS_PROFILE",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name_valid() {
        let provider = AwsSecretProvider::new(None, None, None);

        assert!(provider.validate_name("server/SECRET_KEY").is_ok());
        assert!(provider.validate_name("my-app/api/key").is_ok());
        assert!(provider.validate_name("prod_server/DB_URL").is_ok());
    }

    #[test]
    fn test_validate_name_invalid() {
        let provider = AwsSecretProvider::new(None, None, None);

        // Missing slash
        assert!(provider.validate_name("just-a-name").is_err());
    }

    #[test]
    fn test_prefixed_name() {
        let provider = AwsSecretProvider::new(None, None, Some("pmcp/".to_string()));
        assert_eq!(provider.prefixed_name("test"), "pmcp/test");

        let provider_no_prefix = AwsSecretProvider::new(None, None, None);
        assert_eq!(provider_no_prefix.prefixed_name("test"), "test");
    }
}
