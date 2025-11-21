use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::DeploymentOutputs;

/// CDK Stack outputs format (from deploy/outputs.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CdkStackOutputs {
    #[serde(rename = "ApiUrl")]
    pub api_url: String,

    #[serde(rename = "OAuthDiscoveryUrl", skip_serializing_if = "Option::is_none")]
    pub oauth_discovery_url: Option<String>,

    #[serde(rename = "ClientId", skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    #[serde(rename = "DashboardUrl", skip_serializing_if = "Option::is_none")]
    pub dashboard_url: Option<String>,

    #[serde(rename = "UserPoolId", skip_serializing_if = "Option::is_none")]
    pub user_pool_id: Option<String>,
}

/// Load CDK deployment outputs and convert to standard format
pub fn load_cdk_outputs(project_root: &Path, region: &str, stack_name: &str) -> Result<DeploymentOutputs> {
    let outputs_path = project_root.join("deploy/outputs.json");

    if !outputs_path.exists() {
        anyhow::bail!("No deployment found. Run: cargo pmcp deploy");
    }

    let outputs_str =
        std::fs::read_to_string(&outputs_path).context("Failed to read deploy/outputs.json")?;

    let outputs_json: serde_json::Value =
        serde_json::from_str(&outputs_str).context("Failed to parse deploy/outputs.json")?;

    // CDK outputs are nested under stack name
    // Find the first (and only) stack
    let stack_outputs = outputs_json
        .as_object()
        .and_then(|obj| obj.values().next())
        .ok_or_else(|| anyhow::anyhow!("No stack outputs found"))?;

    let cdk_outputs: CdkStackOutputs =
        serde_json::from_value(stack_outputs.clone()).context("Failed to parse stack outputs")?;

    // Convert to standard DeploymentOutputs
    let mut custom = std::collections::HashMap::new();

    if let Some(oauth_url) = &cdk_outputs.oauth_discovery_url {
        custom.insert("oauth_discovery_url".to_string(), serde_json::json!(oauth_url));
    }
    if let Some(client_id) = &cdk_outputs.client_id {
        custom.insert("client_id".to_string(), serde_json::json!(client_id));
    }
    if let Some(dashboard) = &cdk_outputs.dashboard_url {
        custom.insert("dashboard_url".to_string(), serde_json::json!(dashboard));
    }
    if let Some(pool_id) = &cdk_outputs.user_pool_id {
        custom.insert("user_pool_id".to_string(), serde_json::json!(pool_id));
    }

    Ok(DeploymentOutputs {
        url: Some(cdk_outputs.api_url),
        regions: vec![region.to_string()],
        stack_name: Some(stack_name.to_string()),
        version: None,
        additional_urls: vec![],
        custom,
    })
}
