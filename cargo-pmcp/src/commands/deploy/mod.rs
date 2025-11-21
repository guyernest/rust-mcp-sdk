use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::PathBuf;

pub mod deploy;
pub mod init;
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
    /// Deployment target (aws-lambda, cloudflare-workers)
    #[clap(long, global = true)]
    target: Option<String>,

    #[clap(subcommand)]
    action: Option<DeployAction>,
}

#[derive(Debug, Parser)]
pub enum DeployAction {
    /// Initialize deployment configuration
    Init {
        /// AWS region (for AWS Lambda target, default: us-east-1)
        #[clap(long, default_value = "us-east-1")]
        region: String,

        /// Skip credentials check
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
        // Run async code in tokio runtime
        tokio::runtime::Runtime::new()?.block_on(self.execute_async())
    }

    async fn execute_async(&self) -> Result<()> {
        let project_root = Self::find_project_root()?;

        // Get target from flag or config
        let target_id = self.get_target_id(&project_root)?;

        // Get target registry and resolve target
        let registry = crate::deployment::TargetRegistry::new();
        let target = registry.get(&target_id)?;

        match &self.action {
            Some(action) => match action {
                DeployAction::Init {
                    region,
                    skip_credentials_check,
                } => {
                    // For init, we can use the old approach or new depending on target
                    if target_id == "aws-lambda" {
                        InitCommand::new(project_root)
                            .with_region(region)
                            .with_credentials_check(!skip_credentials_check)
                            .execute()
                    } else {
                        // For other targets, use the new modular approach
                        let config = crate::deployment::DeployConfig::default_for_server(
                            "mcp-server".to_string(),
                            region.clone(),
                            project_root.clone(),
                        );
                        target.init(&config).await
                    }
                },
                DeployAction::Logs { tail, lines } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    target.logs(&config, *tail, *lines).await
                },
                DeployAction::Metrics { period } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    let metrics = target.metrics(&config, period).await?;
                    println!("📊 Metrics for {}: {}", target.name(), metrics.period);
                    Ok(())
                },
                DeployAction::Test { verbose } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    let results = target.test(&config, *verbose).await?;
                    if results.success {
                        println!(
                            "✅ All tests passed ({}/{})",
                            results.tests_passed, results.tests_run
                        );
                    } else {
                        println!(
                            "❌ Some tests failed ({}/{})",
                            results.tests_passed, results.tests_run
                        );
                    }
                    Ok(())
                },
                DeployAction::Rollback { version, yes } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    target.rollback(&config, version.as_deref()).await
                },
                DeployAction::Destroy { yes, clean } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;

                    if !yes {
                        println!("⚠️  This will destroy deployment on {}", target.name());
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

                    target.destroy(&config, *clean).await
                },
                DeployAction::Secrets { action } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    let secrets_action = match action {
                        SecretsAction::Set { key, from_env } => {
                            crate::deployment::SecretsAction::Set {
                                key: key.clone(),
                                from_env: from_env.clone(),
                            }
                        },
                        SecretsAction::List => crate::deployment::SecretsAction::List,
                        SecretsAction::Delete { key, yes } => {
                            crate::deployment::SecretsAction::Delete {
                                key: key.clone(),
                                yes: *yes,
                            }
                        },
                    };
                    target.secrets(&config, secrets_action).await
                },
                DeployAction::Outputs { format } => {
                    let config = crate::deployment::DeployConfig::load(&project_root)?;
                    let outputs = target.outputs(&config).await?;

                    match format.as_str() {
                        "json" => {
                            println!("{}", serde_json::to_string_pretty(&outputs)?);
                        },
                        "text" => {
                            outputs.display();
                        },
                        _ => bail!("Unknown format: {}", format),
                    }
                    Ok(())
                },
            },
            None => {
                // No subcommand = deploy
                let config = crate::deployment::DeployConfig::load(&project_root)?;
                let artifact = target.build(&config).await?;
                let outputs = target.deploy(&config, artifact).await?;

                println!();
                outputs.display();

                Ok(())
            },
        }
    }

    fn get_target_id(&self, project_root: &PathBuf) -> Result<String> {
        // Priority: --target flag > config file > default
        if let Some(target) = &self.target {
            return Ok(target.clone());
        }

        // Try to read from config
        if let Ok(config) = crate::deployment::DeployConfig::load(project_root) {
            return Ok(config.target.target_type.clone());
        }

        // Default to AWS Lambda
        Ok("aws-lambda".to_string())
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
}
