//! Configuration for secret providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::error::{SecretError, SecretResult};

/// Target provider for secret operations.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SecretTarget {
    /// pmcp.run managed platform (enterprise)
    Pmcp,
    /// AWS Secrets Manager
    Aws,
    /// GCP Secret Manager (future)
    Gcp,
    /// Cloudflare Workers secrets (future)
    Cloudflare,
    /// Local filesystem (development)
    #[default]
    Local,
}

impl std::str::FromStr for SecretTarget {
    type Err = SecretError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pmcp" | "pmcp-run" | "pmcp.run" => Ok(SecretTarget::Pmcp),
            "aws" | "aws-secrets-manager" => Ok(SecretTarget::Aws),
            "gcp" | "google" | "gcp-secret-manager" => Ok(SecretTarget::Gcp),
            "cloudflare" | "cf" => Ok(SecretTarget::Cloudflare),
            "local" | "file" | "filesystem" => Ok(SecretTarget::Local),
            _ => Err(SecretError::ConfigError(format!(
                "Unknown secret target: {}. Valid targets: pmcp, aws, gcp, cloudflare, local",
                s
            ))),
        }
    }
}

impl std::fmt::Display for SecretTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretTarget::Pmcp => write!(f, "pmcp"),
            SecretTarget::Aws => write!(f, "aws"),
            SecretTarget::Gcp => write!(f, "gcp"),
            SecretTarget::Cloudflare => write!(f, "cloudflare"),
            SecretTarget::Local => write!(f, "local"),
        }
    }
}

/// Configuration for the local secrets provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalProviderConfig {
    /// Directory to store secrets (default: .pmcp/secrets)
    #[serde(default)]
    pub secrets_dir: Option<PathBuf>,
    /// Optional .env file for secrets (alternative format)
    #[serde(default)]
    pub env_file: Option<PathBuf>,
}

/// Configuration for the pmcp.run secrets provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PmcpProviderConfig {
    /// GraphQL API URL (default: https://api.pmcp.run/graphql)
    #[serde(default)]
    pub api_url: Option<String>,
    /// Organization ID (DEPRECATED: no longer needed, derived from server ID)
    #[serde(default)]
    #[allow(dead_code)]
    pub org_id: Option<String>,
}

/// Configuration for the AWS Secrets Manager provider.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AwsProviderConfig {
    /// AWS profile to use
    #[serde(default)]
    pub profile: Option<String>,
    /// AWS region
    #[serde(default)]
    pub region: Option<String>,
    /// Prefix for secret names in AWS
    #[serde(default)]
    pub secret_prefix: Option<String>,
}

/// Provider configurations.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProvidersConfig {
    #[serde(default)]
    pub local: LocalProviderConfig,
    #[serde(default)]
    pub pmcp: PmcpProviderConfig,
    #[serde(default)]
    pub aws: AwsProviderConfig,
}

/// Security settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Require confirmation for deletes
    #[serde(default = "default_true")]
    pub confirm_deletes: bool,
    /// Warn when outputting to terminal
    #[serde(default = "default_true")]
    pub warn_terminal_output: bool,
    /// Session timeout in seconds
    #[serde(default = "default_session_timeout")]
    pub session_timeout: u64,
}

fn default_true() -> bool {
    true
}

fn default_session_timeout() -> u64 {
    3600
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            confirm_deletes: true,
            warn_terminal_output: true,
            session_timeout: 3600,
        }
    }
}

/// Profile for different environments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretsProfile {
    /// Target provider for this profile
    pub target: Option<String>,
    /// Organization ID (for pmcp)
    pub org_id: Option<String>,
    /// Provider-specific overrides
    #[serde(flatten)]
    pub overrides: HashMap<String, toml::Value>,
}

/// Main secrets configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SecretsConfig {
    /// Default target provider
    #[serde(default)]
    pub target: Option<String>,
    /// Default organization ID
    #[serde(default)]
    pub org_id: Option<String>,
    /// Provider configurations
    #[serde(default)]
    pub providers: ProvidersConfig,
    /// Security settings
    #[serde(default)]
    pub security: SecurityConfig,
    /// Named profiles
    #[serde(default)]
    pub profiles: HashMap<String, SecretsProfile>,
}

impl SecretsConfig {
    /// Load configuration from .pmcp/config.toml
    pub fn load(project_root: &Path) -> SecretResult<Self> {
        let config_path = project_root.join(".pmcp").join("config.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .map_err(|e| SecretError::ConfigError(format!("Failed to read config file: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| SecretError::ConfigError(format!("Failed to parse config file: {}", e)))
    }

    /// Get the target for the current context.
    pub fn get_target(&self, profile: Option<&str>) -> SecretTarget {
        // Check profile first
        if let Some(profile_name) = profile {
            if let Some(profile) = self.profiles.get(profile_name) {
                if let Some(target_str) = &profile.target {
                    if let Ok(target) = target_str.parse() {
                        return target;
                    }
                }
            }
        }

        // Then check default target
        if let Some(target_str) = &self.target {
            if let Ok(target) = target_str.parse() {
                return target;
            }
        }

        // Default to local
        SecretTarget::Local
    }

    /// Get the secrets directory for local provider.
    pub fn get_secrets_dir(&self, project_root: &Path) -> PathBuf {
        self.providers
            .local
            .secrets_dir
            .clone()
            .unwrap_or_else(|| project_root.join(".pmcp").join("secrets"))
    }
}

/// Detect the best target based on available credentials.
pub fn detect_target() -> SecretTarget {
    // Check for pmcp.run OAuth token
    if has_pmcp_credentials() {
        return SecretTarget::Pmcp;
    }

    // Check for AWS credentials
    if has_aws_credentials() {
        return SecretTarget::Aws;
    }

    // Default to local
    SecretTarget::Local
}

fn has_pmcp_credentials() -> bool {
    // Check for stored OAuth token
    let home = dirs::home_dir();
    if let Some(home) = home {
        let token_path = home.join(".pmcp").join("tokens.json");
        if token_path.exists() {
            return true;
        }
    }

    // Check environment variable
    std::env::var("PMCP_ACCESS_TOKEN").is_ok()
}

fn has_aws_credentials() -> bool {
    // Check common AWS credential sources
    std::env::var("AWS_ACCESS_KEY_ID").is_ok()
        || std::env::var("AWS_PROFILE").is_ok()
        || std::env::var("AWS_ROLE_ARN").is_ok()
        || {
            let home = dirs::home_dir();
            home.map(|h| h.join(".aws").join("credentials").exists())
                .unwrap_or(false)
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_target_parse() {
        assert_eq!("pmcp".parse::<SecretTarget>().unwrap(), SecretTarget::Pmcp);
        assert_eq!(
            "pmcp-run".parse::<SecretTarget>().unwrap(),
            SecretTarget::Pmcp
        );
        assert_eq!("aws".parse::<SecretTarget>().unwrap(), SecretTarget::Aws);
        assert_eq!(
            "local".parse::<SecretTarget>().unwrap(),
            SecretTarget::Local
        );
    }

    #[test]
    fn test_secret_target_display() {
        assert_eq!(SecretTarget::Pmcp.to_string(), "pmcp");
        assert_eq!(SecretTarget::Aws.to_string(), "aws");
        assert_eq!(SecretTarget::Local.to_string(), "local");
    }

    #[test]
    fn test_config_default() {
        let config = SecretsConfig::default();
        assert!(config.security.confirm_deletes);
        assert!(config.security.warn_terminal_output);
        assert_eq!(config.security.session_timeout, 3600);
    }
}
