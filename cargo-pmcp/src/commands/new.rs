//! Create new MCP workspace

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::templates;

/// Server tier for composition architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerTier {
    /// Foundation servers: Core data connectors (databases, CRMs, APIs)
    Foundation,
    /// Domain servers: Orchestration that composes foundation servers
    Domain,
}

impl ServerTier {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "foundation" => Some(Self::Foundation),
            "domain" => Some(Self::Domain),
            _ => None,
        }
    }
}

pub fn execute(
    name: String,
    path: Option<String>,
    tier: Option<String>,
    kind: Option<String>,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    let not_quiet = global_flags.should_output();
    let tier = tier.as_deref().and_then(ServerTier::from_str);

    let tier_label = match tier {
        Some(ServerTier::Foundation) => " (foundation)",
        Some(ServerTier::Domain) => " (domain)",
        None => "",
    };

    if not_quiet {
        println!(
            "\n{}",
            format!("Creating MCP workspace{}", tier_label)
                .bright_cyan()
                .bold()
        );
        println!("{}", "────────────────────────────────────".bright_cyan());
    }

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

    // --kind branch: emit a SINGLE runnable crate (distinct from the multi-crate
    // workspace path below). `sql-server` (SQL toolkit) and `openapi-server`
    // (OpenAPI/HTTP toolkit) are supported.
    match kind.as_deref() {
        Some("sql-server") => return execute_sql_server(&workspace_dir, &name, global_flags),
        Some("openapi-server") => {
            return execute_openapi_server(&workspace_dir, &name, global_flags)
        },
        Some("workbook-server") => {
            return execute_workbook_server(&workspace_dir, &name, global_flags)
        },
        Some(k) => anyhow::bail!(
            "unknown --kind '{}'; supported: sql-server, openapi-server, workbook-server",
            k
        ),
        None => {},
    }

    // Create workspace structure
    create_workspace_structure(&workspace_dir, &name, tier)?;

    // Generate workspace files
    templates::workspace::generate(&workspace_dir, &name)?;

    // Generate server-common crate
    templates::server_common::generate(&workspace_dir)?;

    // For domain servers, add composition client setup
    if tier == Some(ServerTier::Domain) {
        create_domain_composition_structure(&workspace_dir, &name)?;
    }

    if not_quiet {
        println!("\n{} Workspace created successfully!", "✓".green().bold());

        // Print tier-specific next steps
        match tier {
            Some(ServerTier::Domain) => print_domain_next_steps(&name),
            Some(ServerTier::Foundation) => print_foundation_next_steps(&name),
            None => print_default_next_steps(&name),
        }
    }

    Ok(())
}

/// Validate that `name` is a legal Cargo package name before any filesystem
/// write (Codex MEDIUM — the directory-exists guard alone is not sufficient).
///
/// Rejects: empty names, a leading digit, any character outside
/// `[A-Za-z0-9_-]`, and any name containing a path separator (`/` or `\`) or a
/// `..` parent-directory component (path-traversal guard, T-86-03-02).
fn validate_crate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("invalid crate name: name must not be empty");
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        anyhow::bail!(
            "invalid crate name '{}': must not contain path separators or '..'",
            name
        );
    }
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        anyhow::bail!("invalid crate name '{}': must not start with a digit", name);
    }
    if let Some(bad) = name
        .chars()
        .find(|c| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-'))
    {
        anyhow::bail!(
            "invalid crate name '{}': illegal character '{}' (allowed: A-Z a-z 0-9 _ -)",
            name,
            bad
        );
    }
    Ok(())
}

/// Emit a SINGLE runnable config-driven SQL server crate (Shape B, SHAP-B-01):
/// `Cargo.toml` + `src/main.rs` + `config.toml` + `schema.sql`. The emitted
/// `src/main.rs` is the Plan 02 Shape C wiring, so the same crate runs locally
/// (`cargo run`) AND deploys unchanged to Lambda (H1).
fn execute_sql_server(
    workspace_dir: &Path,
    name: &str,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    // Validate the crate name BEFORE any fs::write (Codex MEDIUM / T-86-03-02).
    validate_crate_name(name)?;

    fs::create_dir_all(workspace_dir.join("src")).context("Failed to create src directory")?;

    templates::sql_server::generate(workspace_dir, name)?;

    if global_flags.should_output() {
        println!(
            "\n{} SQL server crate created successfully!",
            "✓".green().bold()
        );
        print_sql_server_next_steps(name);
    }

    Ok(())
}

