use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::deployment::{
    metadata::McpMetadata,
    r#trait::{BuildArtifact, DeploymentOutputs},
    DeployConfig,
};

use super::{auth, graphql};

/// Extract the server version from the Cargo workspace.
///
/// Uses `cargo metadata` which properly handles:
/// 1. Workspace root versions
/// 2. Package versions
/// 3. Workspace inheritance (`version.workspace = true`)
///
/// Returns None if version cannot be determined.
fn extract_version_from_cargo(project_root: &Path) -> Option<String> {
    // Use cargo metadata to get package information - this handles all Cargo.toml formats
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(project_root)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

    // Get workspace root to find the "main" package
    let workspace_root = metadata.get("workspace_root")?.as_str()?;

    // Find packages in the workspace
    let packages = metadata.get("packages")?.as_array()?;

    // Strategy: prefer package at workspace root, then any package with version
    let mut root_package_version: Option<String> = None;
    let mut any_version: Option<String> = None;

    for package in packages {
        let version = match package.get("version").and_then(|v| v.as_str()) {
            Some(v) => v,
            None => continue,
        };
        let manifest_path = package
            .get("manifest_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Check if this package's Cargo.toml is at workspace root
        if !manifest_path.is_empty() && manifest_path.starts_with(workspace_root) {
            let relative = &manifest_path[workspace_root.len()..];
            // Package at root has manifest at /Cargo.toml (just one path component)
            if relative == "/Cargo.toml" || relative.matches('/').count() == 1 {
                root_package_version = Some(version.to_string());
            }
        }

        if any_version.is_none() {
            any_version = Some(version.to_string());
        }
    }

    // Prefer root package version, fallback to any package version
    root_package_version.or(any_version)
}

/// Deploy to pmcp.run managed service using 3-step flow:
/// 1. Get presigned S3 URLs
/// 2. Upload files directly to S3
/// 3. Create deployment from S3 files
pub async fn deploy_to_pmcp_run(
    config: &DeployConfig,
    artifact: BuildArtifact,
) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to pmcp.run...");
    println!();

    // Get credentials (OAuth tokens)
    let credentials = auth::get_credentials().await?;

    // Paths
    let deploy_dir = config.project_root.join("deploy");
    let cdk_out = deploy_dir.join("cdk.out");

    // Step 0: Extract MCP metadata for the CloudFormation template
    println!("📋 Extracting MCP server metadata...");
    let metadata = match McpMetadata::extract(&config.project_root) {
        Ok(m) => {
            println!("   Server: {} ({})", m.server_id, m.server_type);
            if !m.resources.secrets.is_empty() {
                println!("   Secrets: {}", m.resources.secrets.len());
            }
            if !m.capabilities.tools.is_empty() {
                println!("   Tools: {}", m.capabilities.tools.len());
            }
            Some(m)
        }
        Err(_) => {
            println!("   No metadata found (using defaults)");
            None
        }
    };

    // Step 1: Synthesize CloudFormation template with metadata context
    println!("📝 Synthesizing CloudFormation template...");

    // Use shell to run npx/cdk to ensure PATH is correctly set
    let shell_cmd = if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "sh"
    };
    let shell_arg = if cfg!(target_os = "windows") {
        "/C"
    } else {
        "-c"
    };

    // Build CDK synth command with metadata context
    let cdk_context_args = metadata
        .as_ref()
        .map(|m| m.to_cdk_context().join(" "))
        .unwrap_or_default();

    let synth_command = if cdk_context_args.is_empty() {
        "npx cdk synth --quiet".to_string()
    } else {
        format!("npx cdk synth --quiet {}", cdk_context_args)
    };

    let synth_output = std::process::Command::new(shell_cmd)
        .current_dir(&deploy_dir)
        .arg(shell_arg)
        .arg(&synth_command)
        .output()
        .context("Failed to run cdk synth. Make sure Node.js and npm are installed")?;

    if !synth_output.status.success() {
        let stderr = String::from_utf8_lossy(&synth_output.stderr);
        bail!("CDK synthesis failed:\n{}", stderr);
    }

    println!("✅ CloudFormation template synthesized");

    // Step 2: Find the synthesized template
    let template_path = find_template_file(&cdk_out)?;
    println!("   Template: {}", template_path.display());

    // Step 3: Extract bootstrap binary path and deployment package from artifact
    let (bootstrap_path, deployment_package) = match artifact {
        BuildArtifact::Binary {
            path,
            deployment_package,
            ..
        } => (path, deployment_package),
        BuildArtifact::Wasm {
            path,
            deployment_package,
            ..
        } => (path, deployment_package),
        BuildArtifact::Custom {
            path,
            deployment_package,
            ..
        } => (path, deployment_package),
    };

    // Determine what to upload: deployment package (zip with assets) or raw binary
    let (bootstrap_data, bootstrap_content_type, has_assets) = if let Some(ref package_path) =
        deployment_package
    {
        if package_path.exists() {
            println!("   📦 Using deployment package with assets");
            println!("   Package: {}", package_path.display());
            let data = std::fs::read(package_path).context("Failed to read deployment package")?;
            (data, "application/zip", true)
        } else {
            // Fall back to raw binary if package doesn't exist
            if !bootstrap_path.exists() {
                bail!("Bootstrap binary not found: {}", bootstrap_path.display());
            }
            println!("   Bootstrap: {}", bootstrap_path.display());
            let data = std::fs::read(&bootstrap_path).context("Failed to read bootstrap binary")?;
            (data, "application/octet-stream", false)
        }
    } else {
        if !bootstrap_path.exists() {
            bail!("Bootstrap binary not found: {}", bootstrap_path.display());
        }
        println!("   Bootstrap: {}", bootstrap_path.display());
        let data = std::fs::read(&bootstrap_path).context("Failed to read bootstrap binary")?;
        (data, "application/octet-stream", false)
    };

    println!();

    // Step 4: Read template file
    let template = std::fs::read_to_string(&template_path)
        .context("Failed to read CloudFormation template")?;

    println!("📦 Template size: {} KB", template.len() / 1024);
    if has_assets {
        println!(
            "📦 Deployment package size: {} KB",
            bootstrap_data.len() / 1024
        );
    } else {
        println!("📦 Bootstrap size: {} KB", bootstrap_data.len() / 1024);
    }
    println!();

    // Step 5: Get presigned S3 URLs from GraphQL
    println!("🔑 Getting upload URLs from pmcp.run...");
    let urls = graphql::get_upload_urls(
        &credentials.access_token,
        &config.server.name,
        template.len(),
        bootstrap_data.len(),
    )
    .await
    .context("Failed to get upload URLs")?;

    println!("   URLs expire in {} seconds", urls.expires_in);
    println!();

    // Step 6: Upload files to S3 in parallel
    println!("⬆️  Uploading files to S3...");

    let template_bytes = template.into_bytes();
    let (template_result, bootstrap_result) = tokio::join!(
        graphql::upload_to_s3(
            &urls.template_upload_url,
            template_bytes,
            "application/json"
        ),
        graphql::upload_to_s3(
            &urls.bootstrap_upload_url,
            bootstrap_data,
            bootstrap_content_type
        )
    );

    template_result.context("Template upload to S3 failed")?;
    bootstrap_result.context("Bootstrap upload to S3 failed")?;

    println!("✅ Files uploaded successfully to S3");
    println!();

    // Step 7: Create deployment via GraphQL with composition settings and version
    println!("🚀 Creating deployment...");

    // Extract version from Cargo.toml (supports workspace inheritance)
    let server_version = extract_version_from_cargo(&config.project_root);
    if let Some(ref version) = server_version {
        println!("   Version: {}", version);
    }

    let composition = graphql::CompositionSettings {
        tier: config.composition.tier.clone(),
        allow_composition: config.composition.allow_composition,
        internal_only: config.composition.internal_only,
        description: config.composition.description.clone(),
        server_version,
    };
    let deployment = graphql::create_deployment_from_s3_with_composition(
        &credentials.access_token,
        &urls,
        &config.server.name,
        composition,
    )
    .await
    .context("Failed to create deployment")?;

    println!("   Deployment ID: {}", deployment.deployment_id);
    println!();

    // Step 8: Poll deployment status (wait for completion)
    let deployment_outputs =
        poll_deployment_status(&credentials.access_token, &deployment.deployment_id)
            .await
            .context("Deployment failed")?;

    // Step 9: Configure OAuth if enabled in local config
    let oauth_config = if config.auth.enabled {
        println!("🔐 Configuring OAuth for MCP server...");

        let scopes = if config.auth.dcr.default_scopes.is_empty() {
            None
        } else {
            Some(config.auth.dcr.default_scopes.clone())
        };

        let public_patterns = if config.auth.dcr.public_client_patterns.is_empty() {
            None
        } else {
            Some(config.auth.dcr.public_client_patterns.clone())
        };

        match graphql::configure_server_oauth(
            &credentials.access_token,
            &deployment.deployment_id,
            true,
            scopes,
            Some(config.auth.dcr.enabled),
            public_patterns,
            None, // shared_pool_name - not supported in local config yet
        )
        .await
        {
            Ok(oauth) => {
                println!("✅ OAuth configured successfully");
                println!();
                Some(oauth)
            },
            Err(e) => {
                eprintln!("⚠️  Failed to configure OAuth: {}", e);
                eprintln!("   You can manually enable OAuth with:");
                eprintln!(
                    "   cargo pmcp oauth enable --server {}",
                    deployment.deployment_id
                );
                println!();
                None
            },
        }
    } else {
        // Even if local config doesn't enable OAuth, check if it's enabled on the backend
        // (e.g., from a previous `cargo pmcp deploy oauth enable` command)
        // Use server name (e.g., "true-agent") not deployment_id - OAuth is keyed by serverId
        match graphql::fetch_server_oauth_endpoints(
            &credentials.access_token,
            &config.server.name,
        )
        .await
        {
            Ok(oauth) => {
                if oauth.oauth_enabled {
                    // Convert OAuthEndpoints to OAuthConfig
                    Some(graphql::OAuthConfig {
                        server_id: oauth.server_id,
                        oauth_enabled: oauth.oauth_enabled,
                        user_pool_id: oauth.user_pool_id,
                        user_pool_region: oauth.user_pool_region,
                        discovery_url: oauth.discovery_url,
                        registration_endpoint: oauth.registration_endpoint,
                        authorization_endpoint: oauth.authorization_endpoint,
                        token_endpoint: oauth.token_endpoint,
                    })
                } else {
                    eprintln!("   (OAuth query returned oauthEnabled=false for {})", config.server.name);
                    None
                }
            }
            Err(e) => {
                eprintln!("   (OAuth status check failed for {}: {})", config.server.name, e);
                None
            }
        }
    };

    // Use URL from server response (contains stable serverId-based URL)
    // Fallback to constructing from deployment ID if not provided
    let mcp_url = deployment_outputs
        .url
        .clone()
        .unwrap_or_else(|| format!("https://api.pmcp.run/{}/mcp", deployment.deployment_id));
    let health_url = mcp_url.replace("/mcp", "/health");

    // Get server_id from the deployment outputs (projectName returned by backend)
    // This is the clean server name like "chess", not the full URL
    let server_id = deployment_outputs
        .custom
        .get("project_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&config.server.name);

    // Display deployment information
    println!("🎉 Deployment successful!");
    println!();
    println!("📊 Deployment Details:");
    println!("   Name: {}", config.server.name);
    println!("   Server ID: {}", server_id);
    println!("   Deployment ID: {}", deployment.deployment_id);

    // Display endpoints based on OAuth status
    if let Some(ref oauth) = oauth_config {
        // OAuth-protected deployment
        println!();
        println!("🔐 MCP Endpoint (OAuth Protected):");
        println!("   URL: {}", mcp_url);
        println!();
        println!("🔑 OAuth Configuration:");
        if let Some(ref discovery) = oauth.discovery_url {
            println!("   Discovery:     {}", discovery);
        }
        if let Some(ref register) = oauth.registration_endpoint {
            println!("   Registration:  {}", register);
        }
        if let Some(ref authorize) = oauth.authorization_endpoint {
            println!("   Authorization: {}", authorize);
        }
        if let Some(ref token) = oauth.token_endpoint {
            println!("   Token:         {}", token);
        }
        println!();
        println!("🏥 Health Check:");
        println!("   URL: {}", health_url);
        println!();
        println!("Clients must authenticate via OAuth to access this server.");
    } else {
        // No OAuth - open access
        println!();
        println!("🔌 MCP Endpoint:");
        println!("   URL: {}", mcp_url);
        println!();
        println!("🏥 Health Check:");
        println!("   URL: {}", health_url);
        println!();
        println!("No authentication required. Anyone can access this server.");
        println!(
            "To enable OAuth: cargo pmcp oauth enable {}",
            deployment.deployment_id
        );
    }

    println!();
    println!("💡 Next steps:");
    println!("   • View logs: cargo pmcp deploy logs --target pmcp-run");
    println!("   • Test deployment: cargo pmcp deploy test --target pmcp-run");
    println!("   • View dashboard: https://pmcp.run/dashboard");
    println!();

    // Build outputs with shared API Gateway URL pattern
    let mut outputs_with_id = DeploymentOutputs {
        // Primary URL is always the shared API Gateway
        url: Some(mcp_url.clone()),
        additional_urls: vec![health_url.clone()],
        regions: vec![],
        stack_name: None,
        version: None,
        custom: std::collections::HashMap::new(),
    };

    outputs_with_id.custom.insert(
        "server_id".to_string(),
        serde_json::Value::String(server_id.to_string()),
    );
    outputs_with_id.custom.insert(
        "deployment_id".to_string(),
        serde_json::Value::String(deployment.deployment_id.clone()),
    );
    outputs_with_id.custom.insert(
        "mcp_endpoint".to_string(),
        serde_json::Value::String(mcp_url),
    );
    outputs_with_id.custom.insert(
        "health_endpoint".to_string(),
        serde_json::Value::String(health_url),
    );

    if let Some(oauth) = oauth_config {
        outputs_with_id.custom.insert(
            "oauth_enabled".to_string(),
            serde_json::Value::Bool(oauth.oauth_enabled),
        );
        if let Some(discovery) = oauth.discovery_url {
            outputs_with_id.custom.insert(
                "oauth_discovery_url".to_string(),
                serde_json::Value::String(discovery),
            );
        }
        if let Some(pool_id) = oauth.user_pool_id {
            outputs_with_id.custom.insert(
                "cognito_user_pool_id".to_string(),
                serde_json::Value::String(pool_id),
            );
        }
    } else {
        outputs_with_id
            .custom
            .insert("oauth_enabled".to_string(), serde_json::Value::Bool(false));
    }

    Ok(outputs_with_id)
}

