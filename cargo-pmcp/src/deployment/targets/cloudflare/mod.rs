mod deploy;
mod init;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use std::process::Command;

use crate::deployment::{
    r#trait::{
        BuildArtifact, DeploymentOutputs, DeploymentTarget, MetricsData, SecretsAction, TestResults,
    },
    DeployConfig,
};

pub struct CloudflareTarget;

impl CloudflareTarget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CloudflareTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeploymentTarget for CloudflareTarget {
    fn id(&self) -> &str {
        "cloudflare-workers"
    }

    fn name(&self) -> &str {
        "Cloudflare Workers"
    }

    fn description(&self) -> &str {
        "Deploy to Cloudflare Workers edge network with WASM"
    }

    async fn is_available(&self) -> Result<bool> {
        // Check for required tools
        let has_wrangler = Command::new("wrangler")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let has_wasm_pack = Command::new("wasm-pack")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Ok(has_wrangler && has_wasm_pack)
    }

    async fn prerequisites(&self) -> Vec<String> {
        let mut missing = Vec::new();

        // Check wrangler
        if !Command::new("wrangler")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("Wrangler CLI (install: npm install -g wrangler)".to_string());
        }

        // Check wasm-pack
        if !Command::new("wasm-pack")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("wasm-pack (install: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh)".to_string());
        }

        // Check wasm32-unknown-unknown target
        let rustup_output = Command::new("rustup")
            .args(&["target", "list", "--installed"])
            .output();

        if let Ok(output) = rustup_output {
            let installed = String::from_utf8_lossy(&output.stdout);
            if !installed.contains("wasm32-unknown-unknown") {
                missing.push("wasm32-unknown-unknown target (install: rustup target add wasm32-unknown-unknown)".to_string());
            }
        }

        missing
    }

    async fn init(&self, config: &DeployConfig) -> Result<()> {
        init::init_cloudflare(config).await
    }

    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact> {
        println!("🔨 Building Cloudflare Workers adapter...");

        // Build the adapter project in deploy/cloudflare/
        let adapter_dir = config.project_root.join("deploy/cloudflare");

        if !adapter_dir.exists() {
            bail!(
                "Cloudflare deployment not initialized.\n\
                 Run: cargo pmcp deploy init --target cloudflare-workers"
            );
        }

        println!("📦 Building adapter: {}", adapter_dir.display());

        // Install worker-build if needed (quiet mode)
        println!("   Installing worker-build (if needed)...");
        let install_status = Command::new("cargo")
            .args(&["install", "-q", "worker-build"])
            .status()
            .context("Failed to install worker-build")?;

        if !install_status.success() {
            println!("   ⚠️  worker-build install failed, may already be installed");
        }

        // Build with worker-build
        println!("   Running worker-build...");
        let status = Command::new("worker-build")
            .arg("--release")
            .current_dir(&adapter_dir)
            .status()
            .context("Failed to run worker-build")?;

        if !status.success() {
            bail!(
                "worker-build failed.\n\n\
                 Make sure your MCP server package:\n\
                 1. Exports: pub fn create_server() -> pmcp::McpServer\n\
                 2. Is referenced in deploy/cloudflare/Cargo.toml\n\n\
                 Check the build output above for details."
            );
        }

        // The build output is in build/worker/
        let build_output = adapter_dir.join("build/worker");

        // Find the wasm file (it will be in the build output)
        let wasm_files: Vec<_> = std::fs::read_dir(&build_output)
            .context("Failed to read build output directory")?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "wasm")
                    .unwrap_or(false)
            })
            .collect();

        let wasm_path = if let Some(wasm_file) = wasm_files.first() {
            wasm_file.path()
        } else {
            // Fallback: use expected name based on project
            build_output.join("index.wasm")
        };

        let wasm_size = if wasm_path.exists() {
            std::fs::metadata(&wasm_path)
                .context("Failed to get WASM size")?
                .len()
        } else {
            0 // worker-build packages everything, size doesn't matter
        };

        println!("✅ Cloudflare Workers adapter built");

        Ok(BuildArtifact::Wasm {
            path: wasm_path,
            size: wasm_size,
        })
    }

    async fn deploy(
        &self,
        config: &DeployConfig,
        _artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs> {
        deploy::deploy_cloudflare(config).await
    }

    async fn destroy(&self, config: &DeployConfig, clean: bool) -> Result<()> {
        let deploy_dir = config.project_root.join("deploy/cloudflare");

        if !deploy_dir.exists() {
            println!("⚠️  No Cloudflare deployment found");
            return Ok(());
        }

        println!("🗑️  Destroying Cloudflare Worker...");
        println!();

        // Delete the worker
        let status = Command::new("wrangler")
            .args(&["delete", "--name", &config.server.name])
            .current_dir(&deploy_dir)
            .status()
            .context("Failed to run wrangler delete")?;

        if !status.success() {
            println!("⚠️  Worker deletion may have failed (this is okay if it doesn't exist)");
        } else {
            println!("✅ Cloudflare Worker destroyed successfully");
        }

        if clean {
            println!();
            println!("🧹 Cleaning up local deployment files...");

            // Remove deploy/cloudflare directory
            if deploy_dir.exists() {
                std::fs::remove_dir_all(&deploy_dir)
                    .context("Failed to remove deploy/cloudflare/ directory")?;
                println!("   ✓ Removed deploy/cloudflare/");
            }

            // Remove target config if this is the only target
            let config_file = config.project_root.join(".pmcp/deploy.toml");
            if config_file.exists() {
                std::fs::remove_file(&config_file).context("Failed to remove .pmcp/deploy.toml")?;
                println!("   ✓ Removed .pmcp/deploy.toml");
            }

            println!();
            println!("✅ All deployment files removed");
        }

        Ok(())
    }

    async fn outputs(&self, config: &DeployConfig) -> Result<DeploymentOutputs> {
        // Cloudflare Workers URL format: {worker-name}.{account}.workers.dev
        // We'll need to get this from wrangler or store it during deploy
        let deploy_dir = config.project_root.join("deploy/cloudflare");
        let outputs_file = deploy_dir.join("outputs.json");

        if outputs_file.exists() {
            let outputs_str = std::fs::read_to_string(&outputs_file)?;
            let outputs: DeploymentOutputs = serde_json::from_str(&outputs_str)?;
            Ok(outputs)
        } else {
            // Try to construct URL from worker name
            Ok(DeploymentOutputs {
                url: Some(format!(
                    "https://{}.workers.dev",
                    config.server.name.replace("_", "-")
                )),
                regions: vec!["global-edge".to_string()],
                stack_name: Some(config.server.name.clone()),
                version: None,
                additional_urls: vec![],
                custom: std::collections::HashMap::new(),
            })
        }
    }

    async fn logs(&self, config: &DeployConfig, tail: bool, _lines: usize) -> Result<()> {
        println!("📜 Streaming Cloudflare Worker logs...");
        println!();

        let deploy_dir = config.project_root.join("deploy/cloudflare");

        if tail {
            // Stream logs in real-time
            let status = Command::new("wrangler")
                .args(&["tail", &config.server.name])
                .current_dir(&deploy_dir)
                .status()
                .context("Failed to run wrangler tail")?;

            if !status.success() {
                bail!("Failed to stream logs");
            }
        } else {
            println!("Use --tail flag to stream logs in real-time");
            println!("  cargo pmcp deploy logs --tail");
        }

        Ok(())
    }

    async fn metrics(&self, _config: &DeployConfig, period: &str) -> Result<MetricsData> {
        println!("📊 Cloudflare Workers metrics coming soon!");
        println!("   View metrics at: https://dash.cloudflare.com");
        Ok(MetricsData {
            period: period.to_string(),
            requests: None,
            errors: None,
            avg_latency_ms: None,
            p99_latency_ms: None,
            custom: std::collections::HashMap::new(),
        })
    }

    async fn secrets(&self, config: &DeployConfig, action: SecretsAction) -> Result<()> {
        let deploy_dir = config.project_root.join("deploy/cloudflare");

        match action {
            SecretsAction::Set { key, from_env } => {
                println!("🔐 Setting secret: {}", key);

                let mut cmd = Command::new("wrangler");
                cmd.args(&["secret", "put", &key]).current_dir(&deploy_dir);

                if let Some(env_var) = from_env {
                    // Read from environment variable
                    let value = std::env::var(&env_var)
                        .context(format!("Environment variable {} not found", env_var))?;
                    cmd.env("WRANGLER_SECRET_VALUE", value);
                }

                let status = cmd.status().context("Failed to run wrangler secret")?;

                if !status.success() {
                    bail!("Failed to set secret");
                }

                println!("✅ Secret set successfully");
            },
            SecretsAction::List => {
                println!("🔐 Listing secrets...");

                let status = Command::new("wrangler")
                    .args(&["secret", "list"])
                    .current_dir(&deploy_dir)
                    .status()
                    .context("Failed to run wrangler secret list")?;

                if !status.success() {
                    bail!("Failed to list secrets");
                }
            },
            SecretsAction::Delete { key, yes } => {
                if !yes {
                    println!("⚠️  This will delete secret: {}", key);
                    print!("Type the secret name to confirm: ");
                    use std::io::{self, Write};
                    io::stdout().flush()?;

                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;

                    if input.trim() != key {
                        println!("❌ Confirmation failed. Aborting.");
                        return Ok(());
                    }
                }

                println!("🗑️  Deleting secret: {}", key);

                let status = Command::new("wrangler")
                    .args(&["secret", "delete", &key])
                    .current_dir(&deploy_dir)
                    .status()
                    .context("Failed to run wrangler secret delete")?;

                if !status.success() {
                    bail!("Failed to delete secret");
                }

                println!("✅ Secret deleted successfully");
            },
        }

        Ok(())
    }

    async fn test(&self, _config: &DeployConfig, _verbose: bool) -> Result<TestResults> {
        println!("🧪 Testing Cloudflare Worker deployment...");
        println!("   Use wrangler dev for local testing");
        Ok(TestResults {
            success: true,
            tests_run: 0,
            tests_passed: 0,
            failures: vec![],
        })
    }

    async fn rollback(&self, _config: &DeployConfig, version: Option<&str>) -> Result<()> {
        println!("🔄 Cloudflare Workers rollback coming soon!");
        println!(
            "   This will rollback to version: {}",
            version.unwrap_or("previous")
        );
        Ok(())
    }
}
