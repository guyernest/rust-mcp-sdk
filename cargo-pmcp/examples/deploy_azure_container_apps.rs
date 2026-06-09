//! Example: Configuring an Azure Container Apps deploy in `.pmcp/deploy.toml`.
//!
//! Demonstrates the Phase 80 config surface for the `azure-container-apps`
//! deploy target:
//!   1. Build a `DeployConfig` via `default_for_server` and set an example
//!      `[azure]` override (location/target_port/min_replicas).
//!   2. Serialise it to TOML and print the rendered `[azure]` table.
//!   3. Print the spike-007-proven `az containerapp up` invocation as STATIC
//!      operator-reference text, parameterised by the rendered `target_port`.
//!
//! This uses ONLY the lib-public `cargo_pmcp::deployment::config` surface
//! (mirroring `deploy_with_iam.rs`). It does NOT call the private `az`-arg
//! builders or the Dockerfile generator — those live in the bin-only
//! `deployment::targets::azure_container_apps` module, unreachable from an
//! example crate compiled against the lib. The never-panic property of those
//! builders is covered IN-CRATE by the Plan 04 proptests instead.
//!
//! no live Azure required — config render only.
//!
//! Run with:
//!     cargo run -p cargo-pmcp --example deploy_azure_container_apps

use cargo_pmcp::deployment::config::DeployConfig;

fn main() {
    println!("=== Phase 80 — Configure an [azure] Container Apps deploy ===\n");

    // 1. Build a default config for a server, then apply an [azure] override.
    let mut config = DeployConfig::default_for_server(
        "demo-mcp".to_string(),
        "eastus".to_string(),
        std::path::PathBuf::from("/tmp/demo-mcp"),
    );
    config.azure.location = "westus2".to_string();
    config.azure.resource_group = Some("demo-mcp-rg".to_string());
    config.azure.environment = Some("demo-mcp-env".to_string());
    config.azure.target_port = 8080;
    config.azure.min_replicas = 1;

    // 2. Render the config to TOML and print it (shows the [azure] section).
    let rendered = toml::to_string(&config).expect("DeployConfig with [azure] serialises");

    println!("--- 1. Rendered .pmcp/deploy.toml (with [azure]) ---\n");
    print_indented(&rendered);
    println!();

    // 3. Print the proven deploy sequence as STATIC operator-reference text,
    //    parameterised by the rendered target_port (NOT by calling the private
    //    containerapp_up_args builder — that is unreachable from the lib).
    let port = config.azure.target_port;
    println!("--- 2. Proven `az containerapp up` invocation (spike 007) ---\n");
    println!("  az containerapp up --source . --ingress external --target-port {port}\n");
    println!("  (cargo pmcp deploy --target-type azure-container-apps runs this for you,");
    println!("   after registering providers + creating the resource group/environment.)\n");

    // 4. Smoke assertions — the example doubles as a config render check.
    assert!(
        rendered.contains("location = \"westus2\""),
        "rendered [azure] must carry location = westus2"
    );
    assert!(
        rendered.contains("target_port = 8080"),
        "rendered [azure] must carry target_port = 8080"
    );

    println!("=== Example complete (no live Azure was contacted) ===");
}

fn print_indented(s: &str) {
    for line in s.lines() {
        println!("  {line}");
    }
}
