use anyhow::Result;

use crate::deployment::{DeployConfig, DeploymentOutputs};

/// Deploy to AWS Lambda (calls the original DeployExecutor)
pub async fn deploy_aws_lambda(config: &DeployConfig) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to AWS Lambda...");
    println!();

    // Use the existing DeployExecutor
    let executor = crate::commands::deploy::deploy::DeployExecutor::new(config.project_root.clone());
    executor.execute()?;

    // Load and return outputs
    let stack_name = format!("{}-stack", config.server.name);
    crate::deployment::load_cdk_outputs(
        &config.project_root,
        &config.aws.region,
        &stack_name,
    )
}