fn print_sql_server_next_steps(name: &str) {
    println!(
        "\n{}",
        "🚀 Next Steps (config-driven SQL server):"
            .bright_white()
            .bold()
    );
    println!();
    println!("  {} Enter your crate:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!(
        "  {} Run it (serves over streamable HTTP):",
        "2.".bright_cyan().bold()
    );
    println!("     {}", "cargo run".bright_yellow());
    println!();
    println!(
        "  {} It prints {} — connect your MCP client there.",
        "3.".bright_cyan().bold(),
        "PMCP_SQL_SERVER_ADDR=http://…".bright_green()
    );
    println!();
    println!(
        "  {} Edit {} (tools, code_mode) and {} (tables/seed); both are read at startup.",
        "4.".bright_cyan().bold(),
        "config.toml".bright_green(),
        "schema.sql".bright_green()
    );
}

/// Emit a SINGLE runnable config-driven OpenAPI server crate (Shape B/C/D,
/// OAPI-07): `Cargo.toml` + `src/main.rs` + `config.toml` + `api.yaml` +
/// `deploy.toml`. The emitted `src/main.rs` is the ≤15-line Shape C wiring
/// (CF-5) over the `pmcp-openapi-server` `dispatch` + `build_server` seam, so the
/// same crate runs locally (`cargo run`) AND deploys to pmcp.run (CF-6).
fn execute_openapi_server(
    workspace_dir: &Path,
    name: &str,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    // Validate the crate name BEFORE any fs::write (path-traversal guard,
    // T-90-08-01 — same guard as the SQL arm).
    validate_crate_name(name)?;

    fs::create_dir_all(workspace_dir.join("src")).context("Failed to create src directory")?;

    templates::openapi_server::generate(workspace_dir, name)?;

    if global_flags.should_output() {
        println!(
            "\n{} OpenAPI server crate created successfully!",
            "✓".green().bold()
        );
        print_openapi_server_next_steps(name);
    }

    Ok(())
}

fn print_openapi_server_next_steps(name: &str) {
    println!(
        "\n{}",
        "🚀 Next Steps (config-driven OpenAPI server):"
            .bright_white()
            .bold()
    );
    println!();
    println!("  {} Enter your crate:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!(
        "  {} Point {} at your REST API and run it (serves over streamable HTTP):",
        "2.".bright_cyan().bold(),
        "config.toml".bright_green()
    );
    println!("     {}", "cargo run".bright_yellow());
    println!();
    println!(
        "  {} It prints {} — connect your MCP client there.",
        "3.".bright_cyan().bold(),
        "PMCP_OPENAPI_SERVER_ADDR=http://…".bright_green()
    );
    println!();
    println!(
        "  {} Edit {} ([backend], tools, code_mode) and {} (the OpenAPI spec);",
        "4.".bright_cyan().bold(),
        "config.toml".bright_green(),
        "api.yaml".bright_green()
    );
    println!(
        "     both are read at startup ({} is optional).",
        "api.yaml".bright_green()
    );
    println!();
    println!(
        "  {} Deploy to pmcp.run: {}",
        "5.".bright_cyan().bold(),
        "cargo pmcp deploy".bright_yellow()
    );
}

/// Emit a SINGLE runnable governed-Excel workbook server crate (Shape B,
/// WBCL-05). The payload is `Cargo.toml`, `src/main.rs` (EmbeddedSource wiring),
/// `pmcp.toml`, `workbook/tax-calc.xlsx` (source), and `bundle/tax-calc@1.1.0/*`
/// (pre-compiled). `cargo run` serves the five workbook tools immediately; the
/// dev can edit the workbook, run `cargo pmcp workbook compile`, then rerun (the
/// full authoring loop, D-06). Mirrors `execute_sql_server`: `validate_crate_name`
/// runs FIRST (path-traversal guard, T-96-04), then the embedded-asset emitter.
fn execute_workbook_server(
    workspace_dir: &Path,
    name: &str,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    // Validate the crate name BEFORE any fs::write (path-traversal guard,
    // T-96-04 — same reused guard as the SQL/OpenAPI arms).
    validate_crate_name(name)?;

    fs::create_dir_all(workspace_dir.join("src")).context("Failed to create src directory")?;

    templates::workbook_server::generate(workspace_dir, name)?;

    if global_flags.should_output() {
        println!(
            "\n{} Workbook server crate created successfully!",
            "✓".green().bold()
        );
        print_workbook_server_next_steps(name);
    }

    Ok(())
}

