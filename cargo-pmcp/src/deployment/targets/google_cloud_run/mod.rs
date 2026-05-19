//! Google Cloud Run deployment target.
//!
//! # Operator workflow
//!
//! Closes upstream issues paiml/rust-mcp-sdk#258 (multi-crate isolated
//! layout), #259 (distroless runtime default), and #260 (deploy.toml
//! schema). The canonical workflow for the pmcp.run team:
//!
//! ```bash
//! # 1. Scaffold the deployment artifacts (Dockerfile, .dockerignore,
//! #    cloudbuild.yaml, .pmcp/deploy.toml). Idempotent — re-running
//! #    preserves existing files.
//! cargo pmcp deploy init --target-type google-cloud-run
//!
//! # 2. Edit .pmcp/deploy.toml to fill in the GCP project, region, and
//! #    any [environment] keys your server requires at startup. For a
//! #    multi-crate isolated layout (e.g. a non-workspace sibling-crate
//! #    test harness), add the [layout] block.
//! cat .pmcp/deploy.toml
//! ```
//!
//! ## Minimum-viable deploy.toml for Cloud Run
//!
//! ```toml
//! [target]
//! type = "google-cloud-run"
//! version = "1.0.0"
//!
//! [gcp]
//! project_id = "my-gcp-project"
//! region = "us-central1"
//!
//! [server]
//! name = "auth-echo-cloud-run"
//! memory = "256Mi"
//! cpu = "1"
//! ingress = "all"
//! allow_unauthenticated = true
//!
//! [environment]
//! EXPECTED_AUDIENCE = "abc.apps.googleusercontent.com"
//! RUST_LOG = "info"
//! ```
//!
//! ## Multi-crate isolated layout (issue #258)
//!
//! For non-workspace sibling crates with path-dep relationships (e.g.
//! a Cloud Run binary crate that declares
//! `auth-echo-core = { path = "../auth-echo-core" }`), add a `[layout]`
//! block. The scaffolder emits surgical per-crate `COPY` lines in the
//! Dockerfile and `cargo build --manifest-path <primary>/Cargo.toml
//! --bin <binary>` instead of `COPY . .` (which would over-bundle any
//! sibling `aws-lambda` crates that intentionally sit outside the
//! workspace).
//!
//! ```toml
//! [layout]
//! kind = "multi-crate-isolated"
//! primary = "gcp-cloud-run"
//! path_deps = ["auth-echo-core"]
//!
//! [server]
//! name = "auth-echo-cloud-run"
//! binary = "server"  # passed as `cargo build --bin server`
//! ```
//!
//! ## Distroless runtime + opt-out (issue #259)
//!
//! The default runtime FROM image is `gcr.io/distroless/cc-debian12`
//! (~20 MB, no shell, no apt, no package manager) — the right default
//! for the cargo-pmcp toolchain shape (rustls is pinned, so no system
//! libssl is needed at runtime). Opt back to a shell-enabled base by
//! setting `[runtime].base`. Declarative apt packages are honored only
//! when the base resolves to a debian-family image:
//!
//! ```toml
//! [runtime]
//! base = "debian:bookworm-slim"
//! apt_packages = ["ca-certificates", "libssl3"]
//! ```
//!
//! ## Deploy
//!
//! ```bash
//! gcloud auth login
//! gcloud config set project my-gcp-project
//! cargo pmcp deploy --target google-cloud-run
//! ```
//!
//! All `gcloud run deploy` flags (`--memory`, `--cpu`,
//! `--allow-unauthenticated`, `--ingress`, `--set-env-vars`, etc.) are
//! sourced from `deploy.toml`. The previous workflow of patching
//! env vars via `gcloud run services update --set-env-vars` after
//! every deploy is no longer required (#260).

mod auth;
mod deploy;
mod dockerfile;
mod env;
mod init;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;

use crate::deployment::{
    r#trait::{
        BuildArtifact, DeploymentOutputs, DeploymentTarget, MetricsData, SecretsAction, TestResults,
    },
    DeployConfig,
};

// Auth functions are used internally

pub struct GoogleCloudRunTarget;

impl GoogleCloudRunTarget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoogleCloudRunTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeploymentTarget for GoogleCloudRunTarget {
    fn id(&self) -> &str {
        "google-cloud-run"
    }

    fn name(&self) -> &str {
        "Google Cloud Run"
    }

    fn description(&self) -> &str {
        "Deploy to Google Cloud Run (managed containers)"
    }

    async fn is_available(&self) -> Result<bool> {
        // Check for required tools
        let has_docker = std::process::Command::new("docker")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        let has_gcloud = std::process::Command::new("gcloud")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        Ok(has_docker && has_gcloud)
    }

    async fn prerequisites(&self) -> Vec<String> {
        let mut missing = Vec::new();

        // Check Docker
        if !std::process::Command::new("docker")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push("Docker (install: https://docs.docker.com/get-docker/)".to_string());
        }

        // Check gcloud CLI
        if !std::process::Command::new("gcloud")
            .args(&["--version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push(
                "Google Cloud SDK (install: https://cloud.google.com/sdk/docs/install)".to_string(),
            );
        }

        // Check authentication
        if auth::check_gcloud_auth().is_err() {
            missing.push("Google Cloud authentication (run: gcloud auth login)".to_string());
        }

