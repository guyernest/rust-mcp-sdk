use anyhow::{bail, Context, Result};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

pub struct DeployExecutor {
    project_root: PathBuf,
}

impl DeployExecutor {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    pub fn execute(&self) -> Result<()> {
        let start = Instant::now();

        println!("🚀 Deploying to AWS Lambda...");
        println!();

        // 1. Load configuration
        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;
        println!("📋 Server: {}", config.server.name);
        println!("🌍 Region: {}", config.aws.region);
        println!();

        // 2. Build Rust binary
        let builder = crate::deployment::builder::BinaryBuilder::new(self.project_root.clone());
        builder.build()?;
        println!();

        // 3. Run CDK deploy
        self.run_cdk_deploy(&config)?;
        println!();

        // 4. Load and display outputs
        let stack_name = format!("{}-stack", config.server.name);
        let outputs = crate::deployment::load_cdk_outputs(
            &self.project_root,
            &config.aws.region,
            &stack_name,
        )?;

        let elapsed = start.elapsed();
        println!("✅ Deployment complete in {:.1}s", elapsed.as_secs_f64());
        println!();

        outputs.display();

        Ok(())
    }

    fn run_cdk_deploy(&self, config: &crate::deployment::config::DeployConfig) -> Result<()> {
        println!("☁️  Deploying CloudFormation stack...");

        let deploy_dir = self.project_root.join("deploy");

        // Set environment variables for CDK app
        let mut cmd = Command::new("npx");
        cmd.args(&[
            "cdk",
            "deploy",
            "--require-approval",
            "never",
            "--outputs-file",
            "outputs.json",
        ])
        .current_dir(&deploy_dir)
        .env("SERVER_NAME", &config.server.name)
        .env("AWS_REGION", &config.aws.region);

        // If account ID is specified, set it
        if let Some(account_id) = &config.aws.account_id {
            cmd.env("CDK_DEFAULT_ACCOUNT", account_id);
        }

        print!("   Synthesizing template...");
        std::io::Write::flush(&mut std::io::stdout())?;

        let status = cmd.status().context("Failed to run CDK deploy")?;

        if !status.success() {
            println!(" ❌");
            bail!("CDK deployment failed");
        }

        println!(" ✅");
        Ok(())
    }
}
