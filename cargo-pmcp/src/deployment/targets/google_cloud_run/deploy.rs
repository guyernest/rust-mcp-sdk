use super::auth;
use crate::deployment::{r#trait::DeploymentOutputs, DeployConfig};
use anyhow::{bail, Context, Result};

/// Deploy Rust MCP server to Google Cloud Run.
///
/// Flow:
/// 1. Verify authentication.
/// 2. Resolve deployment parameters from `.pmcp/deploy.toml`
///    (`[gcp]`, `[server]`, `[environment]`), with legacy env-var fallback
///    for projects that pre-date the schema. Closes upstream issue #260.
/// 3. Build the Docker image (`docker buildx --platform linux/amd64`).
/// 4. Push to Google Container Registry.
/// 5. `gcloud run deploy` with `--set-env-vars` populated from
///    `[environment]`.
/// 6. Return the deployment URL.
pub async fn deploy_to_cloud_run(config: &DeployConfig) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to Google Cloud Run...");
    println!();

    // Step 1: Authentication. The deploy.toml carries the *expected* project
    // id; the actual project id is whatever the operator has gcloud
    // configured for. We prefer the deploy.toml value when present so the
    // CLI is reproducible across machines.
    println!("🔐 Verifying authentication...");
    auth::check_gcloud_auth().context("Not authenticated with Google Cloud")?;
    let project_id = config
        .gcp
        .as_ref()
        .map(|g| g.project_id.clone())
        .filter(|p| !p.is_empty() && p != "your-gcp-project-id")
        .map_or_else(auth::get_project_id, Ok)?;
    println!("   ✓ Project: {}", project_id);
    println!();

    let params = resolve_params(config);

    println!("📋 Deployment configuration:");
    println!("   Region: {}", params.region);
    println!("   Service: {}", params.service_name);
    println!("   Memory: {}", params.memory);
    println!("   CPU: {}", params.cpu);
    println!("   Max instances: {}", params.max_instances);
    println!("   Min instances: {}", params.min_instances);
    println!("   Allow unauthenticated: {}", params.allow_unauth);
    if let Some(ingress) = &params.ingress {
        println!("   Ingress: {}", ingress);
    }
    if !config.environment.is_empty() {
        println!(
            "   Environment variables: {} entries from [environment]",
            config.environment.len()
        );
    }
    println!();

    // Step 3: Path-dep sanity.
    //
    // For multi-crate-isolated layouts, `project_root` is intentionally a
    // parent directory of the primary crate (so multiple sibling crates can
    // be COPY'd into the Docker build context) and has no `Cargo.toml` of
    // its own. Sanity-check the primary crate's manifest instead. For other
    // layouts, the project_root is itself a Cargo package and we check that
    // directly (unchanged behavior).
    let cargo_toml_path = match config.layout.as_ref().filter(|l| l.kind == "multi-crate-isolated") {
        Some(layout) => config.project_root.join(&layout.primary).join("Cargo.toml"),
        None => config.project_root.join("Cargo.toml"),
    };
    let cargo_toml = std::fs::read_to_string(&cargo_toml_path)
        .with_context(|| format!("Failed to read {}", cargo_toml_path.display()))?;

    if cargo_toml.contains("path = \"/") || cargo_toml.contains("path = \"~") {
        println!("   ⚠ Warning: Detected absolute path dependencies in Cargo.toml");
        println!("   These will not work inside Docker build context.");
        bail!(
            "Cannot deploy with absolute path dependencies. \
             Use crates.io dependencies or relative paths."
        );
    }

    // Step 4: docker buildx.
    println!("🔨 Building Docker image for linux/amd64...");
    let image_tag = format!("gcr.io/{}/{}:latest", project_id, params.service_name);

    let build_output = std::process::Command::new("docker")
        .current_dir(&config.project_root)
        .args([
            "buildx",
            "build",
            "--platform",
            "linux/amd64",
            "-t",
            &image_tag,
            "--load",
            ".",
        ])
        .output()
        .context("Failed to run docker buildx")?;

    if !build_output.status.success() {
        let stderr = String::from_utf8_lossy(&build_output.stderr);
        bail!("Docker build failed:\n{}", stderr);
    }

    println!("   ✓ Image built: {}", image_tag);
    println!();

    // Step 5: Push to GCR.
    println!("📤 Pushing image to Google Container Registry...");
    let auth_output = std::process::Command::new("gcloud")
        .args(["auth", "configure-docker", "--quiet"])
        .output()
        .context("Failed to configure docker authentication")?;
    if !auth_output.status.success() {
        let stderr = String::from_utf8_lossy(&auth_output.stderr);
        bail!("Failed to configure Docker authentication:\n{}", stderr);
    }

    let push_output = std::process::Command::new("docker")
        .args(["push", &image_tag])
        .output()
        .context("Failed to push docker image")?;
    if !push_output.status.success() {
        let stderr = String::from_utf8_lossy(&push_output.stderr);
        bail!("Docker push failed:\n{}", stderr);
    }
    println!("   ✓ Image pushed to GCR");
    println!();

    // Step 6: gcloud run deploy with --set-env-vars populated from
    // config.environment (closes the env-var-drift gap in #260).
    println!("🚀 Deploying to Cloud Run...");

    let max_instances_str = params.max_instances.to_string();
    let min_instances_str = params.min_instances.to_string();
    let env_vars_arg = super::env::render_set_env_vars(&config.environment);

    let mut deploy_args = vec![
        "run",
        "deploy",
        &params.service_name,
        "--image",
        &image_tag,
        "--region",
        &params.region,
        "--project",
        &project_id,
        "--platform",
        "managed",
        "--memory",
        &params.memory,
        "--cpu",
        &params.cpu,
        "--max-instances",
        &max_instances_str,
        "--min-instances",
        &min_instances_str,
        "--port",
        "8080",
        "--quiet",
    ];

    if let Some(ingress) = &params.ingress {
        deploy_args.push("--ingress");
        deploy_args.push(ingress);
    }

    if !env_vars_arg.is_empty() {
        deploy_args.push("--set-env-vars");
        deploy_args.push(&env_vars_arg);
    }

    if params.allow_unauth {
        deploy_args.push("--allow-unauthenticated");
    } else {
        deploy_args.push("--no-allow-unauthenticated");
    }

    let deploy_output = std::process::Command::new("gcloud")
        .args(&deploy_args)
        .output()
        .context("Failed to deploy to Cloud Run")?;

    if !deploy_output.status.success() {
        let stderr = String::from_utf8_lossy(&deploy_output.stderr);
        bail!("Cloud Run deployment failed:\n{}", stderr);
    }
    println!("   ✓ Service deployed successfully");
    println!();

    // Step 7: URL.
    println!("🔍 Getting service URL...");
    let url_output = std::process::Command::new("gcloud")
        .args([
            "run",
            "services",
            "describe",
            &params.service_name,
            "--region",
            &params.region,
            "--project",
            &project_id,
            "--format",
            "value(status.url)",
        ])
        .output()
        .context("Failed to get service URL")?;
    if !url_output.status.success() {
        bail!("Failed to retrieve service URL");
    }
    let url = String::from_utf8_lossy(&url_output.stdout)
        .trim()
        .to_string();

    println!("🎉 Deployment successful!");
    println!();
    println!("📊 Deployment Details:");
    println!("   Project: {}", project_id);
    println!("   Region: {}", params.region);
    println!("   Service: {}", params.service_name);
    println!("   URL: {}", url);

    if !params.allow_unauth {
        println!();
        println!("🔒 Authentication required:");
        println!("   gcloud run services proxy {} --region {}", params.service_name, params.region);
    }

    Ok(DeploymentOutputs {
        url: Some(url),
        additional_urls: vec![],
        regions: vec![params.region],
        stack_name: Some(params.service_name),
        version: None,
        custom: {
            let mut custom = std::collections::HashMap::new();
            custom.insert(
                "project_id".to_string(),
                serde_json::Value::String(project_id),
            );
            custom.insert("image".to_string(), serde_json::Value::String(image_tag));
            custom
        },
    })
}

