use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;

mod deploy;
mod init;
mod logs;
mod metrics;
mod secrets;
mod test;

use deploy::DeployExecutor;
use init::InitCommand;
use logs::LogsCommand;
use metrics::MetricsCommand;
use secrets::SecretsCommand;
use test::TestCommand;

#[derive(Debug, Parser)]
pub struct DeployCommand {
    #[clap(subcommand)]
    action: Option<DeployAction>,
}

#[derive(Debug, Parser)]
pub enum DeployAction {
    /// Initialize deployment configuration
    Init {
        /// AWS region (default: us-east-1)
        #[clap(long, default_value = "us-east-1")]
        region: String,

        /// Skip AWS credentials check
        #[clap(long)]
        skip_credentials_check: bool,
    },

    /// View deployment logs
    Logs {
        /// Follow logs in real-time
        #[clap(long)]
        tail: bool,

        /// Number of lines to show
        #[clap(long, default_value = "100")]
        lines: usize,
    },

    /// View deployment metrics
    Metrics {
        /// Time period (1h, 24h, 7d, 30d)
        #[clap(long, default_value = "24h")]
        period: String,
    },

    /// Test the deployment
    Test {
        /// Verbose output
        #[clap(long)]
        verbose: bool,
    },

    /// Rollback to previous version
    Rollback {
        /// Version to rollback to (default: previous)
        version: Option<String>,

        /// Skip confirmation
        #[clap(long)]
        yes: bool,
    },

    /// Destroy the deployment
    Destroy {
        /// Skip confirmation prompt
        #[clap(long)]
        yes: bool,

        /// Remove all deployment files (CDK project, Lambda wrapper, config)
        #[clap(long)]
        clean: bool,
    },

    /// Manage secrets
    Secrets {
        #[clap(subcommand)]
        action: SecretsAction,
    },

    /// Show deployment outputs
    Outputs {
        /// Output format (text, json)
        #[clap(long, default_value = "text")]
        format: String,
    },
}

#[derive(Debug, Parser)]
pub enum SecretsAction {
    /// Set a secret value
    Set {
        /// Secret key
        key: String,

        /// Get value from environment variable
        #[clap(long)]
        from_env: Option<String>,
    },

    /// List all secrets
    List,

    /// Delete a secret
    Delete {
        /// Secret key
        key: String,

        /// Skip confirmation
        #[clap(long)]
        yes: bool,
    },
}

impl DeployCommand {
    pub fn execute(&self) -> Result<()> {
        let project_root = Self::find_project_root()?;

        match &self.action {
            Some(action) => match action {
                DeployAction::Init {
                    region,
                    skip_credentials_check,
                } => InitCommand::new(project_root)
                    .with_region(region)
                    .with_credentials_check(!skip_credentials_check)
                    .execute(),
                DeployAction::Logs { tail, lines } => LogsCommand::new(project_root)
                    .with_tail(*tail)
                    .with_lines(*lines)
                    .execute(),
                DeployAction::Metrics { period } => MetricsCommand::new(project_root)
                    .with_period(period)
                    .execute(),
                DeployAction::Test { verbose } => TestCommand::new(project_root)
                    .with_verbose(*verbose)
                    .execute(),
                DeployAction::Rollback { version, yes } => {
                    self.rollback(&project_root, version.as_deref(), *yes)
                },
                DeployAction::Destroy { yes, clean } => self.destroy(&project_root, *yes, *clean),
                DeployAction::Secrets { action } => {
                    let secrets_action = match action {
                        SecretsAction::Set { key, from_env } => secrets::SecretsAction::Set {
                            key: key.clone(),
                            from_env: from_env.clone(),
                        },
                        SecretsAction::List => secrets::SecretsAction::List,
                        SecretsAction::Delete { key, yes } => secrets::SecretsAction::Delete {
                            key: key.clone(),
                            yes: *yes,
                        },
                    };
                    SecretsCommand::new(project_root, secrets_action).execute()
                },
                DeployAction::Outputs { format } => self.show_outputs(&project_root, format),
            },
            None => {
                // No subcommand = deploy
                DeployExecutor::new(project_root).execute()
            },
        }
    }

    fn find_project_root() -> Result<PathBuf> {
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;

        let mut dir = current_dir.as_path();

        loop {
            if dir.join("Cargo.toml").exists() {
                return Ok(dir.to_path_buf());
            }

            dir = dir.parent().ok_or_else(|| {
                anyhow::anyhow!("Could not find Cargo.toml in any parent directory")
            })?;
        }
    }

    fn rollback(
        &self,
        _project_root: &PathBuf,
        version: Option<&str>,
        _skip_confirm: bool,
    ) -> Result<()> {
        println!("🔄 Rollback functionality coming soon!");
        println!(
            "   This will rollback to version: {}",
            version.unwrap_or("previous")
        );
        Ok(())
    }

