//! Phase 77 worked example: a monorepo with two sibling servers (one `pmcp-run`,
//! one `aws-lambda`), each pinned to a different deployment target via its own
//! `.pmcp/active-target`.
//!
//! Run via:
//!   cargo run --example multi_target_monorepo -p cargo-pmcp
//!
//! The example:
//!   1. Creates a tempdir to act as HOME (so it doesn't touch your real ~/.pmcp/).
//!   2. Defines two targets in `~/.pmcp/config.toml`: `dev` (pmcp-run, us-west-2)
//!      and `prod` (aws-lambda, us-east-1).
//!   3. Creates a tempdir monorepo with two server subdirs, each carrying its own Cargo.toml.
//!   4. Sets `<monorepo>/server-a/.pmcp/active-target = dev`.
//!   5. Sets `<monorepo>/server-b/.pmcp/active-target = prod`.
//!   6. For each server, simulates the active-target resolution that cargo-pmcp
//!      performs at deploy time: read marker file → look up entry in config.toml →
//!      report resolved name + type tag.
//!
//! This demonstrates D-01's core promise: per-server marker semantics in a monorepo.
//!
//! HIGH-1 fix per 77-REVIEWS.md: `commands::configure::*` is a bin-only module tree
//! (per cargo-pmcp/src/lib.rs comments — `commands` is mounted as a private bin module).
//! Direct lib access to `resolver::resolve_target` is therefore not possible from an
//! example binary. Instead, we use the lib-visible `cargo_pmcp::test_support::configure_config::*`
//! schema (the `#[path]`-bridged re-export) and inline the marker-read + lookup logic —
//! this is the same logic the bin-internal resolver runs, just expressed at the schema layer.

use anyhow::{anyhow, Result};
use cargo_pmcp::test_support::configure_config::{
    default_user_config_path, AwsLambdaEntry, PmcpRunEntry, TargetConfigV1, TargetEntry,
};

