mod auth;
mod deploy;
mod dockerfile;

use anyhow::{bail, Context, Result};
use async_trait::async_trait;

use crate::deployment::{
    r#trait::{
        BuildArtifact, DeploymentOutputs, DeploymentTarget, MetricsData, SecretsAction, TestResults,
    },
    DeployConfig,
};

/// Deploy target: Azure Container Apps (container-based, ACR cloud-build).
///
/// Closest analog to the Google Cloud Run target — both build a Docker image and
/// run it as a managed container service. Uses `az containerapp up --source`,
/// which cloud-builds the Dockerfile in ACR (no local Docker required).
/// Validated end-to-end in spike 007.
pub struct AzureContainerAppsTarget;

impl AzureContainerAppsTarget {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AzureContainerAppsTarget {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DeploymentTarget for AzureContainerAppsTarget {
    fn id(&self) -> &str {
        "azure-container-apps"
    }

    fn name(&self) -> &str {
        "Azure Container Apps"
    }

    fn description(&self) -> &str {
        "Deploy to Azure Container Apps (managed containers, ACR cloud-build)"
    }

    async fn is_available(&self) -> Result<bool> {
        // Only the Azure CLI is required — ACR builds in the cloud, so no local Docker.
        let has_az = std::process::Command::new("az")
            .args(["version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        Ok(has_az)
    }

    async fn prerequisites(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if !std::process::Command::new("az")
            .args(["version"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            missing.push(
                "Azure CLI >= 2.62 (install: https://learn.microsoft.com/cli/azure/install-azure-cli)"
                    .to_string(),
            );
        }

        if auth::check_az_auth().is_err() {
            missing.push("Azure authentication (run: az login)".to_string());
        }

        missing
    }

    async fn init(&self, config: &DeployConfig) -> Result<()> {
        println!("🚀 Initializing Azure Container Apps deployment...");
        println!();

        dockerfile::generate_dockerfile(config)?;
        dockerfile::generate_dockerignore(config)?;

        println!();
        println!("✅ Azure Container Apps deployment initialized!");
        println!();
        println!("📝 Next steps:");
        println!("   1. Authenticate: az login");
        println!("   2. Deploy: cargo pmcp deploy --target-type azure-container-apps");
        println!();
        println!("🔧 Configuration (env vars):");
        println!("   • AZURE_RESOURCE_GROUP   (default: <server-name>-rg)");
        println!("   • AZURE_LOCATION         (default: eastus)");
        println!("   • AZURE_CONTAINERAPP_ENV (default: <server-name>-env)");
        println!("   • AZURE_TARGET_PORT      (default: 8080)");
        println!("   • AZURE_MIN_REPLICAS     (default: 1)");
        println!();
        println!("⚠  The server MUST bind 0.0.0.0:$PORT and set");
        println!("   `allowed_origins: Some(AllowedOrigins::any())` (or the FQDN) —");
        println!("   otherwise the DNS-rebinding guard 403s every request through ingress.");

        Ok(())
    }

    async fn build(&self, config: &DeployConfig) -> Result<BuildArtifact> {
        // The image is built in the cloud (ACR) during deploy.
        Ok(BuildArtifact::Custom {
            path: config.project_root.clone(),
            artifact_type: "docker".to_string(),
            deployment_package: None,
        })
    }

    async fn deploy(
        &self,
        config: &DeployConfig,
        _artifact: BuildArtifact,
    ) -> Result<DeploymentOutputs> {
        deploy::deploy_to_azure_container_apps(config).await
    }

    async fn destroy(&self, config: &DeployConfig, clean: bool) -> Result<()> {
        println!("🗑️  Destroying Azure Container Apps deployment...");
        let s = deploy::AcaSettings::from_config(config)?;

        // Deleting the resource group removes the app, environment, and ACR.
        let output = std::process::Command::new("az")
            .args([
                "group",
                "delete",
                "--name",
                &s.resource_group,
                "--yes",
                "--no-wait",
            ])
            .output()
            .context("Failed to run az group delete")?;
        if !output.status.success() {
            bail!(
                "Failed to delete resource group:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        println!(
            "✅ Resource group '{}' deletion initiated",
            s.resource_group
        );

        if clean {
            for file in ["Dockerfile", ".dockerignore"] {
                let path = config.project_root.join(file);
                if path.exists() {
                    std::fs::remove_file(&path).context(format!("Failed to remove {file}"))?;
                    println!("   ✓ Removed {file}");
                }
            }
        }

        Ok(())
    }

    async fn outputs(&self, config: &DeployConfig) -> Result<DeploymentOutputs> {
        let s = deploy::AcaSettings::from_config(config)?;
        let output = std::process::Command::new("az")
            .args(deploy::show_fqdn_args(&s))
            .output()
            .context("Failed to get Container App FQDN")?;
        if !output.status.success() {
            bail!("Failed to get Container App information");
        }
        let fqdn = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(DeploymentOutputs {
            url: Some(format!("https://{fqdn}/")),
            additional_urls: vec![],
            regions: vec![s.location.clone()],
            stack_name: Some(s.resource_group.clone()),
            version: None,
            custom: std::collections::HashMap::new(),
        })
    }

    async fn logs(&self, config: &DeployConfig, tail: bool, lines: usize) -> Result<()> {
        let s = deploy::AcaSettings::from_config(config)?;
        let lines_str = lines.to_string();
        let mut args = vec![
            "containerapp",
            "logs",
            "show",
            "--name",
            &s.app_name,
            "--resource-group",
            &s.resource_group,
            "--tail",
            &lines_str,
        ];
        if tail {
            args.push("--follow");
        }
        let status = std::process::Command::new("az")
            .args(&args)
            .status()
            .context("Failed to fetch logs")?;
        if !status.success() {
            bail!("Failed to fetch Container App logs");
        }
        Ok(())
    }

    async fn metrics(&self, _config: &DeployConfig, period: &str) -> Result<MetricsData> {
        println!("📊 Azure Container Apps metrics are available in the Azure Portal / Monitor.");
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
        println!(
            "🔐 Manage secrets with: az containerapp secret set -n <app> -g <rg> --secrets k=v"
        );
        Ok(())
    }

    async fn test(&self, config: &DeployConfig, _verbose: bool) -> Result<TestResults> {
        let outputs = self.outputs(config).await?;
        if let Some(url) = outputs.url {
            println!("🧪 Testing endpoint: {url}");
            let response = reqwest::get(&url).await?;
            // MCP needs POST; any HTTP response proves ingress + the container are up.
            let success = response.status().as_u16() < 500;
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

    async fn rollback(&self, config: &DeployConfig, version: Option<&str>) -> Result<()> {
        let s = deploy::AcaSettings::from_config(config)?;

        // A revision is mandatory — silently no-op'ing a rollback hides failures.
        let revision = version.ok_or_else(|| {
            anyhow::anyhow!(
                "rollback requires an explicit revision. List them with:\n  \
                 az containerapp revision list -n {} -g {} -o table\n\
                 then re-run: cargo pmcp deploy rollback --version <revision>",
                s.app_name,
                s.resource_group
            )
        })?;

        println!(
            "🔄 Rolling back '{}' to revision '{revision}' (100% traffic)...",
            s.app_name
        );
        let args = deploy::ingress_traffic_set_args(&s, revision);
        let output = std::process::Command::new("az")
            .args(&args)
            .output()
            .context("Failed to run az containerapp ingress traffic set")?;
        if !output.status.success() {
            bail!(
                "Rollback failed:\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        println!("✅ Traffic shifted to revision '{revision}'");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::deploy::*;
    use super::*;
    use crate::deployment::config::DeployConfig;
    use crate::deployment::registry::TargetRegistry;
    use std::sync::Mutex;

    /// Serialises the env-sensitive `from_config` tests. The process-global
    /// `AZURE_*` env vars are shared mutable state; without this lock the
    /// precedence/validation tests race each other under the default parallel
    /// test runner.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn test_config() -> DeployConfig {
        DeployConfig::default_for_server(
            "demo-mcp".to_string(),
            "eastus".to_string(),
            std::path::PathBuf::from("/tmp/aca-spike-008"),
        )
    }

    #[test]
    fn target_identity() {
        let t = AzureContainerAppsTarget::new();
        assert_eq!(t.id(), "azure-container-apps");
        assert_eq!(t.name(), "Azure Container Apps");
    }

    #[test]
    fn registered_in_registry() {
        // The seam: cargo-pmcp's registry resolves the new target by id.
        let registry = TargetRegistry::new();
        assert!(registry.has("azure-container-apps"));
        assert_eq!(
            registry.get("azure-container-apps").unwrap().id(),
            "azure-container-apps"
        );
    }

    /// All `AZURE_*` env vars this target reads. Cleared before precedence
    /// assertions so a developer's ambient shell env can't perturb the test.
    const AZURE_ENV_VARS: [&str; 5] = [
        "AZURE_RESOURCE_GROUP",
        "AZURE_LOCATION",
        "AZURE_CONTAINERAPP_ENV",
        "AZURE_TARGET_PORT",
        "AZURE_MIN_REPLICAS",
    ];

    fn clear_azure_env() {
        for v in AZURE_ENV_VARS {
            std::env::remove_var(v);
        }
    }

    #[test]
    fn settings_defaults_match_spike_007() {
        // Serialised with the other env-sensitive tests via ENV_LOCK.
        let _guard = ENV_LOCK.lock().unwrap();
        clear_azure_env();
        let s = AcaSettings::from_config(&test_config()).unwrap();
        assert_eq!(s.resource_group, "demo-mcp-rg");
        assert_eq!(s.environment, "demo-mcp-env");
        assert_eq!(s.location, "eastus");
        assert_eq!(s.target_port, "8080");
        assert_eq!(s.min_replicas, "1"); // warm replica — no cold starts
    }

    #[test]
    fn env_beats_azure_section() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_azure_env();
        let mut cfg = test_config();
        cfg.azure.location = "westus2".to_string();
        std::env::set_var("AZURE_LOCATION", "eastus2");
        let s = AcaSettings::from_config(&cfg).unwrap();
        clear_azure_env();
        // ENV (eastus2) wins over the [azure] section (westus2).
        assert_eq!(s.location, "eastus2");
    }

    #[test]
    fn azure_section_beats_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_azure_env();
        let mut cfg = test_config();
        cfg.azure.location = "westus2".to_string();
        cfg.azure.resource_group = Some("rg-x".to_string());
        cfg.azure.target_port = 9090;
        cfg.azure.min_replicas = 3;
        let s = AcaSettings::from_config(&cfg).unwrap();
        // No env → the [azure] section overrides the built-in defaults.
        assert_eq!(s.location, "westus2");
        assert_eq!(s.resource_group, "rg-x");
        assert_eq!(s.target_port, "9090");
        assert_eq!(s.min_replicas, "3");
    }

    #[test]
    fn invalid_target_port_env_errors_clearly() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_azure_env();
        std::env::set_var("AZURE_TARGET_PORT", "not-a-port");
        let err = AcaSettings::from_config(&test_config()).unwrap_err();
        clear_azure_env();
        assert!(
            err.to_string().contains("AZURE_TARGET_PORT"),
            "error must name AZURE_TARGET_PORT, got: {err}"
        );
    }

    #[test]
    fn invalid_min_replicas_env_errors_clearly() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_azure_env();
        std::env::set_var("AZURE_MIN_REPLICAS", "abc");
        let err = AcaSettings::from_config(&test_config()).unwrap_err();
        clear_azure_env();
        assert!(
            err.to_string().contains("AZURE_MIN_REPLICAS"),
            "error must name AZURE_MIN_REPLICAS, got: {err}"
        );
    }

    #[test]
    fn providers_include_operational_insights() {
        // Spike 007: env-create fails on a cold subscription without this one.
        assert!(required_providers().contains(&"Microsoft.OperationalInsights"));
    }

    #[test]
    fn up_args_match_proven_sequence() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_azure_env();
        let s = AcaSettings::from_config(&test_config()).unwrap();
        let args = containerapp_up_args(&s, "/tmp/server");
        assert_eq!(args[0], "containerapp");
        assert_eq!(args[1], "up");
        // The flags proven in spike 007's live deploy.
        assert!(args.windows(2).any(|w| w == ["--source", "/tmp/server"]));
        assert!(args.windows(2).any(|w| w == ["--ingress", "external"]));
        assert!(args.windows(2).any(|w| w == ["--target-port", "8080"]));
    }

    #[test]
    fn cors_opens_required_methods() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_azure_env();
        let s = AcaSettings::from_config(&test_config()).unwrap();
        let args = cors_enable_args(&s);
        assert!(args.iter().any(|a| a == "GET,POST,DELETE,OPTIONS"));
    }

    #[test]
    fn dockerfile_carries_007_findings() {
        let dir = std::env::temp_dir().join("aca-spike-008-dockerfile-test");
        let _ = std::fs::create_dir_all(&dir);
        let cfg = DeployConfig::default_for_server(
            "demo-mcp".to_string(),
            "eastus".to_string(),
            dir.clone(),
        );
        super::dockerfile::generate_dockerfile(&cfg).unwrap();
        let body = std::fs::read_to_string(dir.join("Dockerfile")).unwrap();
        assert!(body.contains("ENV PORT=8080"));
        assert!(body.contains("/app/demo-mcp")); // binary name = server name
        assert!(body.contains("RUN cargo build --release\n"));
        // 007: never `--locked` on the build line without a shipped Cargo.lock.
        assert!(!body.contains("cargo build --release --locked"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Resolve default settings under a cleared env (helper for arg-builder tests
    /// that assert the spike-007 default resource_group/location/replica values).
    fn default_settings() -> AcaSettings {
        clear_azure_env();
        AcaSettings::from_config(&test_config()).unwrap()
    }

    #[test]
    fn provider_register_awaits_registration() {
        // Cold-sub finding: registration must be awaited or env-create races it.
        let args = provider_register_args("Microsoft.OperationalInsights");
        assert!(
            args.iter().any(|a| a == "--wait"),
            "provider register must pass --wait, got: {args:?}"
        );
        assert!(args
            .windows(2)
            .any(|w| w == ["--namespace", "Microsoft.OperationalInsights"]));
    }

    #[test]
    fn group_create_carries_resource_group_and_location() {
        let _guard = ENV_LOCK.lock().unwrap();
        let s = default_settings();
        let args = group_create_args(&s);
        assert_eq!(args[0], "group");
        assert_eq!(args[1], "create");
        assert!(args.windows(2).any(|w| w == ["--name", "demo-mcp-rg"]));
        assert!(args.windows(2).any(|w| w == ["--location", "eastus"]));
    }

    #[test]
    fn env_create_carries_resource_group_and_location() {
        let _guard = ENV_LOCK.lock().unwrap();
        let s = default_settings();
        let args = env_create_args(&s);
        // containerapp env create
        assert_eq!(&args[0..3], &["containerapp", "env", "create"]);
        assert!(args.windows(2).any(|w| w == ["--name", "demo-mcp-env"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--resource-group", "demo-mcp-rg"]));
        assert!(args.windows(2).any(|w| w == ["--location", "eastus"]));
    }

    #[test]
    fn show_fqdn_queries_ingress_fqdn_as_tsv() {
        let _guard = ENV_LOCK.lock().unwrap();
        let s = default_settings();
        let args = show_fqdn_args(&s);
        assert!(args
            .windows(2)
            .any(|w| w == ["--query", "properties.configuration.ingress.fqdn"]));
        assert!(args.windows(2).any(|w| w == ["-o", "tsv"]));
    }

    #[test]
    fn min_replicas_carries_resolved_count() {
        let _guard = ENV_LOCK.lock().unwrap();
        let s = default_settings();
        let args = min_replicas_args(&s);
        assert_eq!(&args[0..2], &["containerapp", "update"]);
        // Default warm replica = "1".
        assert!(args.windows(2).any(|w| w == ["--min-replicas", "1"]));
    }

    #[test]
    fn ingress_traffic_set_args_shifts_full_weight_to_revision() {
        let _guard = ENV_LOCK.lock().unwrap();
        let s = default_settings();
        let args = ingress_traffic_set_args(&s, "rev-2");
        assert_eq!(&args[0..4], &["containerapp", "ingress", "traffic", "set"]);
        assert!(args.iter().any(|a| a == "--revision-weight"));
        assert!(args.iter().any(|a| a == "rev-2=100"));
        assert!(args.windows(2).any(|w| w == ["--name", "demo-mcp"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--resource-group", "demo-mcp-rg"]));
    }

    #[test]
    fn provisioning_state_args_queries_state_as_tsv() {
        let _guard = ENV_LOCK.lock().unwrap();
        let s = default_settings();
        let args = provisioning_state_args(&s);
        assert!(args
            .windows(2)
            .any(|w| w == ["--query", "properties.provisioningState"]));
        assert!(args.windows(2).any(|w| w == ["-o", "tsv"]));
    }

    #[test]
    fn is_poll_drop_recognises_remote_disconnected_and_timeouts() {
        // Known transient long-poll drops → true.
        assert!(is_poll_drop(
            "('Connection aborted.', RemoteDisconnected('Remote end closed connection'))"
        ));
        assert!(is_poll_drop("HTTPSConnectionPool: Read timed out."));
        assert!(is_poll_drop("Connection reset by peer"));
        // An ordinary az error → false (must still bail).
        assert!(!is_poll_drop(
            "ERROR: (ResourceGroupNotFound) Resource group 'demo-mcp-rg' could not be found."
        ));
        assert!(!is_poll_drop("ERROR: authentication failed"));
    }
}

/// Property tests (Phase 80 Plan 04) — never-panic robustness for the pure
/// arg-builders and the Dockerfile generator over arbitrary server names /
/// config. These live in-crate because the arg-builders + Dockerfile generator
/// are NOT lib-public (verified lib-surface gap), so an external test crate
/// compiled against `cargo_pmcp` cannot reach them — mirroring Phase 76's
/// struct-level-tests-in-crate decision.
///
/// Threat model: T-80-02 (Dockerfile render must not panic on a hostile
/// server name).
#[cfg(test)]
mod proptests {
    use super::deploy::*;
    use super::dockerfile::generate_dockerfile;
    use crate::deployment::config::{AzureConfig, DeployConfig};
    use proptest::prelude::*;

    /// Build a DeployConfig for an arbitrary server name + Azure config, rooted
    /// at a caller-supplied project root. No env vars are read by the pure
    /// builders, so this is parallel-safe.
    fn config_for(name: String, azure: AzureConfig, root: std::path::PathBuf) -> DeployConfig {
        let mut cfg = DeployConfig::default_for_server(name, "eastus".to_string(), root);
        cfg.azure = azure;
        cfg
    }

    fn arb_azure() -> impl Strategy<Value = AzureConfig> {
        (
            prop_oneof![Just(None), "[\\PC]{0,40}".prop_map(Some)],
            prop_oneof![Just(None), "[\\PC]{0,40}".prop_map(Some)],
            "[\\PC]{1,40}",
            any::<u16>(),
            any::<u32>(),
        )
            .prop_map(
                |(resource_group, environment, location, target_port, min_replicas)| AzureConfig {
                    resource_group,
                    environment,
                    location,
                    target_port,
                    min_replicas,
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// For arbitrary server names (incl. empty / unicode) + arbitrary Azure
        /// config, `AcaSettings::from_config` then every pure arg-builder runs
        /// without panicking. `from_config` is called with a cleared-env guard
        /// elsewhere; here no `AZURE_*` env is set in the proptest process, so
        /// it resolves purely from config.
        #[test]
        fn arg_builders_never_panic(
            name in "[\\PC]{0,40}",
            azure in arb_azure(),
        ) {
            let cfg = config_for(name, azure, std::path::PathBuf::from("/tmp/aca-prop"));
            // from_config can only error on a malformed AZURE_* env var; none is
            // set in this process, so it resolves from config and must succeed.
            let s = AcaSettings::from_config(&cfg).expect("from_config resolves with no env override");

            // Drive every pure builder — none may panic.
            let _ = required_providers();
            for ns in required_providers() {
                let _ = provider_register_args(ns);
            }
            let _ = group_create_args(&s);
            let _ = env_create_args(&s);
            let _ = containerapp_up_args(&s, "/tmp/server");
            let _ = cors_enable_args(&s);
            let _ = min_replicas_args(&s);
            let _ = show_fqdn_args(&s);
            let _ = ingress_traffic_set_args(&s, "rev-1");
            let _ = provisioning_state_args(&s);
            let _ = is_poll_drop("RemoteDisconnected");
        }

        /// For arbitrary server names, `generate_dockerfile` into a unique temp
        /// dir never panics and the produced Dockerfile always carries
        /// `ENV PORT=8080` and never the `--locked` build line (spike-007 finding).
        #[test]
        fn dockerfile_render_never_panics(name in "[a-zA-Z0-9._-]{1,40}") {
            // A per-case unique temp dir avoids cross-case collisions; the name
            // strategy is restricted to filename-safe chars (the binary name is
            // interpolated into the Dockerfile, not into a path component here,
            // but a unique dir keeps the IO isolated). Cleaned up after each case.
            let unique = format!(
                "aca-prop-dockerfile-{}-{:?}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            );
            let dir = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&dir).expect("temp dir");

            let cfg = config_for(name, AzureConfig::default(), dir.clone());
            generate_dockerfile(&cfg).expect("dockerfile renders");

            let body = std::fs::read_to_string(dir.join("Dockerfile")).expect("dockerfile readable");
            let _ = std::fs::remove_dir_all(&dir);

            prop_assert!(body.contains("ENV PORT=8080"), "must carry ENV PORT=8080");
            prop_assert!(
                !body.contains("cargo build --release --locked"),
                "must never emit a --locked build line"
            );
        }
    }
}
