use super::auth;
use crate::deployment::{r#trait::DeploymentOutputs, DeployConfig};
use anyhow::{bail, Context, Result};
use std::str::FromStr;

/// Resolved Azure Container Apps settings.
///
/// Precedence (highest first): `AZURE_*` env vars > the typed `[azure]`
/// deploy.toml section > built-in defaults. Numeric env overrides
/// (`AZURE_TARGET_PORT`, `AZURE_MIN_REPLICAS`) are validated here so a
/// malformed value fails with a clear message instead of leaking into `az`.
#[derive(Debug)]
pub struct AcaSettings {
    pub resource_group: String,
    pub location: String,
    pub environment: String,
    pub app_name: String,
    pub target_port: String,
    pub min_replicas: String,
}

/// Parse a numeric `AZURE_*` env override, if present, attaching a clear
/// context to a parse failure. Returns `Ok(None)` when the var is unset.
fn parse_env_num<T: FromStr>(var: &str, label: &str) -> Result<Option<T>> {
    match std::env::var(var) {
        Ok(raw) => {
            let parsed = raw
                .trim()
                .parse::<T>()
                .map_err(|_| anyhow::anyhow!("{var}={raw:?} is invalid: {label}"))?;
            Ok(Some(parsed))
        },
        Err(_) => Ok(None),
    }
}

impl AcaSettings {
    /// Resolve settings from config with ENV > `[azure]` > defaults precedence.
    ///
    /// # Errors
    /// Returns an error if `AZURE_TARGET_PORT` is not a valid `u16` or
    /// `AZURE_MIN_REPLICAS` is not a valid `u32`.
    pub fn from_config(config: &DeployConfig) -> Result<Self> {
        let app_name = config.server.name.clone();
        let azure = &config.azure;

        let resource_group = std::env::var("AZURE_RESOURCE_GROUP")
            .ok()
            .or_else(|| azure.resource_group.clone())
            .unwrap_or_else(|| format!("{app_name}-rg"));

        let environment = std::env::var("AZURE_CONTAINERAPP_ENV")
            .ok()
            .or_else(|| azure.environment.clone())
            .unwrap_or_else(|| format!("{app_name}-env"));

        let location = std::env::var("AZURE_LOCATION").unwrap_or_else(|_| azure.location.clone());

        let target_port = parse_env_num::<u16>(
            "AZURE_TARGET_PORT",
            "AZURE_TARGET_PORT must be a valid port (1-65535)",
        )?
        .unwrap_or(azure.target_port);

        let min_replicas = parse_env_num::<u32>(
            "AZURE_MIN_REPLICAS",
            "AZURE_MIN_REPLICAS must be a non-negative integer",
        )?
        .unwrap_or(azure.min_replicas);

        Ok(Self {
            resource_group,
            location,
            environment,
            app_name,
            target_port: target_port.to_string(),
            min_replicas: min_replicas.to_string(),
        })
    }
}

// ── Pure arg builders (unit-tested; mirror the spike-007-proven sequence) ──────

/// Resource providers that must be registered (and awaited) on a cold subscription.
/// Spike 007: env-create fails without Microsoft.OperationalInsights.
pub fn required_providers() -> [&'static str; 3] {
    [
        "Microsoft.App",
        "Microsoft.OperationalInsights",
        "Microsoft.ContainerRegistry",
    ]
}

pub fn provider_register_args(namespace: &str) -> Vec<String> {
    vec![
        "provider".into(),
        "register".into(),
        "--namespace".into(),
        namespace.into(),
        "--wait".into(),
        "--only-show-errors".into(),
    ]
}

pub fn group_create_args(s: &AcaSettings) -> Vec<String> {
    vec![
        "group".into(),
        "create".into(),
        "--name".into(),
        s.resource_group.clone(),
        "--location".into(),
        s.location.clone(),
    ]
}

pub fn env_create_args(s: &AcaSettings) -> Vec<String> {
    vec![
        "containerapp".into(),
        "env".into(),
        "create".into(),
        "--name".into(),
        s.environment.clone(),
        "--resource-group".into(),
        s.resource_group.clone(),
        "--location".into(),
        s.location.clone(),
        "--only-show-errors".into(),
    ]
}

/// The core deploy: cloud-build the Dockerfile in ACR + deploy. No local Docker.
pub fn containerapp_up_args(s: &AcaSettings, source_dir: &str) -> Vec<String> {
    vec![
        "containerapp".into(),
        "up".into(),
        "--name".into(),
        s.app_name.clone(),
        "--resource-group".into(),
        s.resource_group.clone(),
        "--environment".into(),
        s.environment.clone(),
        "--source".into(),
        source_dir.into(),
        "--ingress".into(),
        "external".into(),
        "--target-port".into(),
        s.target_port.clone(),
    ]
}

pub fn cors_enable_args(s: &AcaSettings) -> Vec<String> {
    vec![
        "containerapp".into(),
        "ingress".into(),
        "cors".into(),
        "enable".into(),
        "--name".into(),
        s.app_name.clone(),
        "--resource-group".into(),
        s.resource_group.clone(),
        "--allowed-origins".into(),
        "*".into(),
        "--allowed-methods".into(),
        "GET,POST,DELETE,OPTIONS".into(),
        "--allowed-headers".into(),
        "*".into(),
        "--only-show-errors".into(),
    ]
}

pub fn min_replicas_args(s: &AcaSettings) -> Vec<String> {
    vec![
        "containerapp".into(),
        "update".into(),
        "--name".into(),
        s.app_name.clone(),
        "--resource-group".into(),
        s.resource_group.clone(),
        "--min-replicas".into(),
        s.min_replicas.clone(),
        "--only-show-errors".into(),
    ]
}

pub fn show_fqdn_args(s: &AcaSettings) -> Vec<String> {
    vec![
        "containerapp".into(),
        "show".into(),
        "--name".into(),
        s.app_name.clone(),
        "--resource-group".into(),
        s.resource_group.clone(),
        "--query".into(),
        "properties.configuration.ingress.fqdn".into(),
        "-o".into(),
        "tsv".into(),
    ]
}

/// Shift 100% of ingress traffic to `revision` — the rollback primitive.
/// Mirrors the spike-007 `az containerapp ingress traffic set` shape.
pub fn ingress_traffic_set_args(s: &AcaSettings, revision: &str) -> Vec<String> {
    vec![
        "containerapp".into(),
        "ingress".into(),
        "traffic".into(),
        "set".into(),
        "--name".into(),
        s.app_name.clone(),
        "--resource-group".into(),
        s.resource_group.clone(),
        "--revision-weight".into(),
        format!("{revision}=100"),
        "--only-show-errors".into(),
    ]
}

/// Query the app's `provisioningState` — used to re-verify after an `az`
/// long-poll drops (`RemoteDisconnected`) so a poll error isn't mistaken for
/// a hard failure (CONTEXT locked decision).
pub fn provisioning_state_args(s: &AcaSettings) -> Vec<String> {
    vec![
        "containerapp".into(),
        "show".into(),
        "--name".into(),
        s.app_name.clone(),
        "--resource-group".into(),
        s.resource_group.clone(),
        "--query".into(),
        "properties.provisioningState".into(),
        "-o".into(),
        "tsv".into(),
    ]
}

/// Classify an `az` stderr as a known long-poll drop (a transient connection
/// loss while waiting on an awaited operation) vs a genuine error. Azure's CLI
/// surfaces these as `RemoteDisconnected` / read timeouts / connection resets.
pub fn is_poll_drop(stderr: &str) -> bool {
    let s = stderr.to_ascii_lowercase();
    s.contains("remotedisconnected")
        || s.contains("read timed out")
        || s.contains("connection aborted")
        || s.contains("connection reset")
        || s.contains("connection broken")
}

// ── Runner ─────────────────────────────────────────────────────────────────────

/// Run `az` with the given args, bail on non-zero, return trimmed stdout.
fn az(args: &[String]) -> Result<String> {
    let output = std::process::Command::new("az")
        .args(args)
        .output()
        .context("Failed to run `az` (is the Azure CLI installed?)")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("az {} failed:\n{}", args.join(" "), stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Best-effort `az` (used for steps that are non-fatal, e.g. CORS already set).
fn az_best_effort(args: &[String]) {
    let _ = std::process::Command::new("az").args(args).status();
}

/// Run an *awaited* `az` step that may suffer an intermittent long-poll drop.
///
/// On a non-zero exit whose stderr matches [`is_poll_drop`], re-query
/// `provisioningState`: if it is anything other than `Failed`, the operation is
/// still progressing (or already done) — log the drop and continue. Only a
/// genuine `Failed` state (or a non-recoverable `show` error) bails.
fn az_awaited(args: &[String], s: &AcaSettings) -> Result<String> {
    let output = std::process::Command::new("az")
        .args(args)
        .output()
        .context("Failed to run `az` (is the Azure CLI installed?)")?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !is_poll_drop(&stderr) {
        bail!("az {} failed:\n{}", args.join(" "), stderr);
    }

    // Long-poll drop: re-verify the provisioning state before declaring failure.
    println!("   ⚠  az long-poll dropped (RemoteDisconnected); re-checking provisioningState...");
    let state = az(&provisioning_state_args(s))
        .context("poll-drop recovery: failed to query provisioningState")?;
    if state.eq_ignore_ascii_case("Failed") {
        bail!(
            "az {} failed and provisioningState is Failed:\n{}",
            args.join(" "),
            stderr
        );
    }
    println!("   ✓ provisioningState = {state} — continuing past the poll drop");
    Ok(state)
}

/// Deploy a Rust MCP server to Azure Container Apps via ACR cloud-build.
///
/// Mirrors the spike-007-proven `deploy.sh`:
/// register providers (awaited) → group create → env create →
/// `containerapp up --source` → enable CORS → min-replicas → read FQDN.
pub async fn deploy_to_azure_container_apps(config: &DeployConfig) -> Result<DeploymentOutputs> {
    println!("🚀 Deploying to Azure Container Apps...");
    auth::check_az_auth().context("Not authenticated with Azure")?;
    let subscription = auth::get_subscription_id()?;
    let s = AcaSettings::from_config(config)?;
    let source_dir = config.project_root.to_string_lossy().to_string();

    println!("   Subscription: {subscription}");
    println!("   Resource group: {}", s.resource_group);
    println!("   Environment: {}", s.environment);
    println!("   App: {} (port {})", s.app_name, s.target_port);
    println!();

    // 1. Ensure the containerapp extension + providers (awaited — cold-sub gotcha).
    az_best_effort(&[
        "extension".into(),
        "add".into(),
        "--name".into(),
        "containerapp".into(),
        "--upgrade".into(),
        "--only-show-errors".into(),
    ]);
    for ns in required_providers() {
        println!("   registering provider {ns} (awaited)...");
        az_awaited(&provider_register_args(ns), &s)?;
    }

    // 2. Resource group (quick) + Container Apps environment (awaited, poll-resilient).
    az(&group_create_args(&s))?;
    az_awaited(&env_create_args(&s), &s)?;

    // 3. Cloud-build the Dockerfile in ACR and deploy (no local Docker; awaited).
    println!("🔨 az containerapp up --source (ACR cloud-build)...");
    az_awaited(&containerapp_up_args(&s, &source_dir), &s)?;

    // 4. CORS at the ingress layer (browser/Copilot clients). Non-fatal.
    az_best_effort(&cors_enable_args(&s));

    // 5. Keep one warm replica (interactive MCP clients hate cold starts; awaited).
    az_awaited(&min_replicas_args(&s), &s)?;

    // 6. Read the public FQDN.
    let fqdn = az(&show_fqdn_args(&s))?;
    let url = format!("https://{fqdn}/");
    println!();
    println!("🎉 Deployed. MCP endpoint: {url}");

    Ok(DeploymentOutputs {
        url: Some(url),
        additional_urls: vec![],
        regions: vec![s.location.clone()],
        stack_name: Some(s.resource_group.clone()),
        version: None,
        custom: {
            let mut custom = std::collections::HashMap::new();
            custom.insert(
                "subscription".to_string(),
                serde_json::Value::String(subscription),
            );
            custom.insert(
                "environment".to_string(),
                serde_json::Value::String(s.environment.clone()),
            );
            custom.insert("fqdn".to_string(), serde_json::Value::String(fqdn));
            custom
        },
    })
}
