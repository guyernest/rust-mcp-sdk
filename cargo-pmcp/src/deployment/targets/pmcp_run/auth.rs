use anyhow::{bail, Context, Result};
use oauth2::{
    basic::{
        BasicErrorResponse, BasicRevocationErrorResponse, BasicTokenIntrospectionResponse,
        BasicTokenType,
    },
    AuthUrl, AuthorizationCode, Client, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl,
    RefreshToken, Scope, StandardRevocableToken, StandardTokenResponse, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

// Custom token fields to capture Cognito's id_token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitoTokenFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
}

impl oauth2::ExtraTokenFields for CognitoTokenFields {}

type CognitoTokenResponse = StandardTokenResponse<CognitoTokenFields, BasicTokenType>;

// Custom OAuth2 client with Cognito token fields
// This is like BasicClient but with custom token response type
type CognitoClient<
    HasAuthUrl = oauth2::EndpointNotSet,
    HasDeviceAuthUrl = oauth2::EndpointNotSet,
    HasIntrospectionUrl = oauth2::EndpointNotSet,
    HasRevocationUrl = oauth2::EndpointNotSet,
    HasTokenUrl = oauth2::EndpointNotSet,
> = Client<
    BasicErrorResponse,
    CognitoTokenResponse,
    BasicTokenIntrospectionResponse,
    StandardRevocableToken,
    BasicRevocationErrorResponse,
    HasAuthUrl,
    HasDeviceAuthUrl,
    HasIntrospectionUrl,
    HasRevocationUrl,
    HasTokenUrl,
>;

// OAuth callback port for local server
const CALLBACK_PORT: u16 = 8787;

// Production defaults for pmcp.run
const DEFAULT_API_URL: &str = "https://api.pmcp.run";
const DEFAULT_AUTH_DOMAIN: &str = "auth.pmcp.run";

// Cache duration for discovered config (1 hour)
const CONFIG_CACHE_DURATION_SECS: u64 = 3600;

/// pmcp.run service configuration discovered from well-known endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PmcpRunConfig {
    /// OAuth client ID for authentication
    pub cognito_client_id: String,
    /// Cognito domain for OAuth flows
    pub cognito_domain: String,
    /// GraphQL API URL
    #[serde(default)]
    pub graphql_url: Option<String>,
    /// Config version for compatibility checking
    #[serde(default)]
    pub version: Option<String>,
}

/// Cached configuration with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedConfig {
    config: PmcpRunConfig,
    cached_at: String,
}

/// Get the base API URL from environment or use default
fn get_api_base_url() -> String {
    std::env::var("PMCP_RUN_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string())
}

/// Get the config cache file path
fn config_cache_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let pmcp_dir = home.join(".pmcp");
    if !pmcp_dir.exists() {
        std::fs::create_dir_all(&pmcp_dir)?;
    }
    Ok(pmcp_dir.join("pmcp-run-config.json"))
}

/// Load cached config if valid
fn load_cached_config() -> Option<PmcpRunConfig> {
    let path = config_cache_path().ok()?;
    if !path.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&path).ok()?;
    let cached: CachedConfig = serde_json::from_str(&content).ok()?;

    // Check if cache is still valid
    let cached_at = chrono::DateTime::parse_from_rfc3339(&cached.cached_at).ok()?;
    let age = chrono::Utc::now()
        .signed_duration_since(cached_at)
        .num_seconds();

    if age < CONFIG_CACHE_DURATION_SECS as i64 {
        Some(cached.config)
    } else {
        None
    }
}

/// Save config to cache
fn save_config_cache(config: &PmcpRunConfig) -> Result<()> {
    let path = config_cache_path()?;
    let cached = CachedConfig {
        config: config.clone(),
        cached_at: chrono::Utc::now().to_rfc3339(),
    };
    std::fs::write(&path, serde_json::to_string_pretty(&cached)?)?;
    Ok(())
}

