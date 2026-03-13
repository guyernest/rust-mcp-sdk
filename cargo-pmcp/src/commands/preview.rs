//! Preview command - MCP Apps UI testing environment

use anyhow::Result;
use colored::Colorize;

use crate::commands::flags::{AuthFlags, AuthMethod};

/// Start the MCP Apps preview server
pub async fn execute(
    url: String,
    port: u16,
    open: bool,
    tool: Option<String>,
    theme: String,
    locale: String,
    widgets_dir: Option<String>,
    mode: String,
    auth_flags: &AuthFlags,
    global_flags: &crate::commands::GlobalFlags,
) -> Result<()> {
    let preview_mode = if mode == "chatgpt" {
        mcp_preview::PreviewMode::ChatGpt
    } else {
        mcp_preview::PreviewMode::Standard
    };

    if global_flags.should_output() {
        println!("\n{}", "Starting MCP Apps Preview".bright_cyan().bold());
        println!("{}", "─────────────────────────────────".bright_cyan());
        println!("  {} MCP Server: {}", "→".blue(), url.bright_yellow());
        println!(
            "  {} Preview URL: {}",
            "→".blue(),
            format!("http://localhost:{}", port).bright_green()
        );
        if let Some(ref dir) = widgets_dir {
            println!(
                "  {} Widgets Dir: {} (hot-reload)",
                "→".blue(),
                dir.bright_magenta()
            );
        }
        let mode_display = match preview_mode {
            mcp_preview::PreviewMode::ChatGpt => "ChatGPT Strict".bright_red().bold(),
            mcp_preview::PreviewMode::Standard => "Standard".bright_green().bold(),
        };
        println!("  {} Mode:        {}", "→".blue(), mode_display);
        println!();
    }

    // Resolve authentication
    let auth_method = auth_flags.resolve();
    let auth_header = match &auth_method {
        AuthMethod::None => None,
        AuthMethod::ApiKey(key) => Some(format!("Bearer {}", key)),
        AuthMethod::OAuth {
            client_id,
            issuer,
            scopes,
            no_cache,
            redirect_port,
        } => {
            use pmcp::client::oauth::{default_cache_path, OAuthConfig, OAuthHelper};

            let cache_file = if *no_cache {
                None
            } else {
                Some(default_cache_path())
            };
            let config = OAuthConfig {
                issuer: issuer.clone(),
                mcp_server_url: Some(url.clone()),
                client_id: client_id.clone(),
                scopes: scopes.clone(),
                cache_file,
                redirect_port: *redirect_port,
            };
            let helper = OAuthHelper::new(config)
                .map_err(|e| anyhow::anyhow!("OAuth setup failed: {e}"))?;
            let token = helper
                .get_access_token()
                .await
                .map_err(|e| anyhow::anyhow!("OAuth token acquisition failed: {e}"))?;
            Some(format!("Bearer {}", token))
        },
    };

    let widgets_path = widgets_dir.map(std::path::PathBuf::from);

    let config = mcp_preview::PreviewConfig {
        mcp_url: url,
        port,
        initial_tool: tool,
        theme,
        locale,
        widgets_dir: widgets_path,
        mode: preview_mode,
        auth_header,
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