fn main() -> Result<()> {
    // Step 1: isolated HOME so we don't touch the user's real config.
    let home_tmp = tempfile::tempdir()?;
    let saved_home = std::env::var_os("HOME");
    let saved_cwd = std::env::current_dir()?;
    std::env::set_var("HOME", home_tmp.path());
    // Don't let an inherited PMCP_TARGET pollute the demo:
    let saved_target = std::env::var_os("PMCP_TARGET");
    std::env::remove_var("PMCP_TARGET");

    let result = run_demo(home_tmp.path());

    // Restore env regardless of result.
    std::env::set_current_dir(&saved_cwd).ok();
    match saved_home {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    match saved_target {
        Some(v) => std::env::set_var("PMCP_TARGET", v),
        None => std::env::remove_var("PMCP_TARGET"),
    }

    result
}

fn run_demo(home: &std::path::Path) -> Result<()> {
    eprintln!("=== Phase 77 demo: multi-target monorepo ===\n");

    // Step 2: define two targets in ~/.pmcp/config.toml.
    let cfg_path = default_user_config_path();
    std::fs::create_dir_all(cfg_path.parent().unwrap())?;
    let mut cfg = TargetConfigV1::empty();
    cfg.targets.insert(
        "dev".into(),
        TargetEntry::PmcpRun(PmcpRunEntry {
            api_url: Some("https://dev-api.pmcp.run".into()),
            aws_profile: Some("my-dev-profile".into()),
            region: Some("us-west-2".into()),
        }),
    );
    cfg.targets.insert(
        "prod".into(),
        TargetEntry::AwsLambda(AwsLambdaEntry {
            aws_profile: Some("my-prod-profile".into()),
            region: Some("us-east-1".into()),
            account_id: Some("123456789012".into()),
        }),
    );
    cfg.write_atomic(&cfg_path)?;
    eprintln!(
        "✓ Defined 2 targets in {}: dev (pmcp-run), prod (aws-lambda)",
        cfg_path.display()
    );

    // Sanity: the user's real ~/.pmcp/ is NOT being touched.
    debug_assert!(
        cfg_path.starts_with(home),
        "config path must live under tempdir HOME"
    );

    // Step 3: build a tempdir monorepo with two sibling server crates.
    let monorepo = tempfile::tempdir()?;
    let server_a = monorepo.path().join("server-a");
    let server_b = monorepo.path().join("server-b");
    std::fs::create_dir_all(&server_a)?;
    std::fs::create_dir_all(&server_b)?;
    std::fs::write(
        server_a.join("Cargo.toml"),
        "[package]\nname=\"server-a\"\nversion=\"0.0.1\"\nedition=\"2021\"\n",
    )?;
    std::fs::write(
        server_b.join("Cargo.toml"),
        "[package]\nname=\"server-b\"\nversion=\"0.0.1\"\nedition=\"2021\"\n",
    )?;
    eprintln!("✓ Created monorepo with server-a and server-b");

    // Steps 4 + 5: write each server's marker.
    std::fs::create_dir_all(server_a.join(".pmcp"))?;
    std::fs::create_dir_all(server_b.join(".pmcp"))?;
    std::fs::write(server_a.join(".pmcp").join("active-target"), "dev\n")?;
    std::fs::write(server_b.join(".pmcp").join("active-target"), "prod\n")?;
    eprintln!("✓ server-a active = dev, server-b active = prod");

    // Step 6: simulate the resolver — read the marker, look up the entry.
    // (This mirrors what `commands::configure::resolver::resolve_target` does in the
    // bin target. Using the schema layer here keeps the example self-contained without
    // exposing bin-only modules.)
    demo_resolve(&server_a, &cfg, "dev", "pmcp-run")?;
    demo_resolve(&server_b, &cfg, "prod", "aws-lambda")?;

    eprintln!("\n=== Demo complete: per-server marker semantics work as expected. ===");
    Ok(())
}

/// Simulate active-target resolution for `server_dir`: read the marker file,
/// look the name up in the on-disk config, print the resolution + assert it matches.
fn demo_resolve(
    server_dir: &std::path::Path,
    cfg: &TargetConfigV1,
    expected_name: &str,
    expected_kind: &str,
) -> Result<()> {
    let marker_path = server_dir.join(".pmcp").join("active-target");
    let raw = std::fs::read_to_string(&marker_path)
        .map_err(|e| anyhow!("failed to read {}: {e}", marker_path.display()))?;
    let resolved_name = raw.trim();
    let entry = cfg
        .targets
        .get(resolved_name)
        .ok_or_else(|| anyhow!("target '{resolved_name}' not found in config"))?;
    let resolved_kind = entry.type_tag();

    assert_eq!(
        resolved_name,
        expected_name,
        "marker in {} must resolve to {expected_name}; got {resolved_name}",
        server_dir.display()
    );
    assert_eq!(
        resolved_kind, expected_kind,
        "target {expected_name} must have type {expected_kind}; got {resolved_kind}"
    );

    eprintln!("\n[from {}]", server_dir.display());
    eprintln!("  resolved name   = {}", resolved_name);
    eprintln!("  resolved kind   = {}", resolved_kind);
    match entry {
        TargetEntry::PmcpRun(e) => {
            if let Some(u) = &e.api_url {
                eprintln!("  api_url         = {}", u);
            }
            if let Some(p) = &e.aws_profile {
                eprintln!("  aws_profile     = {}", p);
            }
            if let Some(r) = &e.region {
                eprintln!("  region          = {}", r);
            }
        },
        TargetEntry::AwsLambda(e) => {
            if let Some(p) = &e.aws_profile {
                eprintln!("  aws_profile     = {}", p);
            }
            if let Some(r) = &e.region {
                eprintln!("  region          = {}", r);
            }
            if let Some(a) = &e.account_id {
                eprintln!("  account_id      = {}", a);
            }
        },
        TargetEntry::GoogleCloudRun(e) => {
            if let Some(p) = &e.gcp_project {
                eprintln!("  gcp_project     = {}", p);
            }
            if let Some(r) = &e.region {
                eprintln!("  region          = {}", r);
            }
        },
        TargetEntry::CloudflareWorkers(e) => {
            eprintln!("  account_id      = {}", e.account_id);
            eprintln!("  api_token_env   = {}", e.api_token_env);
        },
    }
    Ok(())
}