/// Fetch configuration from pmcp.run discovery endpoint
async fn fetch_pmcp_config() -> Result<PmcpRunConfig> {
    let api_url = get_api_base_url();
    let discovery_url = format!("{}/.well-known/pmcp-config", api_url);

    let client = reqwest::Client::new();
    let response = client
        .get(&discovery_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .context("Failed to fetch pmcp.run configuration")?;

    if !response.status().is_success() {
        bail!(
            "Discovery endpoint returned status {}: {}",
            response.status(),
            discovery_url
        );
    }

    let config: PmcpRunConfig = response
        .json()
        .await
        .context("Failed to parse pmcp.run configuration")?;

    // Cache the fetched config
    if let Err(e) = save_config_cache(&config) {
        eprintln!("⚠️  Warning: Could not cache config: {}", e);
    }

    Ok(config)
}

/// Get pmcp.run configuration with fallback chain:
/// 1. Environment variables (highest priority)
/// 2. Cached config from previous discovery
/// 3. Discovery endpoint fetch
/// 4. Default values (fallback)
pub async fn get_pmcp_config() -> Result<PmcpRunConfig> {
    // Check for environment variable overrides first
    let env_client_id = std::env::var("PMCP_RUN_COGNITO_CLIENT_ID").ok();
    let env_domain = std::env::var("PMCP_RUN_COGNITO_DOMAIN").ok();

    // If both env vars are set, use them directly
    if let (Some(client_id), Some(domain)) = (env_client_id.clone(), env_domain.clone()) {
        return Ok(PmcpRunConfig {
            cognito_client_id: client_id,
            cognito_domain: domain,
            graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL").ok(),
            version: None,
        });
    }

    // Try cached config
    if let Some(cached) = load_cached_config() {
        // Apply any env var overrides to cached config
        return Ok(PmcpRunConfig {
            cognito_client_id: env_client_id.unwrap_or(cached.cognito_client_id),
            cognito_domain: env_domain.unwrap_or(cached.cognito_domain),
            graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL")
                .ok()
                .or(cached.graphql_url),
            version: cached.version,
        });
    }

    // Try discovery endpoint
    match fetch_pmcp_config().await {
        Ok(config) => {
            // Apply any env var overrides
            Ok(PmcpRunConfig {
                cognito_client_id: env_client_id.unwrap_or(config.cognito_client_id),
                cognito_domain: env_domain.unwrap_or(config.cognito_domain),
                graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL")
                    .ok()
                    .or(config.graphql_url),
                version: config.version,
            })
        },
        Err(e) => {
            // Discovery failed - check if we have partial env vars
            if env_client_id.is_some() || env_domain.is_some() {
                eprintln!(
                    "⚠️  Discovery failed, using partial environment config: {}",
                    e
                );
                Ok(PmcpRunConfig {
                    cognito_client_id: env_client_id.unwrap_or_default(),
                    cognito_domain: env_domain.unwrap_or_else(|| DEFAULT_AUTH_DOMAIN.to_string()),
                    graphql_url: std::env::var("PMCP_RUN_GRAPHQL_URL").ok(),
                    version: None,
                })
            } else {
                // No env vars, no cache, discovery failed
                bail!(
                    "❌ Could not retrieve pmcp.run configuration\n\n\
                     Discovery endpoint failed: {}\n\n\
                     💡 Options:\n\
                     1. Check your internet connection\n\
                     2. Set environment variables manually:\n\
                        PMCP_RUN_COGNITO_CLIENT_ID=<client_id>\n\
                        PMCP_RUN_COGNITO_DOMAIN=<domain>\n\
                     3. Visit https://pmcp.run/settings for configuration values\n",
                    e
                )
            }
        },
    }
}

/// Get Cognito domain (legacy function for compatibility)
fn get_cognito_domain_from_config(config: &PmcpRunConfig) -> String {
    config.cognito_domain.clone()
}

/// Get Cognito client ID (legacy function for compatibility)
fn get_cognito_client_id_from_config(config: &PmcpRunConfig) -> String {
    config.cognito_client_id.clone()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
    pub expires_at: String,
}

/// Get credentials file path
fn credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Could not determine home directory")?;
    let pmcp_dir = home.join(".pmcp");

    // Create directory if it doesn't exist
    if !pmcp_dir.exists() {
        std::fs::create_dir_all(&pmcp_dir)?;
    }

    Ok(pmcp_dir.join("credentials.toml"))
}

