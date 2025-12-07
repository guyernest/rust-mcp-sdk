//! JWT Authorizer Lambda template for validating tokens via JWKS.

/// Returns the Rust source code for the Lambda Authorizer.
/// This authorizer validates JWT tokens using JWKS from Cognito.
pub fn get_authorizer_template(user_pool_id: &str, region: &str) -> String {
    format!(
        r#"//! JWT Authorizer Lambda for API Gateway.
//!
//! This Lambda validates JWT tokens using JWKS from Cognito.
//! It returns an IAM policy allowing or denying the request.

use lambda_runtime::{{service_fn, Error, LambdaEvent}};
use serde::{{Deserialize, Serialize}};
use serde_json::{{json, Value}};
use std::collections::HashMap;
use std::sync::OnceLock;

/// JWKS URL for the Cognito User Pool.
const JWKS_URL: &str = "https://cognito-idp.{region}.amazonaws.com/{user_pool_id}/.well-known/jwks.json";

/// Expected issuer for token validation.
const ISSUER: &str = "https://cognito-idp.{region}.amazonaws.com/{user_pool_id}";

/// Cached JWKS for token validation.
static JWKS_CACHE: OnceLock<JwksCache> = OnceLock::new();

/// JWKS cache structure.
struct JwksCache {{
    keys: HashMap<String, jsonwebtoken::DecodingKey>,
}}

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

/// JWT claims from Cognito.
#[derive(Debug, Deserialize)]
struct Claims {{
    sub: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    client_id: String,
    iss: String,
    token_use: String,
    exp: u64,
}}

async fn handler(event: LambdaEvent<AuthorizerRequest>) -> Result<AuthorizerResponse, Error> {{
    let request = event.payload;

    // Extract token from Authorization header
    let token = extract_token(&request)?;

    // Validate token
    match validate_token(&token).await {{
        Ok(claims) => {{
            // Build allow policy
            let mut context = HashMap::new();
            context.insert("sub".to_string(), json!(claims.sub));
            context.insert("scope".to_string(), json!(claims.scope));
            context.insert("clientId".to_string(), json!(claims.client_id));

            Ok(AuthorizerResponse {{
                principal_id: claims.sub,
                policy_document: build_policy("Allow", &request.method_arn),
                context,
            }})
        }}
        Err(e) => {{
            tracing::warn!("Token validation failed: {{}}", e);
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

async fn validate_token(token: &str) -> Result<Claims, Error> {{
    // Get or fetch JWKS
    let jwks = get_jwks().await?;

    // Decode header to get kid
    let header = jsonwebtoken::decode_header(token)?;
    let kid = header.kid.ok_or("No kid in token header")?;

    // Get decoding key for this kid
    let key = jwks.keys.get(&kid).ok_or("Unknown key id")?;

    // Validate token
    let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_required_spec_claims(&["exp", "iss", "sub"]);

    let token_data = jsonwebtoken::decode::<Claims>(token, key, &validation)?;
    let claims = token_data.claims;

    // Verify token_use is "access"
    if claims.token_use != "access" {{
        return Err("Invalid token_use".into());
    }}

    Ok(claims)
}}

async fn get_jwks() -> Result<&'static JwksCache, Error> {{
    if let Some(cache) = JWKS_CACHE.get() {{
        return Ok(cache);
    }}

    // Fetch JWKS
    let client = reqwest::Client::new();
    let response = client.get(JWKS_URL).send().await?;
    let jwks: JwksResponse = response.json().await?;

    // Parse keys
    let mut keys = HashMap::new();
    for key in jwks.keys {{
        if key.kty == "RSA" && key.alg == "RS256" {{
            if let (Some(n), Some(e)) = (&key.n, &key.e) {{
                let decoding_key = jsonwebtoken::DecodingKey::from_rsa_components(n, e)?;
                keys.insert(key.kid.clone(), decoding_key);
            }}
        }}
    }}

    let cache = JwksCache {{ keys }};

    // Try to set the cache (race condition is fine, both values are equivalent)
    let _ = JWKS_CACHE.set(cache);

    Ok(JWKS_CACHE.get().unwrap())
}}

#[derive(Debug, Deserialize)]
struct JwksResponse {{
    keys: Vec<JwkKey>,
}}

#[derive(Debug, Deserialize)]
struct JwkKey {{
    kid: String,
    kty: String,
    alg: String,
    n: Option<String>,
    e: Option<String>,
}}

fn build_policy(effect: &str, resource: &str) -> PolicyDocument {{
    // Parse resource to get the wildcard version
    let resource_parts: Vec<&str> = resource.split('/').collect();
    let wildcard_resource = if resource_parts.len() >= 2 {{
        format!("{{}}/{{}}/{{}}/{{}}/{{}}", resource_parts[0], resource_parts[1], "*", "*", "*")
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
        .init();

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
lambda_runtime = "0.13"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
jsonwebtoken = "9"
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls"] }}
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", default-features = false, features = ["fmt"] }}
"#,
        name = name
    )
}
