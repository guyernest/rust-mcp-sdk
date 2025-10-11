//! OAuth Device Code Flow support for CLI authentication
//!
//! This module implements OAuth 2.0 Device Authorization Grant (RFC 8628)
//! for CLI-friendly authentication without requiring a browser redirect.

use anyhow::{Context, Result};
use colored::*;
use pmcp::client::http_middleware::HttpMiddlewareChain;
use pmcp::client::oauth_middleware::{BearerToken, OAuthClientMiddleware};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

/// OAuth configuration for device code flow
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    /// OAuth issuer URL (e.g., https://auth.example.com)
    pub issuer: String,
    /// OAuth client ID
    pub client_id: String,
    /// OAuth scopes to request
    pub scopes: Vec<String>,
    /// Cache file path for storing tokens
    pub cache_file: Option<PathBuf>,
}

/// Token cache stored on disk
#[derive(Debug, Serialize, Deserialize)]
struct TokenCache {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: Option<u64>,
    scopes: Vec<String>,
}

/// Device code authorization response
#[derive(Debug, Deserialize)]
struct DeviceAuthResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

/// Token response
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    token_type: String,
}

/// OAuth helper for device code flow authentication
pub struct OAuthHelper {
    config: OAuthConfig,
    client: reqwest::Client,
}