/// Load credentials from file or environment (for CI/CD)
///
/// This function supports two authentication methods:
///
/// 1. **Interactive (developers)**: Reads from `~/.pmcp/credentials.toml`
///    after running `cargo pmcp deploy login --target pmcp-run`
///
/// 2. **Client Credentials (CI/CD)**: Uses OAuth 2.0 client_credentials flow
///    when `PMCP_CLIENT_ID` and `PMCP_CLIENT_SECRET` environment variables are set.
///    This is ideal for automated deployments in CI/CD pipelines like GitHub Actions,
///    GitLab CI, AWS CodeBuild, etc.
///
/// For CI/CD setup, create a Cognito App Client with client_credentials grant enabled,
/// then set the environment variables with your client credentials.
pub async fn get_credentials() -> Result<Credentials> {
    // Check for client credentials flow (M2M / service account for CI/CD)
    if let (Ok(client_id), Ok(client_secret)) = (
        std::env::var("PMCP_CLIENT_ID"),
        std::env::var("PMCP_CLIENT_SECRET"),
    ) {
        return get_credentials_via_client_credentials(&client_id, &client_secret).await;
    }

    // Check for direct access token (alternative CI/CD method)
    if let Ok(access_token) = std::env::var("PMCP_ACCESS_TOKEN") {
        return Ok(Credentials {
            access_token,
            refresh_token: String::new(),
            id_token: std::env::var("PMCP_ID_TOKEN").unwrap_or_default(),
            expires_at: chrono::Utc::now()
                .checked_add_signed(chrono::Duration::hours(1))
                .unwrap()
                .to_rfc3339(),
        });
    }

    // Fall back to file-based credentials (interactive login)
    let path = credentials_path()?;

    if !path.exists() {
        bail!(
            "❌ Not authenticated with pmcp.run\n\n\
             💡 Authentication options:\n\n\
             For interactive use (developers):\n\
               cargo pmcp deploy login --target pmcp-run\n\n\
             For CI/CD pipelines:\n\
               Set PMCP_CLIENT_ID and PMCP_CLIENT_SECRET environment variables\n\
               (requires a Cognito App Client with client_credentials grant)\n"
        );
    }

    let content = std::fs::read_to_string(&path)?;
    let value: toml::Value = toml::from_str(&content)?;

    let pmcp_run = value
        .get("pmcp-run")
        .context("pmcp-run credentials not found")?;

    let credentials: Credentials = toml::from_str(&toml::to_string(pmcp_run)?)?;

    // Check if expired
    let expires_at = chrono::DateTime::parse_from_rfc3339(&credentials.expires_at)
        .context("Invalid expires_at format")?;

    if expires_at < chrono::Utc::now() {
        // Try to refresh
        return refresh_credentials(&credentials.refresh_token).await;
    }

    Ok(credentials)
}

/// OAuth 2.0 client_credentials token response
#[derive(Debug, Deserialize)]
struct ClientCredentialsResponse {
    access_token: String,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    token_type: Option<String>,
}

/// Get credentials using OAuth 2.0 client_credentials flow (M2M authentication)
///
/// This is used for CI/CD pipelines and automated deployments where interactive
/// login is not possible. Requires a Cognito App Client configured with:
/// - client_credentials grant type enabled
/// - A client secret
/// - Appropriate resource server scopes
async fn get_credentials_via_client_credentials(
    client_id: &str,
    client_secret: &str,
) -> Result<Credentials> {
    let config = get_pmcp_config().await?;
    let token_url = format!("https://{}/oauth2/token", config.cognito_domain);

    let client = reqwest::Client::new();
    let response = client
        .post(&token_url)
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .context("Failed to request access token via client_credentials")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!(
            "❌ Client credentials authentication failed\n\n\
             Status: {}\n\
             Response: {}\n\n\
             💡 Verify that:\n\
             1. PMCP_CLIENT_ID and PMCP_CLIENT_SECRET are correct\n\
             2. The Cognito App Client has client_credentials grant enabled\n\
             3. The client secret matches the App Client configuration\n",
            status,
            body
        );
    }

    let token_response: ClientCredentialsResponse = response
        .json()
        .await
        .context("Failed to parse token response")?;

    let expires_at = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::seconds(
            token_response.expires_in.unwrap_or(3600) as i64,
        ))
        .unwrap()
        .to_rfc3339();

    Ok(Credentials {
        access_token: token_response.access_token,
        refresh_token: String::new(), // client_credentials doesn't return refresh token
        id_token: token_response.id_token.unwrap_or_default(),
        expires_at,
    })
}

