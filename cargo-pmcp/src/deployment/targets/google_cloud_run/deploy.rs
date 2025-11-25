use super::auth;
use crate::deployment::{r#trait::DeploymentOutputs, DeployConfig};
use anyhow::{bail, Context, Result};

/// Deploy Rust MCP server to Google Cloud Run
///
/// Flow:
/// 1. Verify authentication
/// 2. Build Docker image
/// 3. Push to Google Container Registry
/// 4. Deploy to Cloud Run
/// 5. Return deployment URL
pub async fn deploy_to_cloud_run(config: &DeployConfig) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to Google Cloud Run...");
    println!();

    // Step 1: Verify authentication and get project
    println!("🔐 Verifying authentication...");
    auth::check_gcloud_auth().context("Not authenticated with Google Cloud")?;
    let project_id = auth::get_project_id()?;
    println!("   ✓ Project: {}", project_id);
    println!();

    // Step 2: Get configuration
    let region = std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| auth::get_region());
    let service_name = &config.server.name;
    let memory = std::env::var("CLOUD_RUN_MEMORY").unwrap_or_else(|_| "512Mi".to_string());
    let cpu = std::env::var("CLOUD_RUN_CPU").unwrap_or_else(|_| "1".to_string());
    let max_instances =
        std::env::var("CLOUD_RUN_MAX_INSTANCES").unwrap_or_else(|_| "10".to_string());
    let allow_unauth = std::env::var("CLOUD_RUN_ALLOW_UNAUTHENTICATED")
        .unwrap_or_else(|_| "true".to_string())
        == "true";

    println!("📋 Deployment configuration:");
    println!("   Region: {}", region);
    println!("   Service: {}", service_name);
    println!("   Memory: {}", memory);
    println!("   CPU: {}", cpu);
    println!("   Max instances: {}", max_instances);
    println!("   Allow unauthenticated: {}", allow_unauth);
    println!();

    // Step 3: Prepare for Docker build
    println!("📦 Preparing for containerized build...");

    // Check if Cargo.toml has absolute path dependencies
    let cargo_toml_path = config.project_root.join("Cargo.toml");
    let cargo_toml =
        std::fs::read_to_string(&cargo_toml_path).context("Failed to read Cargo.toml")?;

    if cargo_toml.contains("path = \"/") || cargo_toml.contains("path = \"~") {
        println!("   ⚠ Warning: Detected absolute path dependencies in Cargo.toml");
        println!("   These will not work inside Docker build context.");
        println!("   Consider publishing dependencies to crates.io or using relative paths.");
        println!();
        bail!("Cannot deploy with absolute path dependencies. Please use crates.io dependencies or set up the project with relative paths.");
    }

    println!("   Docker will build the binary for x86_64 Linux inside the container");
    println!();

    // Step 4: Build Docker image for x86_64/AMD64 (Cloud Run architecture)
    println!("🔨 Building Docker image for linux/amd64...");
    let image_tag = format!("gcr.io/{}/{}:latest", project_id, service_name);

    // Use buildx to cross-compile for amd64
    let build_output = std::process::Command::new("docker")
        .current_dir(&config.project_root)
        .args(&[
            "buildx",
            "build",
            "--platform",
            "linux/amd64",
            "-t",
            &image_tag,
            "--load", // Load into local docker
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

    // Step 4: Push to Google Container Registry
    println!("📤 Pushing image to Google Container Registry...");

    // Configure Docker to use gcloud as credential helper
    let auth_output = std::process::Command::new("gcloud")
        .args(&["auth", "configure-docker", "--quiet"])
        .output()
        .context("Failed to configure docker authentication")?;

    if !auth_output.status.success() {
        let stderr = String::from_utf8_lossy(&auth_output.stderr);
        bail!("Failed to configure Docker authentication:\n{}", stderr);
    }

    let push_output = std::process::Command::new("docker")
        .args(&["push", &image_tag])
        .output()
        .context("Failed to push docker image")?;

    if !push_output.status.success() {
        let stderr = String::from_utf8_lossy(&push_output.stderr);
        bail!("Docker push failed:\n{}", stderr);
    }

    println!("   ✓ Image pushed to GCR");
    println!();

    // Step 5: Deploy to Cloud Run
    println!("🚀 Deploying to Cloud Run...");

    let mut deploy_args = vec![
        "run",
        "deploy",
        service_name,
        "--image",
        &image_tag,
        "--region",
        &region,
        "--project",
        &project_id,
        "--platform",
        "managed",
        "--memory",
        &memory,
        "--cpu",
        &cpu,
        "--max-instances",
        &max_instances,
        "--min-instances",
        "0",
        "--port",
        "8080",
        "--quiet",
    ];

    if allow_unauth {
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

    // Step 6: Get service URL
    println!("🔍 Getting service URL...");
    let url_output = std::process::Command::new("gcloud")
        .args(&[
            "run",
            "services",
            "describe",
            service_name,
            "--region",
            &region,
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

    // Display deployment information
    println!("🎉 Deployment successful!");
    println!();
    println!("📊 Deployment Details:");
    println!("   Project: {}", project_id);
    println!("   Region: {}", region);
    println!("   Service: {}", service_name);
    println!("   URL: {}", url);
    println!();

    if !allow_unauth {
        println!("🔒 Authentication required:");
        println!("   This service requires authentication.");
        println!("   To invoke locally:");
        println!(
            "   gcloud run services proxy {} --region {}",
            service_name, region
        );
        println!();
    }

    println!("💡 Next steps:");
    println!("   • View logs: cargo pmcp deploy logs --target google-cloud-run");
    println!("   • Test deployment: cargo pmcp deploy test --target google-cloud-run");
    println!("   • View in console: https://console.cloud.google.com/run");
    println!();

    Ok(DeploymentOutputs {
        url: Some(url),
        additional_urls: vec![],
        regions: vec![region],
        stack_name: Some(service_name.clone()),
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
