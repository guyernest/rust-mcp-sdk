use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};

/// Detect server name from Cargo.toml in the project root or core workspace
fn detect_server_name(project_root: &Path) -> Result<String> {
    // Try to read the main Cargo.toml
    let cargo_toml_path = project_root.join("Cargo.toml");
    if cargo_toml_path.exists() {
        let cargo_toml_content = std::fs::read_to_string(&cargo_toml_path)?;
        if let Ok(cargo_toml) = toml::from_str::<toml::Value>(&cargo_toml_content) {
            // Check if it's a workspace
            if cargo_toml.get("workspace").is_some() {
                // Look for core-workspace or similar
                let core_workspace_dir = project_root.join("core-workspace");
                if core_workspace_dir.exists() {
                    if let Ok(entries) = std::fs::read_dir(&core_workspace_dir) {
                        for entry in entries.flatten() {
                            let cargo_path = entry.path().join("Cargo.toml");
                            if let Ok(content) = std::fs::read_to_string(&cargo_path) {
                                if let Ok(core_cargo) = toml::from_str::<toml::Value>(&content) {
                                    if let Some(name) = core_cargo
                                        .get("package")
                                        .and_then(|p| p.get("name"))
                                        .and_then(|n| n.as_str())
                                    {
                                        // Remove "mcp-" prefix and "-core" suffix to get clean name
                                        let clean_name = name
                                            .strip_prefix("mcp-")
                                            .unwrap_or(name)
                                            .strip_suffix("-core")
                                            .unwrap_or(name);
                                        return Ok(clean_name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Not a workspace, use the package name directly
            if let Some(name) = cargo_toml
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
            {
                return Ok(name.to_string());
            }
        }
    }

    // Fallback to directory name
    Ok(project_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mcp-server")
        .to_string())
}

pub mod deploy;
pub mod init;
mod logs;
mod metrics;
mod secrets;
mod test;

use init::InitCommand;

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
        /// AWS region for deployment (uses AWS_REGION or AWS_DEFAULT_REGION env vars if set)
        #[clap(long, env = "AWS_REGION", default_value = "us-east-1")]
        region: String,

        /// Skip credentials check
        #[clap(long)]
        skip_credentials_check: bool,

        /// OAuth provider (cognito, oidc, none)
        #[clap(long, value_name = "PROVIDER")]
        oauth: Option<String>,

        /// Use shared OAuth infrastructure (format: shared:<name>)
        #[clap(long, value_name = "NAME")]
        oauth_shared: Option<String>,

        /// Existing Cognito User Pool ID (skip creation)
        #[clap(long, value_name = "POOL_ID")]
        cognito_user_pool_id: Option<String>,

        /// Cognito User Pool name (when creating new)
        #[clap(long, value_name = "NAME")]
        cognito_pool_name: Option<String>,

        /// Enable social login providers (comma-separated: github,google,apple)
        #[clap(long, value_name = "PROVIDERS", value_delimiter = ',')]
        social_providers: Option<Vec<String>>,
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

        /// Don't wait for async operations to complete (pmcp-run only)
        #[clap(long)]
        no_wait: bool,
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

    /// Login to deployment target (pmcp-run, cloudflare, etc.)
    Login,

    /// Logout from deployment target
    Logout,

    /// Manage OAuth configuration for pmcp.run servers
    Oauth {
        #[clap(subcommand)]
        action: OAuthAction,
    },

    /// Check status of an async operation (pmcp-run only)
    Status {
        /// Operation ID to check (deployment ID for destroy operations)
        operation_id: String,
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

#[derive(Debug, Parser)]
pub enum OAuthAction {
    /// Enable OAuth for an MCP server on pmcp.run
    Enable {
        /// Server ID (deployment ID) to enable OAuth for
        #[clap(long)]
        server: String,

        /// OAuth scopes (comma-separated)
        #[clap(long, value_delimiter = ',')]
        scopes: Option<Vec<String>>,

        /// Enable Dynamic Client Registration
        #[clap(long, default_value = "true")]
        dcr: bool,

        /// Public client patterns (comma-separated, e.g., claude,cursor)
        #[clap(long, value_delimiter = ',')]
        public_clients: Option<Vec<String>>,

        /// Use a shared User Pool instead of per-server pool
        #[clap(long)]
        shared_pool: Option<String>,
    },

    /// Disable OAuth for an MCP server
    Disable {
        /// Server ID (deployment ID)
        #[clap(long)]
        server: String,
    },

    /// Show OAuth status and endpoints for an MCP server
    Status {
        /// Server ID (deployment ID)
        #[clap(long)]
        server: String,
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
            Some(action) => {
                match action {
                    DeployAction::Init {
                        region,
                        skip_credentials_check,
                        oauth,
                        oauth_shared,
                        cognito_user_pool_id,
                        cognito_pool_name,
                        social_providers,
                    } => {
                        // For init, we can use the old approach or new depending on target
                        if target_id == "aws-lambda" {
                            let mut cmd = InitCommand::new(project_root)
                                .with_region(region)
                                .with_credentials_check(!skip_credentials_check);

                            // Configure OAuth if specified
                            if let Some(provider) = oauth {
                                cmd = cmd.with_oauth_provider(provider);
                            }
                            if let Some(shared_name) = oauth_shared {
                                cmd = cmd.with_oauth_shared(&shared_name);
                            }
                            if let Some(pool_id) = cognito_user_pool_id {
                                cmd = cmd.with_cognito_user_pool_id(&pool_id);
                            }
                            if let Some(pool_name) = cognito_pool_name {
                                cmd = cmd.with_cognito_pool_name(&pool_name);
                            }
                            if let Some(providers) = social_providers {
                                cmd = cmd.with_social_providers(providers.clone());
                            }

                            cmd.execute()
                        } else {
                            // For other targets (pmcp-run, etc.), use the new modular approach
                            // Auto-detect server name from workspace or use package name
                            let server_name = detect_server_name(&project_root)?;
                            let mut config = crate::deployment::DeployConfig::default_for_server(
                                server_name,
                                region.clone(),
                                project_root.clone(),
                            );

                            // Update target type to match the actual target
                            config.target.target_type = target_id.clone();

                            // Configure OAuth if specified (for pmcp-run target)
                            if let Some(provider) = oauth {
                                if provider == "cognito" || provider == "oidc" {
                                    config.auth.enabled = true;
                                    config.auth.provider = provider.clone();

                                    // Set default scopes if not specified
                                    if config.auth.dcr.default_scopes.is_empty() {
                                        config.auth.dcr.default_scopes = vec![
                                            "openid".to_string(),
                                            "email".to_string(),
                                            "mcp/read".to_string(),
                                            "mcp/write".to_string(),
                                        ];
                                    }
                                }
                            }

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
                    DeployAction::Rollback { version, yes: _ } => {
                        let config = crate::deployment::DeployConfig::load(&project_root)?;
                        target.rollback(&config, version.as_deref()).await
                    },
                    DeployAction::Destroy {
                        yes,
                        clean,
                        no_wait,
                    } => {
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

                        // Use async destroy if --no-wait is specified and target supports it
                        if *no_wait && target.supports_async_operations() {
                            let result = target.destroy_async(&config, *clean).await?;
                            if let Some(op) = result.async_operation {
                                println!();
                                println!("⏳ {}", op.message);
                                println!();
                                println!("ℹ️  Destruction initiated. Use the following to check progress:");
                                println!("   cargo pmcp deploy status {}", op.operation_id);
                            } else {
                                println!("✅ {}", result.message);
                            }
                            Ok(())
                        } else {
                            // Default behavior: wait for completion
                            target.destroy(&config, *clean).await
                        }
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
                    DeployAction::Login => {
                        // Login is target-specific
                        match target_id.as_str() {
                            "pmcp-run" => crate::deployment::targets::pmcp_run::login().await,
                            _ => {
                                bail!("Login is not supported for target: {}", target_id);
                            },
                        }
                    },
                    DeployAction::Logout => {
                        // Logout is target-specific
                        match target_id.as_str() {
                            "pmcp-run" => crate::deployment::targets::pmcp_run::logout(),
                            _ => {
                                bail!("Logout is not supported for target: {}", target_id);
                            },
                        }
                    },
                    DeployAction::Oauth { action } => {
                        // OAuth is only supported for pmcp-run target
                        if target_id != "pmcp-run" {
                            bail!("OAuth management is only supported for pmcp-run target");
                        }
                        handle_oauth_action(action).await
                    },
                    DeployAction::Status { operation_id } => {
                        // Status is only supported for targets with async operations
                        if !target.supports_async_operations() {
                            bail!(
                                "Async operation status is not supported for target: {}",
                                target_id
                            );
                        }

                        println!("🔍 Checking operation status...");
                        println!();

                        let status = target.get_operation_status(operation_id).await?;

                        match status.status {
                            crate::deployment::OperationStatus::Initiated => {
                                println!("⏳ Status: Initiated");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Running => {
                                println!("🔄 Status: Running");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Completed => {
                                println!("✅ Status: Completed");
                                println!("   {}", status.message);
                            },
                            crate::deployment::OperationStatus::Failed => {
                                println!("❌ Status: Failed");
                                println!("   {}", status.message);
                            },
                        }

                        if let Some(metadata) = &status.metadata {
                            if let Some(updated_at) = metadata.get("updated_at") {
                                println!();
                                println!("   Last updated: {}", updated_at);
                            }
                        }

                        Ok(())
                    },
                }
            },
            None => {
                // No subcommand = deploy
                let config = crate::deployment::DeployConfig::load(&project_root)?;
                let artifact = target.build(&config).await?;
                let outputs = target.deploy(&config, artifact).await?;

                println!();
                outputs.display();

                // Save deployment info for pmcp-run target (for landing page integration)
                if target_id == "pmcp-run" {
                    Self::save_deployment_info(&project_root, &outputs)?;
                }

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

    /// Save deployment info to .pmcp/deployment.toml for landing page integration
    fn save_deployment_info(
        project_root: &PathBuf,
        outputs: &crate::deployment::DeploymentOutputs,
    ) -> Result<()> {
        use std::io::Write;

        // Extract server_id from custom outputs (the server name, e.g., "chess")
        // NOT deployment_id which is like "dep_xxx" - landing pages use server_id
        let server_id = outputs
            .custom
            .get("server_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("No server_id in outputs"))?;

        // Get URL
        let url = outputs
            .url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No URL in deployment outputs"))?;

        // Create .pmcp directory if it doesn't exist
        let pmcp_dir = project_root.join(".pmcp");
        std::fs::create_dir_all(&pmcp_dir)?;

        // Create deployment.toml content
        let content = format!(
            r#"# Auto-generated deployment info for landing page integration
# This file is created automatically when deploying to pmcp-run

[deployment]
server_id = "{}"
endpoint = "{}"
"#,
            server_id, url
        );

        // Write to .pmcp/deployment.toml
        let deployment_file = pmcp_dir.join("deployment.toml");
        let mut file = std::fs::File::create(&deployment_file)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }
}

/// Handle OAuth subcommands for pmcp.run
async fn handle_oauth_action(action: &OAuthAction) -> Result<()> {
    use crate::deployment::targets::pmcp_run::{auth, graphql};

    // Get credentials
    let credentials = auth::get_credentials().await?;

    match action {
        OAuthAction::Enable {
            server,
            scopes,
            dcr,
            public_clients,
            shared_pool,
        } => {
            println!("🔐 Enabling OAuth for server: {}", server);
            println!();

            let oauth_config = graphql::configure_server_oauth(
                &credentials.access_token,
                server,
                true,
                scopes.clone(),
                Some(*dcr),
                public_clients.clone(),
                shared_pool.clone(),
            )
            .await
            .context("Failed to configure OAuth")?;

            println!("✅ OAuth enabled successfully!");
            println!();
            println!("🔐 OAuth Endpoints:");
            if let Some(ref discovery) = oauth_config.discovery_url {
                println!("   Discovery:     {}", discovery);
            }
            if let Some(ref register) = oauth_config.registration_endpoint {
                println!("   Registration:  {}", register);
            }
            if let Some(ref authorize) = oauth_config.authorization_endpoint {
                println!("   Authorization: {}", authorize);
            }
            if let Some(ref token) = oauth_config.token_endpoint {
                println!("   Token:         {}", token);
            }
            if let Some(ref pool_id) = oauth_config.user_pool_id {
                println!();
                println!("   User Pool ID:  {}", pool_id);
            }
            if let Some(ref region) = oauth_config.user_pool_region {
                println!("   Region:        {}", region);
            }

            Ok(())
        },
        OAuthAction::Disable { server } => {
            println!("🔐 Disabling OAuth for server: {}", server);
            println!();

            graphql::disable_server_oauth(&credentials.access_token, server)
                .await
                .context("Failed to disable OAuth")?;

            println!("✅ OAuth disabled successfully!");
            println!();
            println!("⚠️  Note: The Cognito User Pool was NOT deleted.");
            println!("   You can re-enable OAuth at any time with:");
            println!("   cargo pmcp deploy oauth enable --server {}", server);

            Ok(())
        },
        OAuthAction::Status { server } => {
            println!("🔐 OAuth Status for server: {}", server);
            println!();

            match graphql::fetch_server_oauth_endpoints(&credentials.access_token, server).await {
                Ok(endpoints) => {
                    if endpoints.oauth_enabled {
                        println!("   Status: ✅ Enabled");
                        if let Some(provider) = endpoints.provider {
                            println!("   Provider: {}", provider);
                        }
                        if let Some(dcr) = endpoints.dcr_enabled {
                            println!("   DCR: {}", if dcr { "enabled" } else { "disabled" });
                        }
                        if let Some(scopes) = endpoints.scopes {
                            println!("   Scopes: {}", scopes.join(", "));
                        }
                        println!();
                        println!("🔐 OAuth Endpoints:");
                        if let Some(ref discovery) = endpoints.discovery_url {
                            println!("   Discovery:     {}", discovery);
                        }
                        if let Some(ref register) = endpoints.registration_endpoint {
                            println!("   Registration:  {}", register);
                        }
                        if let Some(ref authorize) = endpoints.authorization_endpoint {
                            println!("   Authorization: {}", authorize);
                        }
                        if let Some(ref token) = endpoints.token_endpoint {
                            println!("   Token:         {}", token);
                        }
                        println!();
                        println!("📋 Cognito Details:");
                        if let Some(ref pool_id) = endpoints.user_pool_id {
                            println!("   User Pool ID:  {}", pool_id);
                        }
                        if let Some(ref region) = endpoints.user_pool_region {
                            println!("   Region:        {}", region);
                        }
                    } else {
                        println!("   Status: ❌ Disabled");
                        println!();
                        println!("💡 Enable OAuth with:");
                        println!("   cargo pmcp deploy oauth enable --server {}", server);
                    }
                },
                Err(_) => {
                    println!("   Status: ❌ Not configured");
                    println!();
                    println!("💡 Enable OAuth with:");
                    println!("   cargo pmcp deploy oauth enable --server {}", server);
                },
            }

            Ok(())
        },
    }
}