/// Refresh access token using refresh token
async fn refresh_credentials(refresh_token: &str) -> Result<Credentials> {
    println!("🔄 Refreshing access token...");

    let config = get_pmcp_config().await?;
    let client = create_oauth_client_from_config(&config)?;
    let http_client = reqwest::Client::new();

    let token_result = client
        .exchange_refresh_token(&RefreshToken::new(refresh_token.to_string()))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            eprintln!("❌ Token refresh failed: {}", e);
            eprintln!();
            eprintln!("💡 Your refresh token may have expired or become invalid.");
            eprintln!("   Please login again:");
            eprintln!("   cargo pmcp deploy login --target pmcp-run");
            eprintln!();
            anyhow::anyhow!("Failed to refresh token: {}", e)
        })?;

    let credentials = Credentials {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: refresh_token.to_string(), // Keep existing refresh token
        id_token: token_result
            .extra_fields()
            .id_token
            .clone()
            .unwrap_or_default(),
        expires_at: chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                token_result
                    .expires_in()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(3600),
            ))
            .unwrap()
            .to_rfc3339(),
    };

    save_credentials(&credentials)?;
    println!("✅ Token refreshed successfully");

    Ok(credentials)
}

/// Save credentials to file
fn save_credentials(credentials: &Credentials) -> Result<()> {
    let path = credentials_path()?;

    let mut config = if path.exists() {
        let content = std::fs::read_to_string(&path)?;
        toml::from_str(&content)?
    } else {
        toml::Value::Table(toml::map::Map::new())
    };

    let creds_toml = toml::to_string(credentials)?;
    let creds_value: toml::Value = toml::from_str(&creds_toml)?;

    config
        .as_table_mut()
        .context("Invalid TOML structure")?
        .insert("pmcp-run".to_string(), creds_value);

    std::fs::write(&path, toml::to_string(&config)?)?;

    Ok(())
}

/// Create OAuth 2.0 client from config
fn create_oauth_client_from_config(
    config: &PmcpRunConfig,
) -> Result<
    CognitoClient<
        oauth2::EndpointSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointNotSet,
        oauth2::EndpointSet,
    >,
> {
    let cognito_domain = get_cognito_domain_from_config(config);
    let cognito_client_id = get_cognito_client_id_from_config(config);

    let auth_url = AuthUrl::new(format!("https://{}/oauth2/authorize", cognito_domain))
        .context("Invalid auth URL")?;
    let token_url = TokenUrl::new(format!("https://{}/oauth2/token", cognito_domain))
        .context("Invalid token URL")?;

    Ok(Client::new(ClientId::new(cognito_client_id))
        .set_auth_uri(auth_url)
        .set_token_uri(token_url))
}

/// Start local HTTP server to receive OAuth callback
fn start_callback_server() -> Result<String> {
    let (tx, rx) = mpsc::channel();

    println!(
        "🌐 Starting local callback server on http://localhost:{}...",
        CALLBACK_PORT
    );

    std::thread::spawn(move || {
        let server = tiny_http::Server::http(format!("127.0.0.1:{}", CALLBACK_PORT)).unwrap();

        for request in server.incoming_requests() {
            let url = request.url().to_string();

            // Parse query parameters
            let mut code_value = None;
            if let Some(query) = url.split('?').nth(1) {
                for param in query.split('&') {
                    if let Some((key, value)) = param.split_once('=') {
                        if key == "code" {
                            code_value = Some(value.to_string());
                            break;
                        }
                    }
                }
            }

            if let Some(code) = code_value {
                // Send success response
                let response = tiny_http::Response::from_string(
                    "<html><body><h1>✅ Authentication Successful!</h1>\
                    <p>You can close this window and return to your terminal.</p>\
                    </body></html>",
                )
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html"[..]).unwrap(),
                );

                let _ = request.respond(response);
                let decoded = urlencoding::decode(&code).unwrap();
                tx.send(decoded.to_string()).unwrap();
                return;
            } else {
                let response = tiny_http::Response::from_string(
                    "<html><body><h1>❌ Authentication Failed</h1>\
                    <p>No code received. Please try again.</p>\
                    </body></html>",
                );
                let _ = request.respond(response);
            }
        }
    });

    rx.recv_timeout(Duration::from_secs(300))
        .context("Authentication timed out (5 minutes)")
}

