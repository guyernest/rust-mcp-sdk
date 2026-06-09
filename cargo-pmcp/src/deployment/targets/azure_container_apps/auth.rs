use anyhow::{bail, Context, Result};

/// Check that the Azure CLI is installed and logged in.
pub fn check_az_auth() -> Result<()> {
    let output = std::process::Command::new("az")
        .args(["account", "show", "--query", "id", "-o", "tsv"])
        .output()
        .context("Failed to run `az` (is the Azure CLI installed?)")?;

    if !output.status.success() {
        bail!("Not logged in to Azure. Run: az login");
    }

    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() {
        bail!("No active Azure subscription. Run: az login");
    }

    Ok(())
}

/// Get the active Azure subscription id.
pub fn get_subscription_id() -> Result<String> {
    let output = std::process::Command::new("az")
        .args(["account", "show", "--query", "id", "-o", "tsv"])
        .output()
        .context("Failed to read active Azure subscription")?;

    if !output.status.success() {
        bail!("No active Azure subscription. Run: az login");
    }

    let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if id.is_empty() {
        bail!("No active Azure subscription. Run: az login");
    }
    Ok(id)
}
