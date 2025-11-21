use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    pub target: TargetConfig,
    pub aws: AwsConfig,
    pub server: ServerConfig,
    pub environment: HashMap<String, String>,
    #[serde(default)]
    pub secrets: HashMap<String, String>,
    pub auth: AuthConfig,
    pub observability: ObservabilityConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_gateway: Option<ApiGatewayConfig>,

    /// Project root directory (not serialized)
    #[serde(skip)]
    pub project_root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    #[serde(rename = "type")]
    pub target_type: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsConfig {
    pub region: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub memory_mb: u32,
    pub timeout_seconds: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reserved_concurrency: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    pub callback_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub log_retention_days: u32,
    pub enable_xray: bool,
    pub create_dashboard: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alarms: Option<AlarmConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmConfig {
    pub error_threshold: u32,
    pub latency_threshold_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiGatewayConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub burst_limit: Option<u32>,
}

impl DeployConfig {
    pub fn load(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".pmcp/deploy.toml");

        if !config_path.exists() {
            anyhow::bail!("Deployment not initialized. Run: cargo pmcp deploy init");
        }

        let config_str =
            std::fs::read_to_string(&config_path).context("Failed to read .pmcp/deploy.toml")?;

        let mut config: Self =
            toml::from_str(&config_str).context("Failed to parse .pmcp/deploy.toml")?;

        // Set the project root
        config.project_root = project_root.to_path_buf();

        Ok(config)
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let config_dir = project_root.join(".pmcp");
        std::fs::create_dir_all(&config_dir).context("Failed to create .pmcp directory")?;

        let config_path = config_dir.join("deploy.toml");
        let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(&config_path, config_str).context("Failed to write .pmcp/deploy.toml")?;

        Ok(())
    }

    pub fn default_for_server(server_name: String, region: String, project_root: PathBuf) -> Self {
        let mut environment = HashMap::new();
        environment.insert("RUST_LOG".to_string(), "info".to_string());

        Self {
            target: TargetConfig {
                target_type: "aws-lambda".to_string(),
                version: "1.0.0".to_string(),
            },
            aws: AwsConfig {
                region,
                account_id: None,
            },
            server: ServerConfig {
                name: server_name,
                memory_mb: 512,
                timeout_seconds: 30,
                reserved_concurrency: None,
            },
            environment,
            secrets: HashMap::new(),
            auth: AuthConfig {
                enabled: true,
                user_pool_id: None,
                client_id: None,
                callback_urls: vec!["http://localhost:3000/callback".to_string()],
            },
            observability: ObservabilityConfig {
                log_retention_days: 30,
                enable_xray: true,
                create_dashboard: true,
                alarms: Some(AlarmConfig {
                    error_threshold: 10,
                    latency_threshold_ms: 5000,
                }),
            },
            api_gateway: None,
            project_root,
        }
    }
}
