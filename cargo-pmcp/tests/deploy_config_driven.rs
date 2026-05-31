//! Phase 86 Plan 05 (Shape D, SHAP-D-01) — config-driven deploy guards.
//!
//! Two integration-level guards live here:
//!
//! 1. `target_entry_enum_unchanged` — a COMPILE-TIME D-10 regression guard:
//!    an exhaustive `match` over exactly the four `TargetEntry` variants
//!    (`PmcpRun | AwsLambda | GoogleCloudRun | CloudflareWorkers`). Adding or
//!    renaming a variant breaks compilation, proving the deploy path reuses the
//!    existing target system with ZERO enum changes.
//!
//! 2. `emitted_deploy_toml_parses_and_selects_pmcp_run` — scaffolds a project via
//!    the REAL `cargo pmcp new --kind sql-server` binary (the actual command
//!    surface, mirroring `scaffold_sql_server.rs`), then `DeployConfig::load`s the
//!    emitted descriptor and asserts `target_type == "pmcp-run"` (M3) and
//!    `assets.include == ["config.toml", "schema.sql"]` (H1).
//!
//! NOTE (M3): the `is_config_driven_project` detection predicate and the
//! single-crate `find_lambda_package_dir` resolution are unit-tested IN-MODULE
//! (deploy/mod.rs + builder.rs) — they are bin-only helpers unreachable from an
//! integration test. The NON-CLOUD packaging + secret-posture assertion (H4) is
//! likewise an in-module `#[cfg(test)]` test in `builder.rs` because
//! `bundle_assets_if_configured` is a private method (the plan's fallback when the
//! bundler is not reachable from an integration test).

use std::process::Command;

use cargo_pmcp::deployment::config::DeployConfig;
// The lib bridges `commands/configure/config.rs` as `test_support::configure_config`
// so the D-10 enum guard can reach the (bin-only) `TargetEntry` from this test.
use cargo_pmcp::test_support::configure_config::{
    AwsLambdaEntry, CloudflareWorkersEntry, GoogleCloudRunEntry, PmcpRunEntry, TargetEntry,
};

/// D-10 (Phase 86 Plan 05): the deploy path reuses the existing `TargetEntry`
/// target system with NO enum changes. This exhaustive `match` over EXACTLY the
/// four variants is a COMPILE-TIME guard — adding or renaming a variant makes this
/// test fail to compile (no wildcard arm), which is the intended tripwire.
#[test]
fn target_entry_enum_unchanged() {
    let entries = [
        TargetEntry::PmcpRun(PmcpRunEntry::default()),
        TargetEntry::AwsLambda(AwsLambdaEntry::default()),
        TargetEntry::GoogleCloudRun(GoogleCloudRunEntry::default()),
        TargetEntry::CloudflareWorkers(CloudflareWorkersEntry {
            account_id: "acct".to_string(),
            api_token_env: "CF_API_TOKEN".to_string(),
        }),
    ];

    for entry in entries {
        // Exhaustive match — NO `_ =>` arm. If a fifth variant is added (or a
        // variant renamed), this fails to compile, tripping the D-10 guard.
        let tag = match entry {
            TargetEntry::PmcpRun(_) => "pmcp-run",
            TargetEntry::AwsLambda(_) => "aws-lambda",
            TargetEntry::GoogleCloudRun(_) => "google-cloud-run",
            TargetEntry::CloudflareWorkers(_) => "cloudflare-workers",
        };
        assert!(!tag.is_empty());
    }
}

/// M3 + H1: scaffold via the REAL binary, load the emitted deploy descriptor, and
/// assert pmcp.run selection + the asset include list.
#[test]
fn emitted_deploy_toml_parses_and_selects_pmcp_run() {
    let tmp = tempfile::tempdir().expect("create tempdir");
    let name = "deploycfg";

    // Scaffold through the actual `cargo pmcp new --kind sql-server` command
    // (CARGO_BIN_EXE_cargo-pmcp is set by Cargo for the crate's own bin).
    let status = Command::new(env!("CARGO_BIN_EXE_cargo-pmcp"))
        .args(["new", "--kind", "sql-server", name])
        .current_dir(tmp.path())
        .env("PMCP_QUIET", "1")
        .status()
        .expect("spawn cargo-pmcp to scaffold");
    assert!(
        status.success(),
        "`cargo pmcp new --kind sql-server {name}` must succeed (exit {status:?})"
    );

    let crate_dir = tmp.path().join(name);

    // The human-visible root descriptor exists (the Task 2 verify command greps it).
    assert!(
        crate_dir.join("deploy.toml").is_file(),
        "scaffold must emit a root deploy.toml"
    );

    // DeployConfig::load reads `<project_root>/.pmcp/deploy.toml` — the scaffold
    // emits a copy there so this load (and `cargo pmcp deploy`) resolve it.
    let config = DeployConfig::load(&crate_dir).expect("emitted deploy.toml must parse");

    assert_eq!(
        config.target.target_type, "pmcp-run",
        "M3: deploy.toml must declare target_type = \"pmcp-run\" (get_target_id does NOT infer it)"
    );
    assert_eq!(
        config.assets.include,
        vec!["config.toml".to_string(), "schema.sql".to_string()],
        "H1: [assets] include must bundle config.toml + schema.sql (→ /var/task/assets/)"
    );
}
