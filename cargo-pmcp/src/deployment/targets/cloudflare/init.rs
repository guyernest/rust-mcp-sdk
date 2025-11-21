use anyhow::{Context, Result};
use std::process::Command;

use crate::deployment::DeployConfig;

/// Initialize Cloudflare Workers deployment
pub async fn init_cloudflare(config: &DeployConfig) -> Result<()> {
    println!("🚀 Initializing Cloudflare Workers deployment...");
    println!();

    // 1. Check wrangler authentication
    check_wrangler_auth()?;

    // 2. Create deploy/cloudflare directory
    let deploy_dir = config.project_root.join("deploy/cloudflare");
    std::fs::create_dir_all(&deploy_dir).context("Failed to create deploy/cloudflare directory")?;

    // 3. Create wrangler.toml
    create_wrangler_toml(&deploy_dir, &config.server.name)?;

    // 4. Create worker entry point
    create_worker_entry(&deploy_dir)?;

    println!();
    println!("✅ Cloudflare Workers deployment initialized!");
    println!();
    println!("Next steps:");
    println!("1. Login to Cloudflare: wrangler login");
    println!("2. Deploy: cargo pmcp deploy --target cloudflare-workers");
    println!();

    Ok(())
}

fn check_wrangler_auth() -> Result<()> {
    print!("🔍 Checking Wrangler authentication...");
    std::io::Write::flush(&mut std::io::stdout())?;

    let output = Command::new("wrangler").args(&["whoami"]).output();

    match output {
        Ok(output) if output.status.success() => {
            println!(" ✅");
            Ok(())
        },
        _ => {
            println!(" ⚠️");
            println!();
            println!("Wrangler not authenticated. Please run:");
            println!("  wrangler login");
            println!();
            Ok(())
        },
    }
}

fn create_wrangler_toml(deploy_dir: &std::path::Path, server_name: &str) -> Result<()> {
    print!("📝 Creating wrangler.toml...");
    std::io::Write::flush(&mut std::io::stdout())?;

    let wrangler_toml = format!(
        r#"name = "{}"
main = "worker.js"
compatibility_date = "2024-11-20"

[dev]
port = 8787

# Build configuration
# Uncomment and adjust if using custom build steps
# [build]
# command = "wasm-pack build --target web"
"#,
        server_name
    );

    std::fs::write(deploy_dir.join("wrangler.toml"), wrangler_toml)
        .context("Failed to write wrangler.toml")?;

    println!(" ✅");
    Ok(())
}

fn create_worker_entry(deploy_dir: &std::path::Path) -> Result<()> {
    print!("📝 Creating worker entry point...");
    std::io::Write::flush(&mut std::io::stdout())?;

    let worker_js = r#"// Cloudflare Worker entry point for MCP Server
// This will be auto-generated during build with wasm-pack

export default {
  async fetch(request, env, ctx) {
    // WASM module will be loaded here
    // The actual implementation will come from wasm-pack build output

    return new Response(
      JSON.stringify({
        error: "Worker not yet built. Run: cargo pmcp deploy --target cloudflare-workers"
      }),
      {
        status: 503,
        headers: {
          "Content-Type": "application/json",
          "Access-Control-Allow-Origin": "*"
        }
      }
    );
  }
};
"#;

    std::fs::write(deploy_dir.join("worker.js"), worker_js).context("Failed to write worker.js")?;

    println!(" ✅");
    Ok(())
}
