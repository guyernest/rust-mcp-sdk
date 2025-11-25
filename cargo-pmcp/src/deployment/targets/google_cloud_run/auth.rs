use anyhow::{bail, Context, Result};

/// Check if gcloud is authenticated
pub fn check_gcloud_auth() -> Result<()> {
    let output = std::process::Command::new("gcloud")
        .args(&[
            "auth",
            "list",
            "--filter=status:ACTIVE",
            "--format=value(account)",
        ])
        .output()
        .context("Failed to check gcloud authentication")?;

    if !output.status.success() {
        bail!("gcloud command failed");
    }

    let accounts = String::from_utf8_lossy(&output.stdout);
    if accounts.trim().is_empty() {
        bail!("No active gcloud account found. Run: gcloud auth login");
    }

    Ok(())
}

/// Prompt user to login with gcloud
#[allow(dead_code)]
pub fn login() -> Result<()> {
    println!("🔐 Authenticating with Google Cloud...");
    println!();
    println!("Opening browser for authentication...");
    println!();

    let output = std::process::Command::new("gcloud")
        .args(&["auth", "login"])
        .output()
        .context("Failed to run gcloud auth login")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("gcloud auth login failed:\n{}", stderr);
    }

    println!("✅ Successfully authenticated with Google Cloud!");
    println!();
    println!("💡 Next steps:");
    println!("   1. Set project: gcloud config set project PROJECT_ID");
    println!("   2. Deploy: cargo pmcp deploy --target google-cloud-run");

    Ok(())
}

/// Get the current gcloud project ID
pub fn get_project_id() -> Result<String> {
    let output = std::process::Command::new("gcloud")
        .args(&["config", "get-value", "project"])
        .output()
        .context("Failed to get gcloud project")?;

    if !output.status.success() {
        bail!("No gcloud project configured. Run: gcloud config set project PROJECT_ID");
    }

    let project_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if project_id.is_empty() {
        bail!("No gcloud project configured. Run: gcloud config set project PROJECT_ID");
    }

    Ok(project_id)
}

/// Get the current gcloud region
pub fn get_region() -> String {
    std::process::Command::new("gcloud")
        .args(&["config", "get-value", "run/region"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                let region = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !region.is_empty() {
                    return Some(region);
                }
            }
            None
        })
        .unwrap_or_else(|| "us-central1".to_string())
}
