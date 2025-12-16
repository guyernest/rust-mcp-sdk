//! JWT Authorizer Lambda template using PMCP SDK's CognitoProvider.

/// Returns the Rust source code for the Lambda Authorizer.
/// This authorizer validates JWT tokens using the SDK's CognitoProvider.
pub fn get_authorizer_template(user_pool_id: &str, region: &str) -> String {
    format!(
        r#"//! JWT Authorizer Lambda for API Gateway.
//!
//! This Lambda validates JWT tokens using PMCP SDK's CognitoProvider.
//! It leverages the SDK's built-in JWKS caching and token validation.
//! Returns an IAM policy allowing or denying the request.

use lambda_runtime::{{service_fn, Error, LambdaEvent}};
use pmcp::server::auth::{{CognitoProvider, IdentityProvider}};
use serde::{{Deserialize, Serialize}};
use serde_json::{{json, Value}};
use std::collections::HashMap;
use std::sync::OnceLock;

/// AWS Region for the Cognito User Pool.
const REGION: &str = "{region}";

/// Cognito User Pool ID.
const USER_POOL_ID: &str = "{user_pool_id}";

/// Client ID for validation (from environment or hardcoded).
fn get_client_id() -> String {{
    std::env::var("AUTH_CLIENT_ID").unwrap_or_else(|_| "default-client".to_string())
}}

/// Cached Cognito provider for token validation.
static PROVIDER: OnceLock<CognitoProvider> = OnceLock::new();

/// API Gateway authorizer request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizerRequest {{
    #[serde(rename = "type")]
    request_type: String,
    authorization_token: Option<String>,
    headers: Option<HashMap<String, String>>,
    method_arn: String,
}}

/// API Gateway authorizer response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthorizerResponse {{
    principal_id: String,
    policy_document: PolicyDocument,
    context: HashMap<String, Value>,
}}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyDocument {{
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Statement")]
    statement: Vec<Statement>,
}}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Statement {{
    #[serde(rename = "Action")]
    action: String,
    #[serde(rename = "Effect")]
    effect: String,
    #[serde(rename = "Resource")]
    resource: String,
}}

async fn handler(event: LambdaEvent<AuthorizerRequest>) -> Result<AuthorizerResponse, Error> {{
    let request = event.payload;

    // Extract token from Authorization header
    let token = extract_token(&request)?;

    // Get or initialize the Cognito provider
    let provider = get_provider().await?;

    // Validate token using SDK's CognitoProvider
    match provider.validate_token(&token).await {{
        Ok(auth_context) => {{
            // Build allow policy with auth context
            let mut context = HashMap::new();
            context.insert("sub".to_string(), json!(auth_context.user_id()));
            context.insert("authenticated".to_string(), json!(auth_context.authenticated));

            // Add email if present
            if let Some(email) = &auth_context.email {{
                context.insert("email".to_string(), json!(email));
            }}

            // Add scopes
            if !auth_context.scopes.is_empty() {{
                context.insert("scope".to_string(), json!(auth_context.scopes.join(" ")));
            }}

            // Add groups
            if !auth_context.groups.is_empty() {{
                context.insert("groups".to_string(), json!(auth_context.groups));
            }}

            tracing::info!(
                user_id = %auth_context.user_id(),
                "Token validated successfully"
            );

            Ok(AuthorizerResponse {{
                principal_id: auth_context.user_id().to_string(),
                policy_document: build_policy("Allow", &request.method_arn),
                context,
            }})
        }}
        Err(e) => {{
            tracing::warn!(error = %e, "Token validation failed");
            // Return deny policy
            Ok(AuthorizerResponse {{
                principal_id: "unauthorized".to_string(),
                policy_document: build_policy("Deny", &request.method_arn),
                context: HashMap::new(),
            }})
        }}
    }}
}}

fn extract_token(request: &AuthorizerRequest) -> Result<String, Error> {{
    // Try authorization_token first (TOKEN type)
    if let Some(token) = &request.authorization_token {{
        return Ok(token.strip_prefix("Bearer ").unwrap_or(token).to_string());
    }}

    // Try headers (REQUEST type)
    if let Some(headers) = &request.headers {{
        if let Some(auth) = headers.get("authorization").or_else(|| headers.get("Authorization")) {{
            return Ok(auth.strip_prefix("Bearer ").unwrap_or(auth).to_string());
        }}
    }}

    Err("No authorization token found".into())
}}

/// Get or initialize the cached Cognito provider.
async fn get_provider() -> Result<&'static CognitoProvider, Error> {{
    if let Some(provider) = PROVIDER.get() {{
        return Ok(provider);
    }}

    // Initialize the provider
    let client_id = get_client_id();
    tracing::info!(
        region = %REGION,
        user_pool_id = %USER_POOL_ID,
        "Initializing CognitoProvider"
    );

    let provider = CognitoProvider::new(REGION, USER_POOL_ID, &client_id)
        .await
        .map_err(|e| Error::from(format!("Failed to initialize CognitoProvider: {{}}", e)))?;

    // Try to set the provider (race condition is fine, both values are equivalent)
    let _ = PROVIDER.set(provider);

    Ok(PROVIDER.get().expect("Provider should be set"))
}}

fn build_policy(effect: &str, resource: &str) -> PolicyDocument {{
    // Parse resource to get the wildcard version for broader access
    let resource_parts: Vec<&str> = resource.split('/').collect();
    let wildcard_resource = if resource_parts.len() >= 2 {{
        format!("{{}}/{{}}/*", resource_parts[0], resource_parts[1])
    }} else {{
        resource.to_string()
    }};

    PolicyDocument {{
        version: "2012-10-17".to_string(),
        statement: vec![Statement {{
            action: "execute-api:Invoke".to_string(),
            effect: effect.to_string(),
            resource: wildcard_resource,
        }}],
    }}
}}

#[tokio::main]
async fn main() -> Result<(), Error> {{
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .json()
        .init();

    tracing::info!("Starting JWT Authorizer Lambda");

    lambda_runtime::run(service_fn(handler)).await
}}
"#,
        region = region,
        user_pool_id = user_pool_id,
    )
}

/// Returns the Cargo.toml for the authorizer Lambda.
pub fn get_authorizer_cargo_toml(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}-authorizer"
version = "0.1.0"
edition = "2021"

[dependencies]
pmcp = {{ version = "0.3", features = ["full"] }}
lambda_runtime = "0.13"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["fmt", "json"] }}
"#,
        name = name
    )
}
