//! MCP Apps project management commands.
//!
//! Provides `cargo pmcp app new <name>` for scaffolding and
//! `cargo pmcp app manifest --url <URL>` for generating ChatGPT-compatible
//! manifest JSON.

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use std::fs;
use std::path::PathBuf;

use crate::publishing;
use crate::templates;

/// MCP Apps project commands.
#[derive(Subcommand)]
pub enum AppCommand {
    /// Create a new MCP Apps project
    New {
        /// Name of the project
        name: String,
        /// Directory to create project in (defaults to current directory)
        #[arg(long)]
        path: Option<String>,
    },
    /// Generate ChatGPT-compatible manifest JSON
    Manifest {
        /// Server URL (required)
        #[arg(long)]
        url: String,
        /// Logo URL (overrides [package.metadata.pmcp].logo)
        #[arg(long)]
        logo: Option<String>,
        /// Output directory
        #[arg(long, default_value = "dist")]
        output: String,
    },
}

impl AppCommand {
    /// Execute the app subcommand.
    pub fn execute(self) -> Result<()> {
        match self {
            AppCommand::New { name, path } => create_app(name, path),
            AppCommand::Manifest { url, logo, output } => run_manifest(url, logo, output),
        }
    }
}

/// Scaffold a new MCP Apps project directory.
///
/// Creates a project directory containing `src/main.rs`, `widgets/hello.html`,
/// `Cargo.toml`, and `README.md`. Errors if the target directory already exists,
/// matching `cargo new` semantics.
fn create_app(name: String, path: Option<String>) -> Result<()> {
    println!("\n{}", "Creating MCP Apps project".bright_cyan().bold());
    println!("{}", "------------------------------------".bright_cyan());

    // Determine project directory
    let project_dir = if let Some(p) = path {
        PathBuf::from(p).join(&name)
    } else {
        PathBuf::from(&name)
    };

    // Error if directory already exists (cargo new semantics)
    if project_dir.exists() {
        anyhow::bail!(
            "directory '{}' already exists. Use a different name or remove the existing directory.",
            project_dir.display()
        );
    }

    // Create directory structure
    fs::create_dir_all(project_dir.join("src")).context("Failed to create src/ directory")?;
    fs::create_dir_all(project_dir.join("widgets"))
        .context("Failed to create widgets/ directory")?;

    println!("  {} Created project structure", "ok".green());

    // Generate all template files
    templates::mcp_app::generate(&project_dir, &name)?;

    println!(
        "\n{} Created MCP Apps project '{}'",
        "ok".green().bold(),
        name
    );

    // Print next steps
    print_next_steps(&name);

    Ok(())
}

/// Generate a ChatGPT-compatible manifest JSON from the current project.
///
/// Detects the MCP Apps project in the current directory, auto-discovers
/// widgets, and writes `manifest.json` to the output directory.
fn run_manifest(url: String, logo: Option<String>, output: String) -> Result<()> {
    println!("\n{}", "Generating manifest".bright_cyan().bold());
    println!("{}", "------------------------------------".bright_cyan());

    let cwd = std::env::current_dir().context("Failed to read current directory")?;
    let project = publishing::detect::detect_project(&cwd)?;

    let widget_count = project.widgets.len();
    let json = publishing::manifest::generate_manifest(&project, &url, logo.as_deref())?;
    publishing::manifest::write_manifest(&output, &json)?;

    println!("  {} Found {} widget(s)", "ok".green(), widget_count);
    println!(
        "\n{} Manifest written to {}/manifest.json",
        "ok".green().bold(),
        output
    );

    Ok(())
}

/// Print post-scaffold next-step instructions.
fn print_next_steps(name: &str) {
    println!("\n{}", "  Next steps:".bright_white().bold());
    println!("    {}", format!("cd {}", name).bright_yellow());
    println!("    {}", "cargo build".bright_yellow());
    println!("    {}", "cargo run &".bright_yellow());
    println!(
        "    {}",
        "cargo pmcp preview --url http://localhost:3000 --open".bright_yellow()
    );
    println!();
    println!(
        "  {}",
        "Add widgets by dropping .html files in the widgets/ directory.".dimmed()
    );
    println!(
        "  {}",
        "Preview auto-refreshes -- just reload your browser.".dimmed()
    );
}