/// Resolved deployment parameters with deploy.toml-then-env-var precedence.
///
/// `config.server.*` is the new source of truth (issue #260). Env vars
/// remain as a fallback only for projects whose deploy.toml pre-dates the
/// Cloud Run schema additions.
struct CloudRunParams {
    region: String,
    service_name: String,
    memory: String,
    cpu: String,
    max_instances: u32,
    min_instances: u32,
    allow_unauth: bool,
    ingress: Option<String>,
}

fn resolve_params(config: &DeployConfig) -> CloudRunParams {
    let region = config
        .gcp
        .as_ref()
        .map(|g| g.region.clone())
        .filter(|r| !r.is_empty())
        .or_else(|| std::env::var("CLOUD_RUN_REGION").ok())
        .unwrap_or_else(auth::get_region);

    let memory = config
        .server
        .memory
        .clone()
        .or_else(|| std::env::var("CLOUD_RUN_MEMORY").ok())
        .unwrap_or_else(|| "512Mi".to_string());

    let cpu = config
        .server
        .cpu
        .clone()
        .or_else(|| std::env::var("CLOUD_RUN_CPU").ok())
        .unwrap_or_else(|| "1".to_string());

    let max_instances = config
        .server
        .max_instances
        .or_else(|| std::env::var("CLOUD_RUN_MAX_INSTANCES").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(10);

    let min_instances = config.server.min_instances.unwrap_or(0);

    let allow_unauth = config
        .server
        .allow_unauthenticated
        .or_else(|| {
            std::env::var("CLOUD_RUN_ALLOW_UNAUTHENTICATED")
                .ok()
                .map(|v| v == "true")
        })
        .unwrap_or(true);

    CloudRunParams {
        region,
        service_name: config.server.name.clone(),
        memory,
        cpu,
        max_instances,
        min_instances,
        allow_unauth,
        ingress: config.server.ingress.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_params_prefers_config_over_env_var() {
        let config = DeployConfig::default_for_cloud_run_server(
            "test-server".to_string(),
            "test-project".to_string(),
            "europe-west1".to_string(),
            std::path::PathBuf::from("/tmp"),
        );
        let params = resolve_params(&config);
        assert_eq!(params.region, "europe-west1");
        assert_eq!(params.memory, "256Mi");
        assert_eq!(params.cpu, "1");
        assert_eq!(params.max_instances, 10);
        assert_eq!(params.min_instances, 0);
        assert!(params.allow_unauth);
        assert_eq!(params.ingress.as_deref(), Some("all"));
        assert_eq!(params.service_name, "test-server");
    }
}
