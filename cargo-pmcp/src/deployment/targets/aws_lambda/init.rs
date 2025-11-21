use crate::deployment::DeployConfig;
use anyhow::Result;

/// Initialize AWS Lambda deployment (calls the original InitCommand)
pub async fn init_aws_lambda(config: &DeployConfig) -> Result<()> {
    // Use the existing InitCommand from commands/deploy/init.rs
    let init_cmd = crate::commands::deploy::init::InitCommand::new(config.project_root.clone())
        .with_region(&config.aws.region)
        .with_credentials_check(true);

    init_cmd.execute()
}
