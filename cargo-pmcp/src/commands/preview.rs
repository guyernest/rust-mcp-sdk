//! Preview command - MCP Apps UI testing environment

use anyhow::Result;
use colored::Colorize;

/// Start the MCP Apps preview server
pub async fn execute(
    url: String,
    port: u16,
    open: bool,
    tool: Option<String>,
    theme: String,
    locale: String,
) -> Result<()> {
    println!("\n{}", "Starting MCP Apps Preview".bright_cyan().bold());
    println!("{}", "─────────────────────────────────".bright_cyan());
    println!("  {} MCP Server: {}", "→".blue(), url.bright_yellow());
    println!(
        "  {} Preview URL: {}",
        "→".blue(),
        format!("http://localhost:{}", port).bright_green()
    );
    println!();

    let config = mcp_preview::PreviewConfig {
        mcp_url: url,
        port,
        initial_tool: tool,
        theme,
        locale,
    };

    // Open browser if requested
    if open {
        let preview_url = format!("http://localhost:{}", port);
        tokio::spawn(async move {
            // Wait a bit for the server to start
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            if let Err(e) = open::that(&preview_url) {
                eprintln!("Failed to open browser: {}", e);
            }
        });
    }

    mcp_preview::PreviewServer::start(config).await
}
