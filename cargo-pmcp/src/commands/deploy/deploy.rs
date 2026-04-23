use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

pub struct DeployExecutor {
    project_root: PathBuf,
    /// Transient env vars (e.g., resolved secrets) passed to the CDK process.
    /// These are NEVER written to deploy.toml -- they exist only as process env
    /// vars for the CDK child process (per D-05: baked at deploy time,
    /// D-06: never persisted).
    extra_env: HashMap<String, String>,
}

impl DeployExecutor {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            extra_env: HashMap::new(),
        }
    }

    /// Set transient environment variables to pass to the CDK child process.
    ///
    /// These are used for resolved secrets that must reach the Lambda
    /// configuration without being written to disk.
    pub fn with_extra_env(mut self, env: HashMap<String, String>) -> Self {
        self.extra_env = env;
        self
    }

    pub fn execute(&self) -> Result<()> {
        let start = Instant::now();

        println!("🚀 Deploying to AWS Lambda...");
        println!();

        let config = crate::deployment::config::DeployConfig::load(&self.project_root)?;

        // Phase 76 Wave 4: IAM validation gate. Hard errors block deploy
        // BEFORE any AWS API call (fail-closed per 76-CONTEXT.md D-04);
        // warnings print to stderr but never block. T-76-02 wildcard
        // escalation lands here as a hard error.
        let warnings = crate::deployment::iam::validate(&config.iam)
            .context("IAM validation failed — fix .pmcp/deploy.toml before deploying")?;
        for w in &warnings {
            eprintln!("  {} {}", console::style("warning:").yellow(), w.message);
        }

        println!("📋 Server: {}", config.server.name);
        println!("🌍 Region: {}", config.aws.region);
        println!();

        let builder = crate::deployment::builder::BinaryBuilder::new(self.project_root.clone());
        builder.build()?;
        println!();

        self.run_cdk_deploy(&config)?;
        println!();

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

        // Pass transient env vars (resolved secrets) to CDK process.
        // These are NOT in deploy.toml -- they flow only as process env vars
        // so the CDK TypeScript stack reads them via process.env and sets
        // them on the Lambda function. Per D-05, secrets are "baked in" at
        // deploy time. Per D-06, they are never written to disk.
        for (key, value) in &self.extra_env {
            cmd.env(key, value);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_env_default_empty() {
        let executor = DeployExecutor::new(PathBuf::from("/tmp"));
        assert!(executor.extra_env.is_empty());
    }

    #[test]
    fn with_extra_env_builder() {
        let env: HashMap<String, String> = [
            ("SECRET_A".into(), "val_a".into()),
            ("SECRET_B".into(), "val_b".into()),
        ]
        .into();

        let executor = DeployExecutor::new(PathBuf::from("/tmp")).with_extra_env(env);

        assert_eq!(executor.extra_env.len(), 2);
        assert_eq!(executor.extra_env["SECRET_A"], "val_a");
        assert_eq!(executor.extra_env["SECRET_B"], "val_b");
    }
}
