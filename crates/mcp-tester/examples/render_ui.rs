//! Example: Render UI for MCP tools with interactive UIs
//!
//! This example demonstrates how to use mcp-tester to:
//! 1. Connect to an MCP server
//! 2. Discover tools with UI metadata
//! 3. Fetch and render the UI HTML
//! 4. View the UI in a browser with debug panel
//!
//! Usage:
//!   cargo run --example render_ui -- http://localhost:3000
//!
//! For the conference venue map example:
//!   1. Start the server: cargo run --example conference_venue_map
//!   2. Run this example: cargo run --example render_ui -- http://localhost:3004
//!   3. Open the generated HTML file in your browser

use anyhow::Result;
use mcp_tester::tester::ServerTester;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Get server URL from command line or use default
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "http://localhost:3004".to_string());

    println!("🔍 Connecting to MCP server at: {}", url);
    println!();

    // Create tester
    let mut tester = ServerTester::new(
        &url,
        Duration::from_secs(30),
        false,
        None,
        Some("http"),
        None,
    )?;

    // Initialize connection
    println!("📡 Initializing connection...");
    let init_result = tester.test_initialize().await;
    if init_result.status != mcp_tester::report::TestStatus::Passed {
        eprintln!("❌ Failed to initialize: {}", init_result.name);
        return Ok(());
    }
    println!("✅ Connected successfully");
    println!();

    // List tools
    println!("🔧 Discovering tools...");
    let tools_result = tester.test_tools_list().await;
    if tools_result.status != mcp_tester::report::TestStatus::Passed {
        eprintln!("❌ Failed to list tools: {}", tools_result.name);
        return Ok(());
    }
    println!("✅ Found tools");
    println!();

    // Discover tools with UIs
    println!("🎨 Discovering tools with UIs...");
    tester.load_all_tool_uis().await?;

    let tool_uis = tester.get_tool_uis();

    if tool_uis.is_empty() {
        println!("ℹ️  No tools with UI metadata found");
        println!();
        println!("💡 Make sure the server implements UI resources with:");
        println!("   - UIResourceBuilder for creating UIs");
        println!("   - TypedTool.with_ui() to associate tools with UIs");
        println!();
        return Ok(());
    }

    println!("✅ Found {} tool(s) with UIs:", tool_uis.len());
    for (tool_name, ui_info) in tool_uis {
        println!("   - {} → {}", tool_name, ui_info.ui_resource_uri);
    }
    println!();

    // Render each UI to HTML
    println!("📝 Rendering UIs to HTML files...");
    for (tool_name, _ui_info) in tool_uis {
        let filename = format!("{}_ui.html", tool_name.replace("_", "-"));
        let output_path = std::env::current_dir()?.join(&filename);

        tester.render_tool_ui(tool_name, output_path.to_str().unwrap())?;
    }

    println!();
    println!("✅ Done! UI files generated.");
    println!();
    println!("📖 To view the UIs:");
    println!("   1. Open the HTML file(s) in your browser");
    println!("   2. Click '📊 Toggle Debug' to show/hide the debug panel");
    println!("   3. Tool calls will be logged in the debug panel");
    println!();
    println!("ℹ️  Note: This is a static viewer. Tool calls are logged but not executed.");
    println!("   For interactive testing, use the HTTP server mode (coming soon).");

    Ok(())
}
