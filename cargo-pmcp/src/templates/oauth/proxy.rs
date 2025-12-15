//! OAuth Proxy Lambda template for handling OAuth flows with Cognito.
//!
//! This template implements DCR (Dynamic Client Registration) because Cognito
//! doesn't support RFC 7591 natively. It also proxies authorization and token
//! requests to Cognito.

/// Returns the Rust source code for the OAuth Proxy Lambda.
/// This Lambda handles:
/// - /.well-known/openid-configuration (OIDC discovery)
/// - /oauth2/register (Dynamic Client Registration - RFC 7591)
/// - /oauth2/authorize (Authorization endpoint - redirects to Cognito)
/// - /oauth2/token (Token endpoint - proxies to Cognito)
/// - /oauth2/revoke (Token revocation)
///
/// IMPORTANT: This template intentionally does NOT implement:
/// - /.well-known/oauth-protected-resource (RFC 8707)
///
/// Reason: RFC 8707 can cause issues with clients like Claude Code that discover
/// and validate the underlying Cognito provider, which may not support S256 PKCE.
/// The OIDC discovery endpoint is sufficient for OAuth client discovery.
///
/// Note: Token validation is handled separately by the Authorizer Lambda or
/// in the MCP server using CognitoProvider.
pub fn get_proxy_template(user_pool_id: &str, region: &str, _server_name: &str) -> String {
    format!(
        r#"//! OAuth Proxy Lambda for MCP servers.
//!
//! Handles OAuth flows including Dynamic Client Registration (DCR).
//! This Lambda works alongside CognitoProvider for token validation.

use lambda_http::{{run, service_fn, Body, Error, Request, Response}};
use serde::{{Deserialize, Serialize}};
use std::collections::HashMap;
use std::env;

/// Cognito User Pool configuration.
const USER_POOL_ID: &str = "{user_pool_id}";
const REGION: &str = "{region}";

/// DynamoDB table for client registrations.
fn get_table_name() -> String {{
    env::var("CLIENT_TABLE_NAME").unwrap_or_else(|_| "ClientRegistrations".to_string())
}}

/// Get the deployment URL from environment.
fn get_base_url() -> String {{
    env::var("BASE_URL").unwrap_or_else(|_| "https://example.execute-api.{region}.amazonaws.com".to_string())
}}

/// OIDC Discovery metadata.
#[derive(Debug, Serialize)]
struct OidcDiscovery {{
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    registration_endpoint: String,
    jwks_uri: String,
    revocation_endpoint: String,
    response_types_supported: Vec<String>,
    grant_types_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
    code_challenge_methods_supported: Vec<String>,
    scopes_supported: Vec<String>,
}}

/// DCR Request (RFC 7591).
#[derive(Debug, Deserialize)]
struct ClientRegistrationRequest {{
    client_name: String,
    #[serde(default)]
    redirect_uris: Vec<String>,
    #[serde(default)]
    grant_types: Vec<String>,
    #[serde(default)]
    response_types: Vec<String>,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    software_id: Option<String>,
    #[serde(default)]
    software_version: Option<String>,
}}

/// DCR Response (RFC 7591).
#[derive(Debug, Serialize)]
struct ClientRegistrationResponse {{
    client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret: Option<String>,
    client_name: String,
    redirect_uris: Vec<String>,
    grant_types: Vec<String>,
    response_types: Vec<String>,
    scope: String,
    client_id_issued_at: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_secret_expires_at: Option<u64>,
}}

/// Known public client patterns (no secret required).
const PUBLIC_CLIENT_PATTERNS: &[&str] = &[
    "claude",
    "cursor",
    "chatgpt",
    "mcp-inspector",
    "desktop",
];

fn is_public_client(client_name: &str) -> bool {{
    let name_lower = client_name.to_lowercase();
    PUBLIC_CLIENT_PATTERNS.iter().any(|p| name_lower.contains(p))
}}

/// Sanitize client name for Cognito compatibility.
/// Cognito only accepts names matching [\w\s+=,.@-]+
/// Characters like parentheses in "Claude Code (chess)" will cause registration to fail.
fn sanitize_client_name(name: &str) -> String {{
    name.chars()
        .map(|c| match c {{
            'a'..='z' | 'A'..='Z' | '0'..='9' | ' ' | '+' | '=' | ',' | '.' | '@' | '-' | '_' => c,
            _ => '-'
        }})
        .collect()
}}

async fn handler(event: Request) -> Result<Response<Body>, Error> {{
    let path = event.uri().path();
    let method = event.method().as_str();

    tracing::info!("OAuth Proxy: {{}} {{}}", method, path);

    match (method, path) {{
        // OIDC Discovery
        ("GET", "/.well-known/openid-configuration") => {{
            handle_oidc_discovery().await
        }}

        // Dynamic Client Registration
        ("POST", "/oauth2/register") => {{
            handle_client_registration(event).await
        }}

        // Authorization (redirect to Cognito)
        ("GET", "/oauth2/authorize") => {{
            handle_authorize(event).await
        }}

        // Token endpoint (proxy to Cognito)
        ("POST", "/oauth2/token") => {{
            handle_token(event).await
        }}

        // Token revocation
        ("POST", "/oauth2/revoke") => {{
            handle_revoke(event).await
        }}

        _ => {{
            Ok(Response::builder()
                .status(404)
                .body(Body::from("Not found"))?)
        }}
    }}
}}

async fn handle_oidc_discovery() -> Result<Response<Body>, Error> {{
    let base_url = get_base_url();
    let cognito_domain = format!(
        "https://cognito-idp.{{}}.amazonaws.com/{{}}",
        REGION, USER_POOL_ID
    );

    let discovery = OidcDiscovery {{
        issuer: cognito_domain.clone(),
        authorization_endpoint: format!("{{}}/oauth2/authorize", base_url),
        token_endpoint: format!("{{}}/oauth2/token", base_url),
        registration_endpoint: format!("{{}}/oauth2/register", base_url),
        jwks_uri: format!("{{}}/.well-known/jwks.json", cognito_domain),
        revocation_endpoint: format!("{{}}/oauth2/revoke", base_url),
        response_types_supported: vec!["code".to_string()],
        grant_types_supported: vec![
            "authorization_code".to_string(),
            "refresh_token".to_string(),
        ],
        token_endpoint_auth_methods_supported: vec![
            "client_secret_basic".to_string(),
            "client_secret_post".to_string(),
            "none".to_string(),
        ],
        code_challenge_methods_supported: vec!["S256".to_string()],
        scopes_supported: vec![
            "openid".to_string(),
            "email".to_string(),
            "profile".to_string(),
            "mcp/read".to_string(),
            "mcp/write".to_string(),
        ],
    }};

    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&discovery)?))?)
}}