/// Execute OAuth login flow with PKCE
pub async fn login() -> Result<()> {
    println!("🔐 Authenticating with pmcp.run...");
    println!();

    // Fetch configuration (from discovery endpoint or env vars)
    println!("📡 Fetching pmcp.run configuration...");
    let config = get_pmcp_config().await?;
    println!("   Using auth domain: {}", config.cognito_domain);
    println!();

    let client = create_oauth_client_from_config(&config)?;

    // Generate PKCE challenge for enhanced security
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    // Build authorization URL
    let (auth_url, _csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .set_redirect_uri(std::borrow::Cow::Owned(
            RedirectUrl::new(format!("http://localhost:{}", CALLBACK_PORT))
                .context("Invalid redirect URL")?,
        ))
        .url();

    // Start callback server in background
    let code_future = tokio::task::spawn_blocking(start_callback_server);

    // Open browser
    println!("📱 Opening browser for authentication...");
    println!("   If the browser doesn't open, visit:");
    println!("   {}", auth_url);
    println!();

    if let Err(e) = open::that(auth_url.as_str()) {
        println!("⚠️  Could not open browser automatically: {}", e);
        println!("   Please open the URL manually");
        println!();
    }

    println!("⏳ Waiting for authentication callback...");

    // Wait for authorization code
    let code = code_future.await??;

    // Exchange code for tokens
    println!("🔐 Exchanging authorization code for tokens...");
    let redirect_url = RedirectUrl::new(format!("http://localhost:{}", CALLBACK_PORT))
        .context("Invalid redirect URL")?;

    let http_client = reqwest::Client::new();
    let token_result = client
        .exchange_code(AuthorizationCode::new(code))
        .set_pkce_verifier(pkce_verifier)
        .set_redirect_uri(std::borrow::Cow::Owned(redirect_url))
        .request_async(&http_client)
        .await
        .map_err(|e| {
            eprintln!("Token exchange error details: {:?}", e);
            anyhow::anyhow!("Failed to exchange authorization code for tokens: {:?}", e)
        })?;

    // Extract tokens
    let credentials = Credentials {
        access_token: token_result.access_token().secret().clone(),
        refresh_token: token_result
            .refresh_token()
            .map(|t| t.secret().clone())
            .unwrap_or_default(),
        id_token: token_result
            .extra_fields()
            .id_token
            .clone()
            .unwrap_or_default(),
        expires_at: chrono::Utc::now()
            .checked_add_signed(chrono::Duration::seconds(
                token_result
                    .expires_in()
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(3600),
            ))
            .unwrap()
            .to_rfc3339(),
    };

    save_credentials(&credentials)?;

    println!();
    println!("✅ Successfully authenticated with pmcp.run!");
    println!("   Access token expires: {}", credentials.expires_at);
    println!();
    println!("💡 You can now deploy with: cargo pmcp deploy --target pmcp-run");

    Ok(())
}

/// Logout (remove credentials)
pub fn logout() -> Result<()> {
    let path = credentials_path()?;

    if !path.exists() {
        println!("ℹ️  Not currently authenticated with pmcp.run");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let mut config: toml::Value = toml::from_str(&content)?;

    if let Some(table) = config.as_table_mut() {
        table.remove("pmcp-run");

        if table.is_empty() {
            std::fs::remove_file(&path)?;
            println!("✅ Logged out from pmcp.run (removed credentials file)");
        } else {
            std::fs::write(&path, toml::to_string(&config)?)?;
            println!("✅ Logged out from pmcp.run");
        }
    }

    Ok(())
}
