//! `cargo pmcp auth login` — PKCE + optional DCR, cache result.

use anyhow::{Context, Result};
use clap::Args;
use colored::Colorize;
use pmcp::client::oauth::{OAuthConfig, OAuthHelper};

use crate::commands::auth_cmd::cache::{
    default_multi_cache_path, normalize_cache_key, TokenCacheEntry, TokenCacheV1,
};
use crate::commands::GlobalFlags;

/// `cargo pmcp auth login <url> [flags]`
///
/// Runs a full OAuth authorization (PKCE + optional DCR) against the named MCP
/// server and persists the resulting `AccessToken`/`RefreshToken`/issuer/scopes
/// under the normalized URL key in `~/.pmcp/oauth-cache.json`.
#[derive(Debug, Args)]
pub struct LoginArgs {
    /// URL of the MCP server to authenticate against
    pub url: String,

    /// Client name for Dynamic Client Registration (RFC 7591).
    /// Mutually exclusive with --oauth-client-id (D-19).
    #[arg(long, conflicts_with = "oauth_client_id")]
    pub client: Option<String>,

    /// Pre-registered OAuth client ID (skips DCR entirely — D-20 escape hatch).
    #[arg(long, env = "MCP_OAUTH_CLIENT_ID")]
    pub oauth_client_id: Option<String>,

    /// OAuth issuer URL for OIDC discovery.
    #[arg(long, env = "MCP_OAUTH_ISSUER")]
    pub oauth_issuer: Option<String>,

    /// OAuth scopes (comma-separated).
    #[arg(long, env = "MCP_OAUTH_SCOPES", value_delimiter = ',')]
    pub oauth_scopes: Option<Vec<String>>,

    /// Localhost port for the OAuth redirect callback.
    #[arg(long, env = "MCP_OAUTH_REDIRECT_PORT", default_value = "8080")]
    pub oauth_redirect_port: u16,
}

/// Execute the `login` subcommand — run the OAuth flow and persist the result.
pub async fn execute(args: LoginArgs, global_flags: &GlobalFlags) -> Result<()> {
    let key = normalize_cache_key(&args.url)
        .with_context(|| format!("normalizing login URL {}", args.url))?;

    // D-04: if neither --client nor --oauth-client-id is passed, cargo-pmcp
    // sets client_name = "cargo-pmcp" so DCR identifies the caller sensibly.
    let client_name = args.client.clone().or_else(|| {
        if args.oauth_client_id.is_none() {
            Some("cargo-pmcp".to_string())
        } else {
            None
        }
    });

    let scopes = args
        .oauth_scopes
        .clone()
        .unwrap_or_else(|| vec!["openid".to_string()]);

    let config = OAuthConfig {
        issuer: args.oauth_issuer.clone(),
        mcp_server_url: Some(args.url.clone()),
        client_id: args.oauth_client_id.clone(),
        client_name,
        dcr_enabled: args.oauth_client_id.is_none(), // DCR only when no preset id
        scopes: scopes.clone(),
        cache_file: None, // multi-server cache is CLI-managed, not SDK-managed
        redirect_port: args.oauth_redirect_port,
    };

    if global_flags.should_output() {
        println!();
        println!("{}", "OAuth Login".bright_cyan().bold());
        println!("  URL: {}", args.url.bright_white());
        if let Some(ref n) = args.client {
            println!("  Client name (DCR): {}", n.bright_white());
        }
        if args.oauth_client_id.is_some() {
            println!("  Client ID: (pre-registered, DCR skipped)");
        }
        println!();
    }

    let helper = OAuthHelper::new(config.clone()).context("OAuth setup failed")?;
    // Blocker #6 fix — use authorize_with_details (Plan 01 Task 1.2b) so we
    // capture refresh_token / expires_at / effective issuer / effective client_id.
    // This makes the cached entry usable for D-15 near-expiry refresh and D-16
    // force-refresh.
    let result = helper
        .authorize_with_details()
        .await
        .context("OAuth flow failed")?;

    let mut cache = TokenCacheV1::read(&default_multi_cache_path())?;
    cache.entries.insert(
        key.clone(),
        TokenCacheEntry {
            // D-12: token is cached, but NEVER printed from this command.
            access_token: result.access_token,
            refresh_token: result.refresh_token,
            expires_at: result.expires_at,
            scopes: if result.scopes.is_empty() {
                scopes.clone()
            } else {
                result.scopes.clone()
            },
            issuer: result.issuer.clone(),
            client_id: result.client_id.clone(),
        },
    );
    cache.write_atomic(&default_multi_cache_path())?;

    if global_flags.should_output() {
        let scope_str = if result.scopes.is_empty() {
            if scopes.is_empty() {
                "<none>".to_string()
            } else {
                scopes.join(",")
            }
        } else {
            result.scopes.join(",")
        };
        let issuer_str = result.issuer.clone().unwrap_or_else(|| "<auto>".to_string());
        let expires_str = match result.expires_at {
            Some(exp) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                if exp > now {
                    format!("{}s", exp - now)
                } else {
                    "already expired".to_string()
                }
            },
            None => "n/a (IdP did not advertise expires_in)".to_string(),
        };
        // D-12 + review LOW-8: NEVER print the token via ANY output path —
        // not println!/eprintln! and not tracing::{info,debug,warn,error,trace}!
        // Only issuer/scopes/expires are safe to surface.
        println!(
            "Logged in to {} (issuer: {}, scopes: {}, expires in: {})",
            key.bright_green().bold(),
            issuer_str,
            scope_str,
            expires_str,
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_rejects_client_with_oauth_client_id() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct TestCli {
            #[command(flatten)]
            args: LoginArgs,
        }
        let result = TestCli::try_parse_from([
            "test-cli",
            "https://x.example",
            "--client",
            "claude-desktop",
            "--oauth-client-id",
            "some-id",
        ]);
        assert!(
            result.is_err(),
            "clap must reject --client with --oauth-client-id (D-19)"
        );
    }

    #[test]
    fn clap_accepts_client_alone() {
        use clap::Parser;
        #[derive(clap::Parser)]
        struct TestCli {
            #[command(flatten)]
            args: LoginArgs,
        }
        let ok = TestCli::try_parse_from([
            "test-cli",
            "https://x.example",
            "--client",
            "claude-desktop",
        ]);
        assert!(ok.is_ok());
    }
}