async fn handle_client_registration(event: Request) -> Result<Response<Body>, Error> {{
    // Parse request body
    let body = match event.body() {{
        Body::Text(s) => s.clone(),
        Body::Binary(b) => String::from_utf8_lossy(b).to_string(),
        Body::Empty => String::new(),
    }};

    let request: ClientRegistrationRequest = serde_json::from_str(&body)
        .map_err(|e| Error::from(format!("Invalid request: {{}}", e)))?;

    // Sanitize client name for Cognito compatibility
    // Clients like "Claude Code (chess)" contain characters Cognito doesn't accept
    let sanitized_name = sanitize_client_name(&request.client_name);
    tracing::info!("Registering client: '{{}}' (sanitized: '{{}}')", request.client_name, sanitized_name);

    // Generate client credentials
    let client_id = uuid::Uuid::new_v4().to_string();
    let is_public = is_public_client(&request.client_name); // Check against original name
    let client_secret = if is_public {{
        None
    }} else {{
        Some(uuid::Uuid::new_v4().to_string())
    }};

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Store in DynamoDB
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb = aws_sdk_dynamodb::Client::new(&config);

    let mut item = HashMap::new();
    item.insert(
        "client_id".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::S(client_id.clone()),
    );
    item.insert(
        "client_name".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::S(sanitized_name.clone()),
    );
    // Store original name for reference/debugging
    item.insert(
        "original_client_name".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::S(request.client_name.clone()),
    );
    if let Some(ref secret) = client_secret {{
        // Hash the secret before storing
        use sha2::{{Sha256, Digest}};
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        let hash = format!("{{:x}}", hasher.finalize());
        item.insert(
            "client_secret_hash".to_string(),
            aws_sdk_dynamodb::types::AttributeValue::S(hash),
        );
    }}
    item.insert(
        "redirect_uris".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::Ss(request.redirect_uris.clone()),
    );
    item.insert(
        "created_at".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::N(now.to_string()),
    );
    item.insert(
        "is_public".to_string(),
        aws_sdk_dynamodb::types::AttributeValue::Bool(is_public),
    );

    dynamodb
        .put_item()
        .table_name(get_table_name())
        .set_item(Some(item))
        .send()
        .await?;

    // Build response (use sanitized name for Cognito compatibility)
    let response = ClientRegistrationResponse {{
        client_id,
        client_secret,
        client_name: sanitized_name,
        redirect_uris: request.redirect_uris,
        grant_types: if request.grant_types.is_empty() {{
            vec!["authorization_code".to_string(), "refresh_token".to_string()]
        }} else {{
            request.grant_types
        }},
        response_types: if request.response_types.is_empty() {{
            vec!["code".to_string()]
        }} else {{
            request.response_types
        }},
        scope: if request.scope.is_empty() {{
            "openid email mcp/read".to_string()
        }} else {{
            request.scope
        }},
        client_id_issued_at: now,
        client_secret_expires_at: None,
    }};

    Ok(Response::builder()
        .status(201)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_string(&response)?))?)
}}