impl OAuthHelper {
    /// Create a new OAuth helper
    pub fn new(config: OAuthConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { config, client })
    }

    /// Get or refresh access token, performing device code flow if needed
    pub async fn get_access_token(&self) -> Result<String> {
        // Try to load cached token first
        if let Some(ref cache_file) = self.config.cache_file {
            if let Ok(cached) = self.load_cached_token(cache_file).await {
                // Check if token is still valid
                if let Some(expires_at) = cached.expires_at {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if now < expires_at {
                        println!("{}", "✓ Using cached OAuth token".green());
                        return Ok(cached.access_token);
                    }
                }

                // Try to refresh if we have a refresh token
                if let Some(refresh_token) = cached.refresh_token {
                    println!("{}", "Refreshing OAuth token...".yellow());
                    if let Ok(new_token) = self.refresh_token(&refresh_token).await {
                        self.cache_token(&new_token, cache_file).await?;
                        return Ok(new_token.access_token);
                    }
                }
            }
        }

        // No valid cached token, perform device code flow
        self.device_code_flow().await
    }

    /// Perform OAuth device code flow
    async fn device_code_flow(&self) -> Result<String> {
        println!("{}", "Starting OAuth device code flow...".cyan().bold());
        println!();

        // Step 1: Request device code
        let device_auth_endpoint = format!("{}/oauth/device/code", self.config.issuer);
        let scope = self.config.scopes.join(" ");

        let response = self
            .client
            .post(&device_auth_endpoint)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("scope", &scope),
            ])
            .send()
            .await
            .context("Failed to request device code")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Device authorization failed: {}",
                response.text().await.unwrap_or_default()
            );
        }

        let device_auth: DeviceAuthResponse = response
            .json()
            .await
            .context("Failed to parse device authorization response")?;

        // Step 2: Display user code and verification URL
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                .cyan()
        );
        println!("{}", "  OAuth Authentication Required".cyan().bold());
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                .cyan()
        );
        println!();
        println!(
            "  {}  {}",
            "1. Visit:".bold(),
            device_auth.verification_uri.yellow()
        );
        println!("  {}  {}", "2. Enter code:".bold(), device_auth.user_code.green().bold());

        if let Some(complete_uri) = &device_auth.verification_uri_complete {
            println!();
            println!("  {} Or scan this URL:", "Shortcut:".bold());
            println!("  {}", complete_uri.yellow());
        }

        println!();
        println!(
            "{}",
            "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
                .cyan()
        );
        println!();

        // Step 3: Poll for token
        let poll_interval = Duration::from_secs(device_auth.interval.unwrap_or(5));
        let token_endpoint = format!("{}/oauth/token", self.config.issuer);
        let expires_at = SystemTime::now() + Duration::from_secs(device_auth.expires_in);

        loop {
            if SystemTime::now() > expires_at {
                anyhow::bail!("Device code expired. Please try again.");
            }

            sleep(poll_interval).await;
            print!("  Waiting for authorization...\r");
            let _ = std::io::Write::flush(&mut std::io::stdout());

            let response = self
                .client
                .post(&token_endpoint)
                .form(&[
                    ("client_id", self.config.client_id.as_str()),
                    ("device_code", &device_auth.device_code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ])
                .send()
                .await
                .context("Failed to poll for token")?;

            let status = response.status();
            let body = response.text().await?;

            if status.is_success() {
                let token_response: TokenResponse =
                    serde_json::from_str(&body).context("Failed to parse token response")?;

                println!("{}", "✓ Authentication successful!".green().bold());
                println!();

                // Cache the token
                if let Some(ref cache_file) = self.config.cache_file {
                    self.cache_token(&token_response, cache_file).await?;
                }

                return Ok(token_response.access_token);
            }

            // Check error response
            if let Ok(error) = serde_json::from_str::<serde_json::Value>(&body) {
                if let Some(error_code) = error.get("error").and_then(|e| e.as_str()) {
                    match error_code {
                        "authorization_pending" => continue,
                        "slow_down" => {
                            sleep(poll_interval).await;
                            continue;
                        },
                        "access_denied" => {
                            anyhow::bail!("User denied authorization");
                        },
                        "expired_token" => {
                            anyhow::bail!("Device code expired");
                        },
                        _ => {
                            anyhow::bail!("OAuth error: {}", error_code);
                        },
                    }
                }
            }
        }
    }

    /// Refresh an existing token
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        let token_endpoint = format!("{}/oauth/token", self.config.issuer);

        let response = self
            .client
            .post(&token_endpoint)
            .form(&[
                ("client_id", self.config.client_id.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .context("Failed to refresh token")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Token refresh failed: {}",
                response.text().await.unwrap_or_default()
            );
        }

        response
            .json()
            .await
            .context("Failed to parse token response")
    }

    /// Load cached token from disk
    async fn load_cached_token(&self, cache_file: &PathBuf) -> Result<TokenCache> {
        let content = tokio::fs::read_to_string(cache_file)
            .await
            .context("Failed to read token cache")?;
        serde_json::from_str(&content).context("Failed to parse token cache")
    }

    /// Cache token to disk
    async fn cache_token(&self, token: &TokenResponse, cache_file: &PathBuf) -> Result<()> {
        let expires_at = token.expires_in.map(|secs| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + secs
        });

        let cache = TokenCache {
            access_token: token.access_token.clone(),
            refresh_token: token.refresh_token.clone(),
            expires_at,
            scopes: self.config.scopes.clone(),
        };

        // Ensure directory exists
        if let Some(parent) = cache_file.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create cache directory")?;
        }

        let json = serde_json::to_string_pretty(&cache).context("Failed to serialize cache")?;
        tokio::fs::write(cache_file, json)
            .await
            .context("Failed to write token cache")?;

        println!(
            "{}",
            format!("Token cached to: {}", cache_file.display())
                .dimmed()
        );

        Ok(())
    }

    /// Create HTTP middleware chain with OAuth
    pub async fn create_middleware_chain(&self) -> Result<Arc<HttpMiddlewareChain>> {
        let access_token = self.get_access_token().await?;

        let bearer_token = BearerToken::new(access_token);
        let oauth_middleware = OAuthClientMiddleware::new(bearer_token);

        let mut chain = HttpMiddlewareChain::new();
        chain.add(Arc::new(oauth_middleware));

        Ok(Arc::new(chain))
    }
}

/// Get default cache file path (~/.mcp-tester/tokens.json)
pub fn default_cache_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".mcp-tester");
    path.push("tokens.json");
    path
}
