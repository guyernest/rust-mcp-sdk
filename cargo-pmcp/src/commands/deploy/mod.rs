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
                DeployAction::Destroy { yes } => self.destroy(&project_root, *yes),
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

    fn destroy(&self, project_root: &PathBuf, skip_confirm: bool) -> Result<()> {
        let config = crate::deployment::config::DeployConfig::load(project_root)?;

        if !skip_confirm {
            println!("⚠️  This will destroy deployment '{}':", config.server.name);
            println!("   - Lambda function");
            println!("   - API Gateway");
            println!("   - CloudWatch logs and dashboards");
            println!("   - Secrets (with 30-day recovery period)");
            println!();
            println!("   Cognito User Pool will be retained (contains user data)");
            println!();
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

        println!("🗑️  Destroying deployment...");

        let deploy_dir = project_root.join("deploy");

        let status = std::process::Command::new("npx")
            .args(&["cdk", "destroy", "--force"])
            .current_dir(&deploy_dir)
            .status()
            .context("Failed to run CDK destroy")?;

        if !status.success() {
            bail!("CDK destroy failed");
        }

        println!("✅ Deployment destroyed");
        println!("📁 Configuration preserved in .pmcp/deploy.toml");

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
