//! `cargo pmcp agent new <name>` — scaffold a new agent package (CLI-01).
//!
//! Validates the crate name (reusing the hardened `validate_crate_name` guard,
//! D-01a) and the destination policy BEFORE any filesystem write, then delegates
//! to [`crate::templates::agent::generate`] to emit a compilable agent crate: an
//! `AgentPackage` manifest, a manifest-driven runner, a full dependency set, and
//! an in-scaffold pin tripwire.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;

use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp agent new`.
#[derive(Debug, Args)]
pub struct NewArgs {
    /// Name of the agent package to scaffold.
    pub name: String,
    /// Parent directory to create the package in (defaults to the current dir,
    /// so the package lands at `./<name>`).
    #[arg(long)]
    pub path: Option<PathBuf>,
    /// Overwrite an existing (non-empty) destination directory.
    #[arg(long)]
    pub force: bool,
}

/// Scaffold a new agent package into `./<name>` (or `<path>/<name>`).
pub fn execute(args: NewArgs, global_flags: &GlobalFlags) -> Result<()> {
    // Validate the crate name BEFORE any fs access (path-traversal + Cargo-name
    // guard, reused from `commands::new` — D-01a / T-110-02-01).
    crate::commands::new::validate_crate_name(&args.name)?;

    let dir = match &args.path {
        Some(parent) => parent.join(&args.name),
        None => PathBuf::from(&args.name),
    };

    // Destination policy (T-110-02-04): reject a symlinked destination and refuse
    // a non-empty directory unless `--force`.
    ensure_destination_writable(&dir, args.force)?;

    fs::create_dir_all(dir.join("src")).context("Failed to create src directory")?;

    crate::templates::agent::generate(&dir, &args.name)?;

    if global_flags.should_output() {
        println!(
            "\n{} Agent package created successfully!",
            "✓".green().bold()
        );
        print_next_steps(&args.name);
    }

    Ok(())
}

/// Enforce the destination-overwrite policy before scaffolding (Codex 110-02
/// MEDIUM). A missing destination is fine; a symlinked destination is refused
/// outright; an existing NON-EMPTY directory requires `--force`.
fn ensure_destination_writable(dir: &Path, force: bool) -> Result<()> {
    // `symlink_metadata` does NOT follow the final component, so a symlinked
    // destination is detected rather than silently followed (T-110-02-01).
    let meta = match fs::symlink_metadata(dir) {
        Ok(meta) => meta,
        Err(_) => return Ok(()), // does not exist — a fresh directory is fine
    };

    if meta.file_type().is_symlink() {
        anyhow::bail!(
            "destination '{}' is a symlink — refusing to scaffold through it",
            dir.display()
        );
    }

    let is_empty = fs::read_dir(dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false);
    if !is_empty && !force {
        anyhow::bail!(
            "destination '{}' is not empty — pass --force to overwrite",
            dir.display()
        );
    }

    Ok(())
}

fn print_next_steps(name: &str) {
    println!(
        "\n{}",
        "🚀 Next Steps (deploy-anywhere agent):"
            .bright_white()
            .bold()
    );
    println!();
    println!("  {} Enter your package:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {name}").bright_yellow());
    println!();
    println!(
        "  {} Run it (drives the agent loop; edit {} to point at your model):",
        "2.".bright_cyan().bold(),
        "agent.package.json".bright_green()
    );
    println!("     {}", "cargo run".bright_yellow());
    println!();
    println!(
        "  {} Verify the pin tripwire stays green:",
        "3.".bright_cyan().bold()
    );
    println!("     {}", "cargo test --test pin".bright_yellow());
}