async fn handle_authorize(event: Request) -> Result<Response<Body>, Error> {{
    // Get query parameters
    let query = event.uri().query().unwrap_or("");
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    // Validate client_id exists in DynamoDB
    let client_id = params.get("client_id")
        .ok_or_else(|| Error::from("Missing client_id"))?;

    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let dynamodb = aws_sdk_dynamodb::Client::new(&config);

    let result = dynamodb
        .get_item()
        .table_name(get_table_name())
        .key("client_id", aws_sdk_dynamodb::types::AttributeValue::S(client_id.clone()))
        .send()
        .await?;

    if result.item().is_none() {{
        return Ok(Response::builder()
            .status(400)
            .body(Body::from("Invalid client_id"))?);
    }}

    // Build Cognito authorize URL
    let cognito_domain = env::var("COGNITO_DOMAIN")
        .unwrap_or_else(|_| format!("{{}}auth.{{}}.amazoncognito.com", USER_POOL_ID.split('_').next().unwrap_or(""), REGION));

    let mut cognito_url = format!("https://{{}}/oauth2/authorize?", cognito_domain);
    cognito_url.push_str(&format!("client_id={{}}", client_id));

    if let Some(redirect_uri) = params.get("redirect_uri") {{
        cognito_url.push_str(&format!("&redirect_uri={{}}", urlencoding::encode(redirect_uri)));
    }}
    if let Some(response_type) = params.get("response_type") {{
        cognito_url.push_str(&format!("&response_type={{}}", response_type));
    }}
    if let Some(scope) = params.get("scope") {{
        cognito_url.push_str(&format!("&scope={{}}", urlencoding::encode(scope)));
    }}
    if let Some(state) = params.get("state") {{
        cognito_url.push_str(&format!("&state={{}}", urlencoding::encode(state)));
    }}
    if let Some(code_challenge) = params.get("code_challenge") {{
        cognito_url.push_str(&format!("&code_challenge={{}}", code_challenge));
    }}
    if let Some(code_challenge_method) = params.get("code_challenge_method") {{
        cognito_url.push_str(&format!("&code_challenge_method={{}}", code_challenge_method));
    }}

    Ok(Response::builder()
        .status(302)
        .header("Location", cognito_url)
        .body(Body::Empty)?)
}}

async fn handle_token(event: Request) -> Result<Response<Body>, Error> {{
    // Parse request body
    let body = match event.body() {{
        Body::Text(s) => s.clone(),
        Body::Binary(b) => String::from_utf8_lossy(b).to_string(),
        Body::Empty => String::new(),
    }};

    // Get Cognito token endpoint
    let cognito_domain = env::var("COGNITO_DOMAIN")
        .unwrap_or_else(|_| format!("{{}}auth.{{}}.amazoncognito.com", USER_POOL_ID.split('_').next().unwrap_or(""), REGION));
    let token_url = format!("https://{{}}/oauth2/token", cognito_domain);

    // Proxy to Cognito
    let client = reqwest::Client::new();
    let response = client
        .post(&token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;

    let status = response.status().as_u16();
    let body = response.text().await?;

    Ok(Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(body))?)
}}

async fn handle_revoke(event: Request) -> Result<Response<Body>, Error> {{
    // Parse request body
    let body = match event.body() {{
        Body::Text(s) => s.clone(),
        Body::Binary(b) => String::from_utf8_lossy(b).to_string(),
        Body::Empty => String::new(),
    }};

    // Get Cognito revoke endpoint
    let cognito_domain = env::var("COGNITO_DOMAIN")
        .unwrap_or_else(|_| format!("{{}}auth.{{}}.amazoncognito.com", USER_POOL_ID.split('_').next().unwrap_or(""), REGION));
    let revoke_url = format!("https://{{}}/oauth2/revoke", cognito_domain);

    // Proxy to Cognito
    let client = reqwest::Client::new();
    let response = client
        .post(&revoke_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await?;

    let status = response.status().as_u16();

    Ok(Response::builder()
        .status(status)
        .body(Body::Empty)?)
}}

#[tokio::main]
async fn main() -> Result<(), Error> {{
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .json()
        .init();

    tracing::info!("Starting OAuth Proxy Lambda");

    run(service_fn(handler)).await
}}
"#,
        user_pool_id = user_pool_id,
        region = region,
    )
}

/// Returns the Cargo.toml for the OAuth Proxy Lambda.
pub fn get_proxy_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}-oauth-proxy"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp = {{ version = "0.3", features = ["full"] }}
lambda_http = "0.14"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
uuid = {{ version = "1", features = ["v4"] }}
sha2 = "0.10"
url = "2.5"
urlencoding = "2.1"
aws-config = "1.5"
aws-sdk-dynamodb = "1.56"
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["fmt", "json"] }}
"#,
        name = name
    )
}