        missing
    }

    async fn init(&self, config: &DeployConfig) -> Result<()> {
        init::init_google_cloud_run(config)
    }

    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact> {
        println!("🔨 Building container image for Google Cloud Run...");

        // The build happens during deploy for Cloud Run (Docker build)
        // Return a placeholder artifact
        Ok(BuildArtifact::Custom {
            path: config.project_root.clone(),
            artifact_type: "docker".to_string(),
            deployment_package: None, // Cloud Run uses Docker image, not deployment packages
        })
    }

    async fn deploy(
        &self,
        config: &DeployConfig,
        _artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs> {
        deploy::deploy_to_cloud_run(config).await
    }

    async fn destroy(&self, config: &DeployConfig, clean: bool) -> Result<()> {
        println!("🗑️  Destroying Google Cloud Run deployment...");
        println!();

        // Get project and region
        let project_id = auth::get_project_id()?;
        let region =
            std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| "us-central1".to_string());
        let service_name = &config.server.name;

        // Delete Cloud Run service
        let output = std::process::Command::new("gcloud")
            .args(&[
                "run",
                "services",
                "delete",
                service_name,
                "--region",
                &region,
                "--project",
                &project_id,
                "--quiet",
            ])
            .output()
            .context("Failed to run gcloud run services delete")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to delete Cloud Run service:\n{}", stderr);
        }

        println!("✅ Cloud Run service deleted successfully");

        if clean {
            println!();
            println!("🧹 Cleaning up local files...");

            // Remove generated files
            let files_to_remove = vec!["Dockerfile", ".dockerignore", "cloudbuild.yaml"];
            for file in files_to_remove {
                let path = config.project_root.join(file);
                if path.exists() {
                    std::fs::remove_file(&path).context(format!("Failed to remove {}", file))?;
                    println!("   ✓ Removed {}", file);
                }
            }

            println!();
            println!("✅ All deployment files removed");
        }

        Ok(())
    }

    async fn outputs(&self, config: &DeployConfig) -> Result<DeploymentOutputs> {
        let project_id = auth::get_project_id()?;
        let region =
            std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| "us-central1".to_string());
        let service_name = &config.server.name;

        // Get service URL
        let output = std::process::Command::new("gcloud")
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
            .context("Failed to get Cloud Run service URL")?;

        if !output.status.success() {
            bail!("Failed to get Cloud Run service information");
        }

        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();

        Ok(DeploymentOutputs {
            url: Some(url),
            additional_urls: vec![],
            regions: vec![region],
            stack_name: Some(service_name.clone()),
            version: None,
            custom: std::collections::HashMap::new(),
        })
    }

    async fn logs(&self, config: &DeployConfig, tail: bool, lines: usize) -> Result<()> {
        let project_id = auth::get_project_id()?;
        let _region =
            std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| "us-central1".to_string());
        let service_name = &config.server.name;

        println!("📜 Fetching logs from Google Cloud Run...");
        println!();

        let mut args = vec!["logs", "read", "--project", &project_id];

        let filter = format!(
            "resource.type=cloud_run_revision AND resource.labels.service_name={}",
            service_name
        );
        let limit_str = lines.to_string();

        args.extend(&["--filter", &filter]);
        args.extend(&["--limit", &limit_str]);

        if tail {
            args.push("--tail");
        }

        let output = std::process::Command::new("gcloud")
            .args(&args)
            .output()
            .context("Failed to fetch logs")?;

        if output.status.success() {
            println!("{}", String::from_utf8_lossy(&output.stdout));
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Failed to fetch logs:\n{}", stderr);
        }

        Ok(())
    }

    async fn metrics(&self, config: &DeployConfig, period: &str) -> Result<MetricsData> {
        let _project_id = auth::get_project_id()?;
        let _region =
            std::env::var("CLOUD_RUN_REGION").unwrap_or_else(|_| "us-central1".to_string());
        let _service_name = &config.server.name;

        println!("📊 Google Cloud Run metrics available in Cloud Console");
        println!("   View at: https://console.cloud.google.com/run");

        Ok(MetricsData {
            period: period.to_string(),
            requests: None,
            errors: None,
            avg_latency_ms: None,
            p99_latency_ms: None,
            custom: std::collections::HashMap::new(),
        })
    }

    async fn secrets(&self, _config: &DeployConfig, _action: SecretsAction) -> Result<()> {
        println!("🔐 Use Google Secret Manager for secrets:");
        println!("   1. Create secret: gcloud secrets create SECRET_NAME --data-file=-");
        println!("   2. Grant access: gcloud secrets add-iam-policy-binding SECRET_NAME \\");
        println!("      --member=serviceAccount:SERVICE_ACCOUNT --role=roles/secretmanager.secretAccessor");
        println!("   3. Mount in Cloud Run via --set-secrets flag");

        Ok(())
    }

    async fn test(&self, config: &DeployConfig, _verbose: bool) -> Result<TestResults> {
        println!("🧪 Testing Google Cloud Run deployment...");

        let outputs = self.outputs(config).await?;

        if let Some(url) = outputs.url {
            println!("   Testing endpoint: {}", url);

            let response = reqwest::get(&url).await?;
            let success = response.status().is_success();

            if success {
                println!("✅ Deployment is healthy");
            } else {
                println!("❌ Deployment returned error: {}", response.status());
            }

            Ok(TestResults {
                success,
                tests_run: 1,
                tests_passed: if success { 1 } else { 0 },
                failures: vec![],
            })
        } else {
            bail!("No deployment URL found");
        }
    }

    async fn rollback(&self, _config: &DeployConfig, version: Option<&str>) -> Result<()> {
        println!("🔄 Cloud Run rollback:");
        println!("   Use: gcloud run services update-traffic SERVICE_NAME \\");
        println!(
            "        --to-revisions=REVISION={}",
            version.unwrap_or("100")
        );

        Ok(())
    }
}