    fn destroy(&self, project_root: &PathBuf, skip_confirm: bool, clean: bool) -> Result<()> {
        let config = crate::deployment::config::DeployConfig::load(project_root)?;

        if !skip_confirm {
            println!("⚠️  This will destroy deployment '{}':", config.server.name);
            println!("   - Lambda function");
            println!("   - API Gateway");
            println!("   - CloudWatch logs");
            println!();

            if clean {
                println!("⚠️  --clean flag: Will also remove local files:");
                println!("   - Deployment configuration (.pmcp/deploy.toml)");
                println!("   - CDK project (deploy/)");
                println!("   - Lambda wrapper code ({}-lambda/)", config.server.name);
                println!();
            } else {
                println!("The following will be preserved:");
                println!("   - Deployment configuration (.pmcp/deploy.toml)");
                println!("   - CDK project (deploy/)");
                println!("   - Lambda wrapper code ({}-lambda/)", config.server.name);
                println!();
            }

            print!("Type '{}' to confirm: ", config.server.name);

            use std::io::{self, Write};
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if input.trim() != config.server.name {
                println!("❌ Confirmation failed. Aborting.");
                return Ok(());
            }
        }

        println!("🗑️  Destroying AWS resources...");
        println!();

        let deploy_dir = project_root.join("deploy");

        // Check if deploy directory exists
        if deploy_dir.exists() {
            // Build the stack name (must match what's in app.ts)
            let stack_name = format!("{}-stack", config.server.name);

            // Run CDK destroy with explicit stack name
            let status = std::process::Command::new("npx")
                .args(&["cdk", "destroy", &stack_name, "--force"])
                .current_dir(&deploy_dir)
                .status()
                .context("Failed to run CDK destroy")?;

            if !status.success() {
                bail!("CDK destroy failed");
            }

            println!();
            println!("✅ AWS resources destroyed successfully");
        } else {
            println!("⚠️  No deployment found (deploy/ directory missing)");
            println!("   Skipping AWS resource cleanup.");
        }

        // Clean up local files if requested
        if clean {
            println!();
            println!("🧹 Cleaning up local deployment files...");

            // Remove deploy directory
            if deploy_dir.exists() {
                std::fs::remove_dir_all(&deploy_dir)
                    .context("Failed to remove deploy/ directory")?;
                println!("   ✓ Removed deploy/");
            }

            // Remove Lambda wrapper directory
            let lambda_dir = project_root.join(format!("{}-lambda", config.server.name));
            if lambda_dir.exists() {
                std::fs::remove_dir_all(&lambda_dir)
                    .context("Failed to remove Lambda wrapper directory")?;
                println!("   ✓ Removed {}-lambda/", config.server.name);
            }

            // Remove deployment config
            let config_file = project_root.join(".pmcp/deploy.toml");
            if config_file.exists() {
                std::fs::remove_file(&config_file).context("Failed to remove .pmcp/deploy.toml")?;
                println!("   ✓ Removed .pmcp/deploy.toml");
            }

            // Remove from workspace members
            self.remove_from_workspace(format!("{}-lambda", config.server.name), project_root)?;

            println!();
            println!("✅ All deployment files removed");
        } else {
            println!();
            println!("Preserved files:");
            println!("   📁 .pmcp/deploy.toml - Deployment configuration");
            println!("   📁 deploy/ - CDK project (can redeploy with 'cargo pmcp deploy')");
            println!("   📁 {}-lambda/ - Lambda wrapper code", config.server.name);
            println!();
            println!("To completely remove deployment files:");
            println!("   cargo pmcp deploy destroy --clean");
        }

        Ok(())
    }

    fn remove_from_workspace(&self, member: String, project_root: &PathBuf) -> Result<()> {
        let cargo_toml_path = project_root.join("Cargo.toml");
        let cargo_toml_str = std::fs::read_to_string(&cargo_toml_path)?;

        let mut cargo_toml: toml::Value = toml::from_str(&cargo_toml_str)?;

        if let Some(workspace) = cargo_toml.get_mut("workspace") {
            if let Some(members) = workspace.get_mut("members").and_then(|m| m.as_array_mut()) {
                members.retain(|m| m.as_str() != Some(&member));
            }
        }

        let new_content = toml::to_string(&cargo_toml)?;
        std::fs::write(&cargo_toml_path, new_content)?;

        Ok(())
    }

    fn show_outputs(&self, project_root: &PathBuf, format: &str) -> Result<()> {
        let outputs = crate::deployment::outputs::DeploymentOutputs::load(project_root)?;

        match format {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&outputs)?);
            },
            "text" => {
                outputs.display();
            },
            _ => bail!("Unknown format: {}", format),
        }

        Ok(())
    }
}
