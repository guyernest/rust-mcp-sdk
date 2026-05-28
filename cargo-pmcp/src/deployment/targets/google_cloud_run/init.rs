//! Cloud Run `cargo pmcp deploy init` flow.
//!
//! Closes upstream issue #260: scaffolds `.pmcp/deploy.toml` with the
//! `[target]` + `[gcp]` + `[server]` + `[environment]` minimum-viable schema
//! alongside the existing `Dockerfile` / `.dockerignore` / `cloudbuild.yaml`
//! outputs.
//!
//! Idempotent: re-running `cargo pmcp deploy init --target-type
//! google-cloud-run` in a project directory that already has a deploy.toml
//! preserves the existing one (so operators' filled-in `project_id`,
//! environment values, and any `[layout]` / `[runtime]` opt-ins are not
//! clobbered). Files that are missing are written; files that exist are
//! left untouched.

use super::dockerfile;
use crate::deployment::DeployConfig;
use anyhow::Result;

/// Initialise a Google Cloud Run deployment for `config.project_root`.
///
/// Writes (when missing):
/// - `.pmcp/deploy.toml` — Cloud Run-shaped schema (issue #260)
/// - `Dockerfile`
/// - `.dockerignore`
/// - `cloudbuild.yaml`
///
/// Pre-existing files are preserved verbatim.
///
/// # Errors
///
/// Returns an error if any of the deploy.toml / Dockerfile / .dockerignore /
/// cloudbuild.yaml artifacts cannot be written.
pub fn init_google_cloud_run(config: &DeployConfig) -> Result<()> {
    println!("🚀 Initializing Google Cloud Run deployment...");
    println!();

    write_deploy_toml(config)?;
    dockerfile::generate_dockerfile(config)?;
    dockerfile::generate_dockerignore(config)?;
    dockerfile::generate_cloudbuild(config)?;

    println!();
    println!("✅ Google Cloud Run deployment initialized!");
    println!();
    println!("📝 Next steps:");
    println!("   1. Edit .pmcp/deploy.toml: set [gcp].project_id, [server].name,");
    println!("      and any [environment] keys your server requires");
    println!("   2. Authenticate: gcloud auth login");
    println!("   3. Set project: gcloud config set project PROJECT_ID");
    println!("   4. Deploy: cargo pmcp deploy --target google-cloud-run");
    println!();
    println!("💡 Generated files:");
    println!("   • .pmcp/deploy.toml - IaC source of truth");
    println!("   • Dockerfile - Multi-stage Rust build");
    println!("   • .dockerignore - Optimize build context");
    println!("   • cloudbuild.yaml - Optional Cloud Build configuration");

    Ok(())
}

/// Write `.pmcp/deploy.toml` only when it doesn't already exist.
///
/// Delegates to [`DeployConfig::save_if_missing`] for the actual
/// serialize-and-write logic; this wrapper exists to print the
/// scaffolder-status line the operator sees.
fn write_deploy_toml(config: &DeployConfig) -> Result<()> {
    if config.save_if_missing(&config.project_root)? {
        println!("   ✓ Generated .pmcp/deploy.toml");
    } else {
        println!("   ⏭  .pmcp/deploy.toml already exists — preserving");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_cloud_run_config(project_root: PathBuf) -> DeployConfig {
        DeployConfig::default_for_cloud_run_server(
            "auth-echo-cloud-run".to_string(),
            "your-gcp-project-id".to_string(),
            "us-central1".to_string(),
            project_root,
        )
    }

    /// Cloud Run init must persist a roundtrippable deploy.toml — closes
    /// the core of upstream issue #260 (the existing AWS Lambda path is the
    /// reference precedent at built-in/test-harness/.../aws-lambda/.pmcp/
    /// deploy.toml).
    #[test]
    fn init_writes_cloud_run_shaped_deploy_toml() {
        let tmp = TempDir::new().expect("tmpdir");
        let config = make_cloud_run_config(tmp.path().to_path_buf());

        write_deploy_toml(&config).expect("write succeeds");

        let written =
            std::fs::read_to_string(tmp.path().join(".pmcp/deploy.toml")).expect("read back");
        assert!(written.contains("type = \"google-cloud-run\""));
        assert!(written.contains("[gcp]"));
        assert!(written.contains("project_id = \"your-gcp-project-id\""));
        assert!(written.contains("region = \"us-central1\""));
        assert!(written.contains("memory = \"256Mi\""));
        assert!(
            !written.contains("[aws]"),
            "Cloud Run deploy.toml must not contain an [aws] block"
        );

        // Roundtrip: the written file must parse back into the same shape.
        let reloaded: DeployConfig = toml::from_str(&written).expect("reload");
        assert_eq!(reloaded.target.target_type, "google-cloud-run");
        assert!(reloaded.aws.is_none());
        let gcp = reloaded.gcp.as_ref().expect("gcp present");
        assert_eq!(gcp.project_id, "your-gcp-project-id");
    }

    /// Re-running init on a project where deploy.toml exists must not
    /// clobber operator edits. This is the scaffolder-immunity invariant
    /// from upstream #260.
    #[test]
    fn init_preserves_existing_deploy_toml() {
        let tmp = TempDir::new().expect("tmpdir");
        let pmcp = tmp.path().join(".pmcp");
        std::fs::create_dir_all(&pmcp).expect("mkdir");
        let sentinel = "# operator edits go here\n[target]\ntype = \"google-cloud-run\"\nversion = \"1.0.0\"\n";
        std::fs::write(pmcp.join("deploy.toml"), sentinel).expect("seed file");

        let config = make_cloud_run_config(tmp.path().to_path_buf());
        write_deploy_toml(&config).expect("re-init succeeds");

        let after =
            std::fs::read_to_string(tmp.path().join(".pmcp/deploy.toml")).expect("read back");
        assert_eq!(after, sentinel, "existing deploy.toml must be preserved");
    }
}
