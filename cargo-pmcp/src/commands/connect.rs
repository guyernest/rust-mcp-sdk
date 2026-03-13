//! Connect server to MCP clients

use anyhow::Result;
use colored::Colorize;
use std::process::Command;

use crate::commands::flags::{AuthFlags, AuthMethod};

/// Connect server to an MCP client
pub fn execute(
    server: String,
    client: String,
    url: String,
    auth_flags: &AuthFlags,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    let _ = global_flags; // quiet handled via PMCP_QUIET in sub-functions
    let auth_method = auth_flags.resolve();
    match client.to_lowercase().as_str() {
        "claude-code" | "claudecode" | "claude" => connect_claude_code(&server, &url, &auth_method),
        "cursor" => connect_cursor(&server, &url, &auth_method),
        "inspector" => connect_inspector(&url, auth_flags),
        _ => {
            anyhow::bail!(
                "Unknown client '{}'. Supported clients: claude-code, cursor, inspector",
                client
            );
        },
    }
}

fn connect_claude_code(server: &str, url: &str, auth_method: &AuthMethod) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();

    if not_quiet {
        println!("  {} Connecting to Claude Code...", "->".blue());
    }

    // Build args: base command + optional auth header
    let mut args = vec!["mcp", "add", "-t", "http"];

    let header_value;
    if let AuthMethod::ApiKey(key) = auth_method {
        header_value = format!("Authorization: Bearer {}", key);
        args.push("--header");
        args.push(&header_value);
    }

    args.push(server);
    args.push(url);

    // Try to run claude mcp add command with -t http for streamable HTTP transport
    let status = Command::new("claude").args(&args).status();

    match status {
        Ok(s) if s.success() => {
            if not_quiet {
                println!("  {} Connected to Claude Code", "OK".green());
                println!("\n{}", "Next steps:".bright_white().bold());
                println!("  - Open Claude Code");
                println!("  - Try: {}", "\"Add 5 and 3\"".bright_yellow());
            }
        },
        Ok(_) => {
            if not_quiet {
                println!("  {} Failed to connect to Claude Code", "FAIL".red());
                println!("\n{}", "Manual setup:".bright_white().bold());
                let manual_cmd = if let AuthMethod::ApiKey(key) = auth_method {
                    format!(
                        "claude mcp add -t http --header \"Authorization: Bearer {}\" {} {}",
                        key, server, url
                    )
                } else {
                    format!("claude mcp add -t http {} {}", server, url)
                };
                println!("  Run: {}", manual_cmd.bright_cyan());
            }
        },
        Err(_) => {
            if not_quiet {
                println!("  {} Claude CLI not found", "WARN".yellow());
                println!("\n{}", "Manual setup:".bright_white().bold());
                println!(
                    "  1. Install Claude CLI: {}",
                    "npm install -g @anthropic-ai/claude-cli".bright_cyan()
                );
                let manual_cmd = if let AuthMethod::ApiKey(key) = auth_method {
                    format!(
                        "claude mcp add -t http --header \"Authorization: Bearer {}\" {} {}",
                        key, server, url
                    )
                } else {
                    format!("claude mcp add -t http {} {}", server, url)
                };
                println!("  2. Run: {}", manual_cmd.bright_cyan());
            }
        },
    }

    if let AuthMethod::OAuth { .. } = auth_method {
        if not_quiet {
            println!(
                "\n  {} OAuth configuration must be set up in the MCP client directly.",
                "Note:".bright_yellow()
            );
        }
    }

    Ok(())
}

fn connect_cursor(server: &str, url: &str, auth_method: &AuthMethod) -> Result<()> {
    let not_quiet = std::env::var("PMCP_QUIET").is_err();

    if not_quiet {
        println!("  {} Setting up Cursor connection...", "->".blue());

        let config_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".cursor")
            .join("mcp.json");

        println!("\n{}", "Manual setup for Cursor:".bright_white().bold());
        println!(
            "  1. Open: {}",
            config_path.display().to_string().bright_cyan()
        );
        println!("  2. Add this configuration:");
        println!();
        println!("  {{");
        println!("    \"mcpServers\": {{");
        println!("      {}: {{", format!("\"{}\"", server).bright_yellow());
        println!(
            "        {}: {},",
            "\"type\"".bright_cyan(),
            "\"streamable-http\"".bright_green()
        );

        // Add headers when API key is configured
        if let AuthMethod::ApiKey(key) = auth_method {
            println!(
                "        {}: {},",
                "\"url\"".bright_cyan(),
                format!("\"{}\"", url).bright_green()
            );
            println!("        {}: {{", "\"headers\"".bright_cyan());
            println!(
                "          {}: {}",
                "\"Authorization\"".bright_cyan(),
                format!("\"Bearer {}\"", key).bright_green()
            );
            println!("        }}");
        } else {
            println!(
                "        {}: {}",
                "\"url\"".bright_cyan(),
                format!("\"{}\"", url).bright_green()
            );
        }

        println!("      }}");
        println!("    }}");
        println!("  }}");
        println!();
        println!("  3. Restart Cursor");

        if let AuthMethod::OAuth { .. } = auth_method {
            println!(
                "\n  {} OAuth configuration must be set up in Cursor directly.",
                "Note:".bright_yellow()
            );
        }
    }

    Ok(())
}

fn connect_inspector(url: &str, auth_flags: &AuthFlags) -> Result<()> {
    let _ = auth_flags; // Inspector handles its own auth
    let not_quiet = std::env::var("PMCP_QUIET").is_err();

    if not_quiet {
        println!("  {} Opening MCP Inspector...", "->".blue());
    }

    let inspector_url = format!(
        "http://localhost:6274/?transport=streamable-http&serverUrl={}",
        urlencoding::encode(url)
    );

    // Try to open inspector with pre-filled URL
    let status = Command::new("npx")
        .args(["@modelcontextprotocol/inspector"])
        .env("BROWSER", "none") // Prevent auto-opening default browser
        .spawn();

    match status {
        Ok(_) => {
            if not_quiet {
                println!("  {} MCP Inspector starting...", "OK".green());
                println!("\n{}", "Next steps:".bright_white().bold());
                println!("  - Open: {}", inspector_url.bright_cyan());
                println!("  - Or visit: {}", "http://localhost:6274".bright_cyan());
                println!("  - Enter server URL: {}", url.bright_yellow());
            }
        },
        Err(_) => {
            if not_quiet {
                println!(
                    "  {} Could not start Inspector automatically",
                    "WARN".yellow()
                );
                println!("\n{}", "Manual setup:".bright_white().bold());
                println!(
                    "  1. Run: {}",
                    "npx @modelcontextprotocol/inspector".bright_cyan()
                );
                println!("  2. Open: {}", "http://localhost:6274".bright_cyan());
                println!(
                    "  3. Select transport: {}",
                    "streamable-http".bright_yellow()
                );
                println!("  4. Enter URL: {}", url.bright_yellow());
            }
        },
    }

    Ok(())
}
