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

    // Resolve authentication method from CLI flags
    let auth_method = auth_flags.resolve();

    // Build OAuth config for browser-based re-authentication FIRST (if OAuth configured).
    // This happens before resolve_auth_header so that even if CLI OAuth fails,
    // the browser can still authenticate via the popup flow.
    let oauth_config = match &auth_method {
        AuthMethod::OAuth {
            client_id,
            issuer,
            scopes,
            ..
        } => {
            // Try OIDC discovery to find authorization and token endpoints
            match discover_oauth_endpoints(&url, issuer.as_deref()).await {
                Ok((authorization_endpoint, token_endpoint)) => {
                    if global_flags.should_output() {
                        println!(
                            "  {} OAuth:       {}",
                            "→".blue(),
                            "Browser PKCE flow enabled".bright_green()
                        );
                    }
                    Some(mcp_preview::OAuthPreviewConfig {
                        client_id: client_id.clone(),
                        authorization_endpoint,
                        token_endpoint,
                        scopes: scopes.clone(),
                    })
                },
                Err(e) => {
                    if global_flags.should_output() {
                        eprintln!("  {} OAuth browser flow unavailable: {}", "!".yellow(), e);
                        eprintln!(
                            "  {} CLI-acquired token will be used (no browser re-login)",
                            "!".yellow(),
                        );
                    }
                    None
                },
            }
        },
        _ => None,
    };

    // Resolve CLI-level auth as best-effort (gets initial token for proxy).
    // If this fails but oauth_config is set, the browser handles auth via popup.
    let auth_header = match super::auth::resolve_auth_header(&url, &auth_method).await {
        Ok(header) => header,
        Err(e) => {
            if oauth_config.is_some() {
                // Browser will handle OAuth -- CLI failure is non-fatal
                if global_flags.should_output() {
                    eprintln!(
                        "  {} CLI token acquisition failed: {} (browser OAuth will handle auth)",
                        "!".yellow(),
                        e
                    );
                }
                None
            } else {
                // No browser OAuth fallback -- propagate the error
                return Err(e);
            }
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
        oauth_config,
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

/// Discover OAuth authorization and token endpoints via OIDC discovery.
///
/// Tries the issuer URL first (if provided), then falls back to the MCP
/// server URL. Fetches `/.well-known/openid-configuration` and extracts
/// `authorization_endpoint` and `token_endpoint`.
async fn discover_oauth_endpoints(mcp_url: &str, issuer: Option<&str>) -> Result<(String, String)> {
    let base = if let Some(iss) = issuer {
        iss.trim_end_matches('/')
    } else {
        mcp_url.trim_end_matches('/')
    };

    let discovery_url = format!("{}/.well-known/openid-configuration", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    match client.get(&discovery_url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await?;
            let auth_ep = body
                .get("authorization_endpoint")
                .and_then(|v| v.as_str())
                .map(String::from);
            let token_ep = body
                .get("token_endpoint")
                .and_then(|v| v.as_str())
                .map(String::from);

            if let (Some(auth), Some(token)) = (auth_ep, token_ep) {
                return Ok((auth, token));
            }

            anyhow::bail!(
                "OIDC discovery at {} missing authorization_endpoint or token_endpoint",
                discovery_url
            );
        },
        Ok(resp) => {
            anyhow::bail!(
                "OIDC discovery at {} returned HTTP {}",
                discovery_url,
                resp.status()
            );
        },
        Err(e) => {
            anyhow::bail!(
                "Could not discover OAuth endpoints from {}: {}",
                discovery_url,
                e
            );
        },
    }
}
