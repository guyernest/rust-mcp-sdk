use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::deployment::{DeployConfig, DeploymentOutputs};

/// Deploy to Cloudflare Workers
pub async fn deploy_cloudflare(config: &DeployConfig) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to Cloudflare Workers...");
    println!();

    let deploy_dir = config.project_root.join("deploy/cloudflare");

    if !deploy_dir.exists() {
        bail!("Cloudflare deployment not initialized. Run: cargo pmcp deploy init --target cloudflare-workers");
    }

    println!("☁️  Deploying to Cloudflare edge network...");

    // Deploy with wrangler
    let output = Command::new("wrangler")
        .args(&["deploy"])
        .current_dir(&deploy_dir)
        .output()
        .context("Failed to run wrangler deploy")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("❌ Deployment failed:");
        println!("{}", stderr);
        bail!("Wrangler deploy failed");
    }

    println!("✅ Deployment successful!");
    println!();

    // Parse deployment URL from output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let url = extract_worker_url(&stdout, &config.server.name);

    // Save outputs
    let outputs = DeploymentOutputs {
        url: Some(url.clone()),
        regions: vec!["global-edge".to_string()],
        stack_name: Some(config.server.name.clone()),
        version: None,
        additional_urls: vec![],
        custom: std::collections::HashMap::new(),
    };

    // Save outputs to file
    let outputs_file = deploy_dir.join("outputs.json");
    std::fs::write(&outputs_file, serde_json::to_string_pretty(&outputs)?)?;

    Ok(outputs)
}

/// Extract worker URL from wrangler deploy output
fn extract_worker_url(output: &str, worker_name: &str) -> String {
    // Look for patterns like:
    // - Published <worker-name> (1.23s)
    // -   https://<worker-name>.<subdomain>.workers.dev
    // Or:
    // - Deployed to https://<worker-name>.<subdomain>.workers.dev

    for line in output.lines() {
        if line.contains("https://") && line.contains(".workers.dev") {
            if let Some(start) = line.find("https://") {
                if let Some(end) = line[start..].find(char::is_whitespace) {
                    return line[start..start + end].to_string();
                } else {
                    // URL is at end of line
                    return line[start..].trim().to_string();
                }
            }
        }
    }

    // Fallback: construct expected URL
    format!("https://{}.workers.dev", worker_name.replace("_", "-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_worker_url() {
        let output = r#"
Total Upload: 45.67 KiB / gzip: 12.34 KiB
Uploaded my-mcp-server (1.23s)
Published my-mcp-server (2.34s)
  https://my-mcp-server.example.workers.dev
Current Deployment ID: abc123def456
"#;
        let url = extract_worker_url(output, "my-mcp-server");
        assert_eq!(url, "https://my-mcp-server.example.workers.dev");
    }

    #[test]
    fn test_extract_worker_url_fallback() {
        let output = "No URL found in output";
        let url = extract_worker_url(output, "my_mcp_server");
        assert_eq!(url, "https://my-mcp-server.workers.dev");
    }
}