fn print_workbook_server_next_steps(name: &str) {
    println!(
        "\n{}",
        "🚀 Next Steps (governed-Excel workbook server):"
            .bright_white()
            .bold()
    );
    println!();
    println!("  {} Enter your crate:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!(
        "  {} Run it (serves over streamable HTTP):",
        "2.".bright_cyan().bold()
    );
    println!("     {}", "cargo run".bright_yellow());
    println!();
    println!(
        "  {} It prints {} — connect your MCP client there (5 workbook tools).",
        "3.".bright_cyan().bold(),
        "PMCP_WORKBOOK_SERVER_ADDR=http://…".bright_green()
    );
    println!();
    println!(
        "  {} Edit {} then recompile the embedded bundle:",
        "4.".bright_cyan().bold(),
        "workbook/tax-calc.xlsx".bright_green()
    );
    println!("     {}", "cargo pmcp workbook compile".bright_yellow());
    println!(
        "     {} then rerun {}.",
        "↳".bright_cyan(),
        "cargo run".bright_yellow()
    );
}

fn create_workspace_structure(
    workspace_dir: &Path,
    _name: &str,
    tier: Option<ServerTier>,
) -> Result<()> {
    // Create main directories
    fs::create_dir_all(workspace_dir).context("Failed to create workspace directory")?;

    fs::create_dir_all(workspace_dir.join("crates"))
        .context("Failed to create crates directory")?;

    fs::create_dir_all(workspace_dir.join("scenarios"))
        .context("Failed to create scenarios directory")?;

    fs::create_dir_all(workspace_dir.join("lambda"))
        .context("Failed to create lambda directory")?;

    // For domain servers, create schemas directory for foundation server schemas
    if tier == Some(ServerTier::Domain) {
        fs::create_dir_all(workspace_dir.join("schemas"))
            .context("Failed to create schemas directory")?;
    }

    if std::env::var("PMCP_QUIET").is_err() {
        println!("  {} Generated workspace structure", "✓".green());
    }

    Ok(())
}

/// Create additional structure for domain servers that will compose foundation servers
fn create_domain_composition_structure(workspace_dir: &Path, name: &str) -> Result<()> {
    // Create src/foundations directory for generated clients
    let foundations_dir = workspace_dir
        .join("crates")
        .join(format!("mcp-{}-core", name))
        .join("src")
        .join("foundations");
    fs::create_dir_all(&foundations_dir).context("Failed to create foundations directory")?;

    // Create mod.rs for foundations
    let mod_content = r#"//! Generated typed clients for foundation MCP servers
//!
//! This module contains auto-generated clients for foundation servers
//! that this domain server composes.
//!
//! To add a foundation server client:
//! 1. Export the schema: cargo pmcp schema export --server foundation-server-id
//! 2. Generate client: cargo pmcp generate client --schema schemas/foundation.json --output src/foundations/

// Example: pub mod calculator;
"#;

    fs::write(foundations_dir.join("mod.rs"), mod_content)
        .context("Failed to create foundations/mod.rs")?;

    if std::env::var("PMCP_QUIET").is_err() {
        println!(
            "  {} Created composition structure for domain server",
            "✓".green()
        );
    }

    Ok(())
}

fn print_default_next_steps(name: &str) {
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
}

fn print_foundation_next_steps(name: &str) {
    println!(
        "\n{}",
        "🔧 Foundation Server - Next Steps:".bright_white().bold()
    );
    println!();
    println!("  {} Enter your workspace:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!(
        "  {} Add tools for your data source:",
        "2.".bright_cyan().bold()
    );
    println!(
        "     {}",
        "cargo pmcp add server my-connector".bright_yellow()
    );
    println!();
    println!(
        "  {} Add output schemas for type-safe composition:",
        "3.".bright_cyan().bold()
    );
    println!(
        "     Use {} to define output types",
        "TypedToolWithOutput<Input, Output>".bright_green()
    );
    println!();
    println!("  {} Deploy to pmcp.run:", "4.".bright_cyan().bold());
    println!("     {}", "cargo pmcp deploy".bright_yellow());
}

fn print_domain_next_steps(name: &str) {
    println!(
        "\n{}",
        "🏗️  Domain Server - Next Steps:".bright_white().bold()
    );
    println!();
    println!("  {} Enter your workspace:", "1.".bright_cyan().bold());
    println!("     {}", format!("cd {}", name).bright_yellow());
    println!();
    println!(
        "  {} Export foundation server schemas:",
        "2.".bright_cyan().bold()
    );
    println!(
        "     {}",
        "cargo pmcp schema export --server calculator --output schemas/".bright_yellow()
    );
    println!();
    println!("  {} Generate typed clients:", "3.".bright_cyan().bold());
    println!(
        "     {}",
        "cargo pmcp generate client --schema schemas/calculator.json".bright_yellow()
    );
    println!();
    println!("  {} Build domain capabilities:", "4.".bright_cyan().bold());
    println!(
        "     Use generated clients like {}",
        "calculator.add(1.0, 2.0).await".bright_green()
    );
    println!();
    println!("  {} Deploy to pmcp.run:", "5.".bright_cyan().bold());
    println!("     {}", "cargo pmcp deploy".bright_yellow());
}
