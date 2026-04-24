//! Upload test scenarios to pmcp.run

use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};

/// Upload test scenarios to pmcp.run for scheduled testing
pub async fn execute(
    server_id: String,
    paths: Vec<PathBuf>,
    name: Option<String>,
    description: Option<String>,
    global_flags: &GlobalFlags,
) -> Result<()> {
    if global_flags.should_output() {
        println!(
            "\n{}",
            "Uploading test scenarios to pmcp.run".bright_cyan().bold()
        );
        println!(
            "{}",
            "─────────────────────────────────────────".bright_cyan()
        );
    }

    // Get credentials
    let credentials = auth::get_credentials().await?;

    let scenario_files = collect_scenario_files(paths)?;

    if scenario_files.is_empty() {
        anyhow::bail!("No scenario files found");
    }

    if global_flags.should_output() {
        println!(
            "  {} Found {} scenario file(s)",
            "→".blue(),
            scenario_files.len()
        );
        println!("  {} Server ID: {}", "→".blue(), server_id);
    }

    let mut uploaded = 0;
    let mut failed = 0;

    for file_path in scenario_files {
        let scenario_name = name
            .clone()
            .unwrap_or_else(|| file_stem_or_unnamed(&file_path));

        if global_flags.should_output() {
            println!("\n  Uploading: {}", file_path.display());
        }

        if upload_one_scenario(
            &credentials.access_token,
            &server_id,
            &scenario_name,
            description.as_deref(),
            &file_path,
            global_flags,
        )
        .await?
        {
            uploaded += 1;
        } else {
            failed += 1;
        }
    }

    if global_flags.should_output() {
        println!();
        println!(
            "{}",
            "═════════════════════════════════════════".bright_cyan()
        );

        if failed == 0 {
            println!(
                "{} Uploaded {} scenario(s) successfully!",
                "✓".green().bold(),
                uploaded
            );
            println!();
            println!("{}", "Next steps:".bright_white().bold());
            println!(
                "  - View scenarios at: https://pmcp.run/servers/{}",
                server_id
            );
            println!("  - Configure scheduled testing in the dashboard");
            println!(
                "  - Or run tests manually: cargo pmcp test remote --server {}",
                server_id
            );
        } else {
            println!(
                "{} Uploaded {} scenario(s), {} failed",
                "⚠".yellow().bold(),
                uploaded,
                failed
            );
        }

        println!(
            "{}",
            "═════════════════════════════════════════".bright_cyan()
        );
    }

    if failed > 0 && uploaded == 0 {
        anyhow::bail!("All scenario uploads failed");
    }

    Ok(())
}

/// Walk the input paths and collect .yaml/.yml files (directory-recursive
/// one level only; single-file inputs accepted verbatim). Bails on missing
/// paths.
fn collect_scenario_files(paths: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut scenario_files: Vec<PathBuf> = Vec::new();

    for path in paths {
        if path.is_dir() {
            collect_yaml_files_from_dir(&path, &mut scenario_files)?;
        } else if path.exists() {
            scenario_files.push(path);
        } else {
            anyhow::bail!("Path does not exist: {}", path.display());
        }
    }

    Ok(scenario_files)
}

/// Push .yaml/.yml files from a directory into `out` (non-recursive).
fn collect_yaml_files_from_dir(dir: &std::path::Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_path = entry.path();
        let ext = file_path.extension().and_then(|s| s.to_str());
        if ext == Some("yaml") || ext == Some("yml") {
            out.push(file_path);
        }
    }
    Ok(())
}

/// Return the file stem of `path` as a String, or "unnamed" if not UTF-8.
fn file_stem_or_unnamed(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unnamed")
        .to_string()
}

/// Read, call graphql::upload_test_scenario, and log success/failure. Returns
/// true on success.
async fn upload_one_scenario(
    access_token: &str,
    server_id: &str,
    scenario_name: &str,
    description: Option<&str>,
    file_path: &std::path::Path,
    global_flags: &GlobalFlags,
) -> Result<bool> {
    let content = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read scenario file: {}", file_path.display()))?;

    let format = if file_path.extension().and_then(|s| s.to_str()) == Some("json") {
        "json"
    } else {
        "yaml"
    };

    match graphql::upload_test_scenario(
        access_token,
        server_id,
        scenario_name,
        description,
        &content,
        format,
    )
    .await
    {
        Ok(result) => {
            if global_flags.should_output() {
                println!(
                    "    {} Uploaded as '{}' (v{})",
                    "✓".green(),
                    scenario_name,
                    result.version
                );
            }
            Ok(true)
        },
        Err(e) => {
            println!("    {} Failed: {}", "✗".red(), e);
            Ok(false)
        },
    }
}
