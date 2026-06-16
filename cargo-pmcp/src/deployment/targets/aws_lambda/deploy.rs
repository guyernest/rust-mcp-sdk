use anyhow::Result;
use std::collections::HashMap;

use crate::deployment::{DeployConfig, DeploymentOutputs};

/// Deploy to AWS Lambda (calls the original DeployExecutor).
///
/// Resolved secrets are passed via `extra_env` and forwarded as transient
/// process env vars to the CDK child process. They are **never** written
/// to `deploy.toml` (per D-05/D-06).
pub async fn deploy_aws_lambda(
    config: &DeployConfig,
    extra_env: HashMap<String, String>,
) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to AWS Lambda...");
    println!();

    // Use the existing DeployExecutor with transient secret env vars.
    // Thread the `--regenerate-stack`/`--force` opt-in (DSTK-01): execute()
    // re-loads DeployConfig from disk (dropping the `#[serde(skip)]` runtime
    // flag), so it is carried on the executor and re-applied there.
    let executor =
        crate::commands::deploy::deploy::DeployExecutor::new(config.project_root.clone())
            .with_extra_env(extra_env)
            .with_regenerate_stack(config.regenerate_stack);
    executor.execute()?;

    // Load and return outputs
    let stack_name = format!("{}-stack", config.server.name);
    crate::deployment::load_cdk_outputs(&config.project_root, &config.aws().region, &stack_name)
}
