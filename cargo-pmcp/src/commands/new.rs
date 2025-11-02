//! Create new MCP workspace

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::templates;

pub fn execute(name: String, path: Option<String>) -> Result<()> {
    println!("\n{}", "Creating MCP workspace".bright_cyan().bold());
    println!("{}", "────────────────────────────────────".bright_cyan());

    // Determine workspace directory
    let workspace_dir = if let Some(p) = path {
        PathBuf::from(p).join(&name)
    } else {
        PathBuf::from(&name)
    };

    // Check if directory already exists
    if workspace_dir.exists() {
        anyhow::bail!("Directory '{}' already exists", workspace_dir.display());
    }

    // Create workspace structure
    create_workspace_structure(&workspace_dir, &name)?;

    // Generate workspace files
    templates::workspace::generate(&workspace_dir, &name)?;

    // Generate server-common crate
    templates::server_common::generate(&workspace_dir)?;

    println!("\n{} Workspace created successfully!", "✓".green().bold());

    println!(
        "\n{}",
        "🚀 Next Steps (2-minute quick start):"
            .bright_white()
            .bold()
    );
    println!();
    println!("  {} Enter your workspace:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!("  {} Add your first server:", "2.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp add server calculator".bright_yellow()
    );
    println!();
    println!("  {} Start the server:", "3.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp dev --server calculator".bright_yellow()
    );
    println!();
    println!("  {} Connect to Claude Code:", "4.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp connect --server calculator --client claude-code".bright_yellow()
    );
    println!();
    println!("  {} Try it:", "5.".bright_cyan().bold());
    println!("     Ask Claude: {}", "\"Add 5 and 3\"".bright_green());

    Ok(())
}

fn create_workspace_structure(workspace_dir: &Path, _name: &str) -> Result<()> {
    // Create main directories
    fs::create_dir_all(workspace_dir).context("Failed to create workspace directory")?;

    fs::create_dir_all(workspace_dir.join("crates"))
        .context("Failed to create crates directory")?;

    fs::create_dir_all(workspace_dir.join("scenarios"))
        .context("Failed to create scenarios directory")?;

    fs::create_dir_all(workspace_dir.join("lambda"))
        .context("Failed to create lambda directory")?;

    println!("  {} Generated workspace structure", "✓".green());

    Ok(())
}