/// Poll deployment status until complete or failed
async fn poll_deployment_status(
    access_token: &str,
    deployment_id: &str,
) -> Result<DeploymentOutputs> {
    println!("⏳ Waiting for deployment to complete...");

    let mut dots = 0;

    loop {
        let status = graphql::get_deployment(access_token, deployment_id).await?;

        match status.status.as_str() {
            "pending" | "validating" | "deploying" => {
                print!(".");
                dots += 1;
                if dots >= 60 {
                    println!();
                    dots = 0;
                }
                std::io::Write::flush(&mut std::io::stdout())?;
                tokio::time::sleep(Duration::from_secs(2)).await;
            },
            "success" => {
                if dots > 0 {
                    println!();
                }
                println!("✅ Deployment completed successfully!");

                // Debug: Log the URL from server response
                if let Some(ref url) = status.url {
                    println!("   Server URL: {}", url);
                } else {
                    println!("   ⚠️  Server did not return URL");
                }
                println!();

                // Include project_name in outputs for use by save_deployment_info
                let mut custom = std::collections::HashMap::new();
                custom.insert(
                    "project_name".to_string(),
                    serde_json::Value::String(status.project_name),
                );

                return Ok(DeploymentOutputs {
                    url: status.url,
                    additional_urls: vec![],
                    regions: vec![],
                    stack_name: None,
                    version: None,
                    custom,
                });
            },
            "failed" => {
                if dots > 0 {
                    println!();
                }
                bail!(
                    "Deployment failed: {}",
                    status
                        .error_message
                        .unwrap_or_else(|| "Unknown error".to_string())
                );
            },
            _ => {
                bail!("Unknown deployment status: {}", status.status);
            },
        }
    }
}

/// Find the CloudFormation template file in cdk.out directory
fn find_template_file(cdk_out: &PathBuf) -> Result<PathBuf> {
    if !cdk_out.exists() {
        bail!("CDK output directory not found: {}", cdk_out.display());
    }

    // Look for *.template.json files
    let entries = std::fs::read_dir(cdk_out).context("Failed to read cdk.out directory")?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                let file_name_str = file_name.to_string_lossy();
                if file_name_str.ends_with(".template.json") {
                    return Ok(path);
                }
            }
        }
    }

    bail!("No CloudFormation template found in {}", cdk_out.display());
}
