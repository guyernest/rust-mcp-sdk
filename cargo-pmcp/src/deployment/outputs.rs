use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentOutputs {
    #[serde(rename = "ApiUrl")]
    pub api_url: String,

    #[serde(rename = "OAuthDiscoveryUrl")]
    pub oauth_discovery_url: String,

    #[serde(rename = "ClientId")]
    pub client_id: String,

    #[serde(rename = "DashboardUrl")]
    pub dashboard_url: String,

    #[serde(rename = "UserPoolId")]
    pub user_pool_id: String,
}

impl DeploymentOutputs {
    pub fn load(project_root: &Path) -> Result<Self> {
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

        serde_json::from_value(stack_outputs.clone()).context("Failed to parse stack outputs")
    }

    pub fn display(&self) {
        println!("╔════════════════════════════════════════════════════════════╗");
        println!("║ 🎉 MCP Server Deployed Successfully!                       ║");
        println!("╠════════════════════════════════════════════════════════════╣");
        println!("║                                                            ║");
        println!("║ 🌐 API URL:                                                ║");
        println!(
            "║ {}                              ║",
            truncate(&self.api_url, 56)
        );
        println!("║                                                            ║");
        println!("║ 🔐 OAuth:                                                  ║");
        println!(
            "║ Discovery: {}  ║",
            truncate(&self.oauth_discovery_url, 44)
        );
        println!(
            "║ Client ID: {}                                     ║",
            truncate(&self.client_id, 44)
        );
        println!("║                                                            ║");
        println!("║ 📊 Dashboard:                                              ║");
        println!("║ {}  ║", truncate(&self.dashboard_url, 56));
        println!("║                                                            ║");
        println!("╚════════════════════════════════════════════════════════════╝");
        println!();
        println!("💡 Connect from Claude Desktop:");
        println!("Add to ~/.config/Claude/claude_desktop_config.json:");
        println!();
        println!("{{");
        println!("  \"mcpServers\": {{");
        println!("    \"my-server\": {{");
        println!("      \"url\": \"{}\",", self.api_url);
        println!("      \"transport\": \"streamable-http\",");
        println!("      \"auth\": {{");
        println!("        \"type\": \"oauth\",");
        println!(
            "        \"discovery_url\": \"{}\",",
            self.oauth_discovery_url
        );
        println!("        \"client_id\": \"{}\"", self.client_id);
        println!("      }}");
        println!("    }}");
        println!("  }}");
        println!("}}");
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
