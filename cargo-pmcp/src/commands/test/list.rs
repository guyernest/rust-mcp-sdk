//! List test scenarios on pmcp.run

use anyhow::{Context, Result};
use colored::Colorize;

use crate::commands::GlobalFlags;
use crate::deployment::targets::pmcp_run::{auth, graphql};
use crate::pentest::schema_utils::truncate_for_evidence;

/// List test scenarios for an MCP server on pmcp.run
pub async fn execute(server_id: String, show_all: bool, global_flags: &GlobalFlags) -> Result<()> {
    if global_flags.should_output() {
        println!("\n{}", "Test scenarios on pmcp.run".bright_cyan().bold());
        println!("{}", "─────────────────────────────────────".bright_cyan());
    }

    let credentials = auth::get_credentials().await?;

    if global_flags.should_output() {
        println!("  {} Server ID: {}", "→".blue(), server_id);
    }

    let result = graphql::list_test_scenarios(&credentials.access_token, &server_id)
        .await
        .context("Failed to list scenarios")?;

    println!();

    if result.scenarios.is_empty() {
        print_empty_scenarios_hint(&server_id, global_flags);
        return Ok(());
    }

    let enabled_count = result.scenarios.iter().filter(|s| s.enabled).count();
    let disabled_count = result.scenarios.len() - enabled_count;

    println!(
        "{} {} scenario(s) ({} enabled, {} disabled)",
        "📋".to_string(),
        result.scenarios.len(),
        enabled_count,
        disabled_count
    );
    println!();

    print_scenarios_table_header();
    for scenario in &result.scenarios {
        if !show_all && !scenario.enabled {
            continue;
        }
        print_scenario_row(scenario);
    }

    print_list_footer(&server_id, disabled_count, show_all, global_flags);

    Ok(())
}

/// Print the "no scenarios found" hint with links to test generate + upload.
fn print_empty_scenarios_hint(server_id: &str, global_flags: &GlobalFlags) {
    println!("{}", "No test scenarios found".yellow());
    if global_flags.should_output() {
        println!();
        println!("{}", "To add scenarios:".bright_white().bold());
        println!("  1. Generate scenarios locally:");
        println!("     cargo pmcp test generate --url <server-url>");
        println!();
        println!("  2. Upload to pmcp.run:");
        println!(
            "     cargo pmcp test upload --server {} scenarios/",
            server_id
        );
    }
}

/// Print the column headers for the scenarios table.
fn print_scenarios_table_header() {
    println!(
        "  {:<40} {:<12} {:<10} {:<8} {}",
        "NAME".bright_white().bold(),
        "SOURCE".bright_white().bold(),
        "STATUS".bright_white().bold(),
        "VERSION".bright_white().bold(),
        "LAST RUN".bright_white().bold()
    );
    println!("  {}", "─".repeat(90));
}

/// Print one scenario row plus optional description.
fn print_scenario_row(scenario: &crate::deployment::targets::pmcp_run::graphql::ScenarioInfo) {
    let status = if scenario.enabled {
        "enabled".green().to_string()
    } else {
        "disabled".yellow().to_string()
    };

    let last_run = format_last_run(
        scenario.last_execution_status.as_deref(),
        scenario.last_executed_at.as_deref(),
    );
    let source_display = format_source(scenario.source.as_str());

    println!(
        "  {:<40} {:<12} {:<10} v{:<7} {}",
        truncate_for_evidence(&scenario.name, 38),
        source_display,
        status,
        scenario.version,
        last_run
    );

    if let Some(ref desc) = scenario.description {
        if !desc.is_empty() {
            println!("    {}", desc.bright_black());
        }
    }
}

/// Format the "last run" cell: "<timestamp> (<status-colored>)" or "-".
fn format_last_run(status: Option<&str>, timestamp: Option<&str>) -> String {
    match status {
        Some(s) => {
            let status_colored = match s {
                "passed" => "passed".green().to_string(),
                "failed" => "failed".red().to_string(),
                "error" => "error".red().to_string(),
                "running" => "running".blue().to_string(),
                other => other.to_string(),
            };
            format!("{} ({})", timestamp.unwrap_or("-"), status_colored)
        },
        None => "-".to_string(),
    }
}

/// Format the "source" cell with color overrides for known kinds.
fn format_source(source: &str) -> String {
    match source {
        "auto_generated" => "auto".cyan().to_string(),
        "user_created" => "user".to_string(),
        "user_modified" => "modified".to_string(),
        "imported" => "imported".to_string(),
        other => other.to_string(),
    }
}

/// Print the trailing command hints + disabled-count message.
fn print_list_footer(
    server_id: &str,
    disabled_count: usize,
    show_all: bool,
    global_flags: &GlobalFlags,
) {
    if !global_flags.should_output() {
        return;
    }

    println!();
    println!(
        "{}",
        "═════════════════════════════════════════════════════════════════".bright_cyan()
    );
    println!();
    println!("{}", "Commands:".bright_white().bold());
    println!("  Download a scenario:  cargo pmcp test download --scenario-id <id>");
    println!(
        "  Upload scenarios:     cargo pmcp test upload --server {} <path>",
        server_id
    );

    if disabled_count > 0 && !show_all {
        println!();
        println!(
            "  {} {} disabled scenario(s) hidden. Use --all to show all.",
            "ℹ".blue(),
            disabled_count
        );
    }
}

