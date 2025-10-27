//! Add components to workspace

use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

use crate::templates;

pub fn server(name: String, template: String) -> Result<()> {
    println!("\n{}", "Adding MCP server".bright_cyan().bold());
    println!("{}", "─────────────────────────".bright_cyan());

    // Verify we're in a workspace
    if !PathBuf::from("Cargo.toml").exists() {
        anyhow::bail!("Not in a workspace directory. Run 'cargo-pmcp new <name>' first.");
    }

    // Generate server crates
    templates::server::generate(&name, &template)?;

    println!(
        "\n{} Server '{}' added successfully!",
        "✓".green().bold(),
        name.bright_yellow()
    );

    // Show template-specific information
    if template == "complete" {
        println!(
            "\n{}",
            "📚 Complete Template Includes:".bright_white().bold()
        );
        println!();
        println!("  {} Tools (5):", "🔧".bright_cyan());
        println!("    • add, subtract, multiply    - Basic arithmetic");
        println!("    • divide                     - With zero-division check");
        println!("    • power                      - Exponentiation");
        println!();
        println!("  {} Prompts (1):", "💬".bright_cyan());
        println!("    • quadratic                  - Solve quadratic equations");
        println!("                                   Shows how prompts compose tools");
        println!();
        println!("  {} Resources (1):", "📖".bright_cyan());
        println!("    • quadratic-formula          - Educational guide");
        println!("                                   Explains the mathematical theory");
        println!();
        println!("  {} What You'll Learn:", "🎓".bright_cyan());
        println!("    ✓ Tool patterns    - See how similar tools follow the same structure");
        println!("    ✓ Error handling   - Division by zero validation");
        println!("    ✓ Composition      - How prompts orchestrate multiple tools");
        println!("    ✓ Resources        - Providing static knowledge/documentation");
        println!("    ✓ Workflows        - Multi-step mathematical operations");
        println!();
    }

    println!("{}", "🚀 Quick Start (2 minutes):".bright_white().bold());
    println!();
    println!("  {} Start your server:", "1.".bright_cyan().bold());
    println!(
        "     {}",
        format!("cargo pmcp dev --server {}", name).bright_yellow()
    );
    println!();
    println!("  {} Connect to a client:", "2.".bright_cyan().bold());
    println!(
        "     Claude Code:  {}",
        format!("cargo pmcp connect --server {} --client claude-code", name).bright_yellow()
    );
    println!(
        "     Inspector:    {}",
        format!("cargo pmcp connect --server {} --client inspector", name).bright_yellow()
    );
    println!(
        "     Cursor:       {}",
        format!("cargo pmcp connect --server {} --client cursor", name).bright_yellow()
    );
    println!();
    println!("  {} Try it out:", "3.".bright_cyan().bold());
    if template == "complete" {
        println!("     In Claude Code:");
        println!("       {}", "\"Multiply 7 and 8\"".bright_green());
        println!("       {}", "\"What's 100 divided by 5?\"".bright_green());
        println!(
            "       {}",
            "\"Solve the quadratic equation: x² - 5x + 6 = 0\"".bright_green()
        );
        println!(
            "       {}",
            "\"Show me the quadratic formula guide\"".bright_green()
        );
    } else {
        println!("     In Claude Code: {}", "\"Add 5 and 3\"".bright_green());
    }
    println!();

    println!("{}", "📋 Additional Commands:".bright_white().bold());
    println!(
        "  • Generate tests: {}",
        format!("cargo pmcp test --server {} --generate-scenarios", name).bright_cyan()
    );
    println!(
        "  • Run tests:      {}",
        format!("cargo pmcp test --server {}", name).bright_cyan()
    );
    println!(
        "  • Unit tests:     {}",
        format!("cargo test -p mcp-{}-core", name).bright_cyan()
    );

    Ok(())
}

pub fn tool(name: String, server: String) -> Result<()> {
    println!("\n{}", "Adding tool".bright_cyan().bold());
    println!("{}", "────────────────".bright_cyan());

    // TODO: Implement tool scaffolding
    println!(
        "  {} Tool '{}' scaffolding for server '{}'",
        "✓".green(),
        name,
        server
    );
    println!("\n{} Coming in next phase", "⚠".yellow().bold());

    Ok(())
}

pub fn workflow(name: String, server: String) -> Result<()> {
    println!("\n{}", "Adding workflow".bright_cyan().bold());
    println!("{}", "──────────────────────".bright_cyan());

    // TODO: Implement workflow scaffolding
    println!(
        "  {} Workflow '{}' scaffolding for server '{}'",
        "✓".green(),
        name,
        server
    );
    println!("\n{} Coming in next phase", "⚠".yellow().bold());

    Ok(())
}
